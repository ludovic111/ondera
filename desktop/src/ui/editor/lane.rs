//! The controller lane's model: which lane (kind, number, channel) it shows, how values map
//! to heights, which point the pointer grabs and how a drawn stroke becomes points. Pure
//! functions; `controllers.rs` paints and edits with them through `controller.*` commands.

use crate::ui::theme::editor::{CONTROLLER_GRIP, CONTROLLER_INSET};
use ryolune_engine::model::{Clip, ClipData, Controller, ControllerKind};
use serde_json::{json, Map, Value};

/// One controller lane: a kind, a number for control changes, and a MIDI channel (0-15).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lane {
    pub kind: ControllerKind,
    pub number: u8,
    pub channel: u8,
}

impl Lane {
    pub const fn cc(number: u8) -> Self {
        Self {
            kind: ControllerKind::Cc,
            number,
            channel: 0,
        }
    }
    pub const fn of_kind(kind: ControllerKind) -> Self {
        Self {
            kind,
            number: 0,
            channel: 0,
        }
    }
    /// The lane a point belongs to.
    pub fn of(point: &Controller) -> Self {
        Self {
            kind: point.kind,
            number: if point.kind == ControllerKind::Cc {
                point.number.unwrap_or(0)
            } else {
                0
            },
            channel: point.channel,
        }
    }
    /// Same lane: the number counts only for control changes.
    pub fn same(&self, other: &Lane) -> bool {
        self.kind == other.kind
            && (self.kind != ControllerKind::Cc || self.number == other.number)
            && self.channel == other.channel
    }
    /// The full name the canvas writes: "Mod Wheel (CC1)", "Pitch Bend · Ch 4".
    pub fn title(&self) -> String {
        let suffix = if self.channel > 0 {
            format!(" · Ch {}", self.channel + 1)
        } else {
            String::new()
        };
        let name = match self.kind {
            ControllerKind::Bend => "Pitch Bend".to_string(),
            ControllerKind::Pressure => "Pressure".to_string(),
            ControllerKind::PolyPressure => "Poly Pressure".to_string(),
            ControllerKind::Cc => match cc_name(self.number) {
                Some(name) => format!("{name} (CC{})", self.number),
                None => format!("CC{}", self.number),
            },
        };
        name + &suffix
    }
    /// The short name on the lane's selector.
    pub fn short_title(&self) -> String {
        match self.kind {
            ControllerKind::Bend => "Bend".into(),
            ControllerKind::Pressure => "Press.".into(),
            ControllerKind::PolyPressure => "Poly".into(),
            ControllerKind::Cc => format!("CC{}", self.number),
        }
    }
    /// Parameters naming this lane for the `controller.*` commands.
    pub fn params(&self) -> Map<String, Value> {
        let mut params = Map::new();
        params.insert("kind".into(), json!(self.kind.as_str()));
        if self.kind == ControllerKind::Cc {
            params.insert("number".into(), json!(self.number));
        }
        if self.channel > 0 {
            params.insert("channel".into(), json!(self.channel));
        }
        params
    }
    pub fn range(&self) -> (i16, i16) {
        self.kind.range()
    }
}

/// The lanes the selector offers first, most useful first.
pub const LANE_CHOICES: [Lane; 8] = [
    Lane::cc(1),
    Lane::cc(11),
    Lane::cc(64),
    Lane::of_kind(ControllerKind::Bend),
    Lane::of_kind(ControllerKind::Pressure),
    Lane::cc(7),
    Lane::cc(10),
    Lane::cc(2),
];

fn cc_name(number: u8) -> Option<&'static str> {
    Some(match number {
        1 => "Mod Wheel",
        2 => "Breath",
        7 => "Volume",
        10 => "Pan",
        11 => "Expression",
        64 => "Sustain",
        _ => return None,
    })
}

/// A number typed as "CC 74", "74" or "cc74", when it names a usable controller (0-119).
pub fn parse_cc_number(text: &str) -> Option<u8> {
    let t = text.trim();
    let t = t
        .strip_prefix("cc")
        .or_else(|| t.strip_prefix("CC"))
        .or_else(|| t.strip_prefix("Cc"))
        .or_else(|| t.strip_prefix("cC"))
        .unwrap_or(t)
        .trim();
    if t.is_empty() || t.len() > 3 || !t.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    t.parse::<u8>().ok().filter(|n| *n <= 119)
}

fn controllers(clip: Option<&Clip>) -> &[Controller] {
    match clip.map(|c| &c.data) {
        Some(ClipData::Midi { controllers, .. }) => controllers,
        _ => &[],
    }
}

/// Points of one lane in a clip, in time order.
pub fn lane_points(clip: Option<&Clip>, lane: &Lane) -> Vec<Controller> {
    let mut points: Vec<Controller> = controllers(clip)
        .iter()
        .filter(|p| Lane::of(p).same(lane))
        .cloned()
        .collect();
    points.sort_by(|a, b| a.time.total_cmp(&b.time));
    points
}

/// Lanes that hold points in a clip, in the order they first appear. Polyphonic pressure
/// belongs to notes, not to a lane, and is left out.
pub fn lanes_in_clip(clip: Option<&Clip>) -> Vec<Lane> {
    let mut out: Vec<Lane> = Vec::new();
    for p in controllers(clip) {
        if p.kind == ControllerKind::PolyPressure {
            continue;
        }
        let lane = Lane::of(p);
        if !out.iter().any(|l| l.same(&lane)) {
            out.push(lane);
        }
    }
    out
}

/// Height of a value in a lane `height` pixels tall; a small inset keeps the extreme
/// values grabbable.
pub fn y_of_value(lane: &Lane, value: i16, height: f32) -> f32 {
    let (low, high) = lane.range();
    let t = (value - low) as f32 / (high - low) as f32;
    CONTROLLER_INSET + (1.0 - t) * (height - CONTROLLER_INSET * 2.0).max(1.0)
}

/// The value at a height, clamped to the lane's range. The bend wheel snaps to its centre
/// near the middle, as a real one does.
pub fn value_at_y(lane: &Lane, y: f32, height: f32) -> i16 {
    let (low, high) = lane.range();
    let t = 1.0 - (y - CONTROLLER_INSET) / (height - CONTROLLER_INSET * 2.0).max(1.0);
    let value = (low as f32 + t.clamp(0.0, 1.0) * (high - low) as f32).round() as i16;
    if lane.kind == ControllerKind::Bend && value.abs() < 128 {
        return 0;
    }
    value
}

/// The point under (x, y), nearest first, within the grip distance.
pub fn hit_point<'a>(
    points: &'a [Controller],
    lane: &Lane,
    px_per_beat: f32,
    height: f32,
    x: f32,
    y: f32,
) -> Option<&'a Controller> {
    let mut best = None;
    let mut best_distance = CONTROLLER_GRIP;
    for p in points {
        let dx = p.time as f32 * px_per_beat - x;
        let dy = y_of_value(lane, p.value, height) - y;
        let d = (dx * dx + dy * dy).sqrt();
        if d <= best_distance {
            best = Some(p);
            best_distance = d;
        }
    }
    best
}

/// A pointer sample while drawing: where in the clip, and the value at that height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    pub beat: f64,
    pub value: i16,
}

/// A drawn stroke as lane points and the range they replace.
#[derive(Clone, Debug, PartialEq)]
pub struct Stroke {
    pub points: Vec<Sample>,
    pub from: f64,
    pub to: f64,
}

/// A drawn stroke as lane points: one per grid step the stroke crossed, holding the last
/// value drawn in that step, and the range they replace. Nothing when the stroke never
/// entered the clip.
pub fn stroke_points(samples: &[Sample], step: f64, length_beats: f64) -> Option<Stroke> {
    use std::collections::BTreeMap;
    let mut cells: BTreeMap<i64, i16> = BTreeMap::new();
    // Fill between samples so a fast stroke leaves no holes.
    for (i, a) in samples.iter().enumerate() {
        let b = samples.get(i + 1).unwrap_or(a);
        let from = a.beat.min(b.beat);
        let to = a.beat.max(b.beat);
        let mut t = (from / step).floor() * step;
        while t <= to {
            let cell = (t / step).round() as i64;
            let at = cell as f64 * step;
            if at >= 0.0 && at < length_beats {
                let k = if b.beat == a.beat {
                    0.0
                } else {
                    ((t - a.beat) / (b.beat - a.beat)).clamp(0.0, 1.0)
                };
                let value = (a.value as f64 + (b.value - a.value) as f64 * k).round() as i16;
                cells.insert(cell, value);
            }
            t += step;
        }
    }
    let first = *cells.keys().next()?;
    let last = *cells.keys().next_back()?;
    Some(Stroke {
        points: cells
            .into_iter()
            .map(|(cell, value)| Sample {
                beat: cell as f64 * step,
                value,
            })
            .collect(),
        from: first as f64 * step,
        to: length_beats.min((last + 1) as f64 * step),
    })
}

/// Plain text for a value: bend in semitones of the usual two-semitone range, the sustain
/// pedal up or down.
pub fn value_label(lane: &Lane, value: i16) -> String {
    match lane.kind {
        ControllerKind::Bend => {
            let semitones = value as f64 / 8192.0 * 2.0;
            format!(
                "{}{semitones:.2} st",
                if semitones >= 0.0 { "+" } else { "" }
            )
        }
        ControllerKind::Cc if lane.number == 64 => if value >= 64 { "down" } else { "up" }.into(),
        _ => value.to_string(),
    }
}

/// A time inside the clip for a point: snapped unless `fine`, never at or past its end.
pub fn point_time(beat: f64, length_beats: f64, division: u32, fine: bool) -> f64 {
    let grid = super::geometry::step_beats(division);
    let snapped = super::geometry::snap(beat, division, fine);
    let last = if fine {
        length_beats - 1e-3
    } else {
        length_beats - grid
    };
    snapped.min(last.max(0.0)).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOD: Lane = Lane::cc(1);
    const BEND: Lane = Lane::of_kind(ControllerKind::Bend);

    fn point(
        id: &str,
        kind: ControllerKind,
        time: f64,
        value: i16,
        number: Option<u8>,
    ) -> Controller {
        Controller {
            id: id.into(),
            kind,
            number,
            time,
            value,
            agent: false,
            channel: 0,
        }
    }
    fn clip(controllers: Vec<Controller>) -> Clip {
        Clip {
            id: "c".into(),
            name: "Clip".into(),
            agent: false,
            track_id: "t".into(),
            start_bar: 0.0,
            length_bars: 1.0,
            data: ClipData::Midi {
                notes: vec![],
                controllers,
            },
        }
    }

    #[test]
    fn lanes_are_named_and_name_the_command_parameters() {
        assert_eq!(MOD.title(), "Mod Wheel (CC1)");
        assert_eq!(Lane::cc(74).title(), "CC74");
        assert_eq!(BEND.title(), "Pitch Bend");
        assert_eq!(
            Value::Object(MOD.params()),
            json!({"kind": "cc", "number": 1})
        );
        assert_eq!(Value::Object(BEND.params()), json!({"kind": "bend"}));
        let ch = Lane { channel: 3, ..BEND };
        assert_eq!(ch.title(), "Pitch Bend · Ch 4");
        assert_eq!(
            Value::Object(Lane { channel: 3, ..MOD }.params()),
            json!({"kind": "cc", "number": 1, "channel": 3})
        );
        assert!(MOD.same(&Lane::cc(1)));
        assert!(!MOD.same(&Lane { channel: 2, ..MOD }));
        assert!(!MOD.same(&Lane::cc(2)));
        let mut p = point("a", ControllerKind::Cc, 0.0, 5, Some(1));
        p.channel = 2;
        assert_eq!(Lane::of(&p), Lane { channel: 2, ..MOD });
        assert_eq!(parse_cc_number("CC 74"), Some(74));
        assert_eq!(parse_cc_number("cc7"), Some(7));
        assert_eq!(parse_cc_number("74"), Some(74));
        assert_eq!(parse_cc_number("120"), None);
        assert_eq!(parse_cc_number("wheel"), None);
        assert_eq!(MOD.short_title(), "CC1");
        assert_eq!(BEND.short_title(), "Bend");
    }

    #[test]
    fn values_map_to_heights_and_back_across_each_range() {
        let pressure = Lane::of_kind(ControllerKind::Pressure);
        for (lane, values) in [
            (MOD, vec![0, 1, 64, 126, 127]),
            (BEND, vec![-8192, -4096, 0, 4096, 8191]),
            (pressure, vec![0, 127]),
        ] {
            for value in values {
                assert_eq!(
                    value_at_y(&lane, y_of_value(&lane, value, 96.0), 96.0),
                    value
                );
            }
        }
        // Above the top and below the bottom clamp to the range.
        assert_eq!(value_at_y(&MOD, -40.0, 96.0), 127);
        assert_eq!(value_at_y(&MOD, 400.0, 96.0), 0);
        // The bend lane snaps to centre near its middle.
        assert_eq!(value_at_y(&BEND, y_of_value(&BEND, 60, 96.0), 96.0), 0);
        assert!((y_of_value(&BEND, 0, 96.0) - 48.0).abs() < 0.5);
    }

    #[test]
    fn a_lanes_points_come_in_time_order_and_a_clip_lists_its_lanes() {
        let c = clip(vec![
            point("b", ControllerKind::Cc, 2.0, 90, Some(1)),
            point("bend", ControllerKind::Bend, 1.0, 100, None),
            point("a", ControllerKind::Cc, 0.0, 10, Some(1)),
            point("pedal", ControllerKind::Cc, 3.0, 127, Some(64)),
            point("poly", ControllerKind::PolyPressure, 3.0, 20, Some(60)),
        ]);
        let ids: Vec<_> = lane_points(Some(&c), &MOD)
            .into_iter()
            .map(|p| p.id)
            .collect();
        assert_eq!(ids, ["a", "b"]);
        assert!(lane_points(Some(&clip(vec![])), &MOD).is_empty());
        assert!(lane_points(None, &MOD).is_empty());
        assert_eq!(lanes_in_clip(Some(&c)), vec![MOD, BEND, Lane::cc(64)]);
    }

    #[test]
    fn the_nearest_point_within_the_grip_is_grabbed() {
        let points = [
            point("a", ControllerKind::Cc, 0.0, 0, Some(1)),
            point("b", ControllerKind::Cc, 1.0, 127, Some(1)),
        ];
        let y0 = y_of_value(&MOD, 0, 96.0);
        assert_eq!(
            hit_point(&points, &MOD, 40.0, 96.0, 2.0, y0 - 1.0)
                .unwrap()
                .id,
            "a"
        );
        let y1 = y_of_value(&MOD, 127, 96.0);
        assert_eq!(
            hit_point(&points, &MOD, 40.0, 96.0, 41.0, y1).unwrap().id,
            "b"
        );
        assert!(hit_point(&points, &MOD, 40.0, 96.0, 20.0, 48.0).is_none());
    }

    #[test]
    fn a_stroke_becomes_one_point_per_grid_step_without_gaps() {
        let stroke = stroke_points(
            &[
                Sample {
                    beat: 0.1,
                    value: 0,
                },
                Sample {
                    beat: 1.1,
                    value: 100,
                },
            ],
            0.25,
            4.0,
        )
        .unwrap();
        let beats: Vec<f64> = stroke.points.iter().map(|p| p.beat).collect();
        assert_eq!(beats, [0.0, 0.25, 0.5, 0.75, 1.0]);
        assert_eq!(stroke.points[0].value, 0);
        // The last sample lands in the last cell and wins it.
        assert_eq!(stroke.points[4].value, 100);
        assert_eq!(stroke.points[2].value, 40);
        assert_eq!(stroke.from, 0.0);
        assert_eq!(stroke.to, 1.25);
        // Nothing lands at or past the clip end.
        let back = stroke_points(
            &[
                Sample {
                    beat: 3.9,
                    value: 10,
                },
                Sample {
                    beat: 5.0,
                    value: 10,
                },
            ],
            0.5,
            4.0,
        )
        .unwrap();
        let beats: Vec<f64> = back.points.iter().map(|p| p.beat).collect();
        assert_eq!(beats, [3.5]);
        assert_eq!(back.to, 4.0);
        assert!(stroke_points(
            &[Sample {
                beat: 6.0,
                value: 1
            }],
            0.25,
            4.0
        )
        .is_none());
    }

    #[test]
    fn values_read_as_semitones_or_pedal_positions() {
        assert_eq!(value_label(&BEND, 4096), "+1.00 st");
        assert_eq!(value_label(&BEND, -8192), "-2.00 st");
        assert_eq!(value_label(&Lane::cc(64), 127), "down");
        assert_eq!(value_label(&MOD, 12), "12");
    }

    #[test]
    fn point_times_stay_inside_the_clip() {
        assert_eq!(point_time(1.13, 4.0, 16, false), 1.25);
        assert_eq!(point_time(-1.0, 4.0, 16, false), 0.0);
        assert_eq!(point_time(3.99, 4.0, 16, false), 3.75);
        assert!(point_time(5.0, 4.0, 16, true) < 4.0);
    }
}
