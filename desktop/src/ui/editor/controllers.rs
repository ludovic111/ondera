//! The controller lane under the piano roll: one controller at a time (a CC, pitch bend or
//! channel pressure, on a channel), its values as held steps with a stem per point. Click
//! adds a point, dragging a point moves it, dragging elsewhere draws a curve, Option-click
//! or right click deletes. Drags are previewed here and land as one `controller.*` command
//! on release, so each is one undo step.

use super::{
    geometry::Roll,
    lane::{
        self, hit_point, lane_points, lanes_in_clip, point_time, stroke_points, value_at_y,
        y_of_value, Lane, Sample,
    },
    paint::{Face, Pen},
    piano_roll::paint_playhead,
    CanvasEvents, Editor, Shown,
};
use crate::ui::{
    theme::{
        editor::{CONTROLLER_POINT, KEY_COLUMN},
        size, Theme,
    },
    widgets::{field, Button, MenuItem},
};
use gpui::{
    canvas, div, prelude::*, px, AnyElement, App, Context, CursorStyle, HitboxBehavior, Point,
    Window,
};
use serde_json::{json, Value};

/// A drag in the lane, previewed until release.
#[derive(Clone, Debug)]
pub enum LaneDrag {
    Point {
        id: String,
        pressed: Point<f32>,
        moved: bool,
        time: f64,
        value: i16,
        original: (f64, i16),
    },
    Stroke {
        pressed: Point<f32>,
        moved: bool,
        fine: bool,
        samples: Vec<Sample>,
    },
}

/// What the lane shows over its points while the pointer works on it.
#[derive(Clone, Debug, Default)]
pub struct LaneOverlay {
    /// A point being dragged: its id and where it would land.
    pub drag: Option<(String, f64, i16)>,
    /// A stroke being drawn, as it would be committed.
    pub stroke: Option<Vec<Sample>>,
    pub hover: Option<String>,
}

const DRAG_THRESHOLD: f32 = 3.0;

fn travel(a: Point<f32>, b: Point<f32>) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

impl Editor {
    /// Opening a clip shows a lane it uses when the current one is empty there.
    pub(super) fn follow_clip(&mut self, shown: &Shown) {
        let id = shown.clip.as_ref().map(|c| c.id.clone());
        if id == self.lane_clip {
            return;
        }
        self.lane_clip = id;
        self.lane_drag = None;
        self.lane_hover = None;
        let present = lanes_in_clip(shown.clip.as_ref());
        if !present.is_empty() && !present.iter().any(|l| l.same(&self.lane)) {
            self.lane = present[0];
        }
    }

    /// The step a stroke lands on: the grid, or a quarter of it when drawn finely.
    fn stroke_step(&self, fine: bool, cx: &App) -> f64 {
        let grid = super::geometry::step_beats(self.division(cx));
        if fine {
            grid / 4.0
        } else {
            grid
        }
    }

    fn lane_down(
        &mut self,
        at: Point<f32>,
        height: f32,
        roll: Roll,
        alt: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(clip) = self.midi_clip(cx) else {
            return;
        };
        let length = clip.length_bars * roll.bpb;
        if length <= 0.0 {
            return;
        }
        let points = lane_points(Some(&clip), &self.lane);
        let hit = hit_point(&points, &self.lane, roll.px_per_beat, height, at.x, at.y).cloned();
        if let (Some(hit), true) = (&hit, alt) {
            self.run(
                "controller.remove",
                json!({ "clipId": clip.id, "controllerId": hit.id }),
                cx,
            );
            return;
        }
        self.lane_drag = Some(match hit {
            Some(p) => LaneDrag::Point {
                id: p.id,
                pressed: at,
                moved: false,
                time: p.time,
                value: p.value,
                original: (p.time, p.value),
            },
            None => LaneDrag::Stroke {
                pressed: at,
                moved: false,
                fine: alt,
                samples: vec![Sample {
                    beat: roll.beat_at_x(at.x).max(0.0),
                    value: value_at_y(&self.lane, at.y, height),
                }],
            },
        });
        self.gesture(true, cx);
        cx.notify();
    }

    fn lane_move(
        &mut self,
        at: Point<f32>,
        height: f32,
        roll: Roll,
        alt: bool,
        hovered: bool,
        cx: &mut Context<Self>,
    ) {
        let clip = self.midi_clip(cx);
        let division = self.division(cx);
        let Some(drag) = self.lane_drag.as_mut() else {
            // Hover: the point under the pointer lights and shows its value.
            let hover = if hovered {
                let points = lane_points(clip.as_ref(), &self.lane);
                hit_point(&points, &self.lane, roll.px_per_beat, height, at.x, at.y)
                    .map(|p| p.id.clone())
            } else {
                None
            };
            let cursor = if hover.is_some() {
                CursorStyle::OpenHand
            } else {
                CursorStyle::Crosshair
            };
            if hover != self.lane_hover || cursor != self.lane_cursor {
                self.lane_hover = hover;
                self.lane_cursor = cursor;
                cx.notify();
            }
            return;
        };
        let Some(clip) = clip else {
            return;
        };
        let length = clip.length_bars * roll.bpb;
        match drag {
            LaneDrag::Point {
                pressed,
                moved,
                time,
                value,
                ..
            } => {
                if !*moved && travel(at, *pressed) < DRAG_THRESHOLD {
                    return;
                }
                *moved = true;
                *time = point_time(roll.beat_at_x(at.x), length, division, alt);
                *value = value_at_y(&self.lane, at.y, height);
            }
            LaneDrag::Stroke {
                pressed,
                moved,
                samples,
                ..
            } => {
                if !*moved && travel(at, *pressed) < DRAG_THRESHOLD {
                    return;
                }
                *moved = true;
                samples.push(Sample {
                    beat: roll.beat_at_x(at.x).clamp(0.0, length),
                    value: value_at_y(&self.lane, at.y, height),
                });
            }
        }
        cx.notify();
    }

    fn lane_up(&mut self, roll: Roll, cx: &mut Context<Self>) {
        let Some(drag) = self.lane_drag.take() else {
            return;
        };
        cx.notify();
        if let Some(clip) = self.midi_clip(cx) {
            let length = clip.length_bars * roll.bpb;
            match drag {
                LaneDrag::Point {
                    id,
                    moved,
                    time,
                    value,
                    original,
                    ..
                } => {
                    if moved && (time, value) != original {
                        self.run(
                            "controller.update",
                            json!({ "clipId": clip.id, "controllerId": id, "time": time, "value": value }),
                            cx,
                        );
                    }
                }
                LaneDrag::Stroke {
                    moved,
                    fine,
                    samples,
                    ..
                } => {
                    let mut params = self.lane.params();
                    params.insert("clipId".into(), json!(clip.id));
                    if !moved {
                        let first = samples[0];
                        let division = self.division(cx);
                        params.insert(
                            "time".into(),
                            json!(point_time(first.beat, length, division, fine)),
                        );
                        params.insert("value".into(), json!(first.value));
                        self.run("controller.add", Value::Object(params), cx);
                    } else if let Some(stroke) =
                        stroke_points(&samples, self.stroke_step(fine, cx), length)
                    {
                        let points: Vec<Value> = stroke
                            .points
                            .iter()
                            .map(|p| json!({ "time": p.beat, "value": p.value }))
                            .collect();
                        params.insert("points".into(), json!(points));
                        params.insert("from".into(), json!(stroke.from));
                        params.insert("to".into(), json!(stroke.to));
                        self.run("controller.setPoints", Value::Object(params), cx);
                    }
                }
            }
        }
        self.gesture(false, cx);
    }

    /// Right click on a point deletes it.
    fn lane_context(&mut self, at: Point<f32>, height: f32, roll: Roll, cx: &mut Context<Self>) {
        let Some(clip) = self.midi_clip(cx) else {
            return;
        };
        let points = lane_points(Some(&clip), &self.lane);
        if let Some(hit) = hit_point(&points, &self.lane, roll.px_per_beat, height, at.x, at.y) {
            let id = hit.id.clone();
            self.run(
                "controller.remove",
                json!({ "clipId": clip.id, "controllerId": id }),
                cx,
            );
        }
    }

    fn lane_overlay(&self, cx: &App) -> LaneOverlay {
        let mut overlay = LaneOverlay {
            hover: self.lane_hover.clone(),
            ..Default::default()
        };
        match &self.lane_drag {
            Some(LaneDrag::Point {
                id,
                moved: true,
                time,
                value,
                ..
            }) => overlay.drag = Some((id.clone(), *time, *value)),
            Some(LaneDrag::Stroke {
                moved: true,
                fine,
                samples,
                ..
            }) => {
                let length = self.midi_clip(cx).map_or(0.0, |c| {
                    c.length_bars * self.daw.read(cx).app.store.session().beats_per_bar()
                });
                overlay.stroke =
                    stroke_points(samples, self.stroke_step(*fine, cx), length).map(|s| s.points);
            }
            _ => {}
        }
        overlay
    }

    /// The lane picker: the common lanes, then any other the clip uses (marked "·"), and a
    /// field for any controller number.
    fn open_lane_menu(
        &mut self,
        position: gpui::Point<gpui::Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let clip = self.midi_clip(cx);
        let used = lanes_in_clip(clip.as_ref());
        let mut choices: Vec<Lane> = lane::LANE_CHOICES.to_vec();
        for l in &used {
            if !choices.iter().any(|c| c.same(l)) {
                choices.push(*l);
            }
        }
        let this = cx.entity().downgrade();
        let mut items: Vec<MenuItem> = choices
            .into_iter()
            .map(|choice| {
                let this = this.clone();
                let mark = if used.iter().any(|l| l.same(&choice)) {
                    " ·"
                } else {
                    ""
                };
                MenuItem::new(format!("{}{mark}", choice.title()), move |_, cx| {
                    let _ = this.update(cx, |editor, cx| {
                        editor.lane = choice;
                        editor.lane_hover = None;
                        cx.notify();
                    });
                })
                .checked(choice.same(&self.lane))
            })
            .collect();
        items.push(MenuItem::Separator);
        let this = cx.entity().downgrade();
        items.push(MenuItem::new("Other CC…", move |window, cx| {
            let this = this.clone();
            // After the menu has handed focus back, so the field keeps it.
            window.defer(cx, move |window, cx| {
                let _ = this.update(cx, |editor, cx| {
                    editor.typing_cc = true;
                    editor.cc_input.update(cx, |input, cx| {
                        input.set_text("CC ", cx);
                        input.focus(window);
                    });
                    cx.notify();
                });
            });
        }));
        self.menu.open(items, position, window, cx);
    }

    /// The lane row: its head (the picker) over the key column, and the lane itself.
    pub(super) fn lane_row(
        &mut self,
        shown: &Shown,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::get(cx).clone();
        let lane = self.lane;
        let head = if self.typing_cc {
            let focused = self.cc_input.read(cx).is_focused(window);
            field(&self.cc_input, focused, cx)
                .w(px(KEY_COLUMN - 8.0))
                .text_size(px(size::XS))
                .into_any_element()
        } else {
            Button::new("lane-pick", lane.short_title())
                .compact()
                .tooltip(format!(
                    "{}: choose the controller this lane shows",
                    lane.title()
                ))
                .on_click(cx.listener(|this, e: &gpui::ClickEvent, window, cx| {
                    let at = e.position();
                    this.open_lane_menu(gpui::point(at.x - px(10.0), at.y + px(12.0)), window, cx);
                }))
                .into_any_element()
        };
        let this = cx.entity();
        let overlay = self.lane_overlay(cx);
        let cursor = if self.lane_drag.is_some() {
            CursorStyle::ClosedHand
        } else {
            self.lane_cursor
        };
        let shown = shown.clone();
        let body = canvas(
            |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
            move |bounds, hitbox, window, cx| {
                let roll = shown.roll(bounds.size);
                let height = f32::from(bounds.size.height);
                paint_lane(
                    Pen::new(bounds),
                    &roll,
                    &shown,
                    &lane,
                    &overlay,
                    &theme,
                    window,
                    cx,
                );
                window.set_cursor_style(
                    if matches!(cursor, CursorStyle::ClosedHand) && overlay.stroke.is_some() {
                        CursorStyle::Crosshair
                    } else {
                        cursor
                    },
                    &hitbox,
                );
                let (a, b, c, d) = (this.clone(), this.clone(), this.clone(), this.clone());
                CanvasEvents {
                    down: Box::new(move |at, ev, _, cx| {
                        a.update(cx, |ed, cx| {
                            ed.lane_down(at, height, roll, ev.modifiers.alt, cx)
                        })
                    }),
                    moved: Box::new(move |at, ev, hovered, _, cx| {
                        b.update(cx, |ed, cx| {
                            ed.lane_move(at, height, roll, ev.modifiers.alt, hovered, cx)
                        })
                    }),
                    up: Box::new(move |_, _, _, cx| c.update(cx, |ed, cx| ed.lane_up(roll, cx))),
                    right: Some(Box::new(move |at, _, _, cx| {
                        d.update(cx, |ed, cx| ed.lane_context(at, height, roll, cx))
                    })),
                    wheel: None,
                }
                .register(bounds, &hitbox, window);
            },
        )
        .size_full();
        let theme = Theme::get(cx);
        div()
            .size_full()
            .flex()
            .child(
                div()
                    .w(px(KEY_COLUMN))
                    .flex_none()
                    .h_full()
                    .flex()
                    .justify_center()
                    .pt(px(6.0))
                    .bg(theme.bg_raised)
                    .border_r_1()
                    .border_color(theme.line)
                    .child(head),
            )
            .child(div().flex_1().min_w_0().h_full().child(body))
            .into_any_element()
    }
}

/// Paint one lane with the geometry of the piano roll above it: grid, the base line (the
/// centre of a bend lane, the bottom of the others), held values as filled steps, a stem
/// and a head per point, a stroke in progress, the lane's name and the value under the
/// pointer, and the playhead.
#[allow(clippy::too_many_arguments)]
pub fn paint_lane(
    pen: Pen,
    roll: &Roll,
    shown: &Shown,
    lane: &Lane,
    overlay: &LaneOverlay,
    theme: &Theme,
    window: &mut Window,
    cx: &mut App,
) {
    let (w, h) = (pen.width(), pen.height());
    pen.clip(window, |window| {
        pen.fill(window, 0.0, 0.0, w, h, theme.editor);
        let bars = (w / roll.px_per_bar).ceil() as i32 + 1;
        for b in 0..bars {
            let x = b as f32 * roll.px_per_bar;
            pen.vline(window, x, 0.0, h, theme.bar_line);
            for k in 1..(roll.bpb.ceil() as i32) {
                pen.vline(
                    window,
                    x + k as f32 * roll.px_per_beat,
                    0.0,
                    h,
                    theme.beat_line,
                );
            }
        }
        let length = if shown.clip.is_some() {
            roll.length_beats
        } else {
            0.0
        };
        let end = roll.x_of_beat(length).round();
        if shown.clip.is_some() && end < w {
            pen.fill(window, end, 0.0, w - end, h, theme.black_key_row);
        }
        let base = y_of_value(lane, 0, h).round();
        pen.hline(window, 0.0, base, w, theme.bar_line);

        let (top, bottom) = theme.note_shades(shown.track_color);
        let mut points = lane_points(shown.clip.as_ref(), lane);
        if let Some((id, time, value)) = &overlay.drag {
            for p in points.iter_mut().filter(|p| &p.id == id) {
                p.time = *time;
                p.value = *value;
            }
            points.sort_by(|a, b| a.time.total_cmp(&b.time));
        }
        if let Some(stroke) = overlay.stroke.as_ref().filter(|s| !s.is_empty()) {
            let (from, to) = (stroke[0].beat, stroke[stroke.len() - 1].beat);
            points.retain(|p| p.time < from || p.time > to);
        }

        // Held values: a filled step from each point to the next, or to the clip end.
        for (i, p) in points.iter().enumerate() {
            let next = points.get(i + 1).map_or(length, |n| n.time);
            let x0 = roll.x_of_beat(p.time).round();
            let x1 = roll.x_of_beat(next.min(length)).round();
            let y = y_of_value(lane, p.value, h).round();
            pen.fill(
                window,
                x0,
                y.min(base),
                x1 - x0,
                (base - y).abs(),
                theme.velocity,
            );
            pen.fill(window, x0, y, (x1 - x0).max(1.0), 1.0, top);
        }
        // Stems and heads.
        for p in &points {
            let x = roll.x_of_beat(p.time).round();
            let y = y_of_value(lane, p.value, h).round();
            let active = overlay.drag.as_ref().is_some_and(|d| d.0 == p.id)
                || overlay.hover.as_deref() == Some(p.id.as_str());
            let head = if active {
                theme.accent
            } else if p.agent {
                theme.accent_hover
            } else {
                top
            };
            pen.fill(
                window,
                x,
                y.min(base),
                1.0,
                (base - y).abs() + 1.0,
                if active { theme.accent } else { bottom },
            );
            pen.circle(window, x + 0.5, y + 0.5, CONTROLLER_POINT, head);
        }
        // A stroke in progress.
        if let Some(stroke) = &overlay.stroke {
            for s in stroke {
                let x = roll.x_of_beat(s.beat).round();
                let y = y_of_value(lane, s.value, h).round();
                pen.fill(
                    window,
                    x,
                    y.min(base),
                    1.0,
                    (base - y).abs() + 1.0,
                    theme.pencil_preview,
                );
                pen.fill(window, x - 1.0, y - 1.0, 3.0, 3.0, theme.accent);
            }
        }

        // The lane's name and the value under the pointer.
        let shown_value = overlay.drag.as_ref().map(|d| d.2).or_else(|| {
            overlay
                .hover
                .as_ref()
                .and_then(|id| points.iter().find(|p| &p.id == id))
                .map(|p| p.value)
        });
        let label = match shown_value {
            Some(v) => format!("{} · {}", lane.title(), lane::value_label(lane, v)),
            None => lane.title(),
        };
        pen.text(window, cx, &label, 5.0, 4.0, 10.0, theme.text_3, Face::Mono);

        paint_playhead(pen, roll, shown, theme, window);
    });
}
