//! One call that tells an agent what the song is: `session.overview`. It answers what a
//! person sees at a glance across the arrangement, the inspector and the mixer, compactly
//! and with a bound on its size, and says which command drills into each part. In the app it
//! also carries `ui.state`, what the window is showing.

use crate::{
    automation::AutomationTarget,
    control::{self, opt, query, Host, Kind, Spec},
    control_params::changed_summary,
    host::scan,
    model::*,
    plugin::Descriptor,
    Result,
};
use serde_json::{json, Map, Value};
use std::collections::HashMap;

pub const SPECS: &[Spec] = &[
    query("session.overview", "Everything about the song in one compact answer; call it first. Song (tempo, meter, key, length in bars and seconds), transport (playhead, cycle, metronome), sections (markers), every track with its instrument (name, format, vendor), inserts (plugin, bypass, parameters changed from their defaults as displayed), sends, fader in dB, pan, mute/solo/arm/monitor, problems that keep it silent, clips (bars, names, note counts and pitch ranges, audio sources, fades), automation lanes and controller lanes; the buses, selection, takes, undo history and, in the app, what the window shows (ui.state). Clips per track are capped by maxClips; `truncated` says what was left out and `next` names the commands that give the details.", &[
        opt("trackId", Kind::String, "Only this track (id or name), with every clip."),
        opt("maxClips", Kind::Integer, "Clips listed per track, 0-200. Default: 12, fewer in songs with many tracks (about 48 in all); the rest are counted and their bars shown in covers."),
        opt("parameters", Kind::Boolean, "List changed plugin parameters (default true, at most 6 per plugin)."),
    ]),
    query("ui.state", "What the window shows right now: open panels and dialogs (agent, mixer, automation, controllers, palette, settings with its section, help, export, recovery), the prompt waiting for an answer, plugin windows with their track, slot and plugin, the editor (clip, mode, lowest pitch), arrangement zoom and scroll with the visible bars, tool, follow mode, browser tab and selection, selection by name, theme and scale, musical typing, status line and error. Needs the running app.", &[]),
];

const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];
/// MIDI pitch as a name, 60 = C4.
pub fn pitch_name(pitch: u8) -> String {
    format!(
        "{}{}",
        NOTE_NAMES[pitch as usize % 12],
        pitch as i32 / 12 - 1
    )
}
fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}
/// Fader position (0-1, 0.75 unity) in dB, as the mixer shows it.
pub fn fader_db(volume: f32) -> Option<f64> {
    (volume > 0.0).then(|| round2(crate::model::fader_gain(volume).log10() as f64 * 20.0))
}

struct Catalog(HashMap<String, Descriptor>);
impl Catalog {
    fn load() -> Self {
        Self(
            scan::installed()
                .into_iter()
                .map(|d| (d.id.clone(), d))
                .collect(),
        )
    }
    /// A plugin as the overview names it.
    fn plugin(&self, insert: &Insert) -> Value {
        let id = insert.plugin_id();
        let mut v = json!({ "name": insert.name, "pluginId": id });
        match self.0.get(&id) {
            Some(d) => {
                v["format"] = json!(d.format.prefix());
                if !d.vendor.is_empty() && d.format.prefix() != "stock" {
                    v["vendor"] = json!(d.vendor);
                }
            }
            None => {
                v["format"] = json!(id.split(':').next().unwrap_or("stock"));
                v["missing"] = json!(true);
            }
        }
        if insert.state == "bypassed" {
            v["bypassed"] = json!(true);
        }
        v
    }
}

fn clip_json(s: &Session, c: &Clip) -> Value {
    let mut v = json!({
        "id": c.id, "name": c.name,
        "bars": [round2(c.start_bar), round2(c.start_bar + c.length_bars)],
    });
    match &c.data {
        ClipData::Midi { notes, controllers } => {
            v["notes"] = json!(notes.len());
            if let (Some(lo), Some(hi)) = (
                notes.iter().map(|n| n.pitch).min(),
                notes.iter().map(|n| n.pitch).max(),
            ) {
                v["pitch"] = json!(format!("{}-{}", pitch_name(lo), pitch_name(hi)));
            }
            if !controllers.is_empty() {
                let mut lanes: Vec<String> = controllers
                    .iter()
                    .map(|k| match k.number {
                        Some(n) => format!("{}{n}", k.kind.as_str()),
                        None => k.kind.as_str().to_string(),
                    })
                    .collect();
                lanes.sort();
                lanes.dedup();
                v["controllers"] = json!(lanes);
            }
        }
        ClipData::Audio {
            source_id,
            offset_seconds,
            fade_in,
            fade_out,
            gain_db,
            ..
        } => {
            v["source"] = json!(s
                .sources
                .get(source_id)
                .map_or(source_id.as_str(), |x| x.name.as_str()));
            if *offset_seconds != 0.0 {
                v["offsetSeconds"] = json!(round2(*offset_seconds));
            }
            if *fade_in > 0.0 || *fade_out > 0.0 {
                v["fades"] = json!([round2(*fade_in), round2(*fade_out)]);
            }
            if *gain_db != 0.0 {
                v["gainDb"] = json!(round2(*gain_db as f64));
            }
        }
    }
    if c.agent {
        v["byAgent"] = json!(true);
    }
    v
}

/// Bars a track's clips cover, overlapping and touching ranges merged.
fn coverage(clips: &[&Clip]) -> Vec<[f64; 2]> {
    let mut spans: Vec<[f64; 2]> = clips
        .iter()
        .map(|c| [c.start_bar, c.start_bar + c.length_bars])
        .collect();
    spans.sort_by(|a, b| a[0].total_cmp(&b[0]));
    let mut out: Vec<[f64; 2]> = vec![];
    for span in spans {
        match out.last_mut() {
            Some(last) if span[0] <= last[1] + 1e-9 => last[1] = last[1].max(span[1]),
            _ => out.push(span),
        }
    }
    out.into_iter()
        .map(|[a, b]| [round2(a), round2(b)])
        .collect()
}

fn lane_name(s: &Session, target: &AutomationTarget) -> String {
    match target {
        AutomationTarget::TrackVolume { .. } => "volume".into(),
        AutomationTarget::TrackPan { .. } => "pan".into(),
        AutomationTarget::MasterVolume => "master volume".into(),
        AutomationTarget::PluginParameter {
            track_id,
            insert_id,
            parameter_id,
            ..
        } => {
            let strip = s.strips.get(track_id).cloned().unwrap_or_default();
            let plugin = strip
                .inserts
                .iter()
                .chain(strip.synth.iter())
                .find(|i| &i.id == insert_id)
                .map_or_else(|| strip.instrument_name(), |i| i.name.clone());
            format!("{plugin} #{parameter_id}")
        }
    }
}

fn automation_for(s: &Session, track: Option<&str>) -> Vec<Value> {
    s.automation
        .iter()
        .filter(|lane| lane.target.track_id() == track)
        .map(|lane| {
            let mut v = json!({
                "laneId": lane.id,
                "target": if lane.name.is_empty() { lane_name(s, &lane.target) } else { lane.name.clone() },
                "points": lane.points.len(),
            });
            if !lane.enabled {
                v["enabled"] = json!(false);
            }
            v
        })
        .collect()
}

/// What keeps a track from being heard, in words.
fn problems(
    s: &Session,
    t: &Track,
    strip: &Strip,
    catalog: &Catalog,
    failures: &[(String, String)],
    clips: usize,
) -> Vec<String> {
    let mut out = vec![];
    if t.mute {
        out.push("muted".to_string());
    }
    if !t.solo {
        let soloed: Vec<&str> = s
            .tracks
            .iter()
            .filter(|x| x.solo)
            .map(|x| x.name.as_str())
            .collect();
        if !soloed.is_empty() {
            out.push(format!("silenced by solo on {}", soloed.join(", ")));
        }
    }
    if t.volume == 0.0 {
        out.push("fader at -inf".into());
    }
    if s.master_volume == 0.0 {
        out.push("master fader at -inf".into());
    }
    if clips == 0 {
        out.push("no clips".into());
    }
    let instrument = (t.kind == "midi").then(|| {
        strip.synth.clone().unwrap_or_else(|| {
            Insert::new(
                strip.synth_key(&t.id),
                &format!("stock:{}", strip.instrument),
                &strip.instrument,
            )
        })
    });
    for insert in instrument
        .iter()
        .chain(strip.inserts.iter().filter(|i| !i.is_empty()))
    {
        let is_instrument = instrument.as_ref().is_some_and(|i| i.id == insert.id);
        if insert.state == "bypassed" && is_instrument {
            out.push(format!("instrument {} bypassed", insert.name));
        }
        if !catalog.0.contains_key(&insert.plugin_id()) {
            out.push(format!(
                "{} is not installed ({})",
                insert.name,
                insert.plugin_id()
            ));
        }
        if let Some((_, why)) = failures.iter().find(|(key, _)| *key == insert.id) {
            out.push(format!("{} failed to load: {why}", insert.name));
        }
    }
    out
}

fn inserts_json(
    host: &mut dyn Host,
    catalog: &Catalog,
    strip: &Strip,
    parameters: bool,
) -> Vec<Value> {
    strip
        .inserts
        .iter()
        .enumerate()
        .filter(|(_, i)| !i.is_empty())
        .map(|(slot, insert)| {
            let mut v = catalog.plugin(insert);
            v["slot"] = json!(slot);
            if parameters {
                if let Some(p) = changed_summary(host, insert, 6) {
                    v["changed"] = p;
                }
            }
            v
        })
        .collect()
}

fn sends_json(strip: &Strip) -> Value {
    let mut map = Map::new();
    for (i, send) in strip.sends.iter().enumerate() {
        if let Some(db) = send.level_db {
            map.insert(
                if i == 0 { "A · Reverb" } else { "B · Delay" }.into(),
                json!(round2(db as f64)),
            );
        }
    }
    Value::Object(map)
}

pub(crate) fn call(host: &mut dyn Host, name: &str, a: &control::Args) -> Result<Value> {
    if name == "ui.state" {
        return host.live(name, &json!({}));
    }
    let only = a.opt_str("trackId").map(str::to_string);
    let max_clips = match (a.opt_int("maxClips"), &only) {
        (Some(n), _) if !(0..=200).contains(&n) => return Err("maxClips must be 0-200".into()),
        (Some(n), _) => n as usize,
        (None, Some(_)) => 200,
        // About 48 clips in all, so a big song still fits one answer; `covers` shows the rest.
        (None, None) => (48 / host.store().session().tracks.len().max(1)).clamp(2, 12),
    };
    let parameters = a.opt_bool("parameters").unwrap_or(true);
    let catalog = Catalog::load();
    let failures = host.plugin_failures();
    let s = host.store().session().clone();
    let bpb = s.beats_per_bar();
    let bar_seconds = bpb * 60.0 / s.transport.tempo;
    let end = s.end_bar();
    let mut truncated: Vec<String> = vec![];

    let mut tracks = vec![];
    for (index, t) in s.tracks.iter().enumerate() {
        if only.as_ref().is_some_and(|id| *id != t.id) {
            continue;
        }
        let strip = control::full_strip(&s, &t.id);
        let clips: Vec<&Clip> = {
            let mut c: Vec<&Clip> = s.clips.iter().filter(|c| c.track_id == t.id).collect();
            c.sort_by(|a, b| a.start_bar.total_cmp(&b.start_bar));
            c
        };
        let mut v = json!({
            "index": index, "id": t.id, "name": t.name, "kind": t.kind,
            "volume": round2(t.volume as f64 * 10.0) / 10.0, "db": fader_db(t.volume), "pan": round2(t.pan as f64),
        });
        for (key, on) in [("mute", t.mute), ("solo", t.solo), ("armed", t.armed)] {
            if on {
                v[key] = json!(true);
            }
        }
        if t.kind == "audio" && t.monitor.as_str() != "off" {
            v["monitor"] = json!(t.monitor.as_str());
        }
        if t.kind == "midi" {
            let instrument = strip.synth.clone().unwrap_or_else(|| {
                Insert::new(
                    strip.synth_key(&t.id),
                    &format!("stock:{}", strip.instrument),
                    &strip.instrument,
                )
            });
            let mut i = catalog.plugin(&instrument);
            if parameters {
                if let Some(p) = changed_summary(host, &instrument, 6) {
                    i["changed"] = p;
                }
            }
            v["instrument"] = i;
        }
        let inserts = inserts_json(host, &catalog, &strip, parameters);
        if !inserts.is_empty() {
            v["inserts"] = json!(inserts);
        }
        let sends = sends_json(&strip);
        if sends.as_object().is_some_and(|m| !m.is_empty()) {
            v["sends"] = sends;
        }
        let problems = problems(&s, t, &strip, &catalog, &failures, clips.len());
        if !problems.is_empty() {
            v["problems"] = json!(problems);
        }
        let notes: usize = clips
            .iter()
            .map(|c| match &c.data {
                ClipData::Midi { notes, .. } => notes.len(),
                ClipData::Audio { .. } => 0,
            })
            .sum();
        let mut summary = json!({ "count": clips.len(), "covers": coverage(&clips) });
        if t.kind == "midi" {
            summary["notes"] = json!(notes);
        }
        summary["items"] = json!(clips
            .iter()
            .take(max_clips)
            .map(|c| clip_json(&s, c))
            .collect::<Vec<_>>());
        if clips.len() > max_clips {
            summary["more"] = json!(clips.len() - max_clips);
            truncated.push(format!("{} clips of {}", clips.len() - max_clips, t.name));
        }
        v["clips"] = summary;
        let lanes = automation_for(&s, Some(&t.id));
        if !lanes.is_empty() {
            v["automation"] = json!(lanes);
        }
        tracks.push(v);
    }

    let mut buses = Map::new();
    for bus in [MASTER, BUS_A, BUS_B] {
        let strip = control::full_strip(&s, bus);
        let mut v = json!({ "name": bus_name(bus) });
        if bus == MASTER {
            v["volume"] = json!(round2(s.master_volume as f64 * 10.0) / 10.0);
            v["db"] = json!(fader_db(s.master_volume));
            let lanes = automation_for(&s, None);
            if !lanes.is_empty() {
                v["automation"] = json!(lanes);
            }
        }
        let inserts = inserts_json(host, &catalog, &strip, parameters);
        if !inserts.is_empty() {
            v["inserts"] = json!(inserts);
        }
        let lanes = automation_for(&s, Some(bus));
        if !lanes.is_empty() {
            v["automation"] = json!(lanes);
        }
        buses.insert(bus.into(), v);
    }

    let sections: Vec<Value> = s
        .markers
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let until = s.markers.get(i + 1).map_or(end.max(m.bar), |n| n.bar);
            json!({ "id": m.id, "name": m.name, "bars": [round2(m.bar), round2(until)] })
        })
        .collect();
    let find_name = |id: &Option<String>, clips: bool| -> Value {
        id.as_ref().map_or(Value::Null, |id| {
            let name = if clips {
                s.clips.iter().find(|c| &c.id == id).map(|c| c.name.clone())
            } else if is_bus(id) {
                Some(bus_name(id).to_string())
            } else {
                s.tracks
                    .iter()
                    .find(|t| &t.id == id)
                    .map(|t| t.name.clone())
            };
            json!({ "id": id, "name": name })
        })
    };
    let view = &s.view;
    let position = host.position();
    let t = &s.transport;
    let store = host.store();
    let history = json!({
        "canUndo": store.can_undo(), "canRedo": store.can_redo(),
        "undoSteps": store.undo_depth(), "revision": store.revision, "dirty": store.dirty(),
    });
    let mut out = json!({
        "song": {
            "name": s.name, "path": host.path(), "mode": host.mode(),
            "tempo": t.tempo,
            "meter": format!("{}/{}", t.time_signature.numerator, t.time_signature.denominator),
            "key": t.key, "snap": format!("1/{}", t.snap_division),
            "lengthBars": round2(end), "lengthSeconds": round2(end * bar_seconds),
            "barSeconds": round2(bar_seconds * 1000.0) / 1000.0,
        },
        "transport": {
            "playing": host.playing(), "recording": host.recording(),
            "positionBar": round2(position / bpb), "positionSeconds": round2(position * 60.0 / t.tempo),
            "cycle": if t.cycle { json!([round2(t.cycle_start_bar), round2(t.cycle_end_bar)]) } else { Value::Null },
            "metronome": t.metronome,
        },
        "sections": sections,
        "tracks": tracks,
        "buses": buses,
        "selection": {
            "track": find_name(&view.selected_track_id, false),
            "clip": find_name(&view.selected_clip_id, true),
            "noteId": view.selected_note_id,
            "editorClip": find_name(&view.editor_clip_id, true),
        },
        "history": history,
        "conventions": "Bars and beats are zero-based (bar 0 is the first bar); note starts are beats from the clip start; 60 = C4; volume 0.75 = 0 dB. Tracks, clips and markers can be named by id or by name.",
        "next": {
            "notes": "note.list clipId=<clip> (or clip.get)",
            "parameters": "strip.parameters trackId=<track> slot=<0-7, omit for the instrument> query=<words>",
            "plugins": "plugin.list query=<words>, then strip.setPlugin plugin=<name>",
            "automation": "automation.list",
            "controllers": "controller.list clipId=<clip>",
            "window": "ui.state, ui.screenshot",
            "commands": "session.commands",
        },
    });
    if !s.sources.is_empty() && only.is_none() {
        // The browser's Files tab: audio in the project, placed with clip.create sourceId.
        let mut sources: Vec<&Source> = s.sources.values().collect();
        sources.sort_by(|a, b| a.name.cmp(&b.name));
        let used = |id: &str| {
            s.clips
                .iter()
                .filter(|c| matches!(&c.data, ClipData::Audio { source_id, .. } if source_id == id))
                .count()
        };
        out["sources"] = json!(sources
            .iter()
            .take(40)
            .map(|x| json!({
                "id": x.id, "name": x.name, "seconds": round2(x.duration_seconds), "clips": used(&x.id),
            }))
            .collect::<Vec<_>>());
        if sources.len() > 40 {
            truncated.push(format!("{} audio sources", sources.len() - 40));
        }
    }
    if let Some(takes) = crate::takes::brief(&s) {
        out["takes"] = takes;
    }
    if let Ok(window) = host.live("ui.state", &json!({})) {
        out["window"] = window;
    }
    if !truncated.is_empty() {
        out["truncated"] = json!(format!(
            "Left out: {}. Pass maxClips or trackId, or use clip.list.",
            truncated.join(", ")
        ));
    }
    Ok(out)
}
