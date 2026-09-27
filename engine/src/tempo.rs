//! The song's tempo over time. `Transport::tempo` is the tempo at the start; each
//! [`TempoPoint`] in `Session::tempo_changes` sets a new one from its bar on, either at once or
//! by a ramp that glides from the tempo before it (linear in beats, the way a conductor slows
//! down). A song without points behaves exactly as a single tempo always did.
//!
//! Positions stay musical everywhere (bars and beats); a [`TempoMap`] turns them into seconds
//! and back for the renderer, exports, audio clips (whose audio plays at its own speed, so a
//! clip covers the seconds between its start and end bars) and MIDI files.
use serde::{Deserialize, Serialize};

pub const MIN_BPM: f64 = 20.0;
pub const MAX_BPM: f64 = 400.0;
/// Tempo changes a song may hold.
pub const MAX_POINTS: usize = 1000;
/// Two points closer than this are on the same bar.
pub const SAME_BAR: f64 = 1e-6;

/// A tempo change at `bar` (zero-based, after bar 0: the start is `Transport::tempo`).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub struct TempoPoint {
    pub bar: f64,
    pub bpm: f64,
    /// Glide from the previous tempo to reach `bpm` at `bar`, instead of jumping there.
    /// Absent from the file when false.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ramp: bool,
}

pub fn valid_bpm(bpm: f64) -> bool {
    bpm.is_finite() && (MIN_BPM..=MAX_BPM).contains(&bpm)
}

/// A stretch of constant or linearly changing tempo.
#[derive(Clone, Copy, Debug)]
struct Segment {
    /// Where it starts, in beats and in seconds from the song's start.
    beat: f64,
    seconds: f64,
    bpm: f64,
    /// Tempo change per beat; zero when the tempo holds.
    slope: f64,
}
impl Segment {
    fn bpm_at(&self, beat: f64) -> f64 {
        self.bpm + self.slope * (beat - self.beat)
    }
    /// Seconds from the segment's start to `beat`.
    fn seconds_to(&self, beat: f64) -> f64 {
        let x = beat - self.beat;
        if self.slope == 0.0 {
            x * 60.0 / self.bpm
        } else {
            60.0 / self.slope * (1.0 + self.slope * x / self.bpm).ln()
        }
    }
    /// Beats from the segment's start after `seconds`.
    fn beats_after(&self, seconds: f64) -> f64 {
        if self.slope == 0.0 {
            seconds * self.bpm / 60.0
        } else {
            self.bpm * ((self.slope * seconds / 60.0).exp_m1()) / self.slope
        }
    }
}

/// Seconds and tempo for any beat of a song. Built off the audio thread; lookups do not
/// allocate.
#[derive(Clone, Debug)]
pub struct TempoMap {
    segments: Vec<Segment>,
}
impl TempoMap {
    pub fn constant(bpm: f64) -> Self {
        Self {
            segments: vec![Segment {
                beat: 0.0,
                seconds: 0.0,
                bpm,
                slope: 0.0,
            }],
        }
    }
    /// `points` in bar order, as `Session::validate` guarantees.
    pub fn new(initial: f64, points: &[TempoPoint], beats_per_bar: f64) -> Self {
        let mut map = Self::constant(initial);
        for point in points {
            let beat = point.bar * beats_per_bar;
            let last = *map.segments.last().expect("a map has a segment");
            if beat <= last.beat {
                continue;
            }
            let segment = map.segments.last_mut().expect("a map has a segment");
            if point.ramp {
                segment.slope = (point.bpm - last.bpm) / (beat - last.beat);
            }
            let seconds = last.seconds + segment.seconds_to(beat);
            map.segments.push(Segment {
                beat,
                seconds,
                bpm: point.bpm,
                slope: 0.0,
            });
        }
        map
    }
    /// One tempo from start to end.
    pub fn is_constant(&self) -> bool {
        self.segments.len() == 1
    }
    /// The segment `beat` is in; beats before zero belong to the first.
    pub fn segment(&self, beat: f64) -> usize {
        self.segments
            .partition_point(|s| s.beat <= beat)
            .saturating_sub(1)
    }
    /// Where segment `index` ends, in beats; infinite for the last one.
    pub fn segment_end(&self, index: usize) -> f64 {
        self.segments
            .get(index + 1)
            .map_or(f64::INFINITY, |s| s.beat)
    }
    /// The tempo at `beat` in segment `index` (see [`segment`](Self::segment)).
    #[inline]
    pub fn bpm_in(&self, index: usize, beat: f64) -> f64 {
        self.segments[index].bpm_at(beat)
    }
    pub fn bpm(&self, beat: f64) -> f64 {
        self.bpm_in(self.segment(beat), beat)
    }
    /// Seconds from the song's start to `beat`.
    pub fn seconds(&self, beat: f64) -> f64 {
        let s = &self.segments[self.segment(beat)];
        s.seconds + s.seconds_to(beat)
    }
    /// The beat `seconds` after the song's start.
    pub fn beat(&self, seconds: f64) -> f64 {
        let index = self
            .segments
            .partition_point(|s| s.seconds <= seconds)
            .saturating_sub(1);
        let s = &self.segments[index];
        s.beat + s.beats_after(seconds - s.seconds)
    }
    /// Seconds between two beats. Exactly `(to - from) * 60 / bpm` at one tempo.
    pub fn duration(&self, from: f64, to: f64) -> f64 {
        if self.is_constant() {
            (to - from) * 60.0 / self.segments[0].bpm
        } else {
            self.seconds(to) - self.seconds(from)
        }
    }
    /// Beats that `seconds` cover from `from` on.
    pub fn beats_for(&self, from: f64, seconds: f64) -> f64 {
        if self.is_constant() {
            seconds * self.segments[0].bpm / 60.0
        } else {
            self.beat(self.seconds(from) + seconds) - from
        }
    }
}

/// Keep points in bar order, one per bar, after bar 0, within range.
pub fn validate(points: &[TempoPoint]) -> crate::Result<()> {
    if points.len() > MAX_POINTS {
        return Err(format!("A song holds at most {MAX_POINTS} tempo changes"));
    }
    let mut previous = 0.0;
    for p in points {
        if !crate::model::valid_time(p.bar) || p.bar <= previous || !valid_bpm(p.bpm) {
            return Err(
                "Tempo changes must be after bar 1, in bar order, one per bar, at 20-400 BPM"
                    .into(),
            );
        }
        previous = p.bar + SAME_BAR / 2.0;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(bar: f64, bpm: f64, ramp: bool) -> TempoPoint {
        TempoPoint { bar, bpm, ramp }
    }

    #[test]
    fn one_tempo_is_the_old_arithmetic() {
        let map = TempoMap::new(120.0, &[], 4.0);
        assert!(map.is_constant());
        assert_eq!(map.seconds(8.0), 4.0);
        assert_eq!(map.beat(4.0), 8.0);
        assert_eq!(map.duration(2.0, 6.0), 2.0);
        assert_eq!(map.beats_for(3.0, 1.5), 3.0);
        assert_eq!(map.bpm(1000.0), 120.0);
        assert_eq!(map.seconds(-2.0), -1.0);
    }

    #[test]
    fn steps_change_the_tempo_at_their_bar() {
        // Bar 2 (beat 8) moves from 120 to 60 BPM.
        let map = TempoMap::new(120.0, &[point(2.0, 60.0, false)], 4.0);
        assert_eq!(map.bpm(7.999), 120.0);
        assert_eq!(map.bpm(8.0), 60.0);
        assert_eq!(map.seconds(8.0), 4.0);
        assert_eq!(map.seconds(12.0), 8.0);
        assert_eq!(map.beat(8.0), 12.0);
        assert_eq!(map.beat(2.0), 4.0);
        assert_eq!(map.duration(6.0, 10.0), 3.0);
        assert_eq!(map.beats_for(6.0, 3.0), 4.0);
    }

    #[test]
    fn ramps_glide_linearly_in_beats_and_invert_exactly() {
        // 60 BPM at the start, ramping to 120 at bar 1 (beat 4), then holding.
        let map = TempoMap::new(60.0, &[point(1.0, 120.0, true)], 4.0);
        assert!((map.bpm(2.0) - 90.0).abs() < 1e-12);
        // 60/k ln(b1/b0) with k = 15 BPM per beat.
        let ramp = 60.0 / 15.0 * 2f64.ln();
        assert!((map.seconds(4.0) - ramp).abs() < 1e-12);
        assert!((map.seconds(6.0) - ramp - 1.0).abs() < 1e-12);
        for beat in [0.0, 0.5, 1.0, 2.5, 3.99, 4.0, 5.0, 9.75] {
            assert!((map.beat(map.seconds(beat)) - beat).abs() < 1e-9, "{beat}");
        }
        // Faster than 60 BPM throughout the ramp, slower than 120.
        assert!(map.seconds(4.0) < 4.0 && map.seconds(4.0) > 2.0);
        // A ramp down works the same way.
        let down = TempoMap::new(120.0, &[point(1.0, 60.0, true)], 4.0);
        assert!((down.seconds(4.0) - ramp).abs() < 1e-12);
        assert!((down.beat(down.seconds(3.0)) - 3.0).abs() < 1e-9);
    }

    #[test]
    fn a_ramp_to_the_same_tempo_is_a_hold() {
        let map = TempoMap::new(100.0, &[point(1.0, 100.0, true)], 4.0);
        assert_eq!(map.seconds(4.0), 2.4);
        assert_eq!(map.bpm(2.0), 100.0);
    }

    #[test]
    fn validation_keeps_points_ordered_and_in_range() {
        assert!(validate(&[point(1.0, 90.0, false), point(2.0, 100.0, true)]).is_ok());
        assert!(
            validate(&[point(0.0, 90.0, false)]).is_err(),
            "bar 0 is the transport's"
        );
        assert!(validate(&[point(2.0, 90.0, false), point(1.0, 90.0, false)]).is_err());
        assert!(validate(&[point(1.0, 90.0, false), point(1.0, 95.0, false)]).is_err());
        assert!(validate(&[point(1.0, 10.0, false)]).is_err());
        assert!(validate(&[point(1.0, f64::NAN, false)]).is_err());
    }

    #[test]
    fn ramp_flag_is_absent_from_the_file_when_off() {
        let text = serde_json::to_string(&point(1.0, 90.0, false)).unwrap();
        assert_eq!(text, r#"{"bar":1.0,"bpm":90.0}"#);
        let back: TempoPoint = serde_json::from_str(r#"{"bar":2,"bpm":80,"ramp":true}"#).unwrap();
        assert!(back.ramp);
    }
}
