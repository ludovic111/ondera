use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, Command, Stdio},
};

struct Mcp {
    child: Child,
    reader: BufReader<std::process::ChildStdout>,
}
impl Mcp {
    fn start(dir: &Path, args: &[&str]) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_ondera-mcp"))
            .args(args)
            .env("ONDERA_CONTROL", dir.join("absent-control.json"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let reader = BufReader::new(child.stdout.take().unwrap());
        Self { child, reader }
    }
    fn notify(&mut self, method: &str) {
        let stdin = self.child.stdin.as_mut().unwrap();
        writeln!(stdin, "{}", json!({ "jsonrpc": "2.0", "method": method })).unwrap();
    }
    fn request(&mut self, id: u64, method: &str, params: Value) -> Value {
        let stdin = self.child.stdin.as_mut().unwrap();
        writeln!(
            stdin,
            "{}",
            json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
        )
        .unwrap();
        let mut line = String::new();
        self.reader.read_line(&mut line).unwrap();
        let reply: Value = serde_json::from_str(&line).unwrap_or_else(|e| panic!("{e}: {line}"));
        assert_eq!(reply["id"], id);
        reply
    }
    fn tool(&mut self, id: u64, name: &str, arguments: Value) -> Value {
        self.request(
            id,
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
        )["result"]
            .clone()
    }
}
impl Drop for Mcp {
    fn drop(&mut self) {
        drop(self.child.stdin.take());
        let _ = self.child.wait();
    }
}

#[test]
fn stdio_server_speaks_mcp_over_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let song = dir.path().join("song.ondera");
    let mut mcp = Mcp::start(dir.path(), &["--file", song.to_str().unwrap()]);
    let init = mcp.request(
        1,
        "initialize",
        json!({ "protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": { "name": "test", "version": "0" } }),
    );
    assert_eq!(init["result"]["protocolVersion"], "2025-03-26");
    assert!(init["result"]["capabilities"]["tools"].is_object());
    assert!(init["result"]["instructions"]
        .as_str()
        .unwrap()
        .contains("Headless"));
    mcp.notify("notifications/initialized");
    assert_eq!(mcp.request(2, "ping", json!({}))["result"], json!({}));

    let tools = mcp.request(3, "tools/list", json!({}))["result"]["tools"].clone();
    let names: Vec<&str> = tools
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(
        names.contains(&"track_add") && names.contains(&"clip_setNotes"),
        "{names:?}"
    );
    let add = tools
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "track_add")
        .unwrap();
    assert_eq!(add["inputSchema"]["required"], json!(["kind"]));
    assert_eq!(add["annotations"]["readOnlyHint"], false);

    let track = mcp.tool(
        4,
        "track_add",
        json!({ "kind": "midi", "name": "Keys", "instrument": "Glass Keys" }),
    );
    assert_eq!(track["isError"], false);
    assert_eq!(track["structuredContent"]["name"], "Keys");
    assert!(track["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("saved"));
    let id = track["structuredContent"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let clip = mcp.tool(
        5,
        "clip_create",
        json!({ "trackId": id, "startBar": 0, "lengthBars": 1, "notes": [{ "start": 0, "length": 4, "pitch": 64 }] }),
    );
    assert_eq!(
        clip["structuredContent"]["agent"], true,
        "MCP edits are marked as agent-made"
    );

    let bad = mcp.tool(6, "track_add", json!({ "kind": "drums" }));
    assert_eq!(bad["isError"], true);
    assert!(bad["content"][0]["text"].as_str().unwrap().contains("midi"));

    let unknown = mcp.request(
        7,
        "tools/call",
        json!({ "name": "track_explode", "arguments": {} }),
    );
    assert_eq!(unknown["error"]["code"], -32602);

    let info = mcp.request(
        8,
        "resources/read",
        json!({ "uri": "ondera://session/info" }),
    );
    let text = info["result"]["contents"][0]["text"].as_str().unwrap();
    let parsed: Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["clipCount"], 1);
    assert_eq!(
        parsed["dirty"], false,
        "file mode saved after the last tool call"
    );

    let missing = mcp.request(9, "nonsense/method", json!({}));
    assert_eq!(missing["error"]["code"], -32601);
    drop(mcp);
    assert!(song.exists());
}

#[test]
fn without_the_app_the_server_falls_back_to_headless() {
    let dir = tempfile::tempdir().unwrap();
    let mut mcp = Mcp::start(dir.path(), &[]);
    let init = mcp.request(1, "initialize", json!({ "protocolVersion": "2099-01-01" }));
    assert_eq!(
        init["result"]["protocolVersion"], "2025-06-18",
        "unknown versions get the newest supported"
    );
    assert!(init["result"]["instructions"]
        .as_str()
        .unwrap()
        .contains("in memory"));
    let info = mcp.tool(2, "session_info", json!({}));
    assert_eq!(info["structuredContent"]["mode"], "headless");
    let play = mcp.tool(3, "transport_play", json!({}));
    assert_eq!(play["isError"], true);
}

#[test]
fn mcp_composes_routes_renders_and_reopens_a_complete_song() {
    let dir = tempfile::tempdir().unwrap();
    let song = dir.path().join("song.ondera");
    let wav = dir.path().join("song.wav");
    let mut mcp = Mcp::start(dir.path(), &["--file", song.to_str().unwrap()]);
    let mut id = 0;
    let mut run = |name: &str, params: Value| -> Value {
        id += 1;
        let reply = mcp.tool(id, name, params);
        assert_eq!(reply["isError"], false, "{name}: {reply}");
        reply["structuredContent"].clone()
    };
    run("session_new", json!({}));
    run("session_rename", json!({"name":"MCP complete song"}));
    let keys = run(
        "track_add",
        json!({"kind":"midi","instrument":"Glass Keys"}),
    )["id"]
        .clone();
    let clip = run(
        "clip_create",
        json!({"trackId":keys,"startBar":0,"lengthBars":1,"notes":[{"start":0,"length":3,"pitch":60},{"start":0,"length":3,"pitch":64},{"start":0,"length":3,"pitch":67}]}),
    );
    run("clip_duplicate", json!({"clipId":clip["id"]}));
    run(
        "strip_setPlugin",
        json!({"trackId":"bus-a","slot":7,"pluginId":"stock:Space"}),
    );
    let meta = run("strip_parameters", json!({"trackId":"bus-a","slot":7}));
    run(
        "strip_setParameter",
        json!({"trackId":"bus-a","slot":7,"parameterId":meta["parameters"][0]["id"],"value":meta["parameters"][0]["default"]}),
    );
    run(
        "strip_setSendLevel",
        json!({"trackId":keys,"send":0,"levelDb":-12}),
    );
    run("master_setVolume", json!({"volume":0.65}));
    run("session_bounce", json!({"path":wav}));
    run("session_open", json!({"path":song}));
    let session = run("session_get", json!({}));
    assert_eq!(session["name"], "MCP complete song");
    assert_eq!(session["clips"].as_array().unwrap().len(), 2);
    assert_eq!(
        session["strips"]["bus-a"]["inserts"][7]["plugin"],
        "stock:Space"
    );
    drop(mcp);
    let audio = ondera_engine::audio::decode(std::fs::read(wav).unwrap(), Some("wav")).unwrap();
    assert!(audio.frames.iter().flatten().all(|s| s.is_finite()));
    assert!(audio.frames.iter().flatten().any(|s| s.abs() > 0.01));
}

#[test]
fn mcp_reports_autosave_failure_and_preserves_memory_for_retry() {
    let dir = tempfile::tempdir().unwrap();
    let song = dir.path().join("song.ondera");
    let mut mcp = Mcp::start(dir.path(), &["--file", song.to_str().unwrap()]);
    // Make the destination unwritable after the headless server has opened it.
    assert_eq!(mcp.tool(1, "session_info", json!({}))["isError"], false);
    std::fs::remove_file(&song).unwrap();
    std::fs::create_dir(&song).unwrap();
    let failed = mcp.tool(2, "session_rename", json!({"name":"Do not lose this"}));
    assert_eq!(failed["isError"], true);
    assert!(failed["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("autosave failed"));
    std::fs::remove_dir(&song).unwrap();
    assert_eq!(mcp.tool(3, "session_save", json!({}))["isError"], false);
    assert_eq!(
        ondera_engine::document::load(&song).unwrap().0.name,
        "Do not lose this"
    );
}

#[test]
fn mcp_rejects_invalid_modes_and_jsonrpc_envelopes() {
    let dir = tempfile::tempdir().unwrap();
    for args in [
        vec!["--file"],
        vec!["--live", "--headless"],
        vec!["--file", "x.ondera", "--live"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_ondera-mcp"))
            .args(args)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
    }
    let mut mcp = Mcp::start(dir.path(), &["--headless"]);
    writeln!(
        mcp.child.stdin.as_mut().unwrap(),
        r#"{{"id":1,"method":"tools/list"}}"#
    )
    .unwrap();
    let mut line = String::new();
    mcp.reader.read_line(&mut line).unwrap();
    let reply: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(reply["error"]["code"], -32600);
    assert_eq!(
        mcp.request(2, "tools/list", json!({"cursor":"invented"}))["error"]["code"],
        -32602
    );
    assert!(mcp.request(3, "tools/list", json!({}))["result"]["tools"].is_array());
}

#[test]
fn mcp_exposes_configured_export_and_atomic_stem_reports() {
    let dir = tempfile::tempdir().unwrap();
    let mut mcp = Mcp::start(dir.path(), &["--headless"]);
    let track = mcp.tool(
        1,
        "track_add",
        json!({"kind":"midi","instrument":"Glass Keys"}),
    )["structuredContent"]["id"]
        .clone();
    assert_eq!(mcp.tool(2,"clip_create",json!({"trackId":track,"startBar":0,"lengthBars":1,"notes":[{"start":0,"length":2,"pitch":67}]}))["isError"],false);
    let folder = dir.path().join("stems");
    let report=mcp.tool(3,"session_exportStems",json!({"directory":folder,"trackIds":[track],"sampleRate":96000,"format":"pcm16","startBeat":0,"endBeat":1,"tailSeconds":0,"includeEffects":false,"includeMaster":false,"dither":false}));
    assert_eq!(report["isError"], false, "{report}");
    assert_eq!(
        report["structuredContent"]["files"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(report["structuredContent"]["files"][0]["sampleRate"], 96000);
    assert!(folder.join("manifest.json").exists());
    assert_eq!(
        mcp.tool(4, "session_exportStems", json!({"directory":folder}))["isError"],
        true
    );
    let tools = mcp.request(5, "tools/list", json!({}));
    let names: Vec<_> = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    for name in [
        "session_importMidi",
        "session_exportMidi",
        "session_exportAudio",
        "session_exportStems",
    ] {
        assert!(names.contains(&name));
    }
}
