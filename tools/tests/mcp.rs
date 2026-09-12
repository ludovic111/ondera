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
