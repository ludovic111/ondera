//! Painting the piano roll and the step view: the keyboard column, the bar ruler, key rows,
//! the bar and beat grid over the clip's range, notes with their velocity, the selection,
//! what the agent wrote in the accent, drag previews and the playhead.

use super::{
    geometry::{is_black_key, note_label, step_beats, Roll},
    paint::{drop, Face, Pen},
    Mode, Shown,
};
use crate::ui::theme::{editor::KEY_ROW, radius, Theme};
use gpui::{linear_color_stop, linear_gradient, px, App, Corners, Window};

/// A note as a drag would leave it: start and length in beats, and its pitch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ghost {
    pub start: f64,
    pub length: f64,
    pub pitch: i32,
}

/// What a drag in progress shows over the notes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Overlay {
    /// Where a note being moved or resized would land.
    pub ghost: Option<Ghost>,
    /// A note being drawn.
    pub pencil: Option<Ghost>,
}

/// Bar numbers as the ruler writes them: whole bars plainly, a bar that starts off the
/// grid with one decimal.
pub fn bar_number(bar: f64) -> String {
    if (bar - bar.round()).abs() < 1e-6 {
        format!("{}", bar.round() as i64)
    } else {
        format!("{bar:.1}")
    }
}

/// The ruler over the roll and the lane: bar ticks and numbers from the clip's first bar.
pub fn paint_ruler(
    pen: Pen,
    roll: &Roll,
    start_bar: f64,
    theme: &Theme,
    window: &mut Window,
    cx: &mut App,
) {
    let (w, top) = (pen.width(), roll.grid_top());
    pen.fill(window, 0.0, 0.0, w, top, theme.ruler);
    pen.hline(window, 0.0, top - 1.0, w, theme.line);
    let bars = (w / roll.px_per_bar).ceil() as i32 + 1;
    for b in 0..bars {
        let x = (b as f32 * roll.px_per_bar).round();
        pen.vline(window, x, 0.0, top, theme.ruler_tick);
        pen.text(
            window,
            cx,
            &bar_number(start_bar + b as f64 + 1.0),
            x + 5.0,
            3.0,
            10.0,
            theme.text_2,
            Face::Mono,
        );
    }
}

/// The playhead, relative to the clip's start: a line of the accent with a soft glow.
pub fn paint_playhead(pen: Pen, roll: &Roll, shown: &Shown, theme: &Theme, window: &mut Window) {
    let Some(clip) = &shown.clip else {
        return;
    };
    let bars = shown.position / shown.bpb.max(1e-9) - clip.start_bar;
    let x = (bars as f32 * roll.px_per_bar).round();
    if x < 0.0 || x > pen.width() {
        return;
    }
    pen.shadow(
        window,
        x,
        0.0,
        1.0,
        pen.height(),
        0.0,
        drop(theme.accent_glow, 0.0, 6.0),
    );
    pen.fill(window, x, 0.0, 1.0, pen.height(), theme.accent);
}

/// The keyboard column: white and black keys on the rows of the roll, C keys named, the
/// key being auditioned lit.
pub fn paint_keys(
    pen: Pen,
    roll: &Roll,
    pressed: Option<i32>,
    theme: &Theme,
    window: &mut Window,
    cx: &mut App,
) {
    let (w, h) = (pen.width(), pen.height());
    pen.clip(window, |window| {
        pen.fill(window, 0.0, 0.0, w, h, theme.key_white);
        for pitch in roll.low..=roll.high() {
            let y = roll.y_of_pitch(pitch);
            let lit = pressed == Some(pitch);
            if is_black_key(pitch) {
                let bw = (w * 0.6).round();
                pen.round(
                    window,
                    0.0,
                    y + 1.0,
                    bw,
                    KEY_ROW - 2.0,
                    Corners {
                        top_left: px(0.0),
                        bottom_left: px(0.0),
                        top_right: px(2.0),
                        bottom_right: px(2.0),
                    },
                    if lit { theme.accent } else { theme.key_black },
                );
            } else {
                if lit {
                    pen.fill(window, 0.0, y, w, KEY_ROW, theme.accent_soft);
                }
                // The seam under a white key; it runs the full width between E-F and B-C,
                // where no black key sits.
                let natural_pair = matches!(pitch.rem_euclid(12), 0 | 5);
                let from = if natural_pair { 0.0 } else { (w * 0.6).round() };
                pen.hline(window, from, y + KEY_ROW - 1.0, w - from, theme.line);
                if pitch.rem_euclid(12) == 0 {
                    pen.text_right(
                        window,
                        cx,
                        &note_label(pitch),
                        w - 5.0,
                        y + 1.0,
                        9.0,
                        if lit {
                            theme.accent_text
                        } else {
                            theme.key_label
                        },
                        Face::Mono,
                    );
                }
            }
        }
        // The ruler's corner over the keys.
        pen.fill(window, 0.0, 0.0, w, roll.grid_top(), theme.ruler);
        pen.hline(window, 0.0, roll.grid_top() - 1.0, w, theme.line);
        pen.vline(window, w - 1.0, 0.0, h, theme.line);
    });
}

/// The piano roll or the step view, whichever `shown.mode` asks for.
pub fn paint_roll(
    pen: Pen,
    roll: &Roll,
    shown: &Shown,
    overlay: &Overlay,
    theme: &Theme,
    window: &mut Window,
    cx: &mut App,
) {
    let (w, h) = (pen.width(), pen.height());
    let top = roll.grid_top();
    let step_mode = shown.mode == Mode::Step;
    pen.clip(window, |window| {
        pen.fill(window, 0.0, 0.0, w, h, theme.editor);

        // Row shading: every other pitch lifted, black-key rows darkened.
        for pitch in roll.low..=roll.high() {
            let y = roll.y_of_pitch(pitch);
            if pitch.rem_euclid(2) == 0 {
                pen.fill(window, 0.0, y, w, KEY_ROW, theme.row_shade);
            }
            if is_black_key(pitch) {
                pen.fill(window, 0.0, y, w, KEY_ROW, theme.black_key_row);
            }
            if pitch.rem_euclid(12) == 0 {
                // The octave seam under each C.
                pen.hline(window, 0.0, y + KEY_ROW - 1.0, w, theme.bar_line);
            }
        }

        // Bar and beat lines; the step view adds a line per grid step.
        let bars = (w / roll.px_per_bar).ceil() as i32 + 1;
        let step = step_beats(shown.division);
        for b in 0..bars {
            let x = b as f32 * roll.px_per_bar;
            pen.vline(window, x, top, h - top, theme.bar_line);
            for k in 1..(roll.bpb.ceil() as i32) {
                pen.vline(
                    window,
                    x + k as f32 * roll.px_per_beat,
                    top,
                    h - top,
                    theme.beat_line,
                );
            }
            if step_mode && step as f32 * roll.px_per_beat >= 6.0 {
                let mut k = step;
                while k < roll.bpb - 1e-9 {
                    if (k - k.round()).abs() > 1e-9 {
                        pen.vline(
                            window,
                            x + k as f32 * roll.px_per_beat,
                            top,
                            h - top,
                            theme.step_cell,
                        );
                    }
                    k += step;
                }
            }
        }

        // Past the clip's end.
        if shown.clip.is_some() {
            let end = roll.x_of_beat(roll.length_beats).round();
            if end < w {
                pen.fill(window, end, top, w - end, h - top, theme.black_key_row);
            }
        }

        // Notes.
        let (top_shade, bottom_shade) = theme.note_shades(shown.track_color);
        for n in shown.notes() {
            let pitch = n.pitch as i32;
            if pitch < roll.low || pitch > roll.high() {
                continue;
            }
            let length = if step_mode {
                n.length.max(step)
            } else {
                n.length
            };
            let x = roll.x_of_beat(n.start).round() + 1.0;
            let nw = ((length as f32 * roll.px_per_beat).round() - 2.0).max(3.0);
            let y = roll.y_of_pitch(pitch) + 0.5;
            let nh = KEY_ROW - 1.0;
            let selected = shown.selected_note.as_deref() == Some(n.id.as_str());
            let (hi, lo) = if n.agent {
                (theme.accent_hover, theme.accent)
            } else {
                (top_shade, bottom_shade)
            };
            paint_note(
                pen, window, theme, x, y, nw, nh, hi, lo, n.velocity, selected,
            );
        }

        // Drag previews.
        let rect = |g: &Ghost| {
            let x = roll.x_of_beat(g.start).round() + 1.0;
            let nw = ((g.length as f32 * roll.px_per_beat).round() - 2.0).max(3.0);
            (x, roll.y_of_pitch(g.pitch) + 0.5, nw, KEY_ROW - 1.0)
        };
        if let Some(g) = &overlay.pencil {
            let (x, y, nw, nh) = rect(g);
            pen.round(window, x, y, nw, nh, px(radius::XS), theme.pencil_preview);
            pen.outline(window, x, y, nw, nh, radius::XS, 1.0, theme.accent);
        }
        if let Some(g) = &overlay.ghost {
            let (x, y, nw, nh) = rect(g);
            pen.round(window, x, y, nw, nh, px(radius::XS), theme.drag_ghost);
            pen.outline(window, x, y, nw, nh, radius::XS, 1.0, theme.drag_ghost_edge);
        }

        paint_ruler(pen, roll, shown.start_bar(), theme, window, cx);
        paint_playhead(pen, roll, shown, theme, window);
    });
}

/// One note: a soft drop, a face shaded from the track colour, a top light, its velocity
/// as a lighter share of the face from the left, and an accent ring when selected.
#[allow(clippy::too_many_arguments)]
fn paint_note(
    pen: Pen,
    window: &mut Window,
    theme: &Theme,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    hi: gpui::Hsla,
    lo: gpui::Hsla,
    velocity: u8,
    selected: bool,
) {
    let r = radius::XS - 1.0;
    pen.shadow(window, x, y, w, h, r, drop(theme.note_shadow, 1.0, 2.0));
    if selected {
        pen.shadow(window, x, y, w, h, r, drop(theme.accent_glow, 0.0, 6.0));
    }
    pen.round(
        window,
        x,
        y,
        w,
        h,
        px(r),
        linear_gradient(
            180.0,
            linear_color_stop(hi, 0.0),
            linear_color_stop(lo, 1.0),
        ),
    );
    let share = (velocity as f32 / 127.0 * w).round();
    if share > 0.0 {
        let full = share >= w - 0.5;
        let right = if full { px(r) } else { px(0.0) };
        pen.round(
            window,
            x,
            y,
            share.min(w),
            h,
            Corners {
                top_left: px(r),
                bottom_left: px(r),
                top_right: right,
                bottom_right: right,
            },
            theme.velocity,
        );
    }
    if w > r * 2.0 {
        pen.hline(window, x + r, y + 0.5, w - r * 2.0, theme.note_highlight);
        pen.hline(window, x + r, y + h - 1.0, w - r * 2.0, theme.note_edge);
    }
    if selected {
        pen.outline(
            window,
            x - 1.5,
            y - 1.5,
            w + 3.0,
            h + 3.0,
            r + 1.5,
            1.5,
            theme.accent,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bars_are_numbered_from_one() {
        assert_eq!(bar_number(1.0), "1");
        assert_eq!(bar_number(12.0), "12");
        assert_eq!(bar_number(3.5), "3.5");
    }
}
