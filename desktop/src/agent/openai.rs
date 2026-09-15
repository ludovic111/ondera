//! The OpenAI chat-completions API with streaming and function calling; also any
//! compatible server (local models, other vendors) through Settings > Agent.

use super::{
    await_tool, bounded, http, read_line_limited, system_prompt, tool_output, tool_specs,
    user_text, Event, Message, Part, ToolCall, Turn,
};
use ondera_engine::{settings::Provider, Result};
use serde_json::{json, Value};
use std::{io::BufReader, sync::atomic::Ordering, sync::mpsc};

const OPENAI: &str = "https://api.openai.com/v1";
const LINE_LIMIT: usize = 4 * 1024 * 1024;

fn tools() -> Vec<Value> {
    tool_specs()
        .into_iter()
        .map(|(_, name, doc, schema)| {
            json!({ "type": "function", "function": { "name": name, "description": doc, "parameters": schema } })
        })
        .collect()
}
fn wire(system: &str, messages: &[Message]) -> Vec<Value> {
    let mut out = vec![json!({ "role": "system", "content": system })];
    for m in messages {
        if m.role == "user" {
            let text: String = m
                .parts
                .iter()
                .filter_map(|p| match p {
                    Part::Text(t) => Some(t.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            if !text.is_empty() {
                out.push(json!({ "role": "user", "content": text }));
            }
            for part in &m.parts {
                if let Part::ToolResult { id, output, .. } = part {
                    out.push(json!({ "role": "tool", "tool_call_id": id, "content": output }));
                }
            }
        } else {
            let text: String = m
                .parts
                .iter()
                .filter_map(|p| match p {
                    Part::Text(t) => Some(t.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            let calls: Vec<Value> = m
                .parts
                .iter()
                .filter_map(|p| match p {
                    Part::ToolUse { id, name, input } => Some(json!({
                        "id": id, "type": "function",
                        "function": { "name": name, "arguments": input.to_string() }
                    })),
                    _ => None,
                })
                .collect();
            let mut message = json!({ "role": "assistant", "content": if text.is_empty() { Value::Null } else { json!(text) } });
            if !calls.is_empty() {
                message["tool_calls"] = json!(calls);
            }
            out.push(message);
        }
    }
    out
}

pub(crate) fn run(turn: Turn) -> Result<()> {
    let provider = turn.settings.agent.provider;
    let base = if provider == Provider::Compatible {
        let base = turn
            .settings
            .agent
            .compatible_base_url
            .trim()
            .trim_end_matches('/');
        if base.is_empty() {
            return Err("Set the compatible endpoint's base URL in Settings > Agent.".into());
        }
        base.to_string()
    } else {
        OPENAI.into()
    };
    let key = turn.settings.api_key(provider);
    if provider == Provider::OpenAi && key.is_none() {
        return Err("No OpenAI API key. Add one in Settings > Agent or set OPENAI_API_KEY.".into());
    }
    let model = turn.settings.model();
    if model.is_empty() {
        return Err("Set a model name in Settings > Agent.".into());
    }
    let mut history = turn.history.clone();
    history.push(Message {
        role: "user",
        parts: vec![Part::Text(user_text(&turn.prompt, &turn.session_summary))],
    });
    let agent = http();
    let tools = tools();
    let system = system_prompt(&turn.settings);
    let mut rounds = 0;
    loop {
        if turn.cancel.load(Ordering::Acquire) {
            let _ = turn.events.send(Event::Done {
                error: None,
                cancelled: true,
                history,
            });
            return Ok(());
        }
        let _ = turn
            .events
            .send(Event::Status(format!("Thinking with {model}…")));
        let mut body = json!({
            "model": model,
            "messages": wire(&system, &history),
            "tools": tools,
            "tool_choice": "auto",
            "stream": true,
            "stream_options": { "include_usage": true },
            "max_completion_tokens": turn.settings.agent.max_output_tokens,
        });
        if !turn.settings.agent.reasoning_effort.is_empty() {
            body["reasoning_effort"] = json!(turn.settings.agent.reasoning_effort);
        }
        let mut request = agent
            .post(&format!("{base}/chat/completions"))
            .header("content-type", "application/json");
        if let Some(key) = &key {
            request = request.header("authorization", &format!("Bearer {key}"));
        }
        let mut response = request
            .send(body.to_string())
            .map_err(|e| format!("Could not reach {base}: {e}"))?;
        if response.status() != 200 {
            let text = response.body_mut().read_to_string().unwrap_or_default();
            return Err(format!(
                "API error {}: {}",
                response.status(),
                api_error(&text)
            ));
        }
        let mut reader = BufReader::new(response.body_mut().as_reader());
        let mut text = String::new();
        let mut calls: Vec<(String, String, String)> = vec![];
        let mut finish = String::new();
        let mut completed = false;
        loop {
            if turn.cancel.load(Ordering::Acquire) {
                let _ = turn.events.send(Event::Done {
                    error: None,
                    cancelled: true,
                    history,
                });
                return Ok(());
            }
            let Some(line) = read_line_limited(&mut reader, LINE_LIMIT)
                .map_err(|e| format!("Stream ended: {e}"))?
            else {
                break;
            };
            let Some(payload) = line.trim().strip_prefix("data:") else {
                continue;
            };
            let payload = payload.trim();
            if payload == "[DONE]" {
                completed = true;
                break;
            }
            let chunk: Value =
                serde_json::from_str(payload).map_err(|e| format!("Invalid stream event: {e}"))?;
            if let Some(error) = chunk["error"]["message"].as_str() {
                return Err(format!("API error: {error}"));
            }
            if let Some(usage) = chunk.get("usage").filter(|u| u.is_object()) {
                let _ = turn.events.send(Event::Usage {
                    input: usage["prompt_tokens"].as_u64().unwrap_or(0),
                    output: usage["completion_tokens"].as_u64().unwrap_or(0),
                });
            }
            let Some(choice) = chunk["choices"].get(0) else {
                continue;
            };
            if let Some(reason) = choice["finish_reason"].as_str() {
                finish = reason.into();
            }
            let delta = &choice["delta"];
            if let Some(t) = delta["content"].as_str() {
                if !t.is_empty() {
                    if text.len() + t.len() > 1024 * 1024 {
                        return Err("Agent response exceeds 1 MiB".into());
                    }
                    text.push_str(t);
                    let _ = turn.events.send(Event::Text {
                        text: t.into(),
                        replace: false,
                    });
                }
            }
            for call in delta["tool_calls"].as_array().into_iter().flatten() {
                let index = call["index"].as_u64().unwrap_or(0) as usize;
                if index >= 128 {
                    return Err("Too many tool calls in one response".into());
                }
                while calls.len() <= index {
                    calls.push((String::new(), String::new(), String::new()));
                }
                if let Some(id) = call["id"].as_str() {
                    calls[index].0 = id.into();
                }
                if let Some(name) = call["function"]["name"].as_str() {
                    calls[index].1.push_str(name);
                    let _ = turn.events.send(Event::Status(format!(
                        "Preparing {}…",
                        name.replacen('_', ".", 1)
                    )));
                }
                if let Some(arguments) = call["function"]["arguments"].as_str() {
                    if calls[index].2.len() + arguments.len() > 1024 * 1024 {
                        return Err("Tool arguments exceed 1 MiB".into());
                    }
                    calls[index].2.push_str(arguments);
                }
            }
        }
        if !completed && finish.is_empty() {
            return Err("The response stream ended before completion. Try again.".into());
        }
        if matches!(finish.as_str(), "length" | "content_filter") {
            return Err(format!(
                "The provider stopped the response ({finish}); no incomplete tool was run."
            ));
        }
        if !calls.is_empty() && !matches!(finish.as_str(), "tool_calls" | "function_call" | "stop")
        {
            return Err("The provider did not finish its tool request; no tool was run.".into());
        }
        if !text.is_empty() {
            let _ = turn.events.send(Event::TextEnd);
        }
        let mut parts = vec![];
        if !text.is_empty() {
            parts.push(Part::Text(bounded(&text, super::TEXT_LIMIT)));
        }
        for (i, (id, name, arguments)) in calls.iter_mut().enumerate() {
            if id.is_empty() {
                *id = format!("call_{i}");
            }
            let input: Value = if arguments.trim().is_empty() {
                json!({})
            } else {
                serde_json::from_str(arguments)
                    .map_err(|e| format!("Invalid tool arguments: {e}"))?
            };
            parts.push(Part::ToolUse {
                id: id.clone(),
                name: name.clone(),
                input,
            });
        }
        if parts.is_empty() {
            parts.push(Part::Text(String::new()));
        }
        history.push(Message {
            role: "assistant",
            parts,
        });
        if calls.is_empty() {
            let _ = turn.events.send(Event::Done {
                error: None,
                cancelled: false,
                history,
            });
            return Ok(());
        }
        rounds += 1;
        if rounds > turn.settings.agent.max_tool_rounds {
            let _ = turn.events.send(Event::Done {
                error: Some(format!(
                    "Stopped after {} tool rounds (Settings > Agent raises the limit).",
                    turn.settings.agent.max_tool_rounds
                )),
                cancelled: false,
                history,
            });
            return Ok(());
        }
        let mut results = vec![];
        for (id, name, arguments) in calls {
            let input: Value = if arguments.trim().is_empty() {
                json!({})
            } else {
                serde_json::from_str(&arguments)
                    .map_err(|e| format!("Invalid tool arguments: {e}"))?
            };
            let (tx, rx) = mpsc::sync_channel(1);
            turn.events
                .send(Event::ToolCall(ToolCall {
                    name: name.clone(),
                    args: input,
                    reply: tx,
                }))
                .map_err(|_| "The interface stopped listening".to_string())?;
            let result = await_tool(&rx, &turn.cancel);
            let (output, is_error) = tool_output(&result);
            results.push(Part::ToolResult {
                id,
                name,
                output,
                is_error,
            });
        }
        history.push(Message {
            role: "user",
            parts: results,
        });
    }
}

fn api_error(text: &str) -> String {
    serde_json::from_str::<Value>(text)
        .ok()
        .and_then(|v| v["error"]["message"].as_str().map(str::to_string))
        .unwrap_or_else(|| bounded(text, 600))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::{atomic::AtomicBool, Arc},
    };

    fn fixture(stream: &str) -> (Result<()>, Vec<Event>, Value) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let stream = stream.to_owned();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let mut byte = [0u8; 1];
            while !request.ends_with(b"\r\n\r\n") {
                socket.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            let headers = String::from_utf8(request).unwrap();
            let length: usize = headers
                .lines()
                .find_map(|s| {
                    s.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|s| s.trim().parse().unwrap())
                })
                .unwrap();
            let mut body = vec![0; length];
            socket.read_exact(&mut body).unwrap();
            write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",stream.len(),stream).unwrap();
            serde_json::from_slice::<Value>(&body).unwrap()
        });
        let (tx, rx) = mpsc::sync_channel(64);
        let mut settings = ondera_engine::settings::Settings::default();
        settings.agent.provider = Provider::Compatible;
        settings.agent.compatible_base_url = endpoint;
        settings.agent.model = "fixture".into();
        settings.agent.reasoning_effort = "high".into();
        let result = run(Turn {
            prompt: "Hi".into(),
            history: vec![],
            settings,
            session_summary: json!({}),
            discovery: Default::default(),
            mcp_executable: String::new(),
            cancel: Arc::new(AtomicBool::new(false)),
            events: tx,
        });
        (result, rx.try_iter().collect(), server.join().unwrap())
    }

    #[test]
    fn streaming_delivers_fragments_and_passes_the_selected_effort() {
        let (result, events, request) = fixture(concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"**Hello\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\" world**\"},\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\n\n"
        ));
        result.unwrap();
        assert_eq!(request["reasoning_effort"], "high");
        let fragments: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                Event::Text {
                    text,
                    replace: false,
                } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(fragments, vec!["**Hello", " world**"]);
        assert!(events.iter().any(|e| matches!(
            e,
            Event::Done {
                error: None,
                cancelled: false,
                ..
            }
        )));
    }

    #[test]
    fn truncated_and_malformed_streams_fail_without_executing_tools() {
        for stream in [
            "data: {\"choices\":[{\"delta\":{\"content\":\"Partial\"}}]}\n\n",
            "data: invalid json\n\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":128}]}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"name\":\"track_remove\",\"arguments\":\"{}\"}}]}}]}\n\ndata: [DONE]\n\n",
        ] {
            let (result, events, _) = fixture(stream);
            assert!(result.is_err());
            assert!(!events.iter().any(|e| matches!(e, Event::ToolCall(_))));
        }
    }
}
