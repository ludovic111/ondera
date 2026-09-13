//! Region editor: piano roll, step view and pitch overview.

use crate::{
    app::{id, Ondera},
    theme::*,
};
use eframe::egui::{self, pos2, vec2, Align2, Pos2, Rect, Sense, Stroke, Vec2};
use ondera_engine::{model::*, store::Command};

pub struct NoteDrag {
    clip: Clip,
    note: Note,
    anchor: Pos2,
    resize: bool,
    current: Note,
}
const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];
const MODES: [(&str, &str); 3] = [
    ("pianoRoll", "Piano Roll"),
    ("score", "Score"),
    ("step", "Step"),
];

fn editor_header(p: &egui::Painter, r: Rect) {
    shade_rect(p, r, 0.0, vertical(r, TRANSPORT_TOP, TRANSPORT_BOTTOM));
    hline(p, r.left(), r.right(), r.top(), white(0.06));
    hline(p, r.left(), r.right(), r.bottom() - 1.0, black(0.6));
}

impl Ondera {
    pub fn editor(&mut self, ui: &mut egui::Ui) {
        let s = self.store.snapshot();
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        let (bar, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), EDITOR_HEADER), Sense::hover());
        editor_header(ui.painter(), bar);
        let Some(clip) = s
            .clips
            .iter()
            .find(|c| Some(&c.id) == s.view.editor_clip_id.as_ref())
        else {
            ui.painter().text(
                pos2(bar.left() + 12.0, bar.center().y),
                Align2::LEFT_CENTER,
                "Select a MIDI region to draw notes, or an audio region to inspect its waveform.",
                font(FS_SECONDARY, Weight::Medium),
                FAINT,
            );
            let (rest, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
            ui.painter().rect_filled(rest, 0.0, EDITOR);
            return;
        };
        let index = s
            .tracks
            .iter()
            .position(|t| t.id == clip.track_id)
            .unwrap_or(0);
        let color = track_color(&s.tracks[index].color, index);
        let velocity = match &clip.data {
            ClipData::Midi { notes } => notes
                .iter()
                .find(|n| Some(&n.id) == s.view.selected_note_id.as_ref())
                .map_or(100, |n| n.velocity),
            _ => 100,
        };
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(bar.shrink2(vec2(10.0, 0.0))),
            |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    let selected = MODES
                        .iter()
                        .position(|(m, _)| *m == s.view.editor_mode)
                        .unwrap_or(0);
                    let labels: Vec<&str> = MODES.iter().map(|(_, l)| *l).collect();
                    if let Some(i) = segmented(ui, &labels, selected, 0.0) {
                        let mut v = s.view.clone();
                        v.editor_mode = MODES[i].0.into();
                        self.dispatch(Command::SetView(v));
                    }
                    let (sw, _) = ui.allocate_exact_size(Vec2::splat(9.0), Sense::hover());
                    swatch(ui.painter(), sw, color);
                    ui.add_space(-5.0);
                    ui.label(text(&clip.name, FS_LIST, Weight::SemiBold, INK));
                    ui.add_space(-6.0);
                    ui.label(text(
                        format!(
                            "· bars {} – {}",
                            clip.start_bar as u64 + 1,
                            (clip.start_bar + clip.length_bars).ceil() as u64
                        ),
                        FS_LIST,
                        Weight::Medium,
                        FAINT,
                    ));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        for (label, value) in [
                            (
                                "Scale",
                                if s.transport.key.is_empty() {
                                    "—".to_string()
                                } else {
                                    s.transport.key.clone()
                                },
                            ),
                            ("Velocity", velocity.to_string()),
                            ("Quantize", format!("1/{}", s.transport.snap_division)),
                        ] {
                            ui.label(text(value, FS_SECONDARY, Weight::Medium, DIM));
                            ui.add_space(-3.0);
                            ui.label(text(label, FS_SECONDARY, Weight::Medium, FAINT));
                            ui.add_space(6.0);
                        }
                    });
                });
            },
        );
        let ClipData::Midi { notes } = &clip.data else {
            if let ClipData::Audio {
                source_id,
                offset_seconds,
            } = &clip.data
            {
                let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
                let painter = ui.painter_at(rect);
                painter.rect_filled(rect, 0.0, TIMELINE_EMPTY);
                if let Some(buffer) = self.library.get(source_id) {
                    painter.text(
                        pos2(rect.left() + 12.0, rect.top() + 12.0),
                        Align2::LEFT_CENTER,
                        format!(
                            "{} Hz · stereo · {:.2} s · offset {:.3} s",
                            buffer.sample_rate,
                            buffer.duration(),
                            offset_seconds
                        ),
                        mono_font(FS_VALUE),
                        FAINT,
                    );
                    let wave = Rect::from_min_max(pos2(rect.left(), rect.top() + 24.0), rect.max);
                    hline(
                        &painter,
                        wave.left(),
                        wave.right(),
                        wave.center().y,
                        white(0.2),
                    );
                    for i in 0..wave.width() as usize {
                        let sec = *offset_seconds
                            + i as f64 / wave.width() as f64
                                * clip.length_bars
                                * s.beats_per_bar()
                                * 60.0
                                / s.transport.tempo;
                        let index = (sec * buffer.sample_rate as f64
                            / (buffer.sample_rate / 400).max(1) as f64)
                            as usize;
                        let h =
                            buffer.peaks.get(index).copied().unwrap_or(0.0) * wave.height() * 0.45;
                        painter.line_segment(
                            [
                                pos2(wave.left() + i as f32 + 0.5, wave.center().y - h),
                                pos2(wave.left() + i as f32 + 0.5, wave.center().y + h),
                            ],
                            Stroke::new(1.0, color.lerp_to_gamma(egui::Color32::WHITE, 0.2)),
                        );
                    }
                }
            }
            return;
        };
        if s.view.editor_mode == "score" {
            self.score(ui, clip, notes, color);
            return;
        }
        let bpb = s.beats_per_bar();
        let beats = clip.length_bars * bpb;
        let step = 4.0 / s.transport.snap_division as f64;
        let beat_width =
            ((ui.available_width() - KEY_WIDTH) / beats as f32).max(24.0) * self.editor_zoom;
        let row_height = KEY_ROW;
        let rows = ((ui.available_height() - EDITOR_RULER) / row_height)
            .floor()
            .clamp(12.0, 64.0) as u8;
        let high = self.editor_low.saturating_add(rows - 1).min(127);
        let step_mode = s.view.editor_mode == "step";
        ui.horizontal(|ui| {
            // Fixed key column.
            let (keys, _) = ui.allocate_exact_size(
                vec2(KEY_WIDTH, EDITOR_RULER + rows as f32 * row_height),
                Sense::hover(),
            );
            let painter = ui.painter_at(keys);
            painter.rect_filled(keys, 0.0, PANEL);
            hline(
                &painter,
                keys.left(),
                keys.right(),
                keys.top() + EDITOR_RULER - 1.0,
                black(0.5),
            );
            for row in 0..rows {
                let pitch = high.saturating_sub(row);
                let black_key = [1, 3, 6, 8, 10].contains(&(pitch % 12));
                let y = keys.top() + EDITOR_RULER + row as f32 * row_height;
                let key =
                    Rect::from_min_size(pos2(keys.left(), y), vec2(KEY_WIDTH - 1.0, row_height));
                painter.rect_filled(key, 0.0, if black_key { BLACK_KEY } else { WHITE_KEY });
                hline(
                    &painter,
                    key.left(),
                    key.right(),
                    key.bottom() - 1.0,
                    black(0.35),
                );
                if !black_key {
                    painter.text(
                        pos2(key.right() - 5.0, key.center().y),
                        Align2::RIGHT_CENTER,
                        format!(
                            "{}{}",
                            NOTE_NAMES[(pitch % 12) as usize],
                            pitch as i32 / 12 - 1
                        ),
                        mono_font(FS_MICRO),
                        KEY_INK,
                    );
                }
                if ui
                    .interact(key, egui::Id::new(("key", pitch)), Sense::click())
                    .clicked()
                {
                    self.preview(&clip.track_id, pitch, 100);
                }
            }
            vline(
                &painter,
                keys.right() - 1.0,
                keys.top(),
                keys.bottom(),
                black(0.7),
            );
            egui::ScrollArea::horizontal()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let (rect, _) = ui.allocate_exact_size(
                        vec2(
                            (beats as f32 * beat_width).max(ui.available_width()),
                            EDITOR_RULER + rows as f32 * row_height,
                        ),
                        Sense::hover(),
                    );
                    let origin = rect.min + vec2(0.0, EDITOR_RULER);
                    let grid = Rect::from_min_max(origin, rect.max);
                    let painter = ui.painter_at(rect);
                    let ruler = Rect::from_min_size(rect.min, vec2(rect.width(), EDITOR_RULER));
                    painter.rect_filled(ruler, 0.0, RULER_BG);
                    hline(
                        &painter,
                        ruler.left(),
                        ruler.right(),
                        ruler.bottom() - 1.0,
                        black(0.6),
                    );
                    painter.rect_filled(grid, 0.0, TIMELINE_EMPTY);
                    for row in 0..rows {
                        let pitch = high.saturating_sub(row);
                        let y = origin.y + row as f32 * row_height;
                        if [1, 3, 6, 8, 10].contains(&(pitch % 12)) {
                            painter.rect_filled(
                                Rect::from_min_max(
                                    pos2(origin.x, y),
                                    pos2(rect.right(), y + row_height),
                                ),
                                0.0,
                                black(0.14),
                            );
                        }
                        if pitch % 12 == 0 {
                            hline(
                                &painter,
                                origin.x,
                                rect.right(),
                                y + row_height - 1.0,
                                white(0.06),
                            );
                        }
                    }
                    let divisions = (beats / step).ceil().min(100_000.0) as usize;
                    for division in 0..=divisions {
                        let beat = division as f64 * step;
                        let x = origin.x + beat as f32 * beat_width;
                        if !ui.clip_rect().x_range().contains(x) {
                            continue;
                        }
                        let bar_line = (beat / bpb).fract().abs() < 1e-6;
                        let beat_line = beat.fract().abs() < 1e-6;
                        if bar_line {
                            vline(&painter, x, rect.top(), rect.bottom(), white(0.09));
                            vline(&painter, x, ruler.top(), ruler.bottom(), white(0.16));
                            painter.text(
                                pos2(x + 5.0, ruler.top() + 3.0),
                                Align2::LEFT_TOP,
                                format!("{}", clip.start_bar as u64 + (beat / bpb) as u64 + 1),
                                mono_font(FS_SMALL),
                                DIM,
                            );
                        } else if beat_line {
                            vline(&painter, x, origin.y, rect.bottom(), white(0.03));
                        } else if beat_width * step as f32 >= 12.0 {
                            vline(&painter, x, origin.y, rect.bottom(), white(0.015));
                        }
                    }
                    let grid_hit = ui.interact(
                        grid,
                        egui::Id::new(("piano-grid", &clip.id)),
                        Sense::click_and_drag(),
                    );
                    // Wheel scrolls the visible octaves; pinch or ⌘-wheel zooms time.
                    if grid_hit.hovered() {
                        let (scroll, zoom, command) =
                            ui.input(|i| (i.raw_scroll_delta, i.zoom_delta(), i.modifiers.command));
                        let zoom = if command && scroll.y != 0.0 {
                            (1.0 + scroll.y / 200.0).clamp(0.5, 2.0)
                        } else {
                            zoom
                        };
                        if zoom != 1.0 {
                            self.editor_zoom = (self.editor_zoom * zoom).clamp(0.5, 4.0);
                        } else if scroll.y != 0.0 {
                            let rows_moved = (scroll.y / row_height).round() as i32;
                            if rows_moved != 0 {
                                let max_low = 127 - rows as i32 + 1;
                                self.editor_low = (self.editor_low as i32 + rows_moved)
                                    .clamp(0, max_low.max(0))
                                    as u8;
                            }
                        }
                    }
                    let pointer = ui.input(|i| i.pointer.interact_pos());
                    let note_rect = |n: &Note| {
                        Rect::from_min_size(
                            pos2(
                                origin.x + n.start as f32 * beat_width + 1.0,
                                origin.y + (high as f32 - n.pitch as f32) * row_height + 0.5,
                            ),
                            vec2(
                                (n.length as f32 * beat_width - 2.0).max(3.0),
                                row_height - 1.0,
                            ),
                        )
                    };
                    let hit_note =
                        pointer.is_some_and(|pt| notes.iter().any(|n| note_rect(n).contains(pt)));
                    // Pencil preview while dragging out a new note.
                    if grid_hit.dragged() && self.note_drag.is_none() {
                        if let (Some(anchor), Some(pt)) = (self.draw_note_anchor, pointer) {
                            let end = ((pt.x - origin.x) / beat_width) as f64;
                            let a = (anchor.min(end) / step).floor().max(0.0) * step;
                            let b = ((anchor.max(end) / step).ceil() * step).max(a + step);
                            let pitch = (high as f32 - ((pt.y - origin.y) / row_height).floor())
                                .clamp(0.0, 127.0);
                            let preview = Rect::from_min_size(
                                pos2(
                                    origin.x + a as f32 * beat_width + 1.0,
                                    origin.y + (high as f32 - pitch) * row_height + 0.5,
                                ),
                                vec2(
                                    ((b - a) as f32 * beat_width - 2.0).max(3.0),
                                    row_height - 1.0,
                                ),
                            );
                            painter.rect_filled(preview, 2.0, accent(0.25));
                            painter.rect_stroke(
                                preview,
                                2.0,
                                Stroke::new(1.0, accent(0.9)),
                                egui::StrokeKind::Inside,
                            );
                        }
                    }
                    for original in notes {
                        if original.pitch > high || original.pitch < high.saturating_sub(rows - 1) {
                            continue;
                        }
                        let n = self
                            .note_drag
                            .as_ref()
                            .filter(|d| d.note.id == original.id)
                            .map_or(original, |d| &d.current)
                            .clone();
                        let nr = note_rect(&n);
                        if !nr.intersects(ui.clip_rect()) {
                            continue;
                        }
                        let selected = Some(&n.id) == s.view.selected_note_id.as_ref();
                        note_slab(&painter, nr, color, n.velocity, selected, n.agent);
                        let r = ui.interact(
                            nr.intersect(grid),
                            egui::Id::new(("note", &clip.id, &n.id)),
                            Sense::click_and_drag(),
                        );
                        if r.clicked() {
                            if step_mode {
                                let mut c = clip.clone();
                                if let ClipData::Midi { notes } = &mut c.data {
                                    notes.retain(|v| v.id != n.id);
                                }
                                self.dispatch(Command::PutClip(c));
                            } else {
                                self.dispatch(Command::Select {
                                    track: Some(clip.track_id.clone()),
                                    clip: Some(clip.id.clone()),
                                    note: Some(n.id.clone()),
                                });
                                self.preview(&clip.track_id, n.pitch, n.velocity);
                            }
                        }
                        if r.drag_started() {
                            if let Some(pt) = r.interact_pointer_pos() {
                                let anchor = ui.input(|i| i.pointer.press_origin()).unwrap_or(pt);
                                self.note_drag = Some(NoteDrag {
                                    clip: clip.clone(),
                                    note: original.clone(),
                                    current: original.clone(),
                                    anchor,
                                    resize: nr.right() - anchor.x < NOTE_EDGE_GRIP,
                                });
                            }
                        }
                        r.context_menu(|ui| {
                            let mut velocity = n.velocity as f32;
                            ui.horizontal(|ui| {
                                ui.label(text("Velocity", FS_SECONDARY, Weight::Medium, FAINT));
                                if hslider(ui, &mut velocity, 1.0..=127.0, 120.0).changed() {
                                    let mut c = clip.clone();
                                    if let ClipData::Midi { notes } = &mut c.data {
                                        if let Some(note) =
                                            notes.iter_mut().find(|note| note.id == n.id)
                                        {
                                            note.velocity = velocity.round() as u8;
                                        }
                                    }
                                    self.dispatch(Command::PutClip(c));
                                }
                                ui.label(mono(
                                    format!("{}", velocity.round() as u8),
                                    FS_VALUE,
                                    DIM,
                                ));
                            });
                            if ui.button("Delete note").clicked() {
                                let mut c = clip.clone();
                                if let ClipData::Midi { notes } = &mut c.data {
                                    notes.retain(|note| note.id != n.id);
                                }
                                self.dispatch(Command::PutClip(c));
                                ui.close();
                            }
                        });
                    }
                    if grid_hit.drag_started() && self.note_drag.is_none() {
                        self.draw_note_anchor = ui
                            .input(|i| i.pointer.press_origin())
                            .map(|pt| ((pt.x - origin.x) / beat_width) as f64);
                    }
                    if (grid_hit.clicked() || grid_hit.drag_stopped())
                        && !hit_note
                        && self.note_drag.is_none()
                    {
                        if let Some(pt) = pointer {
                            let end = ((pt.x - origin.x) / beat_width) as f64;
                            let delta = if grid_hit.drag_stopped() {
                                end - self.draw_note_anchor.take().unwrap_or(end)
                            } else {
                                0.0
                            };
                            let start = ((end - delta).min(end) / step).floor().max(0.0) * step;
                            let length = if step_mode || delta.abs() < step {
                                step
                            } else {
                                (delta.abs() / step).ceil() * step
                            };
                            let pitch = (high as f32 - ((pt.y - origin.y) / row_height).floor())
                                .clamp(0.0, 127.0) as u8;
                            if start < beats {
                                let mut c = clip.clone();
                                if let ClipData::Midi { notes } = &mut c.data {
                                    notes.push(Note {
                                        id: id("note"),
                                        start,
                                        length: length.min(beats - start),
                                        pitch,
                                        velocity: 100,
                                        agent: false,
                                    });
                                }
                                self.dispatch(Command::PutClip(c));
                                self.preview(&clip.track_id, pitch, 100);
                            }
                        }
                    }
                    if let Some(mut drag) = self.note_drag.take() {
                        if let Some(pt) = pointer {
                            let delta = ((pt.x - drag.anchor.x) / beat_width) as f64;
                            let free = ui.input(|i| i.modifiers.alt);
                            let delta = if free {
                                delta
                            } else {
                                (delta / step).round() * step
                            };
                            if drag.resize {
                                drag.current.length = (drag.note.length + delta)
                                    .max(step)
                                    .min(beats - drag.note.start);
                            } else {
                                drag.current.start = (drag.note.start + delta)
                                    .max(0.0)
                                    .min((beats - drag.note.length).max(0.0));
                                drag.current.pitch = (drag.note.pitch as f32
                                    - ((pt.y - drag.anchor.y) / row_height).round())
                                .clamp(0.0, 127.0)
                                    as u8;
                            }
                        }
                        if ui.input(|i| i.pointer.any_released()) {
                            if let ClipData::Midi { notes } = &mut drag.clip.data {
                                if let Some(n) = notes.iter_mut().find(|n| n.id == drag.note.id) {
                                    *n = drag.current;
                                }
                            }
                            self.dispatch(Command::PutClip(drag.clip));
                        } else {
                            self.note_drag = Some(drag);
                        }
                    }
                    if self.playing {
                        let relative = self.position - clip.start_bar * bpb;
                        let x = origin.x + relative as f32 * beat_width;
                        if (0.0..beats).contains(&relative) {
                            playhead(&painter, x, rect.top(), rect.bottom());
                        }
                    }
                });
        });
    }
    fn score(&self, ui: &mut egui::Ui, clip: &Clip, notes: &[Note], color: egui::Color32) {
        let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, TIMELINE_EMPTY);
        painter.text(
            pos2(rect.left() + 12.0, rect.top() + 12.0),
            Align2::LEFT_CENTER,
            "Pitch overview · edit notes in Piano Roll",
            font(FS_SMALL, Weight::Medium),
            FAINT,
        );
        let gap = 8.0;
        let top = rect.center().y - 2.0 * gap;
        for i in 0..5 {
            let y = top + i as f32 * gap;
            hline(
                &painter,
                rect.left() + 24.0,
                rect.right() - 24.0,
                y,
                white(0.35),
            );
        }
        let beats = clip.length_bars * self.store.session().beats_per_bar();
        for n in notes {
            let x = rect.left() + 32.0 + n.start as f32 / beats as f32 * (rect.width() - 64.0);
            let y = top + 4.0 * gap - (n.pitch as f32 - 64.0) * gap * 0.5;
            painter.circle_filled(pos2(x, y), 3.5, color);
            painter.line_segment(
                [pos2(x + 3.0, y), pos2(x + 3.0, y - 24.0)],
                Stroke::new(1.0, color),
            );
        }
    }
}
