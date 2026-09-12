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
    for spec in COMMANDS {
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
fn every_registered_command_is_implemented() {
    let mut host = Headless::new();
    for spec in COMMANDS {
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
