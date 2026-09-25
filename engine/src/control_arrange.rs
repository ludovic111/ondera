//! Arrangement commands: audio clip fades and gain, and markers (the song's sections on the
//! ruler) with navigation between them.
//!
//! Fades are in seconds, like `offsetSeconds`: audio clips are not stretched with the tempo, so
//! a fade is a property of the sound, and the renderer keeps it inside the clip whatever the
//! tempo does. Markers are bars, like clip positions.
use crate::{
    control::{
        check_color, clip_summary, edit, find_clip, new_id, opt, query, req, transport, Args, Host,
        Kind, Spec, CLIP_ID,
    },
    model::{
        clamp_fades, valid_time, ClipData, FadeCurve, Marker, Session, CLIP_GAIN_MAX_DB,
        CLIP_GAIN_MIN_DB, MAX_MARKERS,
    },
    store::Command,
    Result,
};
use serde_json::{json, Value};

const MARKER_ID: crate::control::Param = req(
    "markerId",
    Kind::String,
    "Marker id, as listed by marker.list.",
);

pub const SPECS: &[Spec] = &[
    edit("clip.setFades", "Set an audio clip's fade in, fade out and fade curve, in one undo step. Omitted values keep theirs. Fades are seconds of audio and are kept inside the clip: when they would overlap they shrink in proportion. Split, trim and resize keep them sensible.", &[
        CLIP_ID,
        opt("fadeInSeconds", Kind::Number, "Fade-in length in seconds; 0 removes it."),
        opt("fadeOutSeconds", Kind::Number, "Fade-out length in seconds; 0 removes it."),
        opt("curve", Kind::String, "equalPower (default: keeps loudness through a crossfade), linear or exponential (slow start)."),
    ]),
    edit("clip.setGain", "Set an audio clip's gain, applied before the track's inserts and fader.", &[
        CLIP_ID,
        req("gainDb", Kind::Number, "Gain in dB, -60 to +24; 0 is unchanged."),
    ]),
    query("marker.list", "List the song's markers (sections) in bar order, with the bar where each section ends.", &[]),
    edit("marker.add", "Add a marker, the start of a song section, to the ruler.", &[
        opt("bar", Kind::Number, "Zero-based bar. Defaults to the playhead, on the snap grid."),
        opt("name", Kind::String, "Section name such as \"Verse 1\" or \"Chorus\". Defaults to \"Marker N\"."),
        opt("color", Kind::String, "CSS colour: #rrggbb or oklch(l c h). Defaults to the theme's marker colour."),
    ]),
    edit("marker.rename", "Rename a marker (a song section such as Verse or Chorus). One undo step.", &[MARKER_ID, req("name", Kind::String, "New name, 1-120 characters.")]),
    edit("marker.move", "Move a marker to another bar of the ruler. One undo step.", &[MARKER_ID, req("bar", Kind::Number, "Zero-based bar.")]),
    edit("marker.setColor", "Colour a marker, or give it back the theme's colour.", &[MARKER_ID, opt("color", Kind::String, "CSS colour: #rrggbb or oklch(l c h). Omit or null for the theme's colour.")]),
    edit("marker.remove", "Delete a marker from the ruler; the music does not change. One undo step.", &[MARKER_ID]),
    edit("marker.goto", "Move the playhead to a marker, by id or by name.", &[
        opt("markerId", Kind::String, "Marker id from marker.list."),
        opt("name", Kind::String, "Marker name, case-insensitive, when no id is given."),
    ]),
    edit("marker.next", "Move the playhead to the next marker after it. Answers with marker null, and leaves the playhead, when there is none.", &[]),
    edit("marker.previous", "Move the playhead to the marker before it. Answers with marker null, and leaves the playhead, when there is none.", &[]),
    edit("marker.cycleSection", "Cycle one song section: set the cycle from a marker to the next marker (or the end of the song) and turn cycle on.", &[
        opt("markerId", Kind::String, "Marker that starts the section. Defaults to the section the playhead is in."),
    ]),
];

/// Two markers closer than this are on the same bar.
const SAME_BAR: f64 = 1e-6;

pub(crate) fn find_marker<'a>(s: &'a Session, id: &str) -> Result<&'a Marker> {
    s.markers
        .iter()
        .find(|m| m.id == id)
        .ok_or_else(|| format!("Unknown marker `{id}`. Use marker.list."))
}

fn marker_json(s: &Session, m: &Marker) -> Value {
    json!({
        "id": m.id,
        "bar": m.bar,
        "name": m.name,
        "color": m.color,
        "endBar": section_end(s, m.bar),
    })
}

/// Where the section starting at `bar` ends: the next marker, or the end of the song rounded up
/// to a whole bar (at least one bar later).
pub fn section_end(s: &Session, bar: f64) -> f64 {
    s.markers
        .iter()
        .map(|m| m.bar)
        .find(|b| *b > bar + SAME_BAR)
        .unwrap_or_else(|| s.end_bar().ceil().max(bar.floor() + 1.0))
}

fn check_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 120 {
        return Err("A marker name has 1 to 120 characters".into());
    }
    Ok(name.to_string())
}

fn check_bar(bar: f64) -> Result<f64> {
    if valid_time(bar) {
        Ok(bar)
    } else {
        Err("bar must be between 0 and 1,000,000".into())
    }
}

fn free_bar(s: &Session, bar: f64, except: Option<&str>) -> Result<()> {
    match s
        .markers
        .iter()
        .find(|m| (m.bar - bar).abs() < SAME_BAR && Some(m.id.as_str()) != except)
    {
        Some(m) => Err(format!(
            "Marker \"{}\" is already at bar {}",
            m.name,
            m.bar + 1.0
        )),
        None => Ok(()),
    }
}

/// "Marker N" with the first N from the marker count up that no marker uses yet.
fn default_name(s: &Session) -> String {
    (s.markers.len() + 1..)
        .map(|n| format!("Marker {n}"))
        .find(|name| !s.markers.iter().any(|m| &m.name == name))
        .unwrap_or_else(|| "Marker".into())
}

fn playhead_bar(host: &dyn Host) -> f64 {
    let s = host.store().session();
    let bar = host.position() / s.beats_per_bar();
    // One step of the snap grid, in bars.
    let step = 4.0 / s.transport.snap_division as f64 / s.beats_per_bar();
    ((bar / step).round() * step).max(0.0)
}

fn locate_marker(host: &mut dyn Host, marker: Option<Marker>) -> Result<Value> {
    if let Some(m) = &marker {
        let beats = m.bar * host.store().session().beats_per_bar();
        host.locate(beats)?;
    }
    let s = host.store().session();
    Ok(json!({
        "marker": marker.as_ref().map(|m| marker_json(s, m)),
        "transport": transport(host),
    }))
}

/// The bar a `transport.locate` with `markerId` goes to.
pub(crate) fn marker_bar(s: &Session, id: &str) -> Result<f64> {
    find_marker(s, id).map(|m| m.bar)
}

pub(crate) fn call(host: &mut dyn Host, name: &str, a: &Args) -> Result<Value> {
    match name {
        "clip.setFades" | "clip.setGain" => {
            let s = host.store().session();
            let mut clip = find_clip(s, a.str("clipId")?)?.clone();
            let seconds = clip.length_bars * s.beats_per_bar() * 60.0 / s.transport.tempo;
            let ClipData::Audio {
                fade_in,
                fade_out,
                fade_curve,
                gain_db,
                ..
            } = &mut clip.data
            else {
                return Err(
                    "Only audio clips have fades and gain; MIDI dynamics are note velocities"
                        .into(),
                );
            };
            if name == "clip.setGain" {
                let db = a.f64("gainDb")?;
                if !(CLIP_GAIN_MIN_DB as f64..=CLIP_GAIN_MAX_DB as f64).contains(&db) {
                    return Err(format!(
                        "Clip gain must be between {CLIP_GAIN_MIN_DB} and +{CLIP_GAIN_MAX_DB} dB"
                    ));
                }
                *gain_db = db as f32;
            } else {
                let length = |key: &str| -> Result<Option<f64>> {
                    match a.opt_f64(key) {
                        Some(v) if valid_time(v) => Ok(Some(v)),
                        Some(_) => Err(format!("{key} must be 0 or more seconds")),
                        None => Ok(None),
                    }
                };
                let wanted_in = length("fadeInSeconds")?.unwrap_or(*fade_in);
                let wanted_out = length("fadeOutSeconds")?.unwrap_or(*fade_out);
                (*fade_in, *fade_out) = clamp_fades(wanted_in, wanted_out, seconds);
                if let Some(curve) = a.opt_str("curve") {
                    *fade_curve = FadeCurve::parse(curve)?;
                }
            }
            let id = clip.id.clone();
            host.dispatch(Command::PutClip(clip))?;
            Ok(clip_summary(find_clip(host.store().session(), &id)?))
        }
        "marker.list" => {
            let s = host.store().session();
            Ok(Value::Array(
                s.markers.iter().map(|m| marker_json(s, m)).collect(),
            ))
        }
        "marker.add" => {
            let bar = match a.opt_f64("bar") {
                Some(bar) => check_bar(bar)?,
                None => playhead_bar(host),
            };
            let s = host.store().session();
            if s.markers.len() >= MAX_MARKERS {
                return Err(format!("A song holds at most {MAX_MARKERS} markers"));
            }
            free_bar(s, bar, None)?;
            let marker = Marker {
                id: new_id("marker"),
                bar,
                name: match a.opt_str("name") {
                    Some(name) => check_name(name)?,
                    None => default_name(s),
                },
                color: a
                    .opt_str("color")
                    .map(|c| check_color(c).map(str::to_string))
                    .transpose()?,
            };
            let id = marker.id.clone();
            host.dispatch(Command::PutMarker(marker))?;
            let s = host.store().session();
            Ok(marker_json(s, find_marker(s, &id)?))
        }
        "marker.rename" | "marker.move" | "marker.setColor" => {
            let s = host.store().session();
            let mut marker = find_marker(s, a.str("markerId")?)?.clone();
            match name {
                "marker.rename" => marker.name = check_name(a.str("name")?)?,
                "marker.move" => {
                    marker.bar = check_bar(a.f64("bar")?)?;
                    free_bar(s, marker.bar, Some(&marker.id))?;
                }
                _ => {
                    marker.color = a
                        .opt_str("color")
                        .map(|c| check_color(c).map(str::to_string))
                        .transpose()?
                }
            }
            let id = marker.id.clone();
            host.dispatch(Command::PutMarker(marker))?;
            let s = host.store().session();
            Ok(marker_json(s, find_marker(s, &id)?))
        }
        "marker.remove" => {
            let id = a.str("markerId")?;
            find_marker(host.store().session(), id)?;
            host.dispatch(Command::RemoveMarker(id.into()))?;
            Ok(json!({ "removed": id, "markerCount": host.store().session().markers.len() }))
        }
        "marker.goto" => {
            let s = host.store().session();
            let marker = match (a.opt_str("markerId"), a.opt_str("name")) {
                (Some(id), _) => find_marker(s, id)?.clone(),
                (None, Some(name)) => s
                    .markers
                    .iter()
                    .find(|m| m.name.to_lowercase() == name.trim().to_lowercase())
                    .cloned()
                    .ok_or_else(|| {
                        format!(
                            "No marker named \"{name}\". Markers: {}",
                            s.markers
                                .iter()
                                .map(|m| m.name.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    })?,
                (None, None) => return Err("marker.goto needs `markerId` or `name`".into()),
            };
            locate_marker(host, Some(marker))
        }
        "marker.next" | "marker.previous" => {
            let s = host.store().session();
            let bar = host.position() / s.beats_per_bar();
            let marker = if name == "marker.next" {
                s.markers.iter().find(|m| m.bar > bar + SAME_BAR)
            } else {
                s.markers.iter().rev().find(|m| m.bar < bar - SAME_BAR)
            }
            .cloned();
            locate_marker(host, marker)
        }
        "marker.cycleSection" => {
            let s = host.store().session();
            let start = match a.opt_str("markerId") {
                Some(id) => find_marker(s, id)?.bar,
                None => {
                    let bar = host.position() / s.beats_per_bar();
                    s.markers
                        .iter()
                        .rev()
                        .find(|m| m.bar <= bar + SAME_BAR)
                        .map(|m| m.bar)
                        .ok_or("The playhead is before the first marker; pass markerId")?
                }
            };
            let mut t = s.transport.clone();
            t.cycle = true;
            t.cycle_start_bar = start;
            t.cycle_end_bar = section_end(s, start);
            host.dispatch(Command::SetTransport(t))?;
            Ok(transport(host))
        }
        _ => Err(format!("Unknown command `{name}`")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::{call, Headless};

    fn host_with_audio() -> (Headless, String) {
        let mut host = Headless::new();
        let track = call(&mut host, "track.add", &json!({"kind":"audio"}), false).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();
        let source = crate::model::Source {
            id: "src".into(),
            name: "Tone".into(),
            sample_rate: 48000,
            channels: 2,
            file_name: Some("tone.wav".into()),
            duration_seconds: 8.0,
            origin: "file".into(),
            seed: None,
            wave_kind: None,
        };
        host.store.dispatch(Command::PutSource(source)).unwrap();
        let clip = call(
            &mut host,
            "clip.create",
            &json!({"trackId": track, "startBar": 0, "lengthBars": 2, "sourceId": "src"}),
            false,
        )
        .unwrap();
        (host, clip["id"].as_str().unwrap().to_string())
    }

    fn seconds_per_bar(host: &Headless) -> f64 {
        let s = host.store.session();
        s.beats_per_bar() * 60.0 / s.transport.tempo
    }

    #[test]
    fn fades_and_gain_are_undoable_and_stay_inside_the_clip() {
        let (mut host, id) = host_with_audio();
        let seconds = 2.0 * seconds_per_bar(&host);
        let r = call(
            &mut host,
            "clip.setFades",
            &json!({"clipId": id, "fadeInSeconds": 0.5, "curve": "linear"}),
            false,
        )
        .unwrap();
        assert_eq!(r["fadeInSeconds"], 0.5);
        assert_eq!(r["fadeOutSeconds"], 0.0);
        assert_eq!(r["fadeCurve"], "linear");
        let r = call(
            &mut host,
            "clip.setFades",
            &json!({"clipId": id, "fadeInSeconds": seconds, "fadeOutSeconds": seconds}),
            false,
        )
        .unwrap();
        let (fade_in, fade_out) = (
            r["fadeInSeconds"].as_f64().unwrap(),
            r["fadeOutSeconds"].as_f64().unwrap(),
        );
        assert!((fade_in + fade_out - seconds).abs() < 1e-9);
        assert!(
            (fade_in - fade_out).abs() < 1e-9,
            "overlap shrinks both alike"
        );
        call(
            &mut host,
            "clip.setGain",
            &json!({"clipId": id, "gainDb": -6}),
            false,
        )
        .unwrap();
        assert!(call(
            &mut host,
            "clip.setGain",
            &json!({"clipId": id, "gainDb": 30}),
            false
        )
        .is_err());
        assert!(call(
            &mut host,
            "clip.setFades",
            &json!({"clipId": id, "curve": "cubic"}),
            false
        )
        .is_err());
        call(&mut host, "history.undo", &json!({}), false).unwrap();
        let clip = find_clip(host.store.session(), &id).unwrap();
        let ClipData::Audio {
            gain_db,
            fade_curve,
            ..
        } = &clip.data
        else {
            unreachable!()
        };
        assert_eq!(*gain_db, 0.0);
        assert_eq!(*fade_curve, FadeCurve::Linear);
    }

    #[test]
    fn midi_clips_refuse_fades() {
        let mut host = Headless::new();
        let track =
            call(&mut host, "track.add", &json!({"kind":"midi"}), false).unwrap()["id"].clone();
        let clip = call(
            &mut host,
            "clip.create",
            &json!({"trackId": track, "startBar": 0, "lengthBars": 1}),
            false,
        )
        .unwrap();
        let err = call(
            &mut host,
            "clip.setFades",
            &json!({"clipId": clip["id"], "fadeInSeconds": 1}),
            false,
        )
        .unwrap_err();
        assert!(err.contains("Only audio clips"));
    }

    #[test]
    fn split_trim_resize_duplicate_and_paste_keep_fades_sensible() {
        let (mut host, id) = host_with_audio();
        let bar = seconds_per_bar(&host);
        call(
            &mut host,
            "clip.setFades",
            &json!({"clipId": id, "fadeInSeconds": 0.25 * bar, "fadeOutSeconds": 0.75 * bar}),
            false,
        )
        .unwrap();
        let r = call(
            &mut host,
            "clip.split",
            &json!({"clipId": id, "bar": 1}),
            false,
        )
        .unwrap();
        assert_eq!(r["left"]["fadeInSeconds"], 0.25 * bar);
        assert_eq!(r["left"]["fadeOutSeconds"], 0.0);
        assert_eq!(r["right"]["fadeInSeconds"], 0.0);
        assert_eq!(r["right"]["fadeOutSeconds"], 0.75 * bar);
        let right = r["right"]["id"].as_str().unwrap().to_string();
        // Resizing the left part to an eighth of a bar clamps its fade-in to the clip.
        let r = call(
            &mut host,
            "clip.resize",
            &json!({"clipId": id, "lengthBars": 0.125}),
            false,
        )
        .unwrap();
        assert!((r["fadeInSeconds"].as_f64().unwrap() - 0.125 * bar).abs() < 1e-9);
        // Trimming the right part's start to half a bar before its end clamps the fade-out.
        let r = call(
            &mut host,
            "clip.trim",
            &json!({"clipId": right, "startBar": 1.5}),
            false,
        )
        .unwrap();
        assert!((r["fadeOutSeconds"].as_f64().unwrap() - 0.5 * bar).abs() < 1e-9);
        let d = call(
            &mut host,
            "clip.duplicate",
            &json!({"clipId": right}),
            false,
        )
        .unwrap();
        assert_eq!(d["fadeOutSeconds"], r["fadeOutSeconds"]);
        call(&mut host, "clip.copy", &json!({"clipId": right}), false).unwrap();
        let p = call(&mut host, "clip.paste", &json!({"bar": 8}), false).unwrap();
        assert_eq!(p["fadeOutSeconds"], r["fadeOutSeconds"]);
    }

    #[test]
    fn markers_add_rename_move_navigate_and_undo() {
        let mut host = Headless::new();
        let verse = call(
            &mut host,
            "marker.add",
            &json!({"bar": 4, "name": "Verse 1"}),
            false,
        )
        .unwrap();
        let intro = call(&mut host, "marker.add", &json!({"bar": 0}), false).unwrap();
        assert_eq!(intro["name"], "Marker 2");
        assert!(call(&mut host, "marker.add", &json!({"bar": 4}), false).is_err());
        let list = call(&mut host, "marker.list", &json!({}), false).unwrap();
        assert_eq!(list[0]["id"], intro["id"], "markers stay in bar order");
        assert_eq!(list[0]["endBar"], 4.0);
        call(
            &mut host,
            "marker.rename",
            &json!({"markerId": intro["id"], "name": "Intro"}),
            false,
        )
        .unwrap();
        let chorus = call(
            &mut host,
            "marker.add",
            &json!({"bar": 12, "name": "Chorus"}),
            false,
        )
        .unwrap();
        call(
            &mut host,
            "marker.move",
            &json!({"markerId": chorus["id"], "bar": 8}),
            false,
        )
        .unwrap();
        assert!(call(
            &mut host,
            "marker.move",
            &json!({"markerId": chorus["id"], "bar": 4}),
            false
        )
        .is_err());
        let bpb = host.store.session().beats_per_bar();
        host.position = 5.0 * bpb;
        let r = call(&mut host, "marker.next", &json!({}), false).unwrap();
        assert_eq!(r["marker"]["name"], "Chorus");
        assert_eq!(host.position, 8.0 * bpb);
        let r = call(&mut host, "marker.next", &json!({}), false).unwrap();
        assert!(r["marker"].is_null());
        assert_eq!(host.position, 8.0 * bpb);
        let r = call(&mut host, "marker.previous", &json!({}), false).unwrap();
        assert_eq!(r["marker"]["name"], "Verse 1");
        call(&mut host, "marker.goto", &json!({"name": "intro"}), false).unwrap();
        assert_eq!(host.position, 0.0);
        let t = call(&mut host, "marker.cycleSection", &json!({}), false).unwrap();
        assert_eq!(
            t["cycle"],
            json!({"enabled": true, "startBar": 0.0, "endBar": 4.0})
        );
        call(
            &mut host,
            "marker.remove",
            &json!({"markerId": verse["id"]}),
            false,
        )
        .unwrap();
        assert_eq!(host.store.session().markers.len(), 2);
        call(&mut host, "history.undo", &json!({}), false).unwrap();
        assert_eq!(host.store.session().markers.len(), 3);
        let t = call(
            &mut host,
            "transport.locate",
            &json!({"markerId": verse["id"]}),
            false,
        )
        .unwrap();
        assert_eq!(t["positionBar"], 4.0);
    }

    #[test]
    fn marker_add_defaults_to_the_playhead_on_the_grid() {
        let mut host = Headless::new();
        let bpb = host.store.session().beats_per_bar();
        host.position = 2.0 * bpb + 0.01;
        let m = call(&mut host, "marker.add", &json!({}), false).unwrap();
        assert_eq!(m["bar"], 2.0);
        assert_eq!(m["name"], "Marker 1");
    }

    #[test]
    fn markers_and_envelopes_are_absent_from_the_file_by_default() {
        let (host, _) = host_with_audio();
        let text = serde_json::to_string(host.store.session()).unwrap();
        for key in [
            "markers",
            "fadeInSeconds",
            "fadeOutSeconds",
            "fadeCurve",
            "gainDb",
        ] {
            assert!(!text.contains(key), "{key} should be absent");
        }
    }
}
