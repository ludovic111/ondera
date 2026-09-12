use ondera_engine::control::{self, wire, Headless, Host, COMMANDS};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    io::{BufRead, BufReader, Write},
    net::TcpStream,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

fn call(host: &mut dyn Host, name: &str, params: Value) -> Value {
    control::call(host, name, &params, false).unwrap_or_else(|e| panic!("{name} {params}: {e}"))
}
fn fail(host: &mut dyn Host, name: &str, params: Value) -> String {
    control::call(host, name, &params, false)
        .err()
        .unwrap_or_else(|| panic!("{name} {params} should fail"))
}

#[test]
fn registry_is_unique_introspectable_and_mcp_safe() {
    let mut names = HashSet::new();
    for spec in COMMANDS.iter() {
        assert!(names.insert(spec.name), "duplicate {}", spec.name);
        let (family, action) = spec.name.split_once('.').expect("family.action");
        assert!(!family.is_empty() && !action.is_empty() && !action.contains('.'));
        assert!(
            !spec.name.contains('_'),
            "{} must round-trip to an MCP tool name",
            spec.name
        );
        let tool = spec.name.replacen('.', "_", 1);
        assert_eq!(tool.replacen('_', ".", 1), spec.name);
        assert!(tool.len() <= 64 && tool.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
        let schema = control::schema(spec);
        assert_eq!(schema["type"], "object");
        let mut params = HashSet::new();
        for p in spec.params {
            assert!(params.insert(p.name), "{} repeats {}", spec.name, p.name);
            assert!(schema["properties"][p.name]["type"].is_string());
            assert_eq!(
                schema["required"]
                    .as_array()
                    .unwrap()
                    .contains(&json!(p.name)),
                p.required
            );
        }
        assert!(!spec.doc.is_empty());
    }
    assert_eq!(
        control::describe().as_array().unwrap().len(),
        COMMANDS.len()
    );
}

#[test]
fn plugin_discovery_is_filtered_paged_and_stock_catalog_stays_small() {
    let mut host = Headless::new();
    let catalog = call(&mut host, "session.catalog", json!({}));
    assert_eq!(catalog["plugins"].as_array().unwrap().len(), 24);
    assert!(catalog["plugins"]
        .as_array()
        .unwrap()
        .iter()
        .all(|plugin| plugin["format"] == "stock"));
    assert!(
        serde_json::to_vec(&catalog).unwrap().len() < 12000,
        "a stock sound lookup must not dump installed third-party libraries"
    );
    let first = call(
        &mut host,
        "plugin.list",
        json!({"format":"stock","kind":"instrument","limit":3}),
    );
    assert_eq!(first["total"], 8);
    assert_eq!(first["plugins"].as_array().unwrap().len(), 3);
    assert_eq!(first["nextOffset"], 3);
    let last = call(
        &mut host,
        "plugin.list",
        json!({"format":"stock","kind":"instrument","limit":3,"offset":6}),
    );
    assert_eq!(last["plugins"].as_array().unwrap().len(), 2);
    assert!(last["nextOffset"].is_null());
    let found = call(
        &mut host,
        "plugin.list",
        json!({"format":"stock","query":"PIANO"}),
    );
    assert_eq!(found["total"], 1);
    assert_eq!(found["plugins"][0]["id"], "stock:E-Piano Mk I");
    for params in [
        json!({"limit":0}),
        json!({"limit":201}),
        json!({"offset":-1}),
        json!({"format":"aax"}),
        json!({"kind":"random"}),
    ] {
        assert!(!fail(&mut host, "plugin.list", params).is_empty());
    }
}

#[test]
fn compact_session_inspection_excludes_plugin_payloads_and_only_expands_notes_on_request() {
    let mut host = Headless::new();
    let track = host
        .store
        .session()
        .tracks
        .iter()
        .find(|track| track.kind == "midi")
        .unwrap()
        .id
        .clone();
    call(
        &mut host,
        "clip.create",
        json!({"trackId":track,"startBar":0,"lengthBars":1,"notes":[{"start":0,"length":1,"pitch":60}]}),
    );
    call(
        &mut host,
        "strip.setPlugin",
        json!({"trackId":track,"slot":0,"pluginId":"stock:Utility"}),
    );
    host.store
        .amend(|session| {
            session.strips.get_mut(&track).unwrap().inserts[0].blob = "YWJj".repeat(100000)
        })
        .unwrap();
    let summary = call(&mut host, "session.inspect", json!({}));
    assert_eq!(summary["clips"][0]["noteCount"], 1);
    assert!(summary["clips"][0].get("data").is_none());
    assert_eq!(
        summary["strips"][&track]["inserts"][0]["hasSavedState"],
        true
    );
    assert!(summary["strips"][&track]["inserts"][0]
        .get("blob")
        .is_none());
    assert!(serde_json::to_vec(&summary).unwrap().len() < 20000);
    let expanded = call(&mut host, "session.inspect", json!({"includeNotes":true}));
    assert_eq!(expanded["clips"][0]["data"]["notes"][0]["pitch"], 60);
    let complete = call(&mut host, "session.get", json!({}));
    assert_eq!(
        complete["strips"][&track]["inserts"][0]["blob"]
            .as_str()
            .unwrap()
            .len(),
        400000
    );
}

#[test]
fn every_registered_command_is_implemented() {
    let mut host = Headless::new();
    for spec in COMMANDS.iter() {
        // Scanner tests use isolated fixtures; never probe the user's installed plugins here.
        if spec.name == "plugin.scan" {
            continue;
        }
        let err = control::call(&mut host, spec.name, &json!({}), false).err();
        assert!(
            err.as_deref()
                .is_none_or(|e| !e.contains("not implemented")),
            "{}: {err:?}",
            spec.name
        );
    }
    let err = fail(&mut host, "track.ad", json!({}));
    assert!(err.contains("track.add"), "{err}");
}

#[test]
fn parameters_are_validated_before_the_store_changes() {
    let mut host = Headless::new();
    let revision = host.store.revision;
    let err = fail(
        &mut host,
        "track.add",
        json!({ "kind": "midi", "colour": "#ff0000" }),
    );
    assert!(
        err.contains("Unknown parameter `colour`") && err.contains("color"),
        "{err}"
    );
    let err = fail(&mut host, "track.add", json!({}));
    assert!(err.contains("needs `kind`"), "{err}");
    let err = fail(&mut host, "transport.setTempo", json!({ "bpm": "fast" }));
    assert!(err.contains("must be a number"), "{err}");
    let err = fail(&mut host, "transport.setTempo", json!({ "bpm": 1000 }));
    assert!(err.contains("20 and 400"), "{err}");
    let err = fail(
        &mut host,
        "track.add",
        json!({ "kind": "midi", "color": "red" }),
    );
    assert!(err.contains("#rrggbb"), "{err}");
    let err = fail(
        &mut host,
        "track.add",
        json!({ "kind": "audio", "instrument": "Riser" }),
    );
    assert!(err.contains("Only MIDI tracks"), "{err}");
    assert_eq!(
        host.store.revision, revision,
        "rejected commands leave no trace"
    );
    assert!(!host.store.dirty() && !host.store.can_undo());
}

#[test]
fn headless_host_builds_a_song_with_one_history() {
    let mut host = Headless::new();
    let info = call(&mut host, "session.info", json!({}));
    assert_eq!(info["mode"], "headless");
    assert_eq!(info["trackCount"], 2);
    let bass = call(
        &mut host,
        "track.add",
        json!({ "kind": "midi", "name": "Bass", "instrument": "Sub Bass 808" }),
    );
    assert_eq!(bass["instrument"], "Sub Bass 808");
    assert_eq!(bass["color"], control::TRACK_PALETTE[2]);
    let id = bass["id"].as_str().unwrap().to_string();
    let clip = call(
        &mut host,
        "clip.create",
        json!({
            "trackId": id, "startBar": 0, "lengthBars": 2,
            "notes": [
                { "start": 0, "length": 1, "pitch": 36 },
                { "start": 2, "length": 1, "pitch": 43, "velocity": 90 }
            ]
        }),
    );
    assert_eq!(clip["noteCount"], 2);
    assert_eq!(clip["name"], "Bass 1");
    let clip_id = clip["id"].as_str().unwrap().to_string();
    let note = call(
        &mut host,
        "note.add",
        json!({ "clipId": clip_id, "start": 4, "length": 0.5, "pitch": 48 }),
    );
    assert_eq!(note["velocity"], 100);
    assert_eq!(
        call(&mut host, "note.list", json!({ "clipId": clip_id }))
            .as_array()
            .unwrap()
            .len(),
        3
    );
    let err = fail(
        &mut host,
        "note.add",
        json!({ "clipId": clip_id, "start": 0, "length": 1, "pitch": 200 }),
    );
    assert!(err.contains("0 and 127"), "{err}");
    let updated = call(
        &mut host,
        "note.update",
        json!({ "clipId": clip_id, "noteId": note["id"], "pitch": 50, "velocity": 64 }),
    );
    assert_eq!(
        (updated["pitch"].as_u64(), updated["velocity"].as_u64()),
        (Some(50), Some(64))
    );
    let undone = call(&mut host, "history.undo", json!({}));
    assert_eq!(undone["applied"], true);
    let notes = call(&mut host, "note.list", json!({ "clipId": clip_id }));
    assert_eq!(
        notes[2]["pitch"], 48,
        "undo reverts exactly the last command"
    );
    call(&mut host, "history.undo", json!({}));
    assert_eq!(
        call(&mut host, "clip.get", json!({ "clipId": clip_id }))["data"]["notes"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let split = call(
        &mut host,
        "clip.split",
        json!({ "clipId": clip_id, "bar": 1 }),
    );
    assert_eq!(split["left"]["lengthBars"], 1.0);
    assert_eq!(split["right"]["startBar"], 1.0);
    assert_eq!(
        call(&mut host, "clip.list", json!({ "trackId": id }))
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let strip = call(
        &mut host,
        "strip.setInsert",
        json!({ "trackId": id, "slot": 1, "effect": "Space", "bypassed": true }),
    );
    assert_eq!(strip["inserts"][1]["state"], "bypassed");
    let strip = call(
        &mut host,
        "strip.setSendLevel",
        json!({ "trackId": id, "send": 0, "levelDb": -12 }),
    );
    assert_eq!(strip["sends"][0]["levelDb"], -12.0);
    let t = call(
        &mut host,
        "track.setVolume",
        json!({ "trackId": id, "volume": 0.5 }),
    );
    assert_eq!(t["volume"], 0.5);
    let err = fail(
        &mut host,
        "track.setVolume",
        json!({ "trackId": id, "volume": 2 }),
    );
    assert!(err.contains("0.0 and 1.0"), "{err}");
    let err = fail(&mut host, "transport.play", json!({}));
    assert!(err.contains("bounce"), "{err}");
    let t = call(&mut host, "transport.locate", json!({ "bar": 3 }));
    assert_eq!(t["positionBeats"], 12.0);
    assert_eq!(
        call(&mut host, "session.catalog", json!({}))["loops"]
            .as_array()
            .unwrap()
            .len(),
        9
    );
}

#[test]
fn file_mode_round_trips_audio_and_renders() {
    let dir = tempfile::tempdir().unwrap();
    let song = dir.path().join("song.ondera");
    let mix = dir.path().join("mix.wav");
    let mut host = Headless::new();
    call(&mut host, "transport.setTempo", json!({ "bpm": 124 }));
    let drums = call(
        &mut host,
        "clip.addLoop",
        json!({ "name": "Four Floor 124", "startBar": 0 }),
    );
    assert_eq!(drums["kind"], "midi");
    let saved = call(&mut host, "session.save", json!({ "path": song }));
    assert_eq!(saved["dirty"], false);
    let mut host = Headless::open(&song).unwrap();
    assert_eq!(host.path.as_deref(), Some(song.as_path()));
    assert_eq!(host.store.session().transport.tempo, 124.0);
    let bounced = call(&mut host, "session.bounce", json!({ "path": mix }));
    assert!(bounced["seconds"].as_f64().unwrap() > 3.0);
    assert!(std::fs::metadata(&mix).unwrap().len() > 44);
    let imported = call(
        &mut host,
        "session.importAudio",
        json!({ "path": mix, "startBar": 4 }),
    );
    assert_eq!(imported["clip"]["kind"], "audio");
    assert_eq!(imported["clip"]["startBar"], 4.0);
    let tracks = call(&mut host, "track.list", json!({}));
    assert!(tracks
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["kind"] == "audio" && t["clipCount"] == 1));
    assert!(host.store.dirty());
    call(&mut host, "session.save", json!({}));
    let reopened = Headless::open(&song).unwrap();
    assert_eq!(reopened.store.session().sources.len(), 1);
    assert_eq!(reopened.library.len(), 1, "embedded audio comes back");
    let err = fail(&mut Headless::new(), "session.save", json!({}));
    assert!(err.contains("pass `path`"), "{err}");
}

#[test]
fn live_socket_serves_commands_in_order_with_auth() {
    let dir = tempfile::tempdir().unwrap();
    let discovery = dir.path().join("control.json");
    let server = wire::Server::start_at(discovery.clone(), || {}).unwrap();
    let port = server.port();
    let stop = Arc::new(AtomicBool::new(false));
    let worker = {
        let stop = stop.clone();
        std::thread::spawn(move || {
            let mut host = Headless::new();
            while !stop.load(Ordering::Relaxed) {
                if let Some(request) = server.recv_timeout(Duration::from_millis(20)) {
                    wire::serve(&mut host, request);
                }
            }
            host.store.session().clips.len()
        })
    };
    let mut client = wire::Client::connect_at(&discovery).unwrap();
    assert_eq!(client.pid, std::process::id());
    let info = client.call("session.info", &json!({}), false).unwrap();
    assert_eq!(info["mode"], "headless");
    let track = client
        .call("track.add", &json!({ "kind": "midi" }), false)
        .unwrap();
    let clip = client
        .call(
            "clip.create",
            &json!({ "trackId": track["id"], "startBar": 0, "lengthBars": 1, "notes": [{ "start": 0, "length": 1, "pitch": 60 }] }),
            true,
        )
        .unwrap();
    assert_eq!(clip["agent"], true, "agent flag travels with the request");
    let full = client
        .call("clip.get", &json!({ "clipId": clip["id"] }), false)
        .unwrap();
    assert_eq!(full["data"]["notes"][0]["agent"], true);
    let err = client
        .call("clip.get", &json!({ "clipId": "nope" }), false)
        .unwrap_err();
    assert!(err.contains("Unknown clip"), "{err}");
    let err = client.call("bogus", &json!({}), false).unwrap_err();
    assert!(err.contains("Unknown command"), "{err}");

    // A second client without the token is refused before any command runs.
    let mut raw = TcpStream::connect(("127.0.0.1", port)).unwrap();
    writeln!(raw, r#"{{"jsonrpc":"2.0","id":1,"method":"track.list"}}"#).unwrap();
    let mut line = String::new();
    BufReader::new(raw.try_clone().unwrap())
        .read_line(&mut line)
        .unwrap();
    assert!(line.contains("Authenticate first"), "{line}");
    let mut raw = TcpStream::connect(("127.0.0.1", port)).unwrap();
    writeln!(
        raw,
        r#"{{"jsonrpc":"2.0","id":1,"method":"auth","params":{{"token":"wrong"}}}}"#
    )
    .unwrap();
    let mut line = String::new();
    BufReader::new(raw).read_line(&mut line).unwrap();
    assert!(line.contains("Invalid control token"), "{line}");

    stop.store(true, Ordering::Relaxed);
    assert_eq!(worker.join().unwrap(), 1);
    assert!(
        !discovery.exists(),
        "dropping the server removes its discovery file"
    );
    assert!(wire::Client::connect_at(&discovery).is_err());
}

#[test]
fn plugin_routing_parameters_state_and_master_round_trip() {
    let mut host = Headless::new();
    let track = call(&mut host, "track.add", json!({"kind":"midi"}))["id"].clone();
    call(
        &mut host,
        "track.setArmed",
        json!({"trackId":track,"armed":true}),
    );
    call(
        &mut host,
        "strip.setPlugin",
        json!({"trackId":track,"pluginId":"stock:Glass Keys"}),
    );
    call(
        &mut host,
        "strip.setPlugin",
        json!({"trackId":track,"slot":7,"pluginId":"stock:Space"}),
    );
    let metadata = call(
        &mut host,
        "strip.parameters",
        json!({"trackId":track,"slot":7}),
    );
    let parameter = metadata["parameters"][0].clone();
    let value = parameter["min"].as_f64().unwrap();
    call(
        &mut host,
        "strip.setParameter",
        json!({"trackId":track,"slot":7,"parameterId":parameter["id"],"value":value}),
    );
    let state = call(
        &mut host,
        "strip.getState",
        json!({"trackId":track,"slot":7}),
    );
    assert_eq!(state["params"][parameter["id"].to_string()], value);
    let revision = host.store.revision;
    assert!(fail(&mut host, "strip.setParameter", json!({"trackId":track,"slot":7,"parameterId":parameter["id"],"value":parameter["max"].as_f64().unwrap()+1.0})).contains("between"));
    assert_eq!(host.store.revision, revision);
    call(
        &mut host,
        "strip.setBypass",
        json!({"trackId":track,"slot":7,"bypassed":true}),
    );
    let bypassed = call(
        &mut host,
        "strip.getState",
        json!({"trackId":track,"slot":7}),
    );
    assert_eq!(bypassed["id"], state["id"]);
    assert_eq!(bypassed["params"], state["params"]);
    call(&mut host, "history.undo", json!({}));
    assert_eq!(
        call(
            &mut host,
            "strip.getState",
            json!({"trackId":track,"slot":7})
        )["state"],
        "active"
    );
    for bus in ["master", "bus-a", "bus-b"] {
        call(
            &mut host,
            "strip.setInsert",
            json!({"trackId":bus,"slot":7,"effect":"Space"}),
        );
        assert_eq!(
            call(&mut host, "strip.get", json!({"trackId":bus}))["inserts"][7]["pluginId"],
            "stock:Space"
        );
        assert!(fail(
            &mut host,
            "strip.setSendLevel",
            json!({"trackId":bus,"send":0,"levelDb":-6})
        )
        .contains("feedback"));
    }
    call(&mut host, "master.setVolume", json!({"volume":0.5}));
    assert_eq!(host.store.session().master_volume, 0.5);
    call(
        &mut host,
        "strip.setInstrument",
        json!({"trackId":track,"instrument":"Ondera Synth"}),
    );
    assert!(host.store.session().strips[track.as_str().unwrap()]
        .synth
        .is_none());
    let mut instance = ondera_engine::host::instantiate("stock:Space", "Space", 48000).unwrap();
    let blob = ondera_engine::host::encode_blob(&instance.editor.save().unwrap());
    call(
        &mut host,
        "strip.setState",
        json!({"trackId":track,"slot":7,"blob":blob}),
    );
    let saved = call(
        &mut host,
        "strip.getState",
        json!({"trackId":track,"slot":7}),
    );
    assert_eq!(saved["blob"], blob);
    assert!(saved.get("params").is_none());
    assert!(fail(
        &mut host,
        "strip.setState",
        json!({"trackId":track,"slot":7,"blob":"invalid!"})
    )
    .contains("Invalid plugin state"));
}

#[test]
fn live_wire_limits_clients_and_keeps_pending_requests_until_completion() {
    let dir = tempfile::tempdir().unwrap();
    let discovery = dir.path().join("control.json");
    let server = wire::Server::start_at(discovery.clone(), || {}).unwrap();
    let clients: Vec<_> = (0..16)
        .map(|_| wire::Client::connect_at(&discovery).unwrap())
        .collect();
    assert!(
        wire::Client::connect_at(&discovery).is_err(),
        "excess live clients are refused without allocating another worker"
    );
    drop(clients);
    let mut client = None;
    for _ in 0..100 {
        if let Ok(connection) = wire::Client::connect_at(&discovery) {
            client = Some(connection);
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let mut client = client.expect("closed connections release capacity");
    let (send, receive) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        send.send(client.call(
            "session.rename",
            &json!({"name":"Acknowledged after completion"}),
            false,
        ))
        .unwrap();
    });
    let request = server.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(
        receive.recv_timeout(Duration::from_millis(250)).is_err(),
        "no optimistic result while store work is pending"
    );
    let mut host = Headless::new();
    wire::serve(&mut host, request);
    assert_eq!(
        receive
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap()["name"],
        "Acknowledged after completion"
    );
    worker.join().unwrap();
}

#[test]
fn dropping_live_server_closes_authenticated_idle_sockets() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("control.json");
    let server = wire::Server::start_at(path.clone(), || {}).unwrap();
    let discovery = wire::read_discovery(&path).unwrap();
    let mut socket = TcpStream::connect(("127.0.0.1", server.port())).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    writeln!(
        socket,
        "{}",
        json!({"jsonrpc":"2.0","id":0,"method":"auth","params":{"token":discovery.token}})
    )
    .unwrap();
    let mut reader = BufReader::new(socket);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert!(line.contains("result"));
    drop(server);
    line.clear();
    assert_eq!(
        reader.read_line(&mut line).unwrap(),
        0,
        "shutdown wakes idle connection workers immediately"
    );
}
