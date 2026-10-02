//! Tool calls in plain words. The agent's commands are the registry's (`track.add`,
//! `clip.setNotes`…); a musician should read what happened, not an API log. The chat folds
//! each run of tool calls into one block of steps between two messages.

use crate::agent::{Entry, Role, ToolRecord};
use serde_json::Value;

/// ` “name”` for a non-empty string argument, nothing otherwise.
fn quoted(v: &Value) -> String {
    match v.as_str() {
        Some(s) if !s.is_empty() => format!(" “{s}”"),
        _ => String::new(),
    }
}

/// ` 3 notes` for an array argument, nothing otherwise.
fn count(v: &Value, noun: &str) -> String {
    match v.as_array() {
        Some(items) => format!(
            " {} {noun}{}",
            items.len(),
            if items.len() == 1 { "" } else { "s" }
        ),
        None => String::new(),
    }
}

/// A JSON value as a sentence shows it: numbers without a trailing `.0`, strings bare.
fn plain(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "?".into(),
        other => other.to_string(),
    }
}

/// The command name as the registry spells it: tools come as `track_add`, calls through
/// the bridge as `track.add`.
fn dotted(name: &str) -> String {
    name.replacen('_', ".", 1)
}

/// The phrase for a command that edits, or `None` when it has none.
fn phrase(name: &str, a: &Value) -> Option<String> {
    let q = |key: &str| quoted(&a[key]);
    let either = |on: bool, yes: &str, no: &str| if on { yes } else { no }.to_string();
    Some(match name {
        "track.add" => format!(
            "Added {} track{}",
            if a["kind"] == "audio" {
                "an audio"
            } else {
                "an instrument"
            },
            q("name")
        ),
        "track.remove" => "Removed a track".into(),
        "track.rename" => format!("Renamed a track to{}", q("name")),
        "track.setVolume" => "Set a track's volume".into(),
        "track.setPan" => "Set a track's pan".into(),
        "track.setMute" => either(a["muted"] == false, "Unmuted a track", "Muted a track"),
        "track.setSolo" => either(a["solo"] == false, "Unsoloed a track", "Soloed a track"),
        "track.duplicate" => "Duplicated a track".into(),
        "clip.create" => format!(
            "Created a region{}{}",
            q("name"),
            count(&a["notes"], "note")
        ),
        "clip.setNotes" => {
            let notes = count(&a["notes"], "note");
            format!(
                "Wrote{}",
                if notes.is_empty() {
                    " notes".into()
                } else {
                    notes
                }
            )
        }
        "clip.move" => "Moved a region".into(),
        "clip.resize" => "Resized a region".into(),
        "clip.trim" => "Trimmed a region".into(),
        "clip.split" => "Split a region".into(),
        "clip.duplicate" => "Duplicated a region".into(),
        "clip.remove" => "Deleted a region".into(),
        "clip.addLoop" => format!("Added the loop{}", q("name")),
        "clip.quantize" => "Quantized a region".into(),
        "clip.transpose" => format!("Transposed by {} semitones", plain(&a["semitones"])),
        "clip.humanize" => "Humanized the timing".into(),
        "clip.fitScale" => "Fitted the notes to the scale".into(),
        "clip.setFades" => "Shaped a region's fades".into(),
        "clip.setGain" => format!("Set a region's gain to {} dB", plain(&a["gainDb"])),
        "marker.add" => format!("Marked a section{}", q("name")),
        "marker.rename" => format!("Renamed a section to{}", q("name")),
        "marker.move" => "Moved a section marker".into(),
        "marker.remove" => "Removed a section marker".into(),
        "marker.goto" => {
            let name = q("name");
            format!(
                "Went to{}",
                if name.is_empty() {
                    " a marker".into()
                } else {
                    name
                }
            )
        }
        "marker.cycleSection" => "Looped a section".into(),
        "note.add" => "Added a note".into(),
        "note.update" => "Changed a note".into(),
        "note.remove" => "Removed a note".into(),
        "rhythm.create" => "Built a rhythm".into(),
        "generate.audio" => {
            let kind = a["kind"].as_str().unwrap_or("loop");
            let what = if kind == "instrument" {
                "an instrument".to_string()
            } else {
                format!("a {kind}")
            };
            let prompt = a["prompt"]
                .as_str()
                .filter(|p| !p.is_empty())
                .map(|p| format!(" “{}”", p.chars().take(40).collect::<String>()))
                .unwrap_or_default();
            format!("Generated {what}{prompt}")
        }
        "generate.place" => "Placed a generated sound".into(),
        "strip.loadSample" => "Turned a sound into an instrument".into(),
        "strip.setPlugin" => {
            let id = a["pluginId"].as_str().unwrap_or("a plugin");
            // `clap:Vendor Name` reads as the plugin's name.
            let shown = match id.split_once(':') {
                Some((prefix, rest))
                    if !prefix.is_empty()
                        && prefix
                            .chars()
                            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()) =>
                {
                    rest
                }
                _ => id,
            };
            format!("Loaded {shown}")
        }
        "strip.setInstrument" => format!("Chose the instrument{}", q("instrument")),
        "strip.setInsert" => {
            if a["effect"].as_str().is_some_and(|e| !e.is_empty()) {
                format!("Inserted{}", q("effect"))
            } else {
                match a["bypassed"].as_bool() {
                    Some(true) => "Bypassed an effect".into(),
                    Some(false) => "Enabled an effect".into(),
                    None => "Cleared an insert".into(),
                }
            }
        }
        "strip.setParameter" => "Turned a dial".into(),
        "strip.setParameters" => "Dialled in a sound".into(),
        "strip.setSendLevel" => "Set a send level".into(),
        "strip.setBypass" => either(
            a["bypassed"].as_bool().unwrap_or(false),
            "Bypassed an effect",
            "Enabled an effect",
        ),
        "preset.load" => format!("Loaded the preset{}", q("name")),
        "master.setVolume" => "Set the master volume".into(),
        "transport.setTempo" => format!("Set the tempo to {} bpm", plain(&a["bpm"])),
        "transport.setKey" => format!("Set the key to {}", plain(&a["key"])),
        "transport.setTimeSignature" => format!(
            "Set the metre to {}/{}",
            plain(&a["numerator"]),
            plain(&a["denominator"])
        ),
        "transport.setCycle" => "Set the cycle range".into(),
        "transport.play" => "Started playback".into(),
        "transport.stop" => "Stopped playback".into(),
        "automation.create" => "Added an automation lane".into(),
        "automation.setPoints" => "Drew automation".into(),
        "session.batch" => {
            let edits = count(&a["commands"], "edit");
            format!(
                "Made{} in one step",
                if edits.is_empty() {
                    " several edits".into()
                } else {
                    edits
                }
            )
        }
        "session.importAudio" => "Imported audio".into(),
        "session.importMidi" => "Imported a MIDI file".into(),
        "session.exportAudio" => "Exported the mix".into(),
        "session.exportStems" => "Exported stems".into(),
        "session.save" => "Saved the session".into(),
        "take.create" => format!("Saved the take{}", q("name")),
        "ui.screenshot" => "Looked at the window".into(),
        _ => return None,
    })
}

/// Commands that only look: `*.list`, `*.get`, `*.inspect`…
const LOOKING: &[&str] = &[
    "list",
    "get",
    "info",
    "inspect",
    "catalog",
    "commands",
    "status",
    "parameters",
    "describe",
    "folders",
    "devices",
    "peaks",
    "snapshots",
    "providers",
    "transcript",
    "changes",
    "overview",
];

fn looking(name: &str) -> bool {
    name.rsplit_once('.')
        .is_some_and(|(_, action)| LOOKING.contains(&action))
}

/// One line for a tool call. Reads are described as looking at a part of the project.
pub fn describe_tool(name: &str, args: &Value) -> String {
    let name = dotted(name);
    if let Some(text) = phrase(&name, args) {
        return text;
    }
    if looking(&name) {
        let subject = name.split('.').next().unwrap_or("");
        let what = match subject {
            "session" => "the project",
            "track" => "the tracks",
            "clip" => "the regions",
            "note" => "the notes",
            "strip" => "the channel",
            "plugin" => "the plugin library",
            "preset" => "the presets",
            "automation" => "the automation",
            "view" => "the view",
            "source" => "the audio",
            "history" => "the undo history",
            "marker" => "the song sections",
            other => other,
        };
        return format!("Looked at {what}");
    }
    name
}

/// A call that only reads the project or the window.
pub fn is_reading(name: &str) -> bool {
    let name = dotted(name);
    looking(&name) || name.starts_with("ui")
}

/// Whether a step worked: a call still waiting for its answer counts as working.
pub fn tool_ok(tool: &ToolRecord) -> bool {
    tool.result.as_ref().is_none_or(|r| r.is_ok())
}

/// Why a step failed, as the host said it.
pub fn tool_error(tool: &ToolRecord) -> String {
    match &tool.result {
        Some(Err(message)) => message.clone(),
        Some(Ok(value)) => value["error"]
            .as_str()
            .map(str::to_string)
            .unwrap_or_default(),
        None => String::new(),
    }
}

/// One item of the conversation: a message, or the steps between two messages. Each is
/// keyed by its first entry's id, so trimming old entries never hands one item's state (an
/// open step) to another.
#[derive(Debug, PartialEq)]
pub enum Item {
    Message { key: u64, index: usize },
    Steps { key: u64, indices: Vec<usize> },
}

impl Item {
    #[cfg(test)]
    pub fn key(&self) -> u64 {
        match self {
            Item::Message { key, .. } | Item::Steps { key, .. } => *key,
        }
    }
}

/// Messages in order, each run of tool calls folded into one block of steps. Notices stay
/// out of the chat; Changes shows them.
pub fn thread_items(entries: &[Entry], first_id: u64) -> Vec<Item> {
    let mut items: Vec<Item> = vec![];
    for (index, entry) in entries.iter().enumerate() {
        let key = first_id + index as u64;
        match entry.role {
            Role::Tool if entry.tool.is_some() => match items.last_mut() {
                Some(Item::Steps { indices, .. }) => indices.push(index),
                _ => items.push(Item::Steps {
                    key,
                    indices: vec![index],
                }),
            },
            Role::User | Role::Assistant => items.push(Item::Message { key, index }),
            _ => {}
        }
    }
    items
}

/// What the status line says while the agent works, from the runtime's status.
pub fn human_status(status: &str, running: bool, error: bool) -> &'static str {
    let lower = status.to_lowercase();
    let has = |needle: &str| status.contains(needle);
    if error {
        return "Could not finish this request";
    }
    if status.starts_with("Stopping") {
        return "Stopping…";
    }
    if status.starts_with("Stopped") {
        return "Stopped";
    }
    if !running {
        return "Ready when you are";
    }
    if lower.starts_with("starting")
        || lower.starts_with("checking")
        || lower.contains("connecting")
    {
        return "Connecting…";
    }
    if has("strip.setPlugin") || has("strip.setInstrument") || has("plugin.") {
        return "Loading an instrument or effect…";
    }
    if has("strip.") || has("master.") || has("preset.") {
        return "Adjusting the sound…";
    }
    if has("clip.") || has("note.") || has("track.add") {
        return "Working on your music…";
    }
    if has("transport.") {
        return "Preparing playback…";
    }
    if has("session.save") || has("session.export") || has("session.bounce") {
        return "Preparing your audio files…";
    }
    if has("session.") || has("track.") || has("audio.") {
        return "Checking your project…";
    }
    if status.starts_with("Finished") {
        return "Tool finished · thinking…";
    }
    "Thinking…"
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_the_registrys_own_parameter_names() {
        let step = |name, args| describe_tool(name, &args);
        assert_eq!(
            step("track.setMute", json!({"trackId":"t","muted":false})),
            "Unmuted a track"
        );
        assert_eq!(
            step("track.setMute", json!({"trackId":"t","muted":true})),
            "Muted a track"
        );
        assert_eq!(
            step("track_add", json!({"kind":"midi","name":"Drums"})),
            "Added an instrument track “Drums”"
        );
        assert_eq!(
            step("clip.create", json!({"notes":[1,2,3]})),
            "Created a region 3 notes"
        );
        assert_eq!(
            step("transport.setTempo", json!({"bpm":96})),
            "Set the tempo to 96 bpm"
        );
        assert_eq!(
            step("strip.setPlugin", json!({"pluginId":"clap:Vital"})),
            "Loaded Vital"
        );
    }

    #[test]
    fn tells_a_bypass_from_a_cleared_slot() {
        let step = |args| describe_tool("strip.setInsert", &args);
        assert_eq!(
            step(json!({"slot":0,"bypassed":true})),
            "Bypassed an effect"
        );
        assert_eq!(step(json!({"slot":0})), "Cleared an insert");
        assert_eq!(
            step(json!({"slot":0,"effect":"Chorus"})),
            "Inserted “Chorus”"
        );
    }

    #[test]
    fn reads_are_looking_and_unknown_commands_keep_their_name() {
        assert_eq!(
            describe_tool("session.inspect", &json!({})),
            "Looked at the project"
        );
        assert_eq!(
            describe_tool("plugin_list", &json!({})),
            "Looked at the plugin library"
        );
        assert!(is_reading("track.list"));
        assert!(is_reading("session_overview"));
        assert!(is_reading("ui.state"));
        assert!(!is_reading("clip.create"));
        assert_eq!(describe_tool("future.thing", &json!({})), "future.thing");
    }

    #[test]
    fn maps_unknown_tool_names_and_failures_to_human_status() {
        assert_eq!(
            human_status("Running unknown_future_tool", true, false),
            "Thinking…"
        );
        assert_eq!(
            human_status("HTTP 429", false, true),
            "Could not finish this request"
        );
        assert_eq!(
            human_status("Running session.inspect…", true, false),
            "Checking your project…"
        );
        assert_eq!(human_status("", false, false), "Ready when you are");
        assert_eq!(human_status("Starting…", true, false), "Connecting…");
        assert_eq!(
            human_status("Running strip.setPlugin…", true, false),
            "Loading an instrument or effect…"
        );
    }

    fn entry(role: Role, tool: bool) -> Entry {
        Entry {
            role,
            text: "x".into(),
            tool: tool.then(|| ToolRecord {
                name: "track.add".into(),
                args: json!({}),
                result: Some(Ok(json!({}))),
                sequence: None,
            }),
            streaming: false,
        }
    }

    #[test]
    fn keys_conversation_items_by_entry_id_so_trimming_keeps_each_items_identity() {
        let before = thread_items(
            &[
                entry(Role::User, false),
                entry(Role::Tool, true),
                entry(Role::Tool, true),
                entry(Role::Assistant, false),
            ],
            7,
        );
        assert_eq!(before.iter().map(Item::key).collect::<Vec<_>>(), [7, 8, 10]);
        let after = thread_items(&[entry(Role::Tool, true), entry(Role::Assistant, false)], 9);
        assert_eq!(after.iter().map(Item::key).collect::<Vec<_>>(), [9, 10]);
        // Notices stay out of the chat.
        assert!(thread_items(&[entry(Role::Notice, false)], 0).is_empty());
    }

    #[test]
    fn failures_carry_the_hosts_reason() {
        let failed = ToolRecord {
            name: "clip.create".into(),
            args: json!({}),
            result: Some(Err("Track is full".into())),
            sequence: None,
        };
        assert!(!tool_ok(&failed));
        assert_eq!(tool_error(&failed), "Track is full");
    }
}
