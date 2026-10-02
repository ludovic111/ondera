//! What a press, a drag and a release in the arrangement do, apart from the window: each
//! gesture is a small state machine over the session and the view's geometry. A drag in
//! progress is only an overlay the view draws (the ghost); the registry commands it makes
//! come out on release, so each gesture is one undo step and a cancelled one leaves nothing.

use super::geometry::{self as geo_, Edge, Envelope, Fade, Geo, TempoDrag, SAME_BAR};
use ryolune_engine::model::{Clip, ClipData, Session};
use serde_json::{json, Value};
use std::collections::HashMap;

/// A registry command and its parameters.
pub type Call = (&'static str, Value);

/// How far the pointer travels before a press becomes a drag.
const DRAG_THRESHOLD: f64 = 3.0;
const RULER_THRESHOLD: f64 = 4.0;

/// The arrangement's tools (`ui.setTool`, `app.tool`).
#[cfg_attr(not(test), allow(dead_code))]
pub const POINTER: usize = 0;
pub const PENCIL: usize = 1;
pub const SCISSORS: usize = 2;

/// The pointer shape a place asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Cursor {
    #[default]
    Default,
    ResizeX,
    Crosshair,
    Split,
    Grab,
    Grabbing,
    Copy,
}

/// Clip-shaped outline on a track row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    pub track: usize,
    pub start: f64,
    pub length: f64,
}

/// What an interaction in progress draws over the lanes. Never session state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LaneOverlay {
    /// Where a dragged or trimmed clip would land.
    pub ghost: Option<Span>,
    /// The clip being drawn with the pencil.
    pub pencil: Option<Span>,
    /// Scissors guide: where the cut would fall (track row, bar).
    pub split: Option<(usize, f64)>,
    /// Fade lengths in seconds while a fade handle is dragged (clip id, in, out).
    pub fade: Option<(String, f64, f64)>,
    /// The lane a dragged file would land on.
    pub drop_track: Option<usize>,
}

#[derive(Clone, Debug)]
pub enum LaneDrag {
    Move {
        clip: Clip,
        /// Bars between the pointer and the clip start when it was grabbed.
        grab: f64,
        start_x: f64,
        start_y: f64,
        start_bar: f64,
        track: usize,
        moved: bool,
    },
    Resize {
        clip: Clip,
        edge: Edge,
        start_bar: f64,
        length: f64,
    },
    Fade {
        clip: Clip,
        handle: Fade,
        fade_in: f64,
        fade_out: f64,
    },
    Pencil {
        track: usize,
        anchor: f64,
        start: f64,
        length: f64,
    },
}

/// A press on the lanes: the drag it starts, if any, and what it does at once.
pub fn lane_press(
    s: &Session,
    geo: &Geo,
    tool: usize,
    x: f64,
    y: f64,
    free: bool,
) -> (Option<LaneDrag>, Vec<Call>) {
    let row = geo.row_at(y, s.tracks.len());
    let clip = geo_::clip_at(s, geo, x, y);
    let bar = geo.bar(x);
    if tool == SCISSORS {
        let mut calls = vec![];
        if let Some(clip) = clip {
            let at = geo_::snap(s, bar, free);
            if at > clip.start_bar && at < clip.start_bar + clip.length_bars {
                calls.push(("clip.split", json!({"clipId": clip.id, "bar": at})));
            }
        }
        return (None, calls);
    }
    if tool == PENCIL && clip.is_none() {
        if let Some(track) = row.filter(|&i| !s.tracks[i].is_bus()) {
            let anchor = geo_::snap(s, bar, free).max(0.0);
            let drag = LaneDrag::Pencil {
                track,
                anchor,
                start: anchor,
                length: 0.0,
            };
            return (Some(drag), vec![]);
        }
    }
    if let Some(clip) = clip {
        let calls = vec![("clip.select", json!({"clipId": clip.id}))];
        let handle = Envelope::of(&clip.data)
            .and_then(|env| geo_::fade_handle_at(s, geo, clip, &env, x, y).map(|h| (h, env)));
        let drag = if let Some((handle, env)) = handle {
            LaneDrag::Fade {
                clip: clip.clone(),
                handle,
                fade_in: env.fade_in,
                fade_out: env.fade_out,
            }
        } else if let Some(edge) = geo_::clip_edge_at(geo, clip, x) {
            LaneDrag::Resize {
                clip: clip.clone(),
                edge,
                start_bar: clip.start_bar,
                length: clip.length_bars,
            }
        } else {
            LaneDrag::Move {
                clip: clip.clone(),
                grab: bar - clip.start_bar,
                start_x: x,
                start_y: y,
                start_bar: clip.start_bar,
                track: row.unwrap_or(0),
                moved: false,
            }
        };
        return (Some(drag), calls);
    }
    // A click on an empty lane drops the clip selection and selects the track.
    let mut calls = vec![];
    if s.view.selected_clip_id.is_some() {
        calls.push(("clip.deselect", json!({})));
    }
    if let Some(track) = row.map(|i| &s.tracks[i]) {
        if s.view.selected_track_id.as_deref() != Some(track.id.as_str()) {
            calls.push(("track.select", json!({"trackId": track.id})));
        }
    }
    (None, calls)
}

/// Follow the pointer: update the drag and say what to draw.
pub fn lane_move(
    d: &mut LaneDrag,
    s: &Session,
    geo: &Geo,
    x: f64,
    y: f64,
    free: bool,
) -> LaneOverlay {
    let bar = geo.bar(x);
    let min_len = geo_::snap_step(s);
    match d {
        LaneDrag::Move {
            clip,
            grab,
            start_x,
            start_y,
            start_bar,
            track,
            moved,
        } => {
            if !*moved && (x - *start_x).hypot(y - *start_y) < DRAG_THRESHOLD {
                return LaneOverlay::default();
            }
            *moved = true;
            *start_bar = geo_::snap(s, (bar - *grab).max(0.0), free).max(0.0);
            let row =
                ((y / geo.row).floor().max(0.0) as usize).min(s.tracks.len().saturating_sub(1));
            // Clips only land on tracks of their own kind.
            if s.tracks.get(row).is_some_and(|t| t.kind == clip_kind(clip)) {
                *track = row;
            }
            LaneOverlay {
                ghost: Some(Span {
                    track: *track,
                    start: *start_bar,
                    length: clip.length_bars,
                }),
                ..Default::default()
            }
        }
        LaneDrag::Resize {
            clip,
            edge,
            start_bar,
            length,
        } => {
            let end = clip.start_bar + clip.length_bars;
            match edge {
                Edge::Start => {
                    *start_bar = geo_::snap(s, bar, free).max(0.0).min(end - min_len);
                    *length = end - *start_bar;
                }
                Edge::End => {
                    let new_end = geo_::snap(s, bar, free).max(clip.start_bar + min_len);
                    *length = new_end - clip.start_bar;
                }
            }
            LaneOverlay {
                ghost: Some(Span {
                    track: s
                        .tracks
                        .iter()
                        .position(|t| t.id == clip.track_id)
                        .unwrap_or(0),
                    start: *start_bar,
                    length: *length,
                }),
                ..Default::default()
            }
        }
        LaneDrag::Fade {
            clip,
            handle,
            fade_in,
            fade_out,
        } => {
            // Fades are seconds of audio: free of the grid, and never past the other fade.
            let length = s.bars_seconds(clip.start_bar, clip.start_bar + clip.length_bars);
            let (rate_in, rate_out) =
                geo_::fade_rates(s, geo.ppb, clip.start_bar, clip.length_bars);
            let x0 = geo.x(clip.start_bar);
            let x1 = geo.x(clip.start_bar + clip.length_bars);
            let ms = |v: f64| (v * 1000.0).round() / 1000.0;
            match handle {
                Fade::In => {
                    *fade_in = ms(((x - x0) / rate_in).clamp(0.0, (length - *fade_out).max(0.0)))
                }
                Fade::Out => {
                    *fade_out = ms(((x1 - x) / rate_out).clamp(0.0, (length - *fade_in).max(0.0)))
                }
            }
            LaneOverlay {
                fade: Some((clip.id.clone(), *fade_in, *fade_out)),
                ..Default::default()
            }
        }
        LaneDrag::Pencil {
            track,
            anchor,
            start,
            length,
        } => {
            let b = geo_::snap(s, bar, free).max(0.0);
            *start = anchor.min(b);
            *length = (b - *anchor).abs();
            LaneOverlay {
                pencil: Some(Span {
                    track: *track,
                    start: *start,
                    length: length.max(min_len),
                }),
                ..Default::default()
            }
        }
    }
}

fn clip_kind(clip: &Clip) -> &'static str {
    match clip.data {
        ClipData::Midi { .. } => "midi",
        ClipData::Audio { .. } => "audio",
    }
}

/// The release: the commands the drag makes, if it changed anything.
pub fn lane_release(d: LaneDrag, s: &Session) -> Vec<Call> {
    match d {
        LaneDrag::Move {
            clip,
            start_bar,
            track,
            moved,
            ..
        } => {
            let target = s.tracks.get(track);
            if !moved
                || (start_bar == clip.start_bar && target.is_none_or(|t| t.id == clip.track_id))
            {
                return vec![];
            }
            let mut params = json!({"clipId": clip.id, "startBar": start_bar});
            if let Some(t) = target {
                params["trackId"] = json!(t.id);
            }
            vec![("clip.move", params)]
        }
        LaneDrag::Resize {
            clip,
            start_bar,
            length,
            ..
        } => {
            if start_bar != clip.start_bar {
                vec![(
                    "clip.trim",
                    json!({"clipId": clip.id, "startBar": start_bar, "lengthBars": length}),
                )]
            } else if length != clip.length_bars {
                vec![(
                    "clip.resize",
                    json!({"clipId": clip.id, "lengthBars": length}),
                )]
            } else {
                vec![]
            }
        }
        LaneDrag::Fade {
            clip,
            fade_in,
            fade_out,
            ..
        } => {
            let env = Envelope::of(&clip.data);
            if env.is_some_and(|e| e.fade_in == fade_in && e.fade_out == fade_out) {
                return vec![];
            }
            vec![(
                "clip.setFades",
                json!({"clipId": clip.id, "fadeInSeconds": fade_in, "fadeOutSeconds": fade_out}),
            )]
        }
        LaneDrag::Pencil {
            track,
            start,
            length,
            ..
        } => {
            let Some(track) = s.tracks.get(track) else {
                return vec![];
            };
            if track.kind != "midi" {
                // Audio needs a source: the pencil on an audio track only selects it.
                return vec![("track.select", json!({"trackId": track.id}))];
            }
            let length = if length == 0.0 {
                1.0
            } else {
                length.max(geo_::snap_step(s))
            };
            vec![(
                "clip.create",
                json!({"trackId": track.id, "startBar": start, "lengthBars": length}),
            )]
        }
    }
}

/// The pointer over the lanes with no button down: its shape, and the scissors' guide.
pub fn lane_hover(
    s: &Session,
    geo: &Geo,
    tool: usize,
    x: f64,
    y: f64,
    free: bool,
) -> (Cursor, Option<(usize, f64)>) {
    let clip = geo_::clip_at(s, geo, x, y);
    match tool {
        SCISSORS => match clip {
            Some(_) => (
                Cursor::Split,
                geo.row_at(y, s.tracks.len())
                    .map(|row| (row, geo_::snap(s, geo.bar(x), free))),
            ),
            None => (Cursor::Default, None),
        },
        PENCIL => (
            if clip.is_some() {
                Cursor::Default
            } else {
                Cursor::Crosshair
            },
            None,
        ),
        _ => {
            let Some(clip) = clip else {
                return (Cursor::Default, None);
            };
            let handle = Envelope::of(&clip.data)
                .and_then(|env| geo_::fade_handle_at(s, geo, clip, &env, x, y));
            if handle.is_some() || geo_::clip_edge_at(geo, clip, x).is_some() {
                (Cursor::ResizeX, None)
            } else {
                (Cursor::Default, None)
            }
        }
    }
}

/// What the ruler draws while a gesture runs: a cycle range or a marker being dragged.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RulerOverlay {
    pub cycle: Option<(f64, f64)>,
    pub marker: Option<(String, f64)>,
}

#[derive(Clone, Debug)]
pub enum RulerDrag {
    /// Pressed on the ruler: a click locates, a drag draws a cycle range.
    Pending {
        start_x: f64,
        anchor: f64,
    },
    Range {
        anchor: f64,
        start: f64,
        end: f64,
    },
    /// Dragging one edge of the cycle range.
    CycleEdge {
        edge: Edge,
        start: f64,
        end: f64,
    },
    /// A marker flag: a click goes there, a drag moves it.
    Marker {
        id: String,
        from: f64,
        start_x: f64,
        grab: f64,
        bar: f64,
        moved: bool,
    },
}

fn near_cycle_edge(s: &Session, geo: &Geo, x: f64) -> Option<Edge> {
    let t = &s.transport;
    let grip = crate::ui::theme::arrange::CYCLE_GRIP as f64;
    if !t.cycle {
        None
    } else if (x - geo.x(t.cycle_start_bar)).abs() <= grip {
        Some(Edge::Start)
    } else if (x - geo.x(t.cycle_end_bar)).abs() <= grip {
        Some(Edge::End)
    } else {
        None
    }
}

pub fn ruler_press(
    s: &Session,
    geo: &Geo,
    x: f64,
    y: f64,
    widths: &HashMap<String, f32>,
) -> RulerDrag {
    if let Some(m) = geo_::marker_at(s, geo, x, y, widths) {
        return RulerDrag::Marker {
            grab: geo.bar(x) - m.bar,
            id: m.id,
            from: m.bar,
            start_x: x,
            bar: m.bar,
            moved: false,
        };
    }
    if let Some(edge) = near_cycle_edge(s, geo, x) {
        return RulerDrag::CycleEdge {
            edge,
            start: s.transport.cycle_start_bar,
            end: s.transport.cycle_end_bar,
        };
    }
    RulerDrag::Pending {
        start_x: x,
        anchor: geo.bar(x).round().max(0.0),
    }
}

pub fn ruler_move(d: &mut RulerDrag, s: &Session, geo: &Geo, x: f64, free: bool) -> RulerOverlay {
    if let RulerDrag::Marker {
        id,
        start_x,
        grab,
        bar,
        moved,
        ..
    } = d
    {
        if !*moved && (x - *start_x).abs() < RULER_THRESHOLD {
            return RulerOverlay::default();
        }
        *moved = true;
        *bar = geo_::snap(s, (geo.bar(x) - *grab).max(0.0), free).max(0.0);
        return RulerOverlay {
            marker: Some((id.clone(), *bar)),
            ..Default::default()
        };
    }
    let bar = geo.bar(x).round().max(0.0);
    if let RulerDrag::Pending { start_x, anchor } = *d {
        if (x - start_x).abs() < RULER_THRESHOLD {
            return RulerOverlay::default();
        }
        *d = RulerDrag::Range {
            anchor,
            start: anchor,
            end: anchor + 1.0,
        };
    }
    match d {
        RulerDrag::Range { anchor, start, end } => {
            *start = anchor.min(bar);
            *end = anchor.max(bar).max(*start + 1.0);
            RulerOverlay {
                cycle: Some((*start, *end)),
                ..Default::default()
            }
        }
        RulerDrag::CycleEdge { edge, start, end } => {
            match edge {
                Edge::Start => *start = bar.min(*end - 1.0),
                Edge::End => *end = bar.max(*start + 1.0),
            }
            RulerOverlay {
                cycle: Some((*start, *end)),
                ..Default::default()
            }
        }
        _ => RulerOverlay::default(),
    }
}

pub fn ruler_release(d: RulerDrag, s: &Session, geo: &Geo, x: f64, free: bool) -> Vec<Call> {
    match d {
        RulerDrag::Marker {
            id,
            from,
            bar,
            moved,
            ..
        } => {
            if !moved {
                return vec![("marker.goto", json!({"markerId": id}))];
            }
            let taken = s
                .markers
                .iter()
                .any(|m| m.id != id && (m.bar - bar).abs() < SAME_BAR);
            if bar == from || taken {
                return vec![];
            }
            vec![("marker.move", json!({"markerId": id, "bar": bar}))]
        }
        RulerDrag::Pending { .. } => {
            let bar = geo_::snap(s, geo.bar(x).max(0.0), free).max(0.0);
            vec![(
                "transport.locate",
                json!({"beats": bar * s.beats_per_bar()}),
            )]
        }
        RulerDrag::Range { start, end, .. } | RulerDrag::CycleEdge { start, end, .. } => vec![(
            "transport.setCycle",
            json!({"enabled": true, "startBar": start, "endBar": end}),
        )],
    }
}

pub fn ruler_hover(
    s: &Session,
    geo: &Geo,
    x: f64,
    y: f64,
    widths: &HashMap<String, f32>,
) -> Cursor {
    if geo_::marker_at(s, geo, x, y, widths).is_some() {
        Cursor::Grab
    } else if near_cycle_edge(s, geo, x).is_some() {
        Cursor::ResizeX
    } else {
        Cursor::Default
    }
}

/// Whole BPM, or tenths with Shift.
fn round_bpm(bpm: f64, fine: bool) -> f64 {
    if fine {
        (bpm * 10.0).round() / 10.0
    } else {
        bpm.round()
    }
}

#[derive(Clone, Debug)]
pub enum TempoGesture {
    /// Pressed on empty lane: a click adds a change there.
    Add { start_x: f64 },
    /// A point grabbed: dragging moves it in time and tempo.
    Point {
        start_x: f64,
        start_y: f64,
        grab: f64,
        /// The lane's BPM range when the drag began; it holds still while dragging.
        range: (f64, f64),
        moved: bool,
        drag: TempoDrag,
    },
}

pub fn tempo_press(s: &Session, geo: &Geo, x: f64, y: f64, h: f64) -> TempoGesture {
    match geo_::tempo_at(s, geo, x, y, h) {
        Some(bar) => {
            let points = geo_::tempo_points(s, None);
            let bpm = points
                .iter()
                .find(|p| (p.bar - bar).abs() < SAME_BAR)
                .map_or(s.transport.tempo, |p| p.bpm);
            TempoGesture::Point {
                start_x: x,
                start_y: y,
                grab: geo.bar(x) - bar,
                range: geo_::tempo_range(&points),
                moved: false,
                drag: TempoDrag {
                    from: bar,
                    bar,
                    bpm,
                },
            }
        }
        None => TempoGesture::Add { start_x: x },
    }
}

#[allow(clippy::too_many_arguments)]
pub fn tempo_move(
    g: &mut TempoGesture,
    s: &Session,
    geo: &Geo,
    x: f64,
    y: f64,
    h: f64,
    free: bool,
    fine: bool,
) -> Option<TempoDrag> {
    let TempoGesture::Point {
        start_x,
        start_y,
        grab,
        range,
        moved,
        drag,
    } = g
    else {
        return None;
    };
    if !*moved && (x - *start_x).hypot(y - *start_y) < DRAG_THRESHOLD {
        return None;
    }
    *moved = true;
    let bpm = round_bpm(geo_::bpm_at_y(y, *range, h), fine);
    // The starting tempo stays at bar 1; a change keeps after it.
    let first = if free { 0.001 } else { geo_::snap_step(s) };
    let bar = if drag.from == 0.0 {
        0.0
    } else {
        geo_::snap(s, geo.bar(x) - *grab, free).max(first)
    };
    *drag = TempoDrag {
        from: drag.from,
        bar,
        bpm,
    };
    Some(*drag)
}

#[allow(clippy::too_many_arguments)]
pub fn tempo_release(
    g: TempoGesture,
    s: &Session,
    geo: &Geo,
    x: f64,
    y: f64,
    h: f64,
    free: bool,
    fine: bool,
) -> Vec<Call> {
    match g {
        TempoGesture::Add { start_x } => {
            if (x - start_x).abs() >= DRAG_THRESHOLD {
                return vec![];
            }
            let bar = geo_::snap(s, geo.bar(x), free);
            if bar <= SAME_BAR {
                return vec![];
            }
            let range = geo_::tempo_range(&geo_::tempo_points(s, None));
            let bpm = round_bpm(geo_::bpm_at_y(y, range, h), fine);
            vec![("tempo.set", json!({"bar": bar, "bpm": bpm}))]
        }
        TempoGesture::Point { moved, drag, .. } => {
            if !moved {
                return vec![];
            }
            let TempoDrag { from, bar, bpm } = drag;
            let taken = s
                .tempo_changes
                .iter()
                .any(|p| (p.bar - from).abs() >= SAME_BAR && (p.bar - bar).abs() < SAME_BAR);
            if taken {
                vec![]
            } else if from == 0.0 || (bar - from).abs() < SAME_BAR {
                vec![("tempo.set", json!({"bar": from, "bpm": bpm}))]
            } else {
                vec![("tempo.move", json!({"bar": from, "toBar": bar, "bpm": bpm}))]
            }
        }
    }
}

pub fn tempo_hover(s: &Session, geo: &Geo, x: f64, y: f64, h: f64) -> Cursor {
    if geo_::tempo_at(s, geo, x, y, h).is_some() {
        Cursor::Grab
    } else {
        Cursor::Copy
    }
}

/// A track header being dragged to another place in the list.
#[derive(Clone, Debug)]
pub struct TrackDrag {
    pub id: String,
    pub index: usize,
    pub start_y: f64,
    pub moved: bool,
    /// The gap the track would drop into: 0 is above the first track, `count` below the last.
    pub slot: usize,
}

impl TrackDrag {
    pub fn new(id: String, index: usize, y: f64) -> Self {
        Self {
            id,
            index,
            start_y: y,
            moved: false,
            slot: index,
        }
    }
    /// Follow the pointer, `y` in list coordinates.
    pub fn follow(&mut self, y: f64, row: f64, count: usize) {
        if !self.moved && (y - self.start_y).abs() < DRAG_THRESHOLD {
            return;
        }
        self.moved = true;
        self.slot = ((y / row).round().max(0.0) as usize).min(count);
    }
    /// The release: a move to the new index, or a plain click that selects the track.
    pub fn release(self) -> Vec<Call> {
        if !self.moved {
            return vec![("track.select", json!({"trackId": self.id}))];
        }
        let index = if self.slot > self.index {
            self.slot - 1
        } else {
            self.slot
        };
        if index == self.index {
            return vec![];
        }
        vec![("track.move", json!({"trackId": self.id, "index": index}))]
    }
}

#[cfg(test)]
mod tests {
    use super::super::geometry::tests::song;
    use super::*;
    use crate::ui::theme::arrange;

    fn geo() -> Geo {
        Geo::new(40.0, 0.0)
    }
    fn names(calls: &[Call]) -> Vec<&'static str> {
        calls.iter().map(|c| c.0).collect()
    }

    #[test]
    fn dragging_a_clip_moves_it_on_the_grid_and_only_to_its_kind_of_track() {
        let s = song();
        let g = geo();
        let mid = g.row / 2.0;
        let (drag, calls) = lane_press(&s, &g, POINTER, 60.0, mid, false);
        assert_eq!(names(&calls), ["clip.select"]);
        let mut drag = drag.unwrap();
        assert!(matches!(drag, LaneDrag::Move { .. }));
        // Below the threshold nothing moves.
        assert_eq!(
            lane_move(&mut drag, &s, &g, 61.0, mid, false),
            LaneOverlay::default()
        );
        // Two bars to the right, down onto the audio track: the MIDI clip stays on its track.
        let overlay = lane_move(&mut drag, &s, &g, 141.0, g.row + mid, false);
        let ghost = overlay.ghost.unwrap();
        assert_eq!((ghost.track, ghost.start), (0, 2.0));
        let calls = lane_release(drag, &s);
        assert_eq!(calls[0].0, "clip.move");
        assert_eq!(calls[0].1["startBar"], 2.0);
        assert_eq!(calls[0].1["trackId"], "keys");
    }

    #[test]
    fn a_press_without_a_move_commits_nothing() {
        let s = song();
        let g = geo();
        let (drag, _) = lane_press(&s, &g, POINTER, 60.0, g.row / 2.0, false);
        assert!(lane_release(drag.unwrap(), &s).is_empty());
    }

    #[test]
    fn trimming_the_left_edge_keeps_the_content_and_the_right_edge_resizes() {
        let s = song();
        let g = geo();
        let mid = g.row / 2.0;
        let (drag, _) = lane_press(&s, &g, POINTER, 2.0, mid, false);
        let mut drag = drag.unwrap();
        assert!(matches!(
            drag,
            LaneDrag::Resize {
                edge: Edge::Start,
                ..
            }
        ));
        lane_move(&mut drag, &s, &g, 41.0, mid, false);
        let calls = lane_release(drag, &s);
        assert_eq!(calls[0].0, "clip.trim");
        assert_eq!(
            (
                calls[0].1["startBar"].as_f64(),
                calls[0].1["lengthBars"].as_f64()
            ),
            (Some(1.0), Some(3.0))
        );

        let (drag, _) = lane_press(&s, &g, POINTER, 158.0, mid, false);
        let mut drag = drag.unwrap();
        // Never shorter than one grid step.
        lane_move(&mut drag, &s, &g, -500.0, mid, false);
        let calls = lane_release(drag, &s);
        assert_eq!(calls[0].0, "clip.resize");
        assert_eq!(calls[0].1["lengthBars"], 1.0 / 16.0);
    }

    #[test]
    fn fade_handles_set_seconds_and_stop_at_the_other_fade() {
        let s = song();
        let g = geo();
        let title = g.row + arrange::CLIP_INSET as f64 + 4.0;
        let (drag, _) = lane_press(&s, &g, POINTER, 81.0, title, false);
        let mut drag = drag.unwrap();
        assert!(matches!(
            drag,
            LaneDrag::Fade {
                handle: Fade::In,
                ..
            }
        ));
        // 20 px per second: 40 px in is two seconds; far right stops at the clip's 8 s.
        let overlay = lane_move(&mut drag, &s, &g, 120.0, title, true);
        assert_eq!(overlay.fade, Some(("take".into(), 2.0, 0.0)));
        lane_move(&mut drag, &s, &g, 2000.0, title, true);
        let calls = lane_release(drag, &s);
        assert_eq!(calls[0].0, "clip.setFades");
        assert_eq!(calls[0].1["fadeInSeconds"], 8.0);
    }

    #[test]
    fn the_pencil_draws_midi_clips_and_only_selects_audio_tracks() {
        let s = song();
        let g = geo();
        let mid = g.row / 2.0;
        let (drag, _) = lane_press(&s, &g, PENCIL, 200.0, mid, false);
        let mut drag = drag.unwrap();
        let overlay = lane_move(&mut drag, &s, &g, 280.0, mid, false);
        assert_eq!(
            overlay.pencil.map(|p| (p.start, p.length)),
            Some((5.0, 2.0))
        );
        let calls = lane_release(drag, &s);
        assert_eq!(calls[0].0, "clip.create");
        assert_eq!(calls[0].1["lengthBars"], 2.0);
        // A click without dragging makes a one-bar clip.
        let (drag, _) = lane_press(&s, &g, PENCIL, 200.0, mid, false);
        assert_eq!(lane_release(drag.unwrap(), &s)[0].1["lengthBars"], 1.0);
        let (drag, _) = lane_press(&s, &g, PENCIL, 400.0, g.row + mid, false);
        assert_eq!(names(&lane_release(drag.unwrap(), &s)), ["track.select"]);
    }

    #[test]
    fn scissors_split_on_the_grid_inside_the_clip() {
        let s = song();
        let g = geo();
        let (drag, calls) = lane_press(&s, &g, SCISSORS, 61.0, g.row / 2.0, false);
        assert!(drag.is_none());
        assert_eq!(
            calls[0],
            ("clip.split", json!({"clipId": "hook", "bar": 1.5}))
        );
        let (cursor, guide) = lane_hover(&s, &g, SCISSORS, 61.0, g.row / 2.0, false);
        assert_eq!((cursor, guide), (Cursor::Split, Some((0, 1.5))));
        assert!(
            lane_press(&s, &g, SCISSORS, 1.0, g.row / 2.0, false)
                .1
                .is_empty(),
            "at the edge"
        );
    }

    #[test]
    fn a_click_on_an_empty_lane_clears_the_clip_and_selects_the_track() {
        let mut s = song();
        s.view.selected_clip_id = Some("hook".into());
        let g = geo();
        let (drag, calls) = lane_press(&s, &g, POINTER, 400.0, g.row * 1.5, false);
        assert!(drag.is_none());
        assert_eq!(names(&calls), ["clip.deselect", "track.select"]);
    }

    #[test]
    fn the_ruler_locates_draws_a_cycle_and_moves_markers() {
        let mut s = song();
        let g = geo();
        let none = HashMap::new();
        // A click locates on the grid.
        let d = ruler_press(&s, &g, 61.0, 4.0, &none);
        assert_eq!(
            ruler_release(d, &s, &g, 61.0, false)[0],
            ("transport.locate", json!({"beats": 6.0}))
        );
        // A drag draws whole bars and turns cycle on.
        let mut d = ruler_press(&s, &g, 41.0, 4.0, &none);
        let overlay = ruler_move(&mut d, &s, &g, 200.0, false);
        assert_eq!(overlay.cycle, Some((1.0, 5.0)));
        let calls = ruler_release(d, &s, &g, 200.0, false);
        assert_eq!(
            calls[0],
            (
                "transport.setCycle",
                json!({"enabled": true, "startBar": 1.0, "endBar": 5.0})
            )
        );
        // A marker flag: a click goes there, a drag moves it, never onto another marker.
        s.markers = vec![
            ryolune_engine::model::Marker {
                id: "m".into(),
                bar: 2.0,
                name: "Verse".into(),
                color: None,
            },
            ryolune_engine::model::Marker {
                id: "o".into(),
                bar: 6.0,
                name: "Chorus".into(),
                color: None,
            },
        ];
        let flag = arrange::MARKER_TOP as f64 + 4.0;
        let d = ruler_press(&s, &g, 85.0, flag, &none);
        assert_eq!(
            names(&ruler_release(d, &s, &g, 85.0, false)),
            ["marker.goto"]
        );
        let mut d = ruler_press(&s, &g, 85.0, flag, &none);
        assert_eq!(
            ruler_move(&mut d, &s, &g, 125.0, false).marker,
            Some(("m".into(), 3.0))
        );
        assert_eq!(
            ruler_release(d, &s, &g, 125.0, false)[0],
            ("marker.move", json!({"markerId": "m", "bar": 3.0}))
        );
        let mut d = ruler_press(&s, &g, 85.0, flag, &none);
        ruler_move(&mut d, &s, &g, 245.0, false);
        assert!(
            ruler_release(d, &s, &g, 245.0, false).is_empty(),
            "bar 6 is taken"
        );
    }

    #[test]
    fn the_cycle_edges_trim_the_range() {
        let mut s = song();
        s.transport.cycle = true;
        s.transport.cycle_start_bar = 2.0;
        s.transport.cycle_end_bar = 6.0;
        let g = geo();
        let mut d = ruler_press(&s, &g, 241.0, 4.0, &HashMap::new());
        assert!(matches!(
            d,
            RulerDrag::CycleEdge {
                edge: Edge::End,
                ..
            }
        ));
        assert_eq!(
            ruler_move(&mut d, &s, &g, 40.0, false).cycle,
            Some((2.0, 3.0)),
            "at least a bar"
        );
    }

    #[test]
    fn the_tempo_track_adds_moves_and_retunes_points() {
        const H: f64 = 64.0;
        let mut s = song();
        let g = geo();
        let range = geo_::tempo_range(&geo_::tempo_points(&s, None));
        // A click past bar 1 adds a change at the tempo under the pointer.
        let y = geo_::y_of_bpm(110.0, range, H);
        let press = tempo_press(&s, &g, 160.0, y, H);
        let calls = tempo_release(press, &s, &g, 160.0, y, H, false, false);
        assert_eq!(calls[0], ("tempo.set", json!({"bar": 4.0, "bpm": 110.0})));
        // Dragging a change moves it and sets its tempo in one command.
        s.tempo_changes = vec![ryolune_engine::tempo::TempoPoint {
            bar: 4.0,
            bpm: 110.0,
            ramp: false,
        }];
        let range = geo_::tempo_range(&geo_::tempo_points(&s, None));
        let mut press = tempo_press(&s, &g, 160.0, geo_::y_of_bpm(110.0, range, H), H);
        let to = geo_::y_of_bpm(100.0, range, H);
        let drag = tempo_move(&mut press, &s, &g, 240.0, to, H, false, false).unwrap();
        assert_eq!((drag.bar, drag.bpm), (6.0, 100.0));
        let calls = tempo_release(press, &s, &g, 240.0, to, H, false, false);
        assert_eq!(
            calls[0],
            (
                "tempo.move",
                json!({"bar": 4.0, "toBar": 6.0, "bpm": 100.0})
            )
        );
        // The starting tempo only changes tempo, never its bar.
        let mut press = tempo_press(&s, &g, 20.0, geo_::y_of_bpm(120.0, range, H), H);
        tempo_move(&mut press, &s, &g, 100.0, to, H, false, false);
        let calls = tempo_release(press, &s, &g, 100.0, to, H, false, false);
        assert_eq!(calls[0], ("tempo.set", json!({"bar": 0.0, "bpm": 100.0})));
    }

    #[test]
    fn a_header_drag_reorders_and_a_click_selects() {
        let mut d = TrackDrag::new("a".into(), 0, 10.0);
        d.follow(11.0, 88.0, 3);
        assert_eq!(d.clone().release()[0].0, "track.select");
        d.follow(200.0, 88.0, 3);
        assert_eq!(d.slot, 2);
        assert_eq!(
            d.release()[0],
            ("track.move", json!({"trackId": "a", "index": 1}))
        );
        let mut up = TrackDrag::new("c".into(), 2, 200.0);
        up.follow(0.0, 88.0, 3);
        assert_eq!(up.release()[0].1["index"], 0);
    }
}
