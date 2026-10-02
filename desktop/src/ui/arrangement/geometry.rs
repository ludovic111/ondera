//! Where things fall in the arrangement and what lies under a point: bars and pixels, tracks,
//! clips, their edges and fade handles, marker flags, tempo points and the zoom slider. Pure
//! functions of the session and the view, so painting and hit testing read the same numbers
//! and tests need no window.
//!
//! Lane coordinates: `x` from the lanes' left edge, `y` from the first track's top (the
//! vertical scroll already added).

use crate::ui::{
    format,
    theme::{arrange, layout},
};
use ryolune_engine::{
    model::{Clip, ClipData, FadeCurve, Marker, Session},
    tempo::{TempoPoint, MAX_BPM, MIN_BPM},
};
use std::collections::HashMap;

/// The arrangement's zoom range, in pixels per bar (`view.set pixelsPerBar`).
pub const ZOOM_MIN: f64 = 12.0;
pub const ZOOM_MAX: f64 = 480.0;
/// Zoom of a new song, where a double click on the zoom slider returns.
pub const ZOOM_DEFAULT: f64 = 60.0;
/// Two bars closer than this are the same bar.
pub const SAME_BAR: f64 = 1e-6;

/// The view's mapping between bars and lane pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geo {
    /// Pixels per bar.
    pub ppb: f64,
    /// First visible bar, fractional.
    pub scroll: f64,
    /// Height of one track row.
    pub row: f64,
}

impl Geo {
    pub fn new(pixels_per_bar: f32, scroll_bars: f64) -> Self {
        Self {
            ppb: (pixels_per_bar as f64).max(1.0),
            scroll: scroll_bars,
            row: layout::TRACK_HEIGHT as f64,
        }
    }
    pub fn x(&self, bar: f64) -> f64 {
        (bar - self.scroll) * self.ppb
    }
    pub fn bar(&self, x: f64) -> f64 {
        self.scroll + x / self.ppb
    }
    /// The track row at `y`, if `y` is inside one of `count` rows.
    pub fn row_at(&self, y: f64, count: usize) -> Option<usize> {
        let row = (y / self.row).floor();
        (row >= 0.0 && (row as usize) < count).then_some(row as usize)
    }
}

/// The clip under a point. Later clips paint on top, so they are tried first.
pub fn clip_at<'a>(s: &'a Session, geo: &Geo, x: f64, y: f64) -> Option<&'a Clip> {
    let track = &s.tracks[geo.row_at(y, s.tracks.len())?];
    let bar = geo.bar(x);
    s.clips
        .iter()
        .rev()
        .find(|c| c.track_id == track.id && bar >= c.start_bar && bar < c.start_bar + c.length_bars)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Start,
    End,
}

/// Which edge of a clip is within the trim grip of `x`. Short clips get a third of their width
/// at each end, so the middle still moves them.
pub fn clip_edge_at(geo: &Geo, clip: &Clip, x: f64) -> Option<Edge> {
    let x0 = geo.x(clip.start_bar);
    let x1 = geo.x(clip.start_bar + clip.length_bars);
    let grip = (arrange::CLIP_EDGE_GRIP as f64).min((x1 - x0) / 3.0);
    if x - x0 <= grip {
        Some(Edge::Start)
    } else if x1 - x <= grip {
        Some(Edge::End)
    } else {
        None
    }
}

/// Screen pixels per second of audio over a stretch of bars, at this zoom and the song's tempo.
pub fn pixels_per_second(s: &Session, ppb: f64, from: f64, to: f64) -> f64 {
    let seconds = s.bars_seconds(from, to);
    if seconds <= 0.0 {
        return ppb;
    }
    ppb * (to - from) / seconds
}

/// Pixels per second where a clip's fades lie: over its first bar (in) and its last (out).
pub fn fade_rates(s: &Session, ppb: f64, start_bar: f64, length_bars: f64) -> (f64, f64) {
    let span = length_bars.min(1.0);
    let end = start_bar + length_bars;
    (
        pixels_per_second(s, ppb, start_bar, start_bar + span),
        pixels_per_second(s, ppb, end - span, end),
    )
}

/// An audio clip's fades and gain, as the renderer plays them (`render.rs` `clip_envelope`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Envelope {
    pub fade_in: f64,
    pub fade_out: f64,
    pub curve: FadeCurve,
    pub gain_db: f32,
}

impl Envelope {
    pub fn of(data: &ClipData) -> Option<Self> {
        match data {
            ClipData::Audio {
                fade_in,
                fade_out,
                fade_curve,
                gain_db,
                ..
            } => Some(Self {
                fade_in: *fade_in,
                fade_out: *fade_out,
                curve: *fade_curve,
                gain_db: *gain_db,
            }),
            ClipData::Midi { .. } => None,
        }
    }
    /// Gain `age` seconds into the clip with `left` seconds to go: fades times clip gain.
    pub fn gain_at(&self, age: f64, left: f64) -> f64 {
        let mut g = 10f64.powf(self.gain_db as f64 / 20.0);
        if self.fade_in > 0.0 && age < self.fade_in {
            g *= self.curve.gain(age / self.fade_in);
        }
        if self.fade_out > 0.0 && left < self.fade_out {
            g *= self.curve.gain(left / self.fade_out);
        }
        g
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fade {
    In,
    Out,
}

/// The fade handle of an audio clip under a point. Handles sit in the title strip where the
/// fade-in ends and the fade-out starts (at the corners when there is none); a clip too narrow
/// for four handles has none.
pub fn fade_handle_at(
    s: &Session,
    geo: &Geo,
    clip: &Clip,
    env: &Envelope,
    x: f64,
    y: f64,
) -> Option<Fade> {
    let row = s.tracks.iter().position(|t| t.id == clip.track_id)?;
    let top = row as f64 * geo.row + arrange::CLIP_INSET as f64;
    if y < top || y > top + arrange::CLIP_TITLE as f64 {
        return None;
    }
    let x0 = geo.x(clip.start_bar);
    let x1 = geo.x(clip.start_bar + clip.length_bars);
    if x1 - x0 < arrange::FADE_HANDLE as f64 * 4.0 {
        return None;
    }
    let (rate_in, rate_out) = fade_rates(s, geo.ppb, clip.start_bar, clip.length_bars);
    let din = (x - (x0 + env.fade_in * rate_in)).abs();
    let dout = (x - (x1 - env.fade_out * rate_out)).abs();
    let grip = (arrange::FADE_GRIP + arrange::FADE_HANDLE / 2.0) as f64;
    if din <= grip && din <= dout {
        Some(Fade::In)
    } else if dout <= grip {
        Some(Fade::Out)
    } else {
        None
    }
}

/// Whether a clip shows its fade handles: when it is wide enough, and selected or faded.
pub fn shows_fade_handles(width: f64, fade_in_px: f64, fade_out_px: f64, selected: bool) -> bool {
    width >= arrange::FADE_HANDLE as f64 * 4.0
        && (selected || fade_in_px >= 1.0 || fade_out_px >= 1.0)
}

/// Where a clip's name starts: after the fade-in handle when the handle would sit on the
/// name. `fades` are the fade lengths in pixels, for an audio clip.
pub fn name_start(
    x: f64,
    w: f64,
    name_width: f64,
    fades: Option<(f64, f64)>,
    selected: bool,
) -> f64 {
    let start = x + 6.0;
    let Some((fin, fout)) = fades else {
        return start;
    };
    let fin = fin.min(w);
    if !shows_fade_handles(w, fin, fout, selected) {
        return start;
    }
    let hs = arrange::FADE_HANDLE as f64;
    let left = (x + fin - hs / 2.0).min(x + w - hs - 1.0).max(x + 1.0);
    let end = start + name_width;
    if left + hs + 4.0 <= start || left - 4.0 >= end {
        start
    } else {
        left + hs + 4.0
    }
}

/// A bar on the snap grid, unless `free` (Option held).
pub fn snap(s: &Session, bar: f64, free: bool) -> f64 {
    if free {
        bar
    } else {
        format::snap_bars(bar, s.transport.snap_division, s.beats_per_bar())
    }
}
/// One step of the snap grid, in bars: the shortest clip a drag makes.
pub fn snap_step(s: &Session) -> f64 {
    format::snap_step_bars(s.transport.snap_division, s.beats_per_bar())
}

/// Markers in bar order, with a flag being dragged drawn where it is.
pub fn markers_with(s: &Session, drag: Option<(&str, f64)>) -> Vec<Marker> {
    let mut list: Vec<Marker> = s
        .markers
        .iter()
        .map(|m| match drag {
            Some((id, bar)) if id == m.id => Marker { bar, ..m.clone() },
            _ => m.clone(),
        })
        .collect();
    list.sort_by(|a, b| a.bar.total_cmp(&b.bar));
    list
}

/// A marker flag: the coloured stripe, then the name on a chip.
pub const FLAG_STRIPE: f64 = 3.0;
pub const FLAG_PAD: f64 = 5.0;
/// Width of a name not measured yet (before its first paint).
const FLAG_FALLBACK: f64 = 48.0;

/// The flag of `list[i]`: where it starts and how wide it is, cut short by the next marker so
/// neighbours never overlap. `widths` holds the names' widths from the last paint.
pub fn flag_span(
    list: &[Marker],
    i: usize,
    geo: &Geo,
    widths: &HashMap<String, f32>,
) -> (f64, f64) {
    let m = &list[i];
    let x = geo.x(m.bar).round();
    let full =
        FLAG_STRIPE + FLAG_PAD * 2.0 + widths.get(&m.id).map_or(FLAG_FALLBACK, |w| *w as f64);
    let limit = list
        .get(i + 1)
        .map_or(f64::INFINITY, |n| geo.x(n.bar).round() - 2.0);
    (x, FLAG_STRIPE.max(full.min(limit - x)))
}

/// The marker whose flag is under a point of the ruler. Later flags win ties.
pub fn marker_at(
    s: &Session,
    geo: &Geo,
    x: f64,
    y: f64,
    widths: &HashMap<String, f32>,
) -> Option<Marker> {
    let top = arrange::MARKER_TOP as f64;
    if y < top - 2.0 || y > top + arrange::MARKER_H as f64 + 2.0 {
        return None;
    }
    let list = markers_with(s, None);
    (0..list.len()).rev().find_map(|i| {
        let (fx, w) = flag_span(&list, i, geo, widths);
        (x >= fx - 3.0 && x <= fx + w).then(|| list[i].clone())
    })
}

/// A tempo point being dragged: `from` is the bar it had (0 is the starting tempo).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TempoDrag {
    pub from: f64,
    pub bar: f64,
    pub bpm: f64,
}

/// The starting tempo as bar 0, then every change, a drag applied, in bar order.
pub fn tempo_points(s: &Session, drag: Option<&TempoDrag>) -> Vec<TempoPoint> {
    let mut points: Vec<TempoPoint> = std::iter::once(TempoPoint {
        bar: 0.0,
        bpm: s.transport.tempo,
        ramp: false,
    })
    .chain(s.tempo_changes.iter().copied())
    .map(|p| match drag {
        Some(d) if (p.bar - d.from).abs() < SAME_BAR => TempoPoint {
            bar: d.bar,
            bpm: d.bpm,
            ..p
        },
        _ => p,
    })
    .collect();
    points.sort_by(|a, b| a.bar.total_cmp(&b.bar));
    points
}

/// The BPM range the tempo track shows: every tempo in the song with room, in tens.
pub fn tempo_range(points: &[TempoPoint]) -> (f64, f64) {
    let lo_bpm = points.iter().map(|p| p.bpm).fold(f64::INFINITY, f64::min);
    let hi_bpm = points
        .iter()
        .map(|p| p.bpm)
        .fold(f64::NEG_INFINITY, f64::max);
    let lo = (((lo_bpm - 10.0) / 10.0).floor() * 10.0).max(MIN_BPM);
    let hi = (((hi_bpm + 10.0) / 10.0).ceil() * 10.0)
        .max(lo + 20.0)
        .min(MAX_BPM);
    (lo, hi)
}

/// Room above the fastest and below the slowest tempo in the lane.
const TEMPO_INSET: f64 = 10.0;

pub fn y_of_bpm(bpm: f64, range: (f64, f64), h: f64) -> f64 {
    let (lo, hi) = range;
    TEMPO_INSET + (1.0 - (bpm - lo) / (hi - lo)) * (h - TEMPO_INSET * 2.0).max(1.0)
}
pub fn bpm_at_y(y: f64, range: (f64, f64), h: f64) -> f64 {
    let (lo, hi) = range;
    let t = 1.0 - (y - TEMPO_INSET) / (h - TEMPO_INSET * 2.0).max(1.0);
    lo + t.clamp(0.0, 1.0) * (hi - lo)
}
/// A tempo as the lane labels it: whole BPM, or one decimal when it has one.
pub fn bpm_label(bpm: f64) -> String {
    let tenths = (bpm * 10.0).round() / 10.0;
    if tenths.fract() == 0.0 {
        format!("{}", tenths as i64)
    } else {
        format!("{tenths:.1}")
    }
}

/// The tempo point under a point of the lane (by its bar), nearest first; before the first
/// change, the starting tempo's line (bar 0).
pub fn tempo_at(s: &Session, geo: &Geo, x: f64, y: f64, h: f64) -> Option<f64> {
    let points = tempo_points(s, None);
    let range = tempo_range(&points);
    let grip = arrange::TEMPO_GRIP as f64;
    let mut best = None;
    let mut best_distance = grip;
    for p in &points[1..] {
        let d = (geo.x(p.bar) - x).hypot(y_of_bpm(p.bpm, range, h) - y);
        if d <= best_distance {
            best = Some(p.bar);
            best_distance = d;
        }
    }
    if best.is_some() {
        return best;
    }
    let before_first = points.get(1).is_none_or(|p| x < geo.x(p.bar) - grip);
    (before_first && (y_of_bpm(points[0].bpm, range, h) - y).abs() <= grip / 2.0 + 1.0)
        .then_some(0.0)
}

/// The zoom slider's position for a zoom, on a log scale, and back.
pub fn zoom_to_t(ppb: f64) -> f64 {
    ((ppb.clamp(ZOOM_MIN, ZOOM_MAX).ln() - ZOOM_MIN.ln()) / (ZOOM_MAX.ln() - ZOOM_MIN.ln()))
        .clamp(0.0, 1.0)
}
pub fn t_to_zoom(t: f64) -> f64 {
    (ZOOM_MIN.ln() + t.clamp(0.0, 1.0) * (ZOOM_MAX.ln() - ZOOM_MIN.ln())).exp()
}

/// Zoom by `factor` keeping the bar under `anchor_x` where it is: the new zoom and scroll.
pub fn zoom_around(geo: &Geo, factor: f64, anchor_x: f64) -> (f64, f64) {
    let next = (geo.ppb * factor).clamp(ZOOM_MIN, ZOOM_MAX);
    let anchor = geo.bar(anchor_x);
    (next, (anchor - anchor_x / next).max(0.0))
}

/// While playing with follow on: the scroll that brings the playhead back into view, page by
/// page, when it runs off either side.
pub fn follow_scroll(geo: &Geo, playhead_bar: f64, width: f64) -> Option<f64> {
    let x = geo.x(playhead_bar);
    (x > width - arrange::CLIP_EDGE_GRIP as f64 * 2.0 || x < 0.0)
        .then(|| (playhead_bar - width * 0.08 / geo.ppb).max(0.0))
}

/// How many bars apart the ruler labels its bars at this zoom.
pub fn label_every(ppb: f64) -> u64 {
    if ppb >= 40.0 {
        1
    } else if ppb >= 20.0 {
        2
    } else if ppb >= 10.0 {
        4
    } else {
        8
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use ryolune_engine::model::{Note, Track};

    /// A song with a MIDI track (one clip at bars 0-4) and an audio track (one clip at 2-6),
    /// at 120 BPM in 4/4 with a sixteenth grid.
    pub fn song() -> Session {
        let mut s = ryolune_engine::store::empty();
        let track = |id: &str, kind: &str| -> Track {
            serde_json::from_value(serde_json::json!({
                "id": id, "name": id, "color": "#6ab3fd", "armed": false, "kind": kind,
                "volume": 0.75, "pan": 0.0, "mute": false, "solo": false
            }))
            .unwrap()
        };
        s.tracks = vec![track("keys", "midi"), track("vox", "audio")];
        s.clips = vec![
            Clip {
                id: "hook".into(),
                name: "Hook".into(),
                agent: false,
                track_id: "keys".into(),
                start_bar: 0.0,
                length_bars: 4.0,
                data: ClipData::Midi {
                    notes: vec![Note {
                        id: "n".into(),
                        start: 0.0,
                        length: 1.0,
                        pitch: 60,
                        velocity: 100,
                        agent: false,
                        channel: 0,
                    }],
                    controllers: vec![],
                },
            },
            Clip {
                id: "take".into(),
                name: "Take".into(),
                agent: false,
                track_id: "vox".into(),
                start_bar: 2.0,
                length_bars: 4.0,
                data: ClipData::audio("src", 0.0),
            },
        ];
        s.sources.insert(
            "src".into(),
            ryolune_engine::model::Source {
                id: "src".into(),
                name: "Take".into(),
                sample_rate: 48000,
                channels: 2,
                file_name: Some("take.wav".into()),
                duration_seconds: 8.0,
                origin: "file".into(),
                seed: None,
                wave_kind: None,
            },
        );
        s.markers.clear();
        s.tempo_changes.clear();
        s.transport.tempo = 120.0;
        s.transport.time_signature.numerator = 4;
        s.transport.time_signature.denominator = 4;
        s.transport.snap_division = 16;
        s.view.selected_clip_id = None;
        s.view.selected_track_id = None;
        s
    }

    #[test]
    fn bars_and_pixels_round_trip_and_rows_stop_at_the_last_track() {
        let geo = Geo::new(40.0, 2.0);
        assert_eq!(geo.x(2.0), 0.0);
        assert_eq!(geo.bar(geo.x(7.25)), 7.25);
        assert_eq!(geo.row_at(geo.row * 1.5, 2), Some(1));
        assert_eq!(geo.row_at(geo.row * 2.5, 2), None);
        assert_eq!(geo.row_at(-1.0, 2), None);
    }

    #[test]
    fn hit_tests_find_clips_their_edges_and_fade_handles() {
        let s = song();
        let geo = Geo::new(40.0, 0.0);
        let mid = geo.row / 2.0;
        assert_eq!(
            clip_at(&s, &geo, 60.0, mid).map(|c| c.id.as_str()),
            Some("hook")
        );
        assert!(clip_at(&s, &geo, 200.0, mid).is_none(), "after the clip");
        assert_eq!(
            clip_at(&s, &geo, 100.0, geo.row + mid).map(|c| c.id.as_str()),
            Some("take")
        );
        let hook = &s.clips[0];
        assert_eq!(clip_edge_at(&geo, hook, 2.0), Some(Edge::Start));
        assert_eq!(clip_edge_at(&geo, hook, 158.0), Some(Edge::End));
        assert_eq!(clip_edge_at(&geo, hook, 80.0), None);
        // A fade handle sits in the title strip at the clip's corner when there is no fade.
        let take = &s.clips[1];
        let env = Envelope::of(&take.data).unwrap();
        let title = geo.row + arrange::CLIP_INSET as f64 + 4.0;
        assert_eq!(
            fade_handle_at(&s, &geo, take, &env, 81.0, title),
            Some(Fade::In)
        );
        assert_eq!(
            fade_handle_at(&s, &geo, take, &env, 239.0, title),
            Some(Fade::Out)
        );
        assert_eq!(fade_handle_at(&s, &geo, take, &env, 160.0, title), None);
        assert_eq!(
            fade_handle_at(&s, &geo, take, &env, 81.0, geo.row + 50.0),
            None,
            "below the title"
        );
    }

    #[test]
    fn fades_follow_the_tempo_and_shape_the_gain() {
        let s = song();
        // 120 BPM in 4/4: a bar lasts two seconds, so 40 px per bar is 20 px per second.
        let (rate_in, rate_out) = fade_rates(&s, 40.0, 2.0, 4.0);
        assert!((rate_in - 20.0).abs() < 1e-9 && (rate_out - 20.0).abs() < 1e-9);
        let env = Envelope {
            fade_in: 1.0,
            fade_out: 2.0,
            curve: FadeCurve::Linear,
            gain_db: 0.0,
        };
        assert!((env.gain_at(0.5, 7.5) - 0.5).abs() < 1e-9);
        assert!((env.gain_at(4.0, 1.0) - 0.5).abs() < 1e-9);
        assert!((env.gain_at(4.0, 4.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn the_name_moves_past_a_fade_handle_that_would_cover_it() {
        assert_eq!(
            name_start(100.0, 200.0, 40.0, Some((0.0, 0.0)), false),
            106.0
        );
        assert!(name_start(100.0, 200.0, 40.0, Some((10.0, 0.0)), false) > 110.0);
        assert_eq!(
            name_start(100.0, 200.0, 40.0, Some((150.0, 0.0)), false),
            106.0
        );
        assert_eq!(
            name_start(100.0, 200.0, 40.0, None, true),
            106.0,
            "MIDI clips have none"
        );
    }

    #[test]
    fn marker_flags_are_cut_short_by_the_next_and_hit_from_the_ruler() {
        let mut s = song();
        s.markers = vec![
            Marker {
                id: "b".into(),
                bar: 4.0,
                name: "Verse".into(),
                color: None,
            },
            Marker {
                id: "a".into(),
                bar: 0.0,
                name: "Intro".into(),
                color: None,
            },
        ];
        let geo = Geo::new(10.0, 0.0);
        let widths = HashMap::from([("a".to_string(), 60.0), ("b".to_string(), 30.0)]);
        let list = markers_with(&s, None);
        assert_eq!(list[0].id, "a", "bar order");
        assert_eq!(
            flag_span(&list, 0, &geo, &widths),
            (0.0, 38.0),
            "stops 2 px before the next"
        );
        let y = arrange::MARKER_TOP as f64 + 4.0;
        assert_eq!(
            marker_at(&s, &geo, 20.0, y, &widths).map(|m| m.id),
            Some("a".into())
        );
        assert_eq!(
            marker_at(&s, &geo, 45.0, y, &widths).map(|m| m.id),
            Some("b".into())
        );
        assert!(
            marker_at(&s, &geo, 20.0, 2.0, &widths).is_none(),
            "above the flags"
        );
        let dragged = markers_with(&s, Some(("a", 9.0)));
        let order: Vec<&str> = dragged.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(order, ["b", "a"]);
    }

    #[test]
    fn the_tempo_track_lists_frames_and_finds_points() {
        const H: f64 = 56.0;
        let mut s = song();
        s.tempo_changes = vec![
            TempoPoint {
                bar: 4.0,
                bpm: 90.0,
                ramp: false,
            },
            TempoPoint {
                bar: 8.0,
                bpm: 140.0,
                ramp: true,
            },
        ];
        let bars = |p: Vec<TempoPoint>| p.iter().map(|p| (p.bar, p.bpm)).collect::<Vec<_>>();
        assert_eq!(
            bars(tempo_points(&s, None)),
            [(0.0, 120.0), (4.0, 90.0), (8.0, 140.0)]
        );
        let drag = TempoDrag {
            from: 4.0,
            bar: 10.0,
            bpm: 100.0,
        };
        assert_eq!(
            bars(tempo_points(&s, Some(&drag))),
            [(0.0, 120.0), (8.0, 140.0), (10.0, 100.0)]
        );

        let range = tempo_range(&tempo_points(&s, None));
        assert_eq!(range, (80.0, 150.0));
        let slow = TempoPoint {
            bar: 0.0,
            bpm: 25.0,
            ramp: false,
        };
        assert_eq!(tempo_range(&[slow]), (20.0, 40.0));
        for bpm in [80.0, 97.5, 150.0] {
            assert!((bpm_at_y(y_of_bpm(bpm, range, H), range, H) - bpm).abs() < 1e-9);
        }
        assert!(y_of_bpm(150.0, range, H) < y_of_bpm(80.0, range, H));

        let geo = Geo::new(40.0, 0.0);
        let at = |bar: f64, bpm: f64| tempo_at(&s, &geo, bar * 40.0, y_of_bpm(bpm, range, H), H);
        assert_eq!(at(4.0, 90.0), Some(4.0));
        assert_eq!(at(8.0, 140.0), Some(8.0));
        assert_eq!(at(1.0, 120.0), Some(0.0));
        assert_eq!(
            at(6.0, 120.0),
            None,
            "after the first change the start is not grabbable"
        );
        assert_eq!(at(2.0, 100.0), None);

        assert_eq!(bpm_label(120.0), "120");
        assert_eq!(bpm_label(117.94), "117.9");
        assert_eq!(bpm_label(89.999), "90");
    }

    #[test]
    fn zoom_maps_to_the_slider_and_keeps_the_anchor_still() {
        assert_eq!(zoom_to_t(ZOOM_MIN), 0.0);
        assert_eq!(zoom_to_t(ZOOM_MAX), 1.0);
        assert!((t_to_zoom(zoom_to_t(100.0)) - 100.0).abs() < 1e-9);
        let geo = Geo::new(40.0, 3.0);
        let (ppb, scroll) = zoom_around(&geo, 2.0, 200.0);
        assert_eq!(ppb, 80.0);
        let after = Geo { ppb, scroll, ..geo };
        assert!((after.bar(200.0) - geo.bar(200.0)).abs() < 1e-9);
        assert_eq!(zoom_around(&geo, 100.0, 0.0).0, ZOOM_MAX);
    }

    #[test]
    fn following_pages_the_view_when_the_playhead_leaves_it() {
        let geo = Geo::new(40.0, 0.0);
        assert_eq!(follow_scroll(&geo, 5.0, 800.0), None);
        let scroll = follow_scroll(&geo, 20.0, 800.0).unwrap();
        assert!((scroll - (20.0 - 800.0 * 0.08 / 40.0)).abs() < 1e-9);
        let behind = Geo::new(40.0, 10.0);
        assert!((follow_scroll(&behind, 2.0, 800.0).unwrap() - 0.4).abs() < 1e-9);
    }
}
