use crate::{
    app::{id, Ondera},
    theme::*,
};
use eframe::egui::{self, pos2, vec2, Align2, FontId, Pos2, Rect, Sense, Stroke};
use ondera_engine::{model::*, store::Command};

pub struct NoteDrag {
    clip: Clip,
    note: Note,
    anchor: Pos2,
    resize: bool,
    current: Note,
}
impl Ondera {
    pub fn editor(&mut self, ui: &mut egui::Ui) {
        let s = self.store.snapshot();
        let Some(clip) = s
            .clips
            .iter()
            .find(|c| Some(&c.id) == s.view.editor_clip_id.as_ref())
        else {
            ui.add_space(18.0);
            ui.heading("Editor");
            ui.label(egui::RichText::new("Select a MIDI region to draw notes, or an audio region to inspect its waveform.").color(DIM));
            return;
        };
        let index = s
            .tracks
            .iter()
            .position(|t| t.id == clip.track_id)
            .unwrap_or(0);
        let color = track_color(&s.tracks[index].color, index);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(&clip.name).strong());
            ui.separator();
            for (mode, name) in [
                ("pianoRoll", "Piano roll"),
                ("step", "Step"),
                ("score", "Score"),
            ] {
                if ui
                    .selectable_label(s.view.editor_mode == mode, name)
                    .clicked()
                {
                    let mut v = s.view.clone();
                    v.editor_mode = mode.into();
                    self.dispatch(Command::SetView(v));
                }
            }
            ui.separator();
            ui.add(
                egui::Slider::new(&mut self.editor_low, 0..=104)
                    .text("Low note")
                    .show_value(true),
            );
            ui.add(
                egui::Slider::new(&mut self.editor_zoom, 0.5..=4.0)
                    .text("Zoom")
                    .show_value(false),
            );
        });
        let ClipData::Midi { notes } = &clip.data else {
            if let ClipData::Audio {
                source_id,
                offset_seconds,
            } = &clip.data
            {
                if let Some(buffer) = self.library.get(source_id) {
                    ui.label(format!(
                        "{} Hz · stereo · {:.2} seconds · offset {:.3} s",
                        buffer.sample_rate,
                        buffer.duration(),
                        offset_seconds
                    ));
                    let (rect, _) = ui.allocate_exact_size(
                        vec2(ui.available_width(), ui.available_height()),
                        Sense::hover(),
                    );
                    let painter = ui.painter_at(rect);
                    for i in 0..rect.width() as usize {
                        let sec = *offset_seconds
                            + i as f64 / rect.width() as f64
                                * clip.length_bars
                                * s.beats_per_bar()
                                * 60.0
                                / s.transport.tempo;
                        let index = (sec * buffer.sample_rate as f64
                            / (buffer.sample_rate / 400).max(1) as f64)
                            as usize;
                        let h =
                            buffer.peaks.get(index).copied().unwrap_or(0.0) * rect.height() * 0.45;
                        painter.line_segment(
                            [
                                pos2(rect.left() + i as f32, rect.center().y - h),
                                pos2(rect.left() + i as f32, rect.center().y + h),
                            ],
                            Stroke::new(1.0, color),
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
        let rows = ((ui.available_height() - 8.0) / row_height)
            .floor()
            .clamp(12.0, 48.0) as u8;
        let high = self.editor_low.saturating_add(rows - 1).min(127);
        egui::ScrollArea::horizontal()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let (rect, _) = ui.allocate_exact_size(
                    vec2(
                        KEY_WIDTH
                            + (beats as f32 * beat_width).max(ui.available_width() - KEY_WIDTH),
                        rows as f32 * row_height,
                    ),
                    Sense::hover(),
                );
                let origin = rect.min + vec2(KEY_WIDTH, 0.0);
                let painter = ui.painter_at(rect);
                for row in 0..rows {
                    let pitch = high.saturating_sub(row);
                    let black = [1, 3, 6, 8, 10].contains(&(pitch % 12));
                    let y = rect.top() + row as f32 * row_height;
                    let key = Rect::from_min_size(
                        pos2(rect.left(), y),
                        vec2(KEY_WIDTH - 1.0, row_height - 1.0),
                    );
                    painter.rect_filled(key, 0, if black { BLACK_KEY } else { WHITE_KEY });
                    if pitch % 12 == 0 {
                        painter.text(
                            key.left_center() + vec2(5.0, 0.0),
                            Align2::LEFT_CENTER,
                            format!("C{}", pitch as i32 / 12 - 1),
                            FontId::monospace(SMALL),
                            if black { DIM } else { PANEL },
                        );
                    }
                    let lane =
                        Rect::from_min_max(pos2(origin.x, y), pos2(rect.right(), y + row_height));
                    painter.rect_filled(lane, 0, if black { WELL } else { LANE });
                    painter.line_segment(
                        [lane.left_bottom(), lane.right_bottom()],
                        Stroke::new(0.5, GRID),
                    );
                    if ui
                        .interact(key, egui::Id::new(("key", pitch)), Sense::click())
                        .clicked()
                    {
                        self.preview(&clip.track_id, pitch, 100);
                    }
                }
                let divisions = (beats / step).ceil().min(100_000.0) as usize;
                for division in 0..=divisions {
                    let beat = division as f64 * step;
                    let x = origin.x + beat as f32 * beat_width;
                    if !ui.clip_rect().x_range().contains(x) {
                        continue;
                    }
                    let major = (beat / bpb).fract().abs() < 1e-6;
                    painter.line_segment(
                        [pos2(x, rect.top()), pos2(x, rect.bottom())],
                        Stroke::new(
                            if major { 1.0 } else { 0.5 },
                            if major {
                                FAINT.gamma_multiply(0.6)
                            } else {
                                GRID
                            },
                        ),
                    );
                }
                let grid = Rect::from_min_max(origin, rect.max);
                let grid_hit = ui.interact(
                    grid,
                    egui::Id::new(("piano-grid", &clip.id)),
                    Sense::click_and_drag(),
                );
                let pointer = ui.input(|i| i.pointer.interact_pos());
                let note_rect = |n: &Note| {
                    Rect::from_min_size(
                        pos2(
                            origin.x + n.start as f32 * beat_width,
                            origin.y + (high as f32 - n.pitch as f32) * row_height + 1.0,
                        ),
                        vec2(
                            (n.length as f32 * beat_width - 1.0).max(3.0),
                            row_height - 2.0,
                        ),
                    )
                };
                let hit_note =
                    pointer.is_some_and(|pt| notes.iter().any(|n| note_rect(n).contains(pt)));
                let step_mode = s.view.editor_mode == "step";
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
                    bevel(&painter, nr, color, selected);
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
                            self.note_drag = Some(NoteDrag {
                                clip: clip.clone(),
                                note: original.clone(),
                                current: original.clone(),
                                anchor: pt - r.drag_delta(),
                                resize: nr.right() - pt.x < 6.0,
                            });
                        }
                    }
                    r.context_menu(|ui| {
                        let mut velocity = n.velocity;
                        if ui
                            .add(egui::Slider::new(&mut velocity, 1..=127).text("Velocity"))
                            .changed()
                        {
                            let mut c = clip.clone();
                            if let ClipData::Midi { notes } = &mut c.data {
                                if let Some(note) = notes.iter_mut().find(|note| note.id == n.id) {
                                    note.velocity = velocity;
                                }
                            }
                            self.dispatch(Command::PutClip(c));
                        }
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
                if (grid_hit.clicked() || grid_hit.drag_stopped())
                    && !hit_note
                    && self.note_drag.is_none()
                {
                    if let Some(pt) = pointer {
                        let end = ((pt.x - origin.x) / beat_width) as f64;
                        let delta = if grid_hit.drag_stopped() {
                            (grid_hit.drag_delta().x / beat_width) as f64
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
                        painter.line_segment(
                            [pos2(x, rect.top()), pos2(x, rect.bottom())],
                            Stroke::new(1.5, ACCENT),
                        );
                    }
                }
            });
    }
    fn score(&self, ui: &mut egui::Ui, clip: &Clip, notes: &[Note], color: egui::Color32) {
        ui.label(
            egui::RichText::new("Pitch overview · edit notes in Piano roll")
                .small()
                .color(FAINT),
        );
        let (rect, _) = ui.allocate_exact_size(
            vec2(ui.available_width(), ui.available_height()),
            Sense::hover(),
        );
        let painter = ui.painter_at(rect);
        let gap = 8.0;
        let top = rect.center().y - 2.0 * gap;
        for i in 0..5 {
            let y = top + i as f32 * gap;
            painter.line_segment(
                [pos2(rect.left() + 24.0, y), pos2(rect.right() - 24.0, y)],
                Stroke::new(1.0, FAINT),
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
