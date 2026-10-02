//! Where things are in the piano roll: the clip fitted to the width, key rows from the
//! bottom up, and the note under the pointer. Pure functions, so painting and hit testing
//! read one geometry and the tests can check it without a window.

use crate::ui::theme::editor::{KEY_ROW, NOTE_EDGE_GRIP, RULER};
use ryolune_engine::model::{Clip, ClipData, Note};

/// Highest lowest pitch the registry accepts for `view.set editorLowPitch`.
pub const MAX_LOW_PITCH: i32 = 108;
/// Lowest pitch shown when the open clip has no notes yet: C3.
pub const DEFAULT_LOW_PITCH: i32 = 48;

const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// A black key of the piano.
pub fn is_black_key(pitch: i32) -> bool {
    NOTE_NAMES[pitch.rem_euclid(12) as usize].ends_with('#')
}

/// "C4" for 60, as the key column and the tooltips write a pitch.
pub fn note_label(pitch: i32) -> String {
    format!(
        "{}{}",
        NOTE_NAMES[pitch.rem_euclid(12) as usize],
        pitch.div_euclid(12) - 1
    )
}

/// The notes of a MIDI clip; none for an audio clip.
pub fn notes(clip: Option<&Clip>) -> &[Note] {
    match clip.map(|c| &c.data) {
        Some(ClipData::Midi { notes, .. }) => notes,
        _ => &[],
    }
}

/// How many key rows a roll of this height shows: the top one may be cut by the ruler.
pub fn rows_for(height: f32) -> i32 {
    (((height - RULER) / KEY_ROW).ceil() as i32).max(1)
}

/// The highest lowest pitch that keeps the top row at or under G9 (127).
pub fn max_low(rows: i32) -> i32 {
    (128 - rows).clamp(0, MAX_LOW_PITCH)
}

/// Lowest pitch shown when the view does not set one: the octave C at or below the clip's
/// lowest note.
pub fn auto_low(notes: &[Note], rows: i32) -> i32 {
    let Some(lowest) = notes.iter().map(|n| n.pitch as i32).min() else {
        return DEFAULT_LOW_PITCH.min(max_low(rows));
    };
    (lowest.div_euclid(12) * 12).clamp(0, max_low(rows))
}

/// The piano roll's geometry: the clip spans the width, rows stack from the bottom edge so
/// the lowest pitch is always whole, and the ruler sits over the top.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Roll {
    pub width: f32,
    pub height: f32,
    pub px_per_bar: f32,
    pub px_per_beat: f32,
    pub rows: i32,
    pub low: i32,
    pub bpb: f64,
    /// Length of the open clip in beats (four bars when nothing is open).
    pub length_beats: f64,
}

impl Roll {
    /// `low` is the view's lowest pitch, if it sets one; otherwise the clip is framed.
    pub fn new(
        width: f32,
        height: f32,
        length_bars: Option<f64>,
        bpb: f64,
        low: Option<u8>,
        notes: &[Note],
    ) -> Self {
        let bars = length_bars.unwrap_or(4.0).max(1.0 / 64.0);
        let px_per_bar = (width / bars as f32).max(1.0);
        let rows = rows_for(height);
        let low = match low {
            Some(low) => (low as i32).min(max_low(rows)),
            None => auto_low(notes, rows),
        };
        Self {
            width,
            height,
            px_per_bar,
            px_per_beat: px_per_bar / bpb.max(1e-9) as f32,
            rows,
            low,
            bpb,
            length_beats: bars * bpb,
        }
    }
    pub fn grid_top(&self) -> f32 {
        RULER
    }
    /// The pitch whose row holds `y` (may lie outside 0-127 above or below the keys).
    pub fn pitch_at_y(&self, y: f32) -> i32 {
        self.low + ((self.height - y) / KEY_ROW).floor() as i32
    }
    /// Top edge of a pitch's row.
    pub fn y_of_pitch(&self, pitch: i32) -> f32 {
        self.height - (pitch - self.low + 1) as f32 * KEY_ROW
    }
    /// The highest pitch with any of its row below the ruler.
    pub fn high(&self) -> i32 {
        (self.low + self.rows - 1).min(127)
    }
    pub fn beat_at_x(&self, x: f32) -> f64 {
        (x / self.px_per_beat) as f64
    }
    pub fn x_of_beat(&self, beat: f64) -> f32 {
        beat as f32 * self.px_per_beat
    }

    /// The note under (x, y), the last drawn first, and whether the pointer is on its right
    /// edge (a resize) rather than its body (a move).
    pub fn hit_note<'a>(&self, notes: &'a [Note], x: f32, y: f32) -> Option<(&'a Note, bool)> {
        if y < self.grid_top() {
            return None;
        }
        let pitch = self.pitch_at_y(y);
        let beat = self.beat_at_x(x);
        notes
            .iter()
            .rev()
            .find(|n| n.pitch as i32 == pitch && beat >= n.start && beat < n.start + n.length)
            .map(|n| {
                let end = self.x_of_beat(n.start + n.length);
                let wide = n.length as f32 * self.px_per_beat > NOTE_EDGE_GRIP * 2.0;
                (n, wide && end - x <= NOTE_EDGE_GRIP)
            })
    }
}

/// A beat on the grid, or as it is when `fine` (Option held).
pub fn snap(beat: f64, division: u32, fine: bool) -> f64 {
    if fine {
        beat
    } else {
        crate::ui::format::snap_beats(beat, division)
    }
}

/// One grid step in beats (a sixteenth at division 16).
pub fn step_beats(division: u32) -> f64 {
    4.0 / division.max(1) as f64
}

/// The `view.set editorLowPitch` a wheel turn asks for: `rows_moved` rows up (positive) or
/// down, kept inside the keyboard.
pub fn scrolled_low(low: i32, rows_moved: i32, rows: i32) -> i32 {
    (low + rows_moved).clamp(0, max_low(rows))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(id: &str, start: f64, length: f64, pitch: u8) -> Note {
        Note {
            id: id.into(),
            start,
            length,
            pitch,
            velocity: 100,
            agent: false,
            channel: 0,
        }
    }

    #[test]
    fn keys_are_named_and_coloured_like_a_piano() {
        assert_eq!(note_label(60), "C4");
        assert_eq!(note_label(0), "C-1");
        assert_eq!(note_label(127), "G9");
        assert!(is_black_key(61) && is_black_key(70));
        assert!(!is_black_key(60) && !is_black_key(64) && !is_black_key(65));
    }

    #[test]
    fn the_roll_frames_the_octave_under_the_lowest_note() {
        let notes = [note("a", 0.0, 1.0, 67), note("b", 1.0, 1.0, 62)];
        assert_eq!(auto_low(&notes, 24), 60);
        assert_eq!(auto_low(&[], 24), DEFAULT_LOW_PITCH);
        // Very high notes still leave the top row at or under 127.
        assert_eq!(auto_low(&[note("c", 0.0, 1.0, 126)], 24), 104);
        let roll = Roll::new(800.0, RULER + 24.0 * KEY_ROW, Some(4.0), 4.0, None, &notes);
        assert_eq!(roll.rows, 24);
        assert_eq!(roll.low, 60);
        // A view value too high for this many rows is brought down.
        let high = Roll::new(
            800.0,
            RULER + 32.0 * KEY_ROW,
            Some(4.0),
            4.0,
            Some(108),
            &notes,
        );
        assert_eq!(high.low, 96);
        assert_eq!(high.high(), 127);
    }

    #[test]
    fn rows_stack_from_the_bottom_and_map_both_ways() {
        let roll = Roll::new(800.0, 300.0, Some(4.0), 4.0, Some(48), &[]);
        // The lowest pitch fills the bottom row.
        assert_eq!(roll.y_of_pitch(48), 300.0 - KEY_ROW);
        assert_eq!(roll.pitch_at_y(299.0), 48);
        assert_eq!(roll.pitch_at_y(300.0 - KEY_ROW - 0.5), 49);
        for pitch in 48..60 {
            let y = roll.y_of_pitch(pitch) + KEY_ROW / 2.0;
            assert_eq!(roll.pitch_at_y(y), pitch);
        }
        // The clip spans the width: 4 bars of 4 beats over 800 px.
        assert_eq!(roll.px_per_bar, 200.0);
        assert_eq!(roll.px_per_beat, 50.0);
        assert_eq!(roll.beat_at_x(125.0), 2.5);
        assert_eq!(roll.x_of_beat(2.5), 125.0);
        assert_eq!(roll.length_beats, 16.0);
    }

    #[test]
    fn notes_are_hit_on_their_body_or_their_right_edge() {
        let notes = [note("a", 1.0, 1.0, 50), note("b", 1.5, 2.0, 50)];
        let roll = Roll::new(800.0, 300.0, Some(4.0), 4.0, Some(48), &notes);
        let y = roll.y_of_pitch(50) + 4.0;
        // Overlapping notes: the later one (drawn on top) wins.
        let (hit, edge) = roll.hit_note(&notes, roll.x_of_beat(1.75), y).unwrap();
        assert_eq!((hit.id.as_str(), edge), ("b", false));
        let (hit, edge) = roll.hit_note(&notes, roll.x_of_beat(1.2), y).unwrap();
        assert_eq!((hit.id.as_str(), edge), ("a", false));
        // The last few pixels of a note resize it.
        let (hit, edge) = roll.hit_note(&notes, roll.x_of_beat(3.5) - 2.0, y).unwrap();
        assert_eq!((hit.id.as_str(), edge), ("b", true));
        // Another row, past the end, or in the ruler: nothing.
        assert!(roll
            .hit_note(&notes, roll.x_of_beat(1.2), roll.y_of_pitch(51) + 4.0)
            .is_none());
        assert!(roll.hit_note(&notes, roll.x_of_beat(3.6), y).is_none());
        assert!(roll.hit_note(&notes, roll.x_of_beat(1.2), 4.0).is_none());
        // A note too short to have an edge is always moved.
        let short = [note("s", 0.0, 0.1, 50)];
        let (_, edge) = roll.hit_note(&short, roll.x_of_beat(0.09), y).unwrap();
        assert!(!edge);
    }

    #[test]
    fn snapping_and_scrolling_stay_on_the_grid_and_the_keyboard() {
        assert_eq!(snap(1.13, 16, false), 1.25);
        assert_eq!(snap(1.13, 16, true), 1.13);
        assert_eq!(snap(1.4, 4, false), 1.0);
        assert_eq!(step_beats(16), 0.25);
        assert_eq!(step_beats(1), 4.0);
        assert_eq!(scrolled_low(48, 3, 24), 51);
        assert_eq!(scrolled_low(2, -5, 24), 0);
        assert_eq!(scrolled_low(100, 10, 24), 104);
    }
}
