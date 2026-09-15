//! Codex app-server transport. Dynamic tools use Ondera's ordinary command dispatcher;
//! agentMessage deltas reach the interface before the turn is complete.
use super::{
    await_tool, bounded, cli, read_line_limited, system_prompt, tool_output, tool_specs, Event,
    Message, Part, ToolCall, Turn, TEXT_LIMIT,
};
use ondera_engine::Result;
use serde_json::{json, Value};
use std::{
    io::{BufReader, Write},
    process::{Command, Stdio},
    sync::{atomic::Ordering, mpsc},
    time::{Duration, Instant},
};

const LINE_LIMIT: usize = 2 * 1024 * 1024;

fn write(writer: &mut impl Write, value: Value) -> Result<()> {
    serde_json::to_writer(&mut *writer, &value).map_err(|e| e.to_string())?;
    writer
        .write_all(b"\n")
        .and_then(|_| writer.flush())
        .map_err(|e| e.to_string())
}

/// A private config root keeps user hooks, MCP servers and plugins out of this music
/// session. Link the credential file so OAuth refreshes remain owned by the CLI.
pub(super) fn isolated_home() -> Result<tempfile::TempDir> {
    let root = tempfile::tempdir().map_err(|e| e.to_string())?;
    let original = std::env::var_os("CODEX_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(|home| std::path::PathBuf::from(home).join(".codex"))
        })
        .ok_or("Could not locate the Codex account directory")?;
    let auth = original.join("auth.json");
    if auth.is_file() {
        #[cfg(unix)]
        std::os::unix::fs::symlink(&auth, root.path().join("auth.json"))
            .map_err(|e| e.to_string())?;
        #[cfg(windows)]
        std::fs::copy(&auth, root.path().join("auth.json")).map_err(|e| e.to_string())?;
    } else {
        return Err("Codex streaming needs a file-backed sign-in. Run codex login with cli_auth_credentials_store=\"file\", then reconnect in Settings > Agent.".into());
    }
    Ok(root)
}

pub(crate) fn run(turn: Turn) -> Result<()> {
    let home = isolated_home()?;
    let workspace = tempfile::tempdir().map_err(|e| e.to_string())?;
    let mut command = Command::new(super::discover_codex(&turn.settings.agent.codex_executable));
    command
        .args(["app-server", "--listen", "stdio://"])
        .env("CODEX_HOME", home.path())
        .current_dir(workspace.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for config in [
        "features.shell_tool=false",
        "features.unified_exec=false",
        "features.apps=false",
        "features.plugins=false",
        "features.memories=false",
        "features.multi_agent=false",
        "features.hooks=false",
        "web_search=\"disabled\"",
        "cli_auth_credentials_store=\"file\"",
    ] {
        command.args(["-c", config]);
    }
    cli::group(&mut command);
    let mut child = command
        .spawn()
        .map_err(|e| format!("Could not start Codex streaming: {e}"))?;
    let outcome = (|| {
        let mut stdin = child.stdin.take().ok_or("Missing Codex input")?;
        let stdout = child.stdout.take().ok_or("Missing Codex output")?;
        let stderr = child.stderr.take().ok_or("Missing Codex error output")?;
        // Bounded readers never hold up Stop. Their senders exit when this run drops rx.
        let (tx, rx) = mpsc::sync_channel(256);
        let reader = std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                match read_line_limited(&mut reader, LINE_LIMIT) {
                    Ok(Some(line)) => {
                        let event = serde_json::from_str::<Value>(&line)
                            .map_err(|e| format!("Invalid Codex event: {e}"));
                        let failed = event.is_err();
                        if tx.send(event).is_err() || failed {
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        let _ = tx.send(Err(e.to_string()));
                        break;
                    }
                }
            }
        });
        let errors = std::thread::spawn(move || cli::read_errors(BufReader::new(stderr)));
        let result = session(&turn, workspace.path(), &mut stdin, &rx);
        cli::terminate_tree(&mut child);
        drop(rx);
        drop(stdin);
        let _ = reader.join();
        let diagnostics = errors
            .join()
            .ok()
            .and_then(std::result::Result::ok)
            .unwrap_or_default();
        result.map_err(|error| {
            if diagnostics.is_empty() {
                error
            } else {
                format!("{error}\n{}", bounded(&diagnostics, 1200))
            }
        })
    })();
    cli::terminate_tree(&mut child);
    outcome
}

fn session(
    turn: &Turn,
    workspace: &std::path::Path,
    stdin: &mut impl Write,
    rx: &mpsc::Receiver<Result<Value>>,
) -> Result<()> {
    write(
        stdin,
        json!({"id":1,"method":"initialize","params":{
        "clientInfo":{"name":"ondera","version":env!("CARGO_PKG_VERSION")},
        "capabilities":{"experimentalApi":true}}}),
    )?;
    let mut initialized = false;
    let mut active = false;
    let started = Instant::now();
    let mut transcript = Stream::default();
    let mut calls = 0;
    loop {
        if turn.cancel.load(Ordering::Acquire) {
            finish(turn, transcript.response, None, true);
            return Ok(());
        }
        if !active && started.elapsed() > Duration::from_secs(60) {
            return Err("Codex did not connect within 60 seconds. Check your sign-in.".into());
        }
        let event = match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(value) => value?,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => return Err("Codex disconnected before completing the response".into()),
        };
        if let Some(error) = event.get("error") {
            return Err(bounded(
                error["message"].as_str().unwrap_or("Codex request failed"),
                TEXT_LIMIT,
            ));
        }
        // JSON-RPC request IDs are independent in each direction. A server tool
        // request can use the same ID as one of our initialization requests.
        let response = event.get("method").is_none() && event.get("result").is_some();
        if response && event["id"] == 1 && !initialized {
            initialized = true;
            write(stdin, json!({"method":"initialized"}))?;
            let tools: Vec<_> = tool_specs().into_iter().map(|(_, name, description, schema)|
                json!({"type":"function","name":name,"description":description,"inputSchema":schema})).collect();
            let model = turn.settings.model();
            write(
                stdin,
                json!({"id":2,"method":"thread/start","params":{
                "cwd":workspace,"ephemeral":true,"sandbox":"read-only","approvalPolicy":"never",
                "model":if model.is_empty() { None } else { Some(model) },
                "developerInstructions":system_prompt(&turn.settings),"dynamicTools":tools}}),
            )?;
        } else if response && event["id"] == 2 {
            let id = event["result"]["thread"]["id"]
                .as_str()
                .ok_or("Codex returned no session ID")?;
            let effort = &turn.settings.agent.reasoning_effort;
            write(
                stdin,
                json!({"id":3,"method":"turn/start","params":{
                "threadId":id,"input":[{"type":"text","text":cli::prompt_with_context(turn, &cli::turn_prefix(turn))}],
                "effort":if effort.is_empty() { None } else { Some(effort) }}}),
            )?;
        } else if response && event["id"] == 3 {
            active = true;
        } else if event["method"] == "item/tool/call" {
            calls += 1;
            if calls > turn.settings.agent.max_tool_rounds {
                return Err(format!(
                    "Stopped after {} tool calls. Finished edits remain in Undo.",
                    turn.settings.agent.max_tool_rounds
                ));
            }
            let params = &event["params"];
            let name = params["tool"]
                .as_str()
                .ok_or("Codex tool call has no name")?;
            let (tx, answer) = mpsc::sync_channel(1);
            turn.events
                .send(Event::ToolCall(ToolCall {
                    name: name.into(),
                    args: params["arguments"].clone(),
                    reply: tx,
                }))
                .map_err(|_| "The interface stopped listening")?;
            let result = await_tool(&answer, &turn.cancel);
            let (text, failed) = tool_output(&result);
            write(
                stdin,
                json!({"id":event["id"],"result":{"contentItems":[{"type":"inputText","text":text}],"success":!failed}}),
            )?;
        } else if event.get("id").is_some() && event.get("method").is_some() {
            write(
                stdin,
                json!({"id":event["id"],"error":{"code":-32601,"message":"Only Ondera music tools are available"}}),
            )?;
        } else if transcript.event(&event, &turn.events) {
            let status = event["params"]["turn"]["status"].as_str().unwrap_or("");
            let error = (status == "failed").then(|| {
                event["params"]["turn"]["error"]["message"]
                    .as_str()
                    .unwrap_or("Codex turn failed")
                    .to_string()
            });
            finish(turn, transcript.response, error, status == "interrupted");
            return Ok(());
        }
    }
}

#[derive(Default)]
struct Stream {
    response: String,
}
impl Stream {
    fn event(&mut self, event: &Value, events: &mpsc::SyncSender<Event>) -> bool {
        let params = &event["params"];
        match event["method"].as_str().unwrap_or("") {
            "item/started" if params["item"]["type"] == "agentMessage" => self.response.clear(),
            "item/agentMessage/delta" => {
                if let Some(text) = params["delta"].as_str() {
                    self.response = bounded(&format!("{}{text}", self.response), TEXT_LIMIT);
                    let _ = events.send(Event::Text {
                        text: text.into(),
                        replace: false,
                    });
                }
            }
            "item/completed" if params["item"]["type"] == "agentMessage" => {
                if let Some(text) = params["item"]["text"].as_str() {
                    self.response = bounded(text, TEXT_LIMIT);
                    let _ = events.send(Event::Text {
                        text: self.response.clone(),
                        replace: true,
                    });
                }
                let _ = events.send(Event::TextEnd);
            }
            "turn/completed" => return true,
            _ => {}
        }
        false
    }
}
fn finish(turn: &Turn, response: String, error: Option<String>, cancelled: bool) {
    let mut history = turn.history.clone();
    history.push(Message {
        role: "user",
        parts: vec![Part::Text(turn.prompt.clone())],
    });
    if !response.is_empty() {
        history.push(Message {
            role: "assistant",
            parts: vec![Part::Text(response)],
        });
    }
    let _ = turn.events.send(Event::Done {
        error,
        cancelled,
        history,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn server_tool_request_ids_do_not_collide_with_client_response_ids() {
        let (input, rx) = mpsc::sync_channel(8);
        for event in [
            json!({"id":1,"result":{}}),
            json!({"id":2,"result":{"thread":{"id":"test-thread"}}}),
            json!({"id":3,"result":{}}),
            json!({"id":2,"method":"item/tool/call","params":{"tool":"session_info","arguments":{}}}),
            json!({"method":"turn/completed","params":{"turn":{"status":"completed"}}}),
        ] {
            input.send(Ok(event)).unwrap();
        }
        let (events, output) = mpsc::sync_channel(8);
        let interface = std::thread::spawn(move || {
            match output.recv_timeout(Duration::from_secs(2)).unwrap() {
                Event::ToolCall(call) => {
                    assert_eq!(call.name, "session_info");
                    call.reply.send(Ok(json!({"name":"Test song"}))).unwrap();
                }
                _ => panic!("Expected a DAW tool call"),
            }
        });
        let turn = Turn {
            prompt: "Inspect the song".into(),
            history: vec![],
            settings: Default::default(),
            session_summary: json!({}),
            discovery: Default::default(),
            mcp_executable: String::new(),
            cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            events,
        };
        let mut wire = vec![];
        session(&turn, std::path::Path::new("."), &mut wire, &rx).unwrap();
        interface.join().unwrap();
        let frames: Vec<Value> = String::from_utf8(wire)
            .unwrap()
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect();
        assert!(frames
            .iter()
            .any(|frame| frame["id"] == 2 && frame["result"]["success"] == true));
        assert_eq!(
            frames
                .iter()
                .filter(|frame| frame["method"] == "turn/start")
                .count(),
            1
        );
    }

    #[test]
    fn emits_text_before_completion_and_ignores_reasoning() {
        let (tx, rx) = mpsc::sync_channel(10);
        let mut stream = Stream::default();
        stream.event(
            &json!({"method":"item/agentMessage/delta","params":{"delta":"A **drum"}}),
            &tx,
        );
        assert!(
            matches!(rx.try_recv(), Ok(Event::Text { text, replace:false }) if text == "A **drum")
        );
        stream.event(
            &json!({"method":"item/reasoning/textDelta","params":{"delta":"private"}}),
            &tx,
        );
        assert!(rx.try_recv().is_err());
        stream.event(&json!({"method":"item/completed","params":{"item":{"type":"agentMessage","text":"A **drum beat**"}}}), &tx);
        assert_eq!(stream.response, "A **drum beat**");
        assert!(matches!(
            rx.try_recv(),
            Ok(Event::Text { replace: true, .. })
        ));
        assert!(matches!(rx.try_recv(), Ok(Event::TextEnd)));
    }
}
