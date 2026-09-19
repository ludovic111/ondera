//! Euclidean lane rhythms: independent subdivisions in each bar, ordinary MIDI output.
use crate::{
    control::{self, Args, Host},
    model::{Clip, ClipData, Note, Strip},
    store::Command,
    Result,
};
use serde::Deserialize;
use serde_json::{json, Value};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Lane {
    steps: u32,
    pulses: u32,
    rotation: u32,
    pitch: u8,
    velocity: u8,
}
pub(crate) fn call(host: &mut dyn Host, args: &Args<'_>, agent: bool) -> Result<Value> {
    let lanes: Vec<Lane> =
        serde_json::from_value(args.get("lanes").cloned().ok_or("Missing lanes")?)
            .map_err(|e| e.to_string())?;
    if lanes.is_empty() || lanes.len() > 8 {
        return Err("Use 1–8 rhythm lanes".into());
    }
    let bars = args.int("bars")?;
    if !(1..=16).contains(&bars) {
        return Err("Use 1–16 bars".into());
    }
    let s = host.store().session();
    let bpb = s.beats_per_bar();
    let mut notes = vec![];
    for lane in lanes {
        if !(1..=64).contains(&lane.steps)
            || lane.pulses > lane.steps
            || lane.rotation >= lane.steps
            || lane.pitch > 127
            || !(1..=127).contains(&lane.velocity)
        {
            return Err("Each lane needs steps 1–64, pulses 0–steps, rotation 0–steps-1, pitch 0–127 and velocity 1–127".into());
        }
        let step = bpb / lane.steps as f64;
        for bar in 0..bars {
            for i in 0..lane.steps {
                if hit(i, lane.steps, lane.pulses, lane.rotation) {
                    notes.push(Note {
                        id: control::new_id("note"),
                        start: bar as f64 * bpb + i as f64 * step,
                        length: (step * 0.4).min(0.15),
                        pitch: lane.pitch,
                        velocity: lane.velocity,
                        agent,
                    });
                }
            }
        }
    }
    if notes.is_empty() {
        return Err("Add at least one pulse to the groove".into());
    }
    notes.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.pitch.cmp(&b.pitch)));
    let name = args.opt_str("name").unwrap_or("Rhythm Lab").trim();
    if name.is_empty() || name.len() > 120 {
        return Err("Groove names must be 1–120 characters".into());
    }
    let mut track = control::new_track(s, "midi", Some(name.into()), "#7cced8".into());
    // Leave headroom when several percussion lanes hit together.
    track.volume = 0.5;
    let strip = Strip {
        instrument: "Drum Machine".into(),
        ..Strip::default()
    };
    let clip = Clip {
        id: control::new_id("clip"),
        name: name.into(),
        track_id: track.id.clone(),
        start_bar: args.opt_f64("startBar").unwrap_or(0.0),
        length_bars: bars as f64,
        agent,
        data: ClipData::Midi { notes },
    };
    let result = json!({"trackId":track.id,"clipId":clip.id,"noteCount":control::midi_notes(&clip)?.len(),"excludedBySolo":s.tracks.iter().any(|t|t.solo)});
    let select = Command::Select {
        track: Some(track.id.clone()),
        clip: Some(clip.id.clone()),
        note: None,
    };
    host.dispatch(Command::Batch(vec![
        Command::AddTrack(track.clone()),
        Command::SetStrip {
            track: track.id,
            strip,
        },
        Command::PutClip(clip),
    ]))?;
    host.dispatch(select)?;
    Ok(result)
}
/// Render a groove to a WAV without touching the session: the same lanes as `rhythm.create`,
/// at the session's tempo and meter, on a scratch document.
pub(crate) fn preview(host: &dyn Host, params: &Value) -> Result<Value> {
    use base64::Engine as _;
    let bars = params["bars"].as_u64().unwrap_or(0);
    if !(1..=4).contains(&bars) {
        return Err("A preview is 1 to 4 bars".into());
    }
    let mut scratch = crate::control::Headless::new();
    scratch.store.dispatch(crate::store::Command::SetTransport(
        host.store().session().transport.clone(),
    ))?;
    let seconds = bars as f64 * scratch.store.session().beats_per_bar() * 60.0
        / scratch.store.session().transport.tempo;
    if seconds > 30.0 {
        return Err("A preview is at most 30 seconds. Choose fewer bars or a faster tempo.".into());
    }
    let mut create = json!({ "lanes": params["lanes"], "bars": bars, "startBar": 0 });
    if let Some(name) = params.get("name") {
        create["name"] = name.clone();
    }
    crate::control::call(&mut scratch, "rhythm.create", &create, false)?;
    let path = match params["path"].as_str() {
        Some(path) => std::path::PathBuf::from(path),
        // One file, replaced by each preview, so previews never pile up.
        None => crate::host::scan::data_dir()
            .join("previews")
            .join("groove.wav"),
    };
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    crate::control::call(
        &mut scratch,
        "session.exportAudio",
        &json!({
            "path": path, "sampleRate": 44100, "format": "pcm16", "startBar": 0,
            "endBar": bars, "tailSeconds": 0.5,
        }),
        false,
    )?;
    let mut result = json!({ "path": path, "seconds": seconds + 0.5, "bars": bars });
    if params["inline"].as_bool().unwrap_or(false) {
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        result["wavBase64"] = json!(base64::engine::general_purpose::STANDARD.encode(bytes));
    }
    Ok(result)
}

fn hit(index: u32, steps: u32, pulses: u32, rotation: u32) -> bool {
    (((index + steps - rotation) % steps) * pulses) % steps < pulses
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_pulse_count_and_rotation_preserves_density_and_even_spacing() {
        for steps in 1..=64 {
            for pulses in 0..=steps {
                for rotation in 0..steps {
                    let hits: Vec<_> = (0..steps)
                        .filter(|i| hit(*i, steps, pulses, rotation))
                        .collect();
                    assert_eq!(hits.len(), pulses as usize);
                    if pulses > 0 {
                        let mut gaps = vec![];
                        for (i, &value) in hits.iter().enumerate() {
                            gaps.push((hits[(i + 1) % hits.len()] + steps - value - 1) % steps + 1);
                        }
                        assert!(gaps.iter().max().unwrap() - gaps.iter().min().unwrap() <= 1);
                    }
                }
            }
        }
    }
}
