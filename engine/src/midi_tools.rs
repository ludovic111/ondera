//! Bounded, undoable region transformations shared by the editor, CLI and agents.
use crate::{
    control::{self, Args, Host},
    model::ClipData,
    store::Command,
    Result,
};
use serde_json::{json, Value};

pub(crate) fn call(host: &mut dyn Host, method: &str, a: &Args<'_>, agent: bool) -> Result<Value> {
    let mut clip = control::find_clip(host.store().session(), a.str("clipId")?)?.clone();
    let length = clip.length_bars * host.store().session().beats_per_bar();
    if method == "clip.repeat" {
        let count = a.int("count")?;
        if !(1..=64).contains(&count) {
            return Err("Repeat count must be 1–64".into());
        }
        let mut commands = vec![];
        let mut ids = vec![];
        for index in 1..=count {
            let mut copy = clip.clone();
            copy.id = control::new_id("clip");
            copy.start_bar += index as f64 * clip.length_bars;
            copy.agent = agent;
            if let ClipData::Midi { notes, controllers } = &mut copy.data {
                for note in notes {
                    note.id = control::new_id("note");
                    note.agent = agent;
                }
                crate::controllers::renew_ids(controllers, agent, || control::new_id("ctl"));
            }
            ids.push(copy.id.clone());
            commands.push(Command::PutClip(copy));
        }
        host.dispatch(Command::Batch(commands))?;
        return Ok(json!({"created":ids,"count":count}));
    }
    let ClipData::Midi { notes, controllers } = &mut clip.data else {
        return Err("This operation needs a MIDI region".into());
    };
    match method {
        "clip.humanize" => {
            let timing = a.opt_f64("timingMs").unwrap_or(10.0);
            let velocity = a.opt_int("velocity").unwrap_or(8);
            if !(0.0..=100.0).contains(&timing) || !(0..=32).contains(&velocity) {
                return Err("Use timingMs 0–100 and velocity 0–32".into());
            }
            let seed = a.opt_int("seed").unwrap_or(1);
            if !(0..=u32::MAX as i64).contains(&seed) {
                return Err("Seed must be an unsigned 32-bit integer".into());
            }
            let mut state = seed as u64 + 1;
            let mut random = || {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                (state >> 11) as f64 / ((1u64 << 53) as f64) * 2.0 - 1.0
            };
            let beats = timing / 1000.0 * host.store().session().transport.tempo / 60.0;
            for n in notes.iter_mut() {
                n.start = (n.start + random() * beats).clamp(0.0, (length - 0.001).max(0.0));
                n.length = n.length.min(length - n.start);
                n.velocity = (n.velocity as f64 + random() * velocity as f64)
                    .round()
                    .clamp(1.0, 127.0) as u8;
            }
        }
        "clip.velocityRamp" => {
            let from = a.int("from")?;
            let to = a.int("to")?;
            if !(1..=127).contains(&from) || !(1..=127).contains(&to) {
                return Err("Velocity must be 1–127".into());
            }
            let first = notes
                .iter()
                .map(|n| n.start)
                .min_by(f64::total_cmp)
                .unwrap_or(0.0);
            let last = notes
                .iter()
                .map(|n| n.start)
                .max_by(f64::total_cmp)
                .unwrap_or(first);
            for n in notes.iter_mut() {
                let ratio = if last > first {
                    (n.start - first) / (last - first)
                } else {
                    0.0
                };
                n.velocity = (from as f64 + (to - from) as f64 * ratio).round() as u8;
            }
        }
        "clip.fitScale" => {
            let root = a.int("root")?;
            if !(0..=11).contains(&root) {
                return Err("Scale root must be 0–11, with C=0".into());
            }
            let intervals: &[i64] = match a.str("scale")? {
                "major" => &[0, 2, 4, 5, 7, 9, 11],
                "minor" => &[0, 2, 3, 5, 7, 8, 10],
                "dorian" => &[0, 2, 3, 5, 7, 9, 10],
                "mixolydian" => &[0, 2, 4, 5, 7, 9, 10],
                "pentatonicMajor" => &[0, 2, 4, 7, 9],
                "pentatonicMinor" => &[0, 3, 5, 7, 10],
                _ => return Err(
                    "Choose major, minor, dorian, mixolydian, pentatonicMajor or pentatonicMinor"
                        .into(),
                ),
            };
            for n in notes.iter_mut() {
                // Search the finite MIDI domain; ties choose the lower pitch.
                if let Some(pitch) = (0..=127i64)
                    .filter(|p| intervals.contains(&(p - root).rem_euclid(12)))
                    .min_by_key(|p| (p - n.pitch as i64).abs())
                {
                    n.pitch = pitch as u8;
                }
            }
        }
        "clip.reverseMidi" => {
            for n in notes.iter_mut() {
                n.start = (length - n.start - n.length).max(0.0);
            }
            *controllers = crate::controllers::reverse(controllers, length);
        }
        "clip.legato" => {
            let mut starts: Vec<_> = notes
                .iter()
                .map(|n| n.start)
                .filter(|&start| start < length)
                .collect();
            starts.sort_by(f64::total_cmp);
            starts.dedup();
            // Notes left past the end by a shorter region are not heard: leave them be, or
            // their length would come out zero or negative and fail the whole edit.
            for n in notes.iter_mut().filter(|n| n.start < length) {
                let index = starts.partition_point(|&start| start <= n.start);
                n.length = starts.get(index).copied().unwrap_or(length) - n.start;
            }
        }
        _ => return Err("Unknown MIDI operation".into()),
    }
    notes.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.pitch.cmp(&b.pitch)));
    host.dispatch(Command::PutClip(clip.clone()))?;
    Ok(control::clip_summary(&clip))
}
