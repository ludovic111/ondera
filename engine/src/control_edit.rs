//! Commands that close the gap between what a hand can do in the window and what a script can
//! ask for: trimming a clip from its left edge, clearing the selection, reading a waveform,
//! and running many commands as one request and one undo step.
use crate::{
    control::{
        self, clip_summary, edit, find_clip, opt, query, req, selection, Host, Kind, Spec, CLIP_ID,
    },
    model::ClipData,
    store::Command,
    Result,
};
use serde_json::{json, Value};

/// Most commands one `session.batch` may carry.
pub const MAX_BATCH: usize = 1000;

pub const SPECS: &[Spec] = &[
    edit("clip.trim", "Move a clip's left edge while its content stays where it is on the timeline: notes keep their bar positions and audio keeps its alignment. The right edge does not move unless lengthBars is given.", &[
        CLIP_ID,
        req("startBar", Kind::Number, "New absolute start bar of the clip."),
        opt("lengthBars", Kind::Number, "New length in bars. Defaults to keeping the right edge in place."),
    ]),
    edit("clip.deselect", "Clear the clip and note selection, like clicking an empty lane. The selected track stays.", &[]),
    query("source.peaks", "The waveform of an audio source as the window draws it: peak magnitudes 0-1, reduced to at most `points` values.", &[
        req("sourceId", Kind::String, "Audio source id, from clip.get or session.inspect."),
        opt("points", Kind::Integer, "Most values to return, 16-4000 (default 400)."),
    ]),
    edit("session.batch", "Run many commands in one request. They share one undo step, and with atomic=true (default) a failing command rolls back every edit made before it. Far faster than separate calls: the window answers the whole list in one frame.", &[
        req("commands", Kind::Array, "List of {\"command\": name, \"params\": {...}} in the order to run. File, history, application and agent commands are not accepted."),
        opt("atomic", Kind::Boolean, "Roll back earlier edits when one command fails (default true)."),
    ]),
];

/// Parse and vet the entries of a `session.batch` request.
pub fn batch_entries(params: &Value) -> Result<Vec<(String, Value)>> {
    let list = params["commands"]
        .as_array()
        .ok_or("session.batch needs `commands`")?;
    if list.is_empty() || list.len() > MAX_BATCH {
        return Err(format!("A batch holds 1 to {MAX_BATCH} commands"));
    }
    list.iter()
        .enumerate()
        .map(|(i, entry)| {
            let name = ["command", "method", "name"]
                .iter()
                .find_map(|k| entry[*k].as_str())
                .ok_or_else(|| format!("Batch entry {i} has no `command`"))?;
            if !batchable(name) {
                return Err(format!(
                    "{name} cannot run inside a batch (entry {i}); send it on its own"
                ));
            }
            let params = match entry.get("params").or_else(|| entry.get("arguments")) {
                None | Some(Value::Null) => json!({}),
                Some(v) if v.is_object() => v.clone(),
                Some(_) => return Err(format!("Batch entry {i}: `params` must be an object")),
            };
            control::validate_request(name, &params)
                .map_err(|e| format!("Batch entry {i}: {e}"))?;
            Ok((name.to_string(), params))
        })
        .collect()
}

/// Whether a command may be part of a batch. Anything that replaces the document, touches
/// files, runs as a background job or steps the history would break the single undo step.
pub fn batchable(name: &str) -> bool {
    let family = name.split('.').next().unwrap_or("");
    !(matches!(
        family,
        "history" | "app" | "agent" | "take" | "settings" | "preset"
    ) || matches!(
        name,
        "session.batch"
            | "session.new"
            | "session.open"
            | "session.save"
            | "session.bounce"
            | "session.importAudio"
            | "session.importMidi"
            | "session.exportMidi"
            | "session.exportAudio"
            | "session.exportStems"
            | "session.restoreSnapshot"
            | "plugin.scan"
            | "ui.screenshot"
    ))
}

/// Shape the reply of a finished batch.
pub fn batch_reply(results: Vec<Value>) -> Value {
    json!({ "count": results.len(), "results": results })
}

/// The message for a batch that stopped at `index`.
pub fn batch_error(index: usize, name: &str, error: &str, rolled_back: bool) -> String {
    format!(
        "Batch stopped at entry {index} ({name}): {error}{}",
        if rolled_back {
            " Earlier edits in the batch were rolled back."
        } else {
            ""
        }
    )
}

pub(crate) fn call(host: &mut dyn Host, name: &str, params: &Value, agent: bool) -> Result<Value> {
    match name {
        "clip.trim" => {
            let id = params["clipId"]
                .as_str()
                .ok_or("clip.trim needs `clipId`")?;
            let session = host.store().session();
            let mut clip = find_clip(session, id)?.clone();
            let start = params["startBar"]
                .as_f64()
                .filter(|v| crate::model::valid_time(*v))
                .ok_or("startBar must be between 0 and 1,000,000")?;
            let length = match params["lengthBars"].as_f64() {
                Some(v) => v,
                None => clip.start_bar + clip.length_bars - start,
            };
            if !(length.is_finite() && length > 0.) {
                return Err("The new start must stay left of the clip's end".into());
            }
            let bpb = session.beats_per_bar();
            let delta = start - clip.start_bar;
            match &mut clip.data {
                ClipData::Midi { notes } => {
                    for note in notes.iter_mut() {
                        let end = note.start + note.length - delta * bpb;
                        note.start = (note.start - delta * bpb).max(0.);
                        note.length = (end.min(length * bpb) - note.start).max(0.);
                    }
                    notes.retain(|n| n.length > 0.);
                }
                ClipData::Audio { offset_seconds, .. } => {
                    let offset = *offset_seconds + delta * bpb * 60. / session.transport.tempo;
                    if offset < 0. {
                        return Err("Cannot trim before the start of the audio".into());
                    }
                    *offset_seconds = offset;
                }
            }
            clip.start_bar = start;
            clip.length_bars = length;
            host.dispatch(Command::PutClip(clip))?;
            Ok(clip_summary(find_clip(host.store().session(), id)?))
        }
        "clip.deselect" => {
            let track = host.store().session().view.selected_track_id.clone();
            host.dispatch(Command::Select {
                track,
                clip: None,
                note: None,
            })?;
            Ok(selection(host))
        }
        "source.peaks" => {
            let id = params["sourceId"]
                .as_str()
                .ok_or("source.peaks needs `sourceId`")?;
            let points = params["points"].as_i64().unwrap_or(400);
            if !(16..=4000).contains(&points) {
                return Err("points must be between 16 and 4000".into());
            }
            if !host.store().session().sources.contains_key(id) {
                return Err(format!("Unknown audio source `{id}`"));
            }
            let buffer = host
                .library()
                .get(id)
                .ok_or("That audio is still loading")?;
            let step = buffer.peaks.len().div_ceil(points as usize).max(1);
            let peaks: Vec<f64> = buffer
                .peaks
                .chunks(step)
                .map(|c| (c.iter().fold(0f32, |m, v| m.max(*v)) as f64 * 1000.).round() / 1000.)
                .collect();
            let seconds = buffer.frames.len() as f64 / buffer.sample_rate.max(1) as f64;
            Ok(json!({
                "sourceId": id, "seconds": seconds, "sampleRate": buffer.sample_rate,
                "points": peaks.len(), "secondsPerPoint": seconds / peaks.len().max(1) as f64,
                "peak": peaks.iter().cloned().fold(0., f64::max), "peaks": peaks,
            }))
        }
        "session.batch" => {
            let entries = batch_entries(params)?;
            let atomic = params["atomic"].as_bool().unwrap_or(true);
            host.store_mut().set_gesture(true);
            let mut results = Vec::with_capacity(entries.len());
            for (index, (command, params)) in entries.iter().enumerate() {
                match control::call(host, command, params, agent) {
                    Ok(value) => results.push(value),
                    Err(error) => {
                        let rolled_back = atomic && host.store_mut().cancel_gesture();
                        host.store_mut().set_gesture(false);
                        return Err(batch_error(index, command, &error, rolled_back));
                    }
                }
            }
            host.store_mut().set_gesture(false);
            Ok(batch_reply(results))
        }
        _ => Err(format!("Unknown command `{name}`")),
    }
}
