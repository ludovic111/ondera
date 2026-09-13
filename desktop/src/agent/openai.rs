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
        let body = json!({
            "model": model,
            "messages": wire(&system, &history),
            "tools": tools,
            "tool_choice": "auto",
            "stream": true,
            "stream_options": { "include_usage": true },
            "max_completion_tokens": turn.settings.agent.max_output_tokens,
        });
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
                break;
            }
            let chunk: Value = serde_json::from_str(payload).unwrap_or(Value::Null);
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
                    text.push_str(t);
                    let _ = turn.events.send(Event::Text {
                        text: t.into(),
                        replace: false,
                    });
                }
            }
            for call in delta["tool_calls"].as_array().into_iter().flatten() {
                let index = call["index"].as_u64().unwrap_or(0) as usize;
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
                    calls[index].2.push_str(arguments);
                }
            }
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
                serde_json::from_str(arguments).unwrap_or(json!({}))
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
        if calls.is_empty()
            || (finish != "tool_calls"
                && finish != "function_call"
                && !finish.is_empty()
                && finish != "stop")
        {
            let _ = turn.events.send(Event::Done {
                error: None,
                cancelled: false,
                history,
            });
            return Ok(());
        }
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
                serde_json::from_str(&arguments).unwrap_or(json!({}))
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
