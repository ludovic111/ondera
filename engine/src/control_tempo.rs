//! Tempo changes inside a song. The starting tempo is `transport.tempo`; each change takes over
//! at its bar, at once or by a ramp from the tempo before it. Positions are bars, like clips and
//! markers, so the music stays on its bars and only its speed changes; audio clips keep their
//! bars too and play their audio at its own speed (see `tempo.rs`).
use crate::{
    control::{edit, opt, query, req, transport, Args, Host, Kind, Spec},
    model::{valid_time, Session},
    store::Command,
    tempo::{valid_bpm, TempoPoint, MAX_POINTS, SAME_BAR},
    Result,
};
use serde_json::{json, Value};

const BAR: crate::control::Param = req(
    "bar",
    Kind::Number,
    "Zero-based bar of the tempo change, as listed by tempo.list.",
);

pub const SPECS: &[Spec] = &[
    query("tempo.list", "List the song's tempo: the starting tempo and every change after it in bar order, with where each one falls in seconds, and the tempo at the playhead.", &[]),
    edit("tempo.set", "Set the tempo from a bar on, like adding or dragging a point on the tempo track. Bar 0 sets the starting tempo; any later bar adds a change there or replaces the one already there. One undo step.", &[
        BAR,
        req("bpm", Kind::Number, "Beats per minute, 20-400."),
        opt("ramp", Kind::Boolean, "Glide from the previous tempo to reach this one at the bar, instead of jumping to it there (a ritardando or accelerando). Defaults to the ramp of the change already at that bar, else false. Not for bar 0."),
    ]),
    edit("tempo.move", "Move a tempo change to another bar, like dragging its point on the tempo track, keeping its ramp and, unless bpm is given, its tempo. One undo step.", &[
        BAR,
        req("toBar", Kind::Number, "New zero-based bar, after bar 0 and free of another change."),
        opt("bpm", Kind::Number, "A new tempo for it at the same time, 20-400."),
    ]),
    edit("tempo.remove", "Delete a tempo change: the tempo before it carries on. One undo step.", &[BAR]),
    edit("tempo.clear", "Delete every tempo change, or those in a range of bars, so the song keeps its starting tempo there. One undo step.", &[
        opt("startBar", Kind::Number, "First bar of the range (default 0)."),
        opt("endBar", Kind::Number, "Bar where the range ends, exclusive (default: the end of the song)."),
    ]),
];

fn check_bar(bar: f64) -> Result<f64> {
    if valid_time(bar) {
        Ok(bar)
    } else {
        Err("bar must be between 0 and 1,000,000".into())
    }
}

fn check_bpm(bpm: f64) -> Result<f64> {
    if valid_bpm(bpm) {
        Ok(bpm)
    } else {
        Err("Tempo must be between 20 and 400 BPM".into())
    }
}

/// The change at `bar`, by index.
fn index_at(s: &Session, bar: f64) -> Result<usize> {
    s.tempo_changes
        .iter()
        .position(|p| (p.bar - bar).abs() < SAME_BAR)
        .ok_or_else(|| {
            let bars: Vec<String> = s
                .tempo_changes
                .iter()
                .map(|p| format!("{}", p.bar))
                .collect();
            if bars.is_empty() {
                format!("No tempo change at bar {bar}: the song keeps one tempo")
            } else {
                format!(
                    "No tempo change at bar {bar}. Changes are at bars {}",
                    bars.join(", ")
                )
            }
        })
}

pub(crate) fn list(host: &dyn Host) -> Value {
    let s = host.store().session();
    let map = s.tempo_map();
    let bpb = s.beats_per_bar();
    json!({
        "start": s.transport.tempo,
        "changes": s.tempo_changes.iter().map(|p| json!({
            "bar": p.bar,
            "bpm": p.bpm,
            "ramp": p.ramp,
            "seconds": map.seconds(p.bar * bpb),
        })).collect::<Vec<_>>(),
        "tempoAtPlayhead": map.bpm(host.position()),
        "lengthSeconds": map.seconds(s.end_bar() * bpb),
    })
}

fn put(host: &mut dyn Host, points: Vec<TempoPoint>) -> Result<Value> {
    host.dispatch(Command::SetTempoChanges(points))?;
    Ok(list(host))
}

pub(crate) fn call(host: &mut dyn Host, name: &str, a: &Args) -> Result<Value> {
    match name {
        "tempo.list" => Ok(list(host)),
        "tempo.set" => {
            let bar = check_bar(a.f64("bar")?)?;
            let bpm = check_bpm(a.f64("bpm")?)?;
            let ramp = a.opt_bool("ramp");
            let s = host.store().session();
            if bar < SAME_BAR {
                if ramp == Some(true) {
                    return Err("The starting tempo cannot ramp; set ramp on a later bar".into());
                }
                let mut t = s.transport.clone();
                t.tempo = bpm;
                host.dispatch(Command::SetTransport(t))?;
                let mut out = list(host);
                out["transport"] = transport(host);
                return Ok(out);
            }
            let mut points = s.tempo_changes.clone();
            match points.iter_mut().find(|p| (p.bar - bar).abs() < SAME_BAR) {
                Some(p) => {
                    p.bpm = bpm;
                    p.ramp = ramp.unwrap_or(p.ramp);
                }
                None => {
                    if points.len() >= MAX_POINTS {
                        return Err(format!("A song holds at most {MAX_POINTS} tempo changes"));
                    }
                    points.push(TempoPoint {
                        bar,
                        bpm,
                        ramp: ramp.unwrap_or(false),
                    });
                }
            }
            put(host, points)
        }
        "tempo.move" => {
            let s = host.store().session();
            let from = index_at(s, a.f64("bar")?)?;
            let to = check_bar(a.f64("toBar")?)?;
            if to < SAME_BAR {
                return Err("A tempo change must be after bar 0; use tempo.set bar=0 for the starting tempo".into());
            }
            if s.tempo_changes
                .iter()
                .enumerate()
                .any(|(i, p)| i != from && (p.bar - to).abs() < SAME_BAR)
            {
                return Err(format!("Another tempo change is already at bar {to}"));
            }
            let bpm = a.opt_f64("bpm").map(check_bpm).transpose()?;
            let mut points = s.tempo_changes.clone();
            points[from].bar = to;
            if let Some(bpm) = bpm {
                points[from].bpm = bpm;
            }
            put(host, points)
        }
        "tempo.remove" => {
            let s = host.store().session();
            let index = index_at(s, a.f64("bar")?)?;
            let mut points = s.tempo_changes.clone();
            points.remove(index);
            put(host, points)
        }
        "tempo.clear" => {
            let start = check_bar(a.opt_f64("startBar").unwrap_or(0.0))?;
            let end = match a.opt_f64("endBar") {
                Some(end) => check_bar(end)?,
                None => f64::INFINITY,
            };
            if end <= start {
                return Err("endBar must be after startBar".into());
            }
            let s = host.store().session();
            let points: Vec<TempoPoint> = s
                .tempo_changes
                .iter()
                .filter(|p| p.bar < start - SAME_BAR || p.bar >= end - SAME_BAR)
                .copied()
                .collect();
            if points.len() == s.tempo_changes.len() {
                return Ok(list(host));
            }
            put(host, points)
        }
        _ => Err(format!("Unknown command `{name}`")),
    }
}

#[cfg(test)]
mod tests {
    use crate::control::{call, Headless};
    use serde_json::json;

    fn run(host: &mut Headless, name: &str, params: serde_json::Value) -> serde_json::Value {
        call(host, name, &params, false).unwrap()
    }

    #[test]
    fn tempo_changes_add_replace_move_remove_and_undo() {
        let mut host = Headless::new();
        let start = host.store.session().transport.tempo;
        let r = run(&mut host, "tempo.set", json!({"bar": 8, "bpm": 90}));
        assert_eq!(r["start"], start);
        assert_eq!(r["changes"][0]["bar"], 8.0);
        assert_eq!(r["changes"][0]["ramp"], false);
        run(
            &mut host,
            "tempo.set",
            json!({"bar": 4, "bpm": 100, "ramp": true}),
        );
        let r = run(&mut host, "tempo.set", json!({"bar": 8, "bpm": 95}));
        assert_eq!(r["changes"].as_array().unwrap().len(), 2);
        assert_eq!(r["changes"][0]["bar"], 4.0, "kept in bar order");
        assert_eq!(r["changes"][0]["ramp"], true);
        assert_eq!(r["changes"][1]["bpm"], 95.0);
        // Replacing without `ramp` keeps the ramp.
        let r = run(&mut host, "tempo.set", json!({"bar": 4, "bpm": 110}));
        assert_eq!(r["changes"][0]["ramp"], true);
        assert!(call(
            &mut host,
            "tempo.move",
            &json!({"bar": 4, "toBar": 8}),
            false
        )
        .unwrap_err()
        .contains("already"));
        let r = run(
            &mut host,
            "tempo.move",
            json!({"bar": 4, "toBar": 12, "bpm": 104}),
        );
        assert_eq!(r["changes"][1]["bar"], 12.0);
        assert_eq!(r["changes"][1]["bpm"], 104.0);
        assert_eq!(r["changes"][1]["ramp"], true);
        let r = run(&mut host, "tempo.remove", json!({"bar": 8}));
        assert_eq!(r["changes"].as_array().unwrap().len(), 1);
        assert!(call(&mut host, "tempo.remove", &json!({"bar": 3}), false)
            .unwrap_err()
            .contains("bars 12"));
        run(&mut host, "history.undo", json!({}));
        assert_eq!(host.store.session().tempo_changes.len(), 2);
        let r = run(&mut host, "tempo.clear", json!({"startBar": 10}));
        assert_eq!(r["changes"].as_array().unwrap().len(), 1);
        run(&mut host, "tempo.clear", json!({}));
        assert!(host.store.session().tempo_changes.is_empty());
    }

    #[test]
    fn bar_zero_is_the_starting_tempo() {
        let mut host = Headless::new();
        let r = run(&mut host, "tempo.set", json!({"bar": 0, "bpm": 84}));
        assert_eq!(r["start"], 84.0);
        assert_eq!(host.store.session().transport.tempo, 84.0);
        assert!(host.store.session().tempo_changes.is_empty());
        assert!(call(
            &mut host,
            "tempo.set",
            &json!({"bar": 0, "bpm": 84, "ramp": true}),
            false
        )
        .is_err());
        assert!(call(
            &mut host,
            "tempo.set",
            &json!({"bar": 2, "bpm": 500}),
            false
        )
        .is_err());
    }

    #[test]
    fn seconds_follow_the_changes() {
        let mut host = Headless::new();
        run(&mut host, "tempo.set", json!({"bar": 0, "bpm": 120}));
        let bpb = host.store.session().beats_per_bar();
        let r = run(&mut host, "tempo.set", json!({"bar": 2, "bpm": 60}));
        // Two bars at 120 BPM.
        assert_eq!(r["changes"][0]["seconds"], 2.0 * bpb * 0.5);
        host.position = 3.0 * bpb;
        let t = run(&mut host, "transport.locate", json!({"bar": 3}));
        assert_eq!(t["tempoAtPosition"], 60.0);
        assert_eq!(t["positionSeconds"], 2.0 * bpb * 0.5 + bpb);
        let text = serde_json::to_string(host.store.session()).unwrap();
        assert!(text.contains("\"tempoChanges\":[{\"bar\":2.0,\"bpm\":60.0}]"));
    }

    #[test]
    fn songs_with_one_tempo_write_no_tempo_changes() {
        let host = Headless::new();
        let text = serde_json::to_string(host.store.session()).unwrap();
        assert!(!text.contains("tempoChanges"));
    }
}
