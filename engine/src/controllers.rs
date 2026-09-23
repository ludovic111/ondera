//! MIDI controller points in clips: control changes, pitch bend and channel pressure. A point
//! holds its value until the next point of its lane, so every edit that cuts a clip in time
//! carries the value in force at the cut to the new start ("chasing" it), and reversing a clip
//! reverses the held spans rather than only the points.

use crate::{
    model::{Controller, ControllerKind},
    plugin::{event, Event},
};
use std::collections::BTreeMap;

/// A lane: the kind, and the controller number for control changes.
pub type Lane = (ControllerKind, Option<u8>);

pub const SUSTAIN: u8 = 64;
pub const MOD_WHEEL: u8 = 1;

/// Order points by time, then lane. Stable, so points at one time keep their order.
pub fn sort(points: &mut [Controller]) {
    points.sort_by(|a, b| a.time.total_cmp(&b.time).then(a.lane().cmp(&b.lane())));
}

/// The points of each lane, in time order.
pub fn lanes(points: &[Controller]) -> BTreeMap<Lane, Vec<&Controller>> {
    let mut lanes: BTreeMap<Lane, Vec<&Controller>> = BTreeMap::new();
    for point in points {
        lanes.entry(point.lane()).or_default().push(point);
    }
    for list in lanes.values_mut() {
        list.sort_by(|a, b| a.time.total_cmp(&b.time));
    }
    lanes
}

/// The value a lane holds just before `time`: its last point earlier than that.
pub fn value_before(points: &[Controller], lane: Lane, time: f64) -> Option<i16> {
    points
        .iter()
        .filter(|p| p.lane() == lane && p.time < time)
        .max_by(|a, b| a.time.total_cmp(&b.time))
        .map(|p| p.value)
}

/// The part of a clip's points from `from` for `length` beats, re-timed to start at zero. A
/// lane that had a value before `from` and no point exactly there gets one at zero, so the
/// new clip starts where the old one was. `from` may be negative (the clip grows to the left).
pub fn window(
    points: &[Controller],
    from: f64,
    length: f64,
    mut fresh_id: impl FnMut() -> String,
) -> Vec<Controller> {
    let mut out: Vec<Controller> = points
        .iter()
        .filter(|p| p.time >= from && p.time - from < length)
        .map(|p| Controller {
            time: p.time - from,
            ..p.clone()
        })
        .collect();
    if from > 0.0 && length > 0.0 {
        for (lane, list) in lanes(points) {
            let at_start = out.iter().any(|p| p.lane() == lane && p.time <= 1e-9);
            let Some(last) = list.iter().rev().find(|p| p.time < from) else {
                continue;
            };
            if !at_start {
                out.push(Controller {
                    id: fresh_id(),
                    time: 0.0,
                    ..(*last).clone()
                });
            }
        }
    }
    sort(&mut out);
    out
}

/// Play the clip backwards: the span a value held from `a` to `b` now holds from
/// `length - b` to `length - a`. The value before a lane's first point is unknown, so the
/// reversed lane ends on its first value.
pub fn reverse(points: &[Controller], length: f64) -> Vec<Controller> {
    let mut out = vec![];
    for (_, list) in lanes(points) {
        let inside: Vec<&&Controller> = list.iter().filter(|p| p.time < length).collect();
        for (i, point) in inside.iter().enumerate() {
            let end = inside.get(i + 1).map_or(length, |next| next.time);
            out.push(Controller {
                time: (length - end).max(0.0),
                ..(**point).clone()
            });
        }
    }
    sort(&mut out);
    out
}

/// Give every point a new id, for copies of a clip.
pub fn renew_ids(points: &mut [Controller], agent: bool, mut fresh_id: impl FnMut() -> String) {
    for point in points {
        point.id = fresh_id();
        point.agent = agent;
    }
}

/// The value a lane rests at when nothing drives it: bend centred, pedal up, no pressure.
/// Other controllers have no agreed rest value, so they are left where they are.
pub fn neutral(kind: ControllerKind, number: Option<u8>) -> Option<i16> {
    match (kind, number) {
        (ControllerKind::Bend, _) | (ControllerKind::Pressure, _) => Some(0),
        (ControllerKind::Cc, Some(SUSTAIN)) | (ControllerKind::Cc, Some(MOD_WHEEL)) => Some(0),
        _ => None,
    }
}

/// One controller change as playback sends it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Played {
    /// Beats from the clip start.
    pub time: f64,
    pub kind: ControllerKind,
    pub number: Option<u8>,
    pub value: i16,
    /// Sent at the clip's end to return a bend or a held pedal to rest.
    pub reset: bool,
}

/// What a clip sends: its points inside the clip, in time order, then at the clip's end a
/// return to rest for pitch bend, pressure and the sustain pedal when the clip leaves them
/// away from it. A clip cut short therefore never leaves a note bent or held.
pub fn playback(points: &[Controller], length: f64) -> Vec<Played> {
    let mut out: Vec<Played> = points
        .iter()
        .filter(|p| p.time < length && p.is_valid())
        .map(|p| Played {
            time: p.time,
            kind: p.kind,
            number: p.number,
            value: p.value,
            reset: false,
        })
        .collect();
    out.sort_by(|a, b| a.time.total_cmp(&b.time));
    for ((kind, number), list) in lanes(points) {
        let resting = matches!(
            (kind, number),
            (ControllerKind::Bend, _)
                | (ControllerKind::Pressure, _)
                | (ControllerKind::Cc, Some(SUSTAIN))
        );
        let last = list.iter().rev().find(|p| p.time < length);
        if let (true, Some(last)) = (resting, last) {
            if last.value != 0 {
                out.push(Played {
                    time: length,
                    kind,
                    number,
                    value: 0,
                    reset: true,
                });
            }
        }
    }
    out
}

/// The kind, number and value of a live controller event, as a clip stores it.
pub fn from_event(e: &Event) -> Option<(ControllerKind, Option<u8>, i16)> {
    match e.kind {
        event::CONTROL if e.key < 120 => {
            Some((ControllerKind::Cc, Some(e.key), e.value.min(127) as i16))
        }
        event::PITCH_BEND => Some((ControllerKind::Bend, None, e.bend.clamp(-8192, 8191))),
        event::CHANNEL_PRESSURE => Some((ControllerKind::Pressure, None, e.value.min(127) as i16)),
        _ => None,
    }
}

/// Controller events captured during a take, each at an absolute beat, as points of a clip
/// that starts at `origin`. Events before the clip land on its start; at one time the last
/// event of a lane wins, and a repeat of the value a lane already holds is dropped.
pub fn recorded(
    events: &[(f64, Event)],
    origin: f64,
    agent: bool,
    mut fresh_id: impl FnMut() -> String,
) -> Vec<Controller> {
    let mut out: Vec<Controller> = vec![];
    for (beats, e) in events {
        let Some((kind, number, value)) = from_event(e) else {
            continue;
        };
        let time = (beats - origin).max(0.0);
        let lane = (kind, number);
        match out.iter_mut().rev().find(|p| p.lane() == lane) {
            Some(last) if (last.time - time).abs() < 1e-9 => {
                last.value = value;
                continue;
            }
            Some(last) if last.value == value => continue,
            _ => {}
        }
        out.push(Controller {
            id: fresh_id(),
            kind,
            number,
            time,
            value,
            agent,
        });
    }
    sort(&mut out);
    out
}

/// A readable lane name for lists and errors.
pub fn lane_name(kind: ControllerKind, number: Option<u8>) -> String {
    match (kind, number) {
        (ControllerKind::Bend, _) => "Pitch Bend".into(),
        (ControllerKind::Pressure, _) => "Pressure".into(),
        (ControllerKind::Cc, Some(1)) => "Mod Wheel (CC1)".into(),
        (ControllerKind::Cc, Some(2)) => "Breath (CC2)".into(),
        (ControllerKind::Cc, Some(7)) => "Volume (CC7)".into(),
        (ControllerKind::Cc, Some(10)) => "Pan (CC10)".into(),
        (ControllerKind::Cc, Some(11)) => "Expression (CC11)".into(),
        (ControllerKind::Cc, Some(64)) => "Sustain (CC64)".into(),
        (ControllerKind::Cc, Some(n)) => format!("CC{n}"),
        (ControllerKind::Cc, None) => "CC".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn point(
        id: &str,
        kind: ControllerKind,
        number: Option<u8>,
        time: f64,
        value: i16,
    ) -> Controller {
        Controller {
            id: id.into(),
            kind,
            number,
            time,
            value,
            agent: false,
        }
    }
    #[test]
    fn a_window_chases_the_value_in_force_at_its_start() {
        let points = vec![
            point("a", ControllerKind::Cc, Some(1), 0.0, 20),
            point("b", ControllerKind::Cc, Some(1), 3.0, 90),
            point("c", ControllerKind::Bend, None, 2.0, 0),
            point("d", ControllerKind::Bend, None, 1.0, 4000),
        ];
        let mut serial = 0;
        let right = window(&points, 2.0, 4.0, || {
            serial += 1;
            format!("new-{serial}")
        });
        let summary: Vec<_> = right
            .iter()
            .map(|p| (p.kind, p.number, p.time, p.value))
            .collect();
        assert_eq!(
            summary,
            vec![
                (ControllerKind::Cc, Some(1), 0.0, 20),
                (ControllerKind::Bend, None, 0.0, 0),
                (ControllerKind::Cc, Some(1), 1.0, 90),
            ]
        );
        assert_eq!(right[0].id, "new-1", "The chased point is a new point");
        let left = window(&points, 0.0, 2.0, || unreachable!());
        assert_eq!(left.len(), 2);
        assert!(left.iter().all(|p| p.time < 2.0));
    }
    #[test]
    fn a_take_keeps_each_lane_s_changes_relative_to_the_clip() {
        let events = [
            (7.5, Event::control(0, 1, 10)),
            (8.0, Event::control(0, 1, 20)),
            (8.0, Event::control(0, 1, 30)),
            (8.5, Event::control(0, 1, 30)),
            (9.0, Event::pitch_bend(0, 0.5)),
            (9.25, Event::channel_pressure(0, 64)),
            (9.5, Event::control(0, 123, 0)),
            (10.0, Event::note_on(0, 60, 100)),
        ];
        let mut serial = 0;
        let points = recorded(&events, 8.0, false, || {
            serial += 1;
            format!("p{serial}")
        });
        let summary: Vec<_> = points
            .iter()
            .map(|p| (p.kind, p.number, p.time, p.value))
            .collect();
        assert_eq!(
            summary,
            vec![
                (ControllerKind::Cc, Some(1), 0.0, 30),
                (ControllerKind::Bend, None, 1.0, 4096),
                (ControllerKind::Pressure, None, 1.25, 64),
            ]
        );
    }
    #[test]
    fn playback_returns_bend_and_pedal_to_rest_at_the_clip_end() {
        let points = vec![
            point("a", ControllerKind::Cc, Some(64), 0.0, 127),
            point("b", ControllerKind::Bend, None, 1.0, 2000),
            point("c", ControllerKind::Cc, Some(1), 1.0, 90),
            point("d", ControllerKind::Bend, None, 6.0, 0),
        ];
        let played = playback(&points, 4.0);
        let resets: Vec<_> = played
            .iter()
            .filter(|p| p.reset)
            .map(|p| (p.kind, p.number, p.time))
            .collect();
        assert_eq!(
            resets,
            vec![
                (ControllerKind::Cc, Some(64), 4.0),
                (ControllerKind::Bend, None, 4.0)
            ]
        );
        assert_eq!(played.iter().filter(|p| !p.reset).count(), 3);
    }
    #[test]
    fn reverse_moves_held_spans_not_just_points() {
        let points = vec![
            point("a", ControllerKind::Cc, Some(64), 1.0, 127),
            point("b", ControllerKind::Cc, Some(64), 3.0, 0),
        ];
        let reversed = reverse(&points, 4.0);
        // Down from 1 to 3 in the original is down from 1 to 3 backwards too.
        let summary: Vec<_> = reversed.iter().map(|p| (p.time, p.value)).collect();
        assert_eq!(summary, vec![(0.0, 0), (1.0, 127)]);
    }
}
