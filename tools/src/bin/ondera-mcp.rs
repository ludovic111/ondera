//! Model Context Protocol server for Ondera over stdio.
//!
//! Tools are generated from the shared command registry (`track.add` becomes `track_add`).
//! In live mode each call runs inside the open Ondera window, so an agent and a person edit the
//! same session with one undo history. Without the app, the server hosts a session itself.

use ondera_engine::control;
use ondera_tools::Backend;
use serde_json::{json, Value};
use std::{
    io::{BufRead, Read, Write},
    path::PathBuf,
};

const PROTOCOLS: [&str; 3] = ["2024-11-05", "2025-03-26", "2025-06-18"];
const USAGE: &str = "ondera-mcp — Model Context Protocol server for Ondera (stdio)

USAGE
  ondera-mcp                 control the running Ondera app; if it is not running, host a
                             session in this process
  ondera-mcp --live          require the running app
  ondera-mcp --headless      host a new empty session in this process
  ondera-mcp --file <path>   host that .ondera file in this process and save after each change

Register with an MCP client, for example in Claude Code:
  claude mcp add ondera -- /path/to/ondera-mcp";

fn main() {
    let input_args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(result) = ondera_tools::scan_child(&input_args) {
        if let Err(error) = result {
            eprintln!("{error}");
            std::process::exit(2);
        }
        return;
    }
    let mut file: Option<PathBuf> = None;
    let (mut live, mut headless) = (false, false);
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                println!("{USAGE}");
                return;
            }
            "--version" | "-V" => {
                println!("ondera-mcp {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            "--file" | "-f" => {
                let Some(path) = args.next().filter(|p| !p.starts_with("--")) else {
                    eprintln!("--file needs a path");
                    std::process::exit(2);
                };
                file = Some(PathBuf::from(path));
            }
            "--live" => live = true,
            "--headless" => headless = true,
            _ => {
                eprintln!("Unknown option `{arg}`\n\n{USAGE}");
                std::process::exit(2);
            }
        }
    }
    if usize::from(file.is_some()) + usize::from(live) + usize::from(headless) > 1 {
        eprintln!("--file, --live and --headless are exclusive");
        std::process::exit(2);
    }
    let backend = match (file, live, headless) {
        (Some(path), _, _) => Backend::headless(Some(&path), true),
        (None, true, _) => Backend::live(),
        (None, false, true) => Backend::headless(None, false),
        (None, false, false) => Backend::live().or_else(|e| {
            eprintln!("ondera-mcp: {e}\nondera-mcp: hosting a session in this process instead");
            Backend::headless(None, false)
        }),
    };
    let backend = match backend {
        Ok(b) => b,
        Err(e) => {
            eprintln!("ondera-mcp: {e}");
            std::process::exit(1);
        }
    };
    eprintln!(
        "ondera-mcp: {} mode{}",
        backend.mode(),
        backend
            .path()
            .map(|p| format!(" on {}", p.display()))
            .unwrap_or_default()
    );
    let mut server = Server { backend };
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut input = stdin.lock();
    loop {
        let mut line = String::new();
        match input
            .by_ref()
            .take(control::wire::MAX_LINE as u64)
            .read_line(&mut line)
        {
            Ok(0) | Err(_) => break,
            Ok(_) if line.len() >= control::wire::MAX_LINE => {
                eprintln!("ondera-mcp: input exceeds 64 MiB");
                break;
            }
            Ok(_) => {}
        }
        if line.trim().is_empty() {
            continue;
        }
        let Some(reply) = server.handle_line(&line) else {
            continue;
        };
        let mut out = stdout.lock();
        let written = serde_json::to_writer(&mut out, &reply)
            .map_err(std::io::Error::other)
            .and_then(|()| out.write_all(b"\n"))
            .and_then(|()| out.flush());
        if written.is_err() {
            break;
        }
    }
}

struct Server {
    backend: Backend,
}
impl Server {
    fn handle_line(&mut self, line: &str) -> Option<Value> {
        let frame: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => return Some(error(Value::Null, -32700, &format!("Parse error: {e}"))),
        };
        let Some(obj) = frame.as_object() else {
            return Some(error(
                Value::Null,
                -32600,
                "Batch requests are not supported",
            ));
        };
        let id = obj.get("id").cloned();
        if obj.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
            || obj
                .get("method")
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
            || id
                .as_ref()
                .is_some_and(|id| !id.is_null() && !id.is_string() && !id.is_number())
        {
            return Some(error(Value::Null, -32600, "Invalid JSON-RPC 2.0 request"));
        }
        let method = obj.get("method").and_then(Value::as_str).unwrap_or("");
        let params = obj.get("params").cloned().unwrap_or(Value::Null);
        // Notifications (initialized, cancelled, progress) need no answer.
        let id = id?;
        Some(match self.dispatch(method, &params) {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err((code, message)) => error(id, code, &message),
        })
    }
    fn dispatch(&mut self, method: &str, params: &Value) -> Result<Value, (i64, String)> {
        match method {
            "initialize" => {
                let requested = params
                    .get("protocolVersion")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let version = if PROTOCOLS.contains(&requested) {
                    requested
                } else {
                    PROTOCOLS[PROTOCOLS.len() - 1]
                };
                Ok(json!({
                    "protocolVersion": version,
                    "capabilities": {
                        "tools": { "listChanged": false },
                        "resources": { "subscribe": false, "listChanged": false },
                        "prompts": { "listChanged": false },
                    },
                    "serverInfo": { "name": "ondera", "version": env!("CARGO_PKG_VERSION") },
                    "instructions": self.instructions(),
                }))
            }
            "ping" => Ok(json!({})),
            "tools/list" => {
                if params.get("cursor").is_some_and(|c| !c.is_null()) {
                    return Err((-32602, "This server returned no pagination cursor".into()));
                }
                Ok(json!({ "tools": tools() }))
            }
            "tools/call" => {
                if !params.is_object() {
                    return Err((-32602, "tools/call needs an object of parameters".into()));
                }
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or((-32602, "tools/call needs `name`".to_string()))?;
                let command = name.replacen('_', ".", 1);
                if control::spec(&command).is_none() {
                    return Err((-32602, format!("Unknown tool `{name}`")));
                }
                let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);
                Ok(match self.backend.call(&command, &arguments, true) {
                    Ok(result) => {
                        let mut text = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());
                        match self.backend.autosave() {
                            Ok(Some(path)) => {
                                text.push_str(&format!("\n(saved {})", path.display()))
                            }
                            Ok(None) => {}
                            Err(e) => {
                                return Ok(json!({
                                    "content": [{ "type": "text", "text": format!("The command changed the in-memory session but autosave failed: {e}. Retry session_save before closing the server.") }],
                                    "isError": true,
                                }))
                            }
                        }
                        let mut reply = json!({
                            "content": [{ "type": "text", "text": text }],
                            "isError": false,
                        });
                        if result.is_object() {
                            reply["structuredContent"] = result;
                        }
                        reply
                    }
                    Err(message) => json!({
                        "content": [{ "type": "text", "text": message }],
                        "isError": true,
                    }),
                })
            }
            "resources/list" => Ok(
                json!({ "resources": RESOURCES.iter().map(|(uri, name, description, _)| json!({
                "uri": uri, "name": name, "description": description, "mimeType": "application/json"
            })).collect::<Vec<_>>() }),
            ),
            "resources/read" => {
                let uri = params
                    .get("uri")
                    .and_then(Value::as_str)
                    .ok_or((-32602, "resources/read needs `uri`".to_string()))?;
                let command = RESOURCES
                    .iter()
                    .find(|(u, _, _, _)| *u == uri)
                    .map(|(_, _, _, command)| *command)
                    .ok_or((-32002, format!("Unknown resource `{uri}`")))?;
                let value = self
                    .backend
                    .call(command, &Value::Null, false)
                    .map_err(|e| (-32000, e))?;
                Ok(json!({ "contents": [{
                    "uri": uri,
                    "mimeType": "application/json",
                    "text": serde_json::to_string_pretty(&value).unwrap_or_default(),
                }] }))
            }
            "resources/templates/list" => Ok(json!({ "resourceTemplates": [] })),
            "prompts/list" => Ok(json!({ "prompts": PROMPTS.iter().map(|p| json!({
                "name": p.name, "description": p.description,
                "arguments": p.arguments.iter().map(|(name, description, required)| json!({
                    "name": name, "description": description, "required": required
                })).collect::<Vec<_>>()
            })).collect::<Vec<_>>() })),
            "prompts/get" => {
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or((-32602, "prompts/get needs `name`".to_string()))?;
                let prompt = PROMPTS
                    .iter()
                    .find(|p| p.name == name)
                    .ok_or((-32602, format!("Unknown prompt `{name}`")))?;
                let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
                let text = (prompt.render)(&arguments);
                Ok(json!({
                    "description": prompt.description,
                    "messages": [{ "role": "user", "content": { "type": "text", "text": text } }]
                }))
            }
            "completion/complete" => Ok(json!({ "completion": { "values": [] } })),
            _ => Err((-32601, format!("Method not found: {method}"))),
        }
    }
    fn instructions(&self) -> String {
        let mode = match &self.backend {
            Backend::Live(_) => "Live mode: every tool runs inside the open Ondera window. A person may be editing at the same time; you share one undo history, and transport_play is audible.".to_string(),
            Backend::Headless(h, _, _) => match &h.path {
                Some(p) => format!("Headless mode on {}: the file is saved after every change. transport_play is unavailable; use session_bounce to render audio.", p.display()),
                None => "Headless mode: this process hosts a session in memory. Call session_open or session_save with a path to work on files. transport_play is unavailable; use session_bounce to render audio.".into(),
            },
        };
        format!(
            "Ondera is a digital audio workstation. {mode}\n\
             Start with session_overview: one call returns the song, sections, every track with its instrument, inserts, sends, fader, problems, clips and automation, the selection, undo history and (live) what the window shows. Drill down with note_list, strip_parameters, automation_list, controller_list or ui_state. session_catalog lists stock instruments, effects and bundled loops; plugin_list query=... searches installed Ondera-native, CLAP, VST3 and AU plugins; strip_setPlugin loads one by name or ID; strip_parameters, strip_setParameter (by name, plain value, normalized 0-1 or display text) and strip_programs / strip_setProgram control it.\n\
             Tracks, clips and markers can be named by id or by exact name. Bars and beats are zero-based. Note start/length are beats relative to the clip; pitch 60 is C4; velocity 1-127.\n\
             clip_create with notes, or clip_setNotes, writes a whole pattern in one undo step; clip_quantize and clip_transpose edit a region; history_undo with steps reverts several edits.\n\
             In live mode ui_screenshot returns a PNG of the window so you can see the interface, view_set scrolls and zooms it, and audio_status reports the engine. settings_get and settings_set read and change preferences.\n\
             Clips and notes you create are marked as agent-made so the person can see them."
        )
    }
}

/// Registry-backed resources: uri, name, description, command.
const RESOURCES: [(&str, &str, &str, &str); 9] = [
    (
        "ondera://session/overview",
        "Song overview",
        "Everything about the song in one compact answer: tracks, plugins, clips, sections, mix problems and history.",
        "session.overview",
    ),
    (
        "ondera://session",
        "Open session",
        "The complete session document as JSON.",
        "session.get",
    ),
    (
        "ondera://session/info",
        "Session summary",
        "Name, file, transport, counts and history state.",
        "session.info",
    ),
    (
        "ondera://session/inspect",
        "Arrangement and mixer",
        "Tracks, clip summaries, strips and automation without plugin state.",
        "session.inspect",
    ),
    (
        "ondera://catalog",
        "Catalog",
        "Built-in instruments, effects and loops.",
        "session.catalog",
    ),
    (
        "ondera://plugins",
        "Installed plugins",
        "The first page of scanned plugins.",
        "plugin.list",
    ),
    (
        "ondera://presets",
        "Presets",
        "Factory and user plugin presets.",
        "preset.list",
    ),
    (
        "ondera://settings",
        "Preferences",
        "Ondera settings with secrets masked.",
        "settings.get",
    ),
    (
        "ondera://app",
        "Application",
        "Version, paths and mode.",
        "app.info",
    ),
];

struct Prompt {
    name: &'static str,
    description: &'static str,
    arguments: &'static [(&'static str, &'static str, bool)],
    render: fn(&Value) -> String,
}
fn arg<'a>(arguments: &'a Value, key: &str, default: &'a str) -> &'a str {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(default)
}
const PROMPTS: [Prompt; 3] = [
    Prompt {
        name: "compose",
        description:
            "Write a short arrangement from a style, length and key using the stock instruments.",
        arguments: &[
            (
                "style",
                "Genre or mood, for example lo-fi hip hop or cinematic pad",
                true,
            ),
            ("bars", "Length in bars (default 8)", false),
            ("key", "Song key, for example A minor", false),
        ],
        render: |a| {
            format!(
            "Compose {bars} bars of {style} in {key}. Start with session_overview and session_catalog. Set the tempo and key with transport_setTempo and transport_setKey, add one instrument track per part with track_add (choose stock instruments that fit), then write each part with clip_create and a full notes array in one call. Keep every note inside its clip, use velocities between 60 and 110 for dynamics, and set sends or inserts with strip_setSendLevel and strip_setPlugin for depth. Finish with session_overview and describe what you made in a few sentences.",
            bars = arg(a, "bars", "8"), style = arg(a, "style", "a warm, simple groove"), key = arg(a, "key", "the current key")
        )
        },
    },
    Prompt {
        name: "mix-review",
        description: "Inspect the mix and propose or apply balanced levels, panning and effects.",
        arguments: &[(
            "apply",
            "true to apply the changes, otherwise only propose them",
            false,
        )],
        render: |a| {
            format!(
            "Review this session's mix: call session_overview (it lists every track's plugins, sends, fader in dB and problems), then strip_parameters where a plugin needs a closer look. Consider level balance (track_setVolume, 0.75 is unity), panning width (track_setPan), the reverb and delay sends, and the master chain. {} Keep changes small and explain each one.",
            if arg(a, "apply", "false") == "true" { "Apply the improvements with the strip and track commands, one undo step each." } else { "Do not change anything yet; list the concrete commands you would run." }
        )
        },
    },
    Prompt {
        name: "see-the-window",
        description:
            "Take a screenshot of the running app and describe what the person is looking at.",
        arguments: &[],
        render: |_| {
            "Call ui_state, then ui_screenshot and look at the image file it returns. Describe the arrangement, the selected track or region, any open panels and anything that looks wrong.".into()
        },
    },
];

fn tools() -> Vec<Value> {
    control::COMMANDS
        .iter()
        .map(|spec| {
            let destructive = spec.mutates
                && [
                    "remove", "delete", "new", "open", "quit", "reset", "restore", "setNotes",
                    "clear",
                ]
                .iter()
                .any(|w| spec.name.contains(w));
            json!({
                "name": spec.name.replacen('.', "_", 1),
                "title": spec.name,
                "description": spec.doc,
                "inputSchema": control::schema(spec),
                "annotations": {
                    "readOnlyHint": !spec.mutates,
                    "destructiveHint": destructive,
                    "idempotentHint": !spec.mutates,
                    "openWorldHint": false,
                },
            })
        })
        .collect()
}
fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}
