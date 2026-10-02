//! The score view: a read-only treble staff with a head per note by pitch and beat, ledger
//! lines, stems, sharps and duration tails. Notes are edited in the piano roll or the step
//! view; this one only reads.

use super::{
    geometry::{is_black_key, Roll},
    paint::{Face, Pen},
    piano_roll::{paint_playhead, paint_ruler},
    Shown,
};
use crate::ui::theme::{
    editor::{NOTE_HEAD_R, STAFF_GAP},
    Theme,
};
use gpui::{App, Window};

/// Diatonic steps above C for each pitch class (sharps sit on the natural below).
const DIATONIC: [i32; 12] = [0, 0, 1, 1, 2, 3, 3, 4, 4, 5, 5, 6];
/// The diatonic step of E4, the staff's bottom line.
const BOTTOM_LINE: i32 = 4 * 7 + 2;

/// A pitch's diatonic step counted from C0 (C4, pitch 60, is 28).
pub fn diatonic_step(pitch: i32) -> i32 {
    (pitch.div_euclid(12) - 1) * 7 + DIATONIC[pitch.rem_euclid(12) as usize]
}

/// Height of a diatonic step on a staff whose top line is at `staff_top`.
pub fn y_of_step(staff_top: f32, step: i32) -> f32 {
    staff_top + STAFF_GAP * 4.0 - (step - BOTTOM_LINE) as f32 * STAFF_GAP / 2.0
}

pub fn paint_score(
    pen: Pen,
    roll: &Roll,
    shown: &Shown,
    theme: &Theme,
    window: &mut Window,
    cx: &mut App,
) {
    let (w, h) = (pen.width(), pen.height());
    let top = roll.grid_top();
    let staff_top = top + ((h - top) / 2.0).round() - STAFF_GAP * 2.0;
    pen.clip(window, |window| {
        pen.fill(window, 0.0, 0.0, w, h, theme.editor);
        for i in 0..5 {
            pen.hline(
                window,
                0.0,
                staff_top + i as f32 * STAFF_GAP,
                w,
                theme.staff,
            );
        }
        let bars = (w / roll.px_per_bar).ceil() as i32 + 1;
        for b in 0..bars {
            let x = b as f32 * roll.px_per_bar;
            pen.vline(
                window,
                x,
                staff_top.round(),
                STAFF_GAP * 4.0 + 1.0,
                theme.staff,
            );
        }
        let (n, d) = shown.signature;
        pen.text(
            window,
            cx,
            &format!("{n}/{d} · {} · treble", shown.key),
            6.0,
            top + 6.0,
            11.0,
            theme.text_3,
            Face::UiBold,
        );

        for note in shown.notes() {
            let pitch = note.pitch as i32;
            let step = diatonic_step(pitch);
            let x = roll.x_of_beat(note.start).round() + NOTE_HEAD_R + 2.0;
            let y = y_of_step(staff_top, step);
            if y < top + 4.0 || y > h - 4.0 {
                continue;
            }
            // Ledger lines outside the staff.
            let ledger = |window: &mut Window, s: i32| {
                pen.hline(window, x - 7.0, y_of_step(staff_top, s), 14.0, theme.staff);
            };
            if step < BOTTOM_LINE {
                let mut s = BOTTOM_LINE - 2;
                while s >= step {
                    ledger(window, s);
                    s -= 2;
                }
            }
            if step > BOTTOM_LINE + 8 {
                let mut s = BOTTOM_LINE + 10;
                while s <= step {
                    ledger(window, s);
                    s += 2;
                }
            }
            let ink = if note.agent { theme.accent } else { theme.text };
            pen.ellipse(window, x, y, NOTE_HEAD_R + 1.0, NOTE_HEAD_R, -0.35, ink);
            // Stem: up for low notes, down for high ones.
            let up = step < BOTTOM_LINE + 4;
            let (sx, sy) = if up {
                (x + NOTE_HEAD_R, y - STAFF_GAP * 3.0)
            } else {
                (x - NOTE_HEAD_R - 1.0, y)
            };
            pen.fill(window, sx, sy, 1.0, STAFF_GAP * 3.0, ink);
            if is_black_key(pitch) {
                pen.text(window, cx, "#", x - 14.0, y - 7.0, 10.0, ink, Face::Mono);
            }
            // A tail for notes longer than a beat.
            if note.length > 1.0 {
                let tail = ((note.length - 1.0) as f32 * roll.px_per_beat).round() - 4.0;
                pen.hline(window, x + NOTE_HEAD_R + 2.0, y, tail, theme.staff);
            }
        }

        paint_ruler(pen, roll, shown.start_bar(), theme, window, cx);
        paint_playhead(pen, roll, shown, theme, window);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pitches_sit_on_the_treble_staff() {
        // E4 on the bottom line, F5 on the top one, C4 one ledger below.
        assert_eq!(diatonic_step(64), BOTTOM_LINE);
        assert_eq!(y_of_step(100.0, diatonic_step(64)), 100.0 + STAFF_GAP * 4.0);
        assert_eq!(y_of_step(100.0, diatonic_step(77)), 100.0);
        assert_eq!(diatonic_step(60), BOTTOM_LINE - 2);
        // A sharp shares its natural's line.
        assert_eq!(diatonic_step(61), diatonic_step(60));
    }
}
