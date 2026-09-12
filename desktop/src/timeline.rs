use crate::{
    app::{id, Ondera},
    theme::*,
};
use eframe::egui::{self, pos2, vec2, Align2, FontId, Pos2, Rect, Sense, Stroke};
use ondera_engine::{
    model::*,
    store::{self, Command},
};

#[derive(Clone)]
pub struct ClipDrag {
    pub original: Clip,
    pub current: Clip,
    pub mode: u8,
    pub anchor: Pos2,
}
fn snap(v: f64, step: f64, free: bool) -> f64 {
    if free {
        v
    } else {
        (v / step).round() * step
    }
}

impl Ondera {
    pub fn arrangement(&mut self, ui: &mut egui::Ui) {
        let state = self.store.snapshot();
        let bpb = state.beats_per_bar();
        ui.horizontal(|ui| {
            ui.add_space(GAP);
            if ui.button("+ MIDI").clicked() {
                self.add_track("midi");
            }
            if ui.button("+ Audio").clicked() {
                self.add_track("audio");
            }
            ui.separator();
            for (i, label) in ["Pointer", "Pencil", "Scissors"].iter().enumerate() {
                ui.selectable_value(&mut self.tool, i, *label);
            }
            ui.separator();
            let mut t = state.transport.clone();
            let mut changed = false;
            egui::ComboBox::from_id_salt("snap")
                .selected_text(format!("1/{}", t.snap_division))
                .width(50.0)
                .show_ui(ui, |ui| {
                    for d in [1, 2, 4, 8, 16, 32, 64] {
                        changed |= ui
                            .selectable_value(&mut t.snap_division, d, format!("1/{d}"))
                            .changed();
                    }
                });
            if changed {
                self.dispatch(Command::SetTransport(t));
            }
            ui.add(
                egui::Slider::new(&mut self.zoom, 12.0..=480.0)
                    .logarithmic(true)
                    .show_value(false)
                    .text("Zoom"),
            );
        });
        let width = ui.available_width();
        let ruler_y = ui.cursor().top();
        let (ruler_rect, _) = ui.allocate_exact_size(vec2(width, RULER), Sense::hover());
        let lane_x = ruler_rect.left() + HEADER;
        let visible_bars = ((width - HEADER) / self.zoom).max(1.0) as f64;
        if self.playing && state.view.follow_playhead {
            let bar = self.position / bpb;
            if bar < self.scroll || bar > self.scroll + visible_bars * 0.88 {
                self.scroll = (bar - visible_bars * 0.15).max(0.0);
            }
        }
        let painter = ui.painter_at(ruler_rect);
        painter.rect_filled(ruler_rect, 0, PANEL);
        if state.transport.cycle {
            let a = lane_x + ((state.transport.cycle_start_bar - self.scroll) as f32) * self.zoom;
            let b = lane_x + ((state.transport.cycle_end_bar - self.scroll) as f32) * self.zoom;
            let cycle = Rect::from_min_max(
                pos2(a.max(lane_x), ruler_y),
                pos2(b.min(ruler_rect.right()), ruler_y + RULER),
            );
            if cycle.is_positive() {
                painter.rect_filled(cycle, 0, GOLD.gamma_multiply(0.3));
            }
        }
        let first = self.scroll.floor() as u64;
        for bar in first..=first + (visible_bars.ceil() as u64) + 1 {
            let x = lane_x + ((bar as f64 - self.scroll) as f32) * self.zoom;
            if x < lane_x {
                continue;
            }
            painter.line_segment(
                [pos2(x, ruler_y + RULER - 7.0), pos2(x, ruler_y + RULER)],
                Stroke::new(1.0, FAINT),
            );
            painter.text(
                pos2(x + 5.0, ruler_y + 7.0),
                Align2::LEFT_TOP,
                format!("{}", bar + 1),
                FontId::monospace(SMALL),
                DIM,
            );
        }
        let ruler_hit = ui.interact(
            Rect::from_min_max(pos2(lane_x, ruler_y), ruler_rect.max),
            egui::Id::new("ruler-drag"),
            Sense::click_and_drag(),
        );
        if ruler_hit.clicked() {
            if let Some(p) = ruler_hit.interact_pointer_pos() {
                self.locate((self.scroll + ((p.x - lane_x) / self.zoom) as f64).max(0.0) * bpb);
            }
        }
        if ruler_hit.drag_started() {
            self.ruler_anchor = ui
                .input(|i| i.pointer.press_origin())
                .map(|p| self.scroll + ((p.x - lane_x) / self.zoom) as f64);
        }
        if ruler_hit.drag_stopped() {
            if let Some(p) = ruler_hit.interact_pointer_pos() {
                let end = self.scroll + ((p.x - lane_x) / self.zoom) as f64;
                let start = self.ruler_anchor.take().unwrap_or(end);
                let mut t = state.transport.clone();
                t.cycle_start_bar = start.min(end).floor().max(0.0);
                t.cycle_end_bar = start.max(end).ceil().max(t.cycle_start_bar + 0.25);
                t.cycle = true;
                self.dispatch(Command::SetTransport(t));
            }
        }
        let mut timeline_rect = Rect::NOTHING;
        egui::ScrollArea::vertical()
            .max_height((ui.available_height() - 26.0).max(70.0))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let row_top = ui.cursor().top();
                timeline_rect = Rect::from_min_max(
                    pos2(lane_x, row_top),
                    pos2(
                        ui.max_rect().right(),
                        row_top + state.tracks.len() as f32 * ROW,
                    ),
                );
                ui.spacing_mut().item_spacing.y = 0.0;
                for (index, track) in state.tracks.iter().enumerate() {
                    let (row, _) = ui.allocate_exact_size(vec2(width, ROW), Sense::hover());
                    let selected = Some(&track.id) == state.view.selected_track_id.as_ref();
                    let color = track_color(&track.color, index);
                    let p = ui.painter();
                    p.rect_filled(row, 0, if selected { LANE_SELECTED } else { LANE });
                    p.line_segment(
                        [row.left_bottom(), row.right_bottom()],
                        Stroke::new(1.0, DESK),
                    );
                    let header = Rect::from_min_size(row.min, vec2(HEADER, ROW));
                    p.rect_filled(header, 0, if selected { RAISED } else { PANEL });
                    p.rect_filled(Rect::from_min_size(row.min, vec2(4.0, ROW)), 0, color);
                    let title = Rect::from_min_max(
                        row.min + vec2(12.0, 5.0),
                        pos2(header.right() - 8.0, row.top() + 25.0),
                    );
                    let response =
                        ui.interact(title, egui::Id::new(("track", &track.id)), Sense::click());
                    ui.painter().text(
                        title.left_center(),
                        Align2::LEFT_CENTER,
                        &track.name,
                        FontId::proportional(BODY),
                        INK,
                    );
                    if response.clicked() {
                        self.dispatch(Command::Select {
                            track: Some(track.id.clone()),
                            clip: None,
                            note: None,
                        });
                    }
                    response.context_menu(|ui| {
                        if ui.button("Move up").clicked() {
                            self.dispatch(Command::MoveTrack {
                                id: track.id.clone(),
                                index: index.saturating_sub(1),
                            });
                            ui.close();
                        }
                        if ui.button("Move down").clicked() {
                            self.dispatch(Command::MoveTrack {
                                id: track.id.clone(),
                                index: index + 1,
                            });
                            ui.close();
                        }
                        ui.menu_button("Colour", |ui| {
                            for color in TRACKS {
                                if ui.add(egui::Button::new("    ").fill(color)).clicked() {
                                    let mut t = track.clone();
                                    t.color = format!(
                                        "#{:02x}{:02x}{:02x}",
                                        color.r(),
                                        color.g(),
                                        color.b()
                                    );
                                    self.dispatch(Command::UpdateTrack(t));
                                    ui.close();
                                }
                            }
                        });
                        if ui.button("Delete track").clicked() {
                            self.dispatch(Command::RemoveTrack(track.id.clone()));
                            ui.close();
                        }
                    });
                    ui.scope_builder(
                        egui::UiBuilder::new().max_rect(Rect::from_min_size(
                            row.min + vec2(10.0, 29.0),
                            vec2(HEADER - 20.0, 34.0),
                        )),
                        |ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            ui.horizontal(|ui| {
                                let mut t = track.clone();
                                let mut changed = false;
                                if ui.selectable_label(t.mute, "M").clicked() {
                                    t.mute = !t.mute;
                                    changed = true;
                                }
                                if ui.selectable_label(t.solo, "S").clicked() {
                                    t.solo = !t.solo;
                                    changed = true;
                                }
                                if ui
                                    .selectable_label(t.armed, egui::RichText::new("R").color(RED))
                                    .clicked()
                                {
                                    t.armed = !t.armed;
                                    changed = true;
                                }
                                ui.spacing_mut().slider_width = 55.0;
                                changed |= ui
                                    .add(
                                        egui::Slider::new(&mut t.volume, 0.0..=1.0)
                                            .show_value(false),
                                    )
                                    .changed();
                                if changed {
                                    self.dispatch(Command::UpdateTrack(t));
                                }
                            });
                        },
                    );
                    let lane = Rect::from_min_max(pos2(lane_x, row.top()), row.max);
                    let p = ui.painter_at(lane);
                    for bar in first..=first + (visible_bars.ceil() as u64) + 1 {
                        let x = lane_x + ((bar as f64 - self.scroll) as f32) * self.zoom;
                        p.line_segment(
                            [pos2(x, row.top()), pos2(x, row.bottom())],
                            Stroke::new(1.0, GRID),
                        );
                    }
                    let hit = ui.interact(
                        lane,
                        egui::Id::new(("lane", &track.id)),
                        Sense::click_and_drag(),
                    );
                    let clips: Vec<_> = state
                        .clips
                        .iter()
                        .filter(|c| c.track_id == track.id)
                        .collect();
                    let pointer = ui.input(|i| i.pointer.interact_pos());
                    let over_clip = pointer.is_some_and(|pt| {
                        clips.iter().any(|c| {
                            clip_rect(c, lane_x, row.top(), self.zoom, self.scroll).contains(pt)
                        })
                    });
                    if hit.double_clicked() && !over_clip && track.kind == "midi" {
                        if let Some(pt) = pointer {
                            let start = (self.scroll + ((pt.x - lane_x) / self.zoom) as f64)
                                .floor()
                                .max(0.0);
                            self.add_clip(track.id.clone(), start, 1.0);
                        }
                    }
                    if hit.clicked() && !over_clip {
                        self.dispatch(Command::Select {
                            track: Some(track.id.clone()),
                            clip: None,
                            note: None,
                        });
                    }
                    if hit.drag_started()
                        && self.tool == 1
                        && self.clip_drag.is_none()
                        && track.kind == "midi"
                    {
                        if let Some(pt) = ui.input(|i| i.pointer.press_origin()) {
                            if !clips.iter().any(|c| {
                                clip_rect(c, lane_x, row.top(), self.zoom, self.scroll).contains(pt)
                            }) {
                                self.draw_clip_anchor = Some((
                                    track.id.clone(),
                                    self.scroll + ((pt.x - lane_x) / self.zoom) as f64,
                                ));
                            }
                        }
                    }
                    if hit.drag_stopped()
                        && self.tool == 1
                        && self.clip_drag.is_none()
                        && track.kind == "midi"
                    {
                        if let Some(pt) = pointer {
                            let end = self.scroll + ((pt.x - lane_x) / self.zoom) as f64;
                            let start = self
                                .draw_clip_anchor
                                .take()
                                .filter(|(id, _)| id == &track.id)
                                .map_or(end, |(_, start)| start);
                            let step = 4.0 / state.transport.snap_division as f64 / bpb;
                            let a = snap(start.min(end), step, false).max(0.0);
                            let b = snap(start.max(end), step, false).max(a + step);
                            self.add_clip(track.id.clone(), a, b - a);
                        }
                    }
                    for original in clips {
                        let c = self
                            .clip_drag
                            .as_ref()
                            .filter(|d| d.current.id == original.id)
                            .map_or(original, |d| &d.current)
                            .clone();
                        let rect = clip_rect(&c, lane_x, row.top(), self.zoom, self.scroll);
                        if !rect.intersects(lane) {
                            continue;
                        }
                        let chosen = Some(&original.id) == state.view.selected_clip_id.as_ref();
                        bevel(&p, rect, color, chosen);
                        let text_rect = Rect::from_min_max(
                            rect.min + vec2(6.0, 3.0),
                            pos2(rect.right() - 4.0, rect.top() + 17.0),
                        );
                        if text_rect.width() > 10.0 {
                            p.with_clip_rect(text_rect.intersect(lane)).text(
                                text_rect.min,
                                Align2::LEFT_TOP,
                                &c.name,
                                FontId::proportional(SMALL),
                                INK,
                            );
                        }
                        let body = Rect::from_min_max(
                            rect.min + vec2(3.0, 20.0),
                            rect.max - vec2(3.0, 4.0),
                        );
                        match &c.data {
                            ClipData::Midi { notes } => {
                                for n in notes {
                                    let x = rect.left() + (n.start / bpb) as f32 * self.zoom;
                                    let w = (n.length / bpb) as f32 * self.zoom;
                                    let y = body.bottom()
                                        - (n.pitch as f32 - 24.0) / 84.0 * body.height();
                                    p.rect_filled(
                                        Rect::from_min_size(pos2(x, y), vec2(w.max(2.0), 2.0))
                                            .intersect(body),
                                        0,
                                        color,
                                    );
                                }
                            }
                            ClipData::Audio {
                                source_id,
                                offset_seconds,
                            } => {
                                if let Some(buffer) = self.library.get(source_id) {
                                    let visible = body.intersect(lane).intersect(ui.clip_rect());
                                    let start_x = (visible.left() - body.left()).max(0.0) as usize;
                                    let end_x = (visible.right() - body.left()).max(0.0) as usize;
                                    for x in (start_x..end_x).step_by(2) {
                                        let seconds = *offset_seconds
                                            + x as f64 / self.zoom as f64 * bpb * 60.0
                                                / state.transport.tempo;
                                        let index = (seconds * buffer.sample_rate as f64
                                            / (buffer.sample_rate / 400).max(1) as f64)
                                            as usize;
                                        let peak = buffer.peaks.get(index).copied().unwrap_or(0.0);
                                        let height = peak.min(1.0) * body.height() * 0.48;
                                        p.line_segment(
                                            [
                                                pos2(
                                                    body.left() + x as f32,
                                                    body.center().y - height,
                                                ),
                                                pos2(
                                                    body.left() + x as f32,
                                                    body.center().y + height,
                                                ),
                                            ],
                                            Stroke::new(1.0, color),
                                        );
                                    }
                                }
                            }
                        }
                        let response = ui.interact(
                            rect.intersect(lane),
                            egui::Id::new(("clip", &c.id)),
                            Sense::click_and_drag(),
                        );
                        if response.clicked() {
                            self.dispatch(Command::Select {
                                track: Some(track.id.clone()),
                                clip: Some(c.id.clone()),
                                note: None,
                            });
                            if self.tool == 2 {
                                if let Some(pt) = response.interact_pointer_pos() {
                                    let bar = snap(
                                        self.scroll + ((pt.x - lane_x) / self.zoom) as f64,
                                        4.0 / state.transport.snap_division as f64 / bpb,
                                        ui.input(|i| i.modifiers.alt),
                                    );
                                    self.split_selected(bar);
                                }
                            }
                        }
                        response.context_menu(|ui| {
                            if ui.button("Duplicate").clicked() {
                                let mut c = c.clone();
                                c.id = id("clip");
                                c.start_bar += c.length_bars;
                                self.dispatch(Command::PutClip(c));
                                ui.close();
                            }
                            if ui.button("Delete").clicked() {
                                self.dispatch(Command::RemoveClip(c.id.clone()));
                                ui.close();
                            }
                        });
                        if response.drag_started() {
                            if let Some(pt) = response.interact_pointer_pos() {
                                let anchor = ui.input(|i| i.pointer.press_origin()).unwrap_or(pt);
                                let mode = if anchor.x - rect.left() < 7.0 {
                                    1
                                } else if rect.right() - anchor.x < 7.0 {
                                    2
                                } else {
                                    0
                                };
                                self.clip_drag = Some(ClipDrag {
                                    original: original.clone(),
                                    current: original.clone(),
                                    mode,
                                    anchor,
                                });
                                self.dispatch(Command::Select {
                                    track: Some(track.id.clone()),
                                    clip: Some(c.id.clone()),
                                    note: None,
                                });
                            }
                        }
                    }
                }
                let x = lane_x + ((self.position / bpb - self.scroll) as f32) * self.zoom;
                let p = ui.painter_at(timeline_rect);
                p.line_segment(
                    [pos2(x, row_top), pos2(x, timeline_rect.bottom())],
                    Stroke::new(1.5, ACCENT),
                );
            });
        if let Some(mut drag) = self.clip_drag.take() {
            if let Some(pt) = ui.input(|i| i.pointer.interact_pos()) {
                let step = 4.0 / state.transport.snap_division as f64 / bpb;
                let delta = snap(
                    ((pt.x - drag.anchor.x) / self.zoom) as f64,
                    step,
                    ui.input(|i| i.modifiers.alt),
                );
                let old = &drag.original;
                drag.current = old.clone();
                match drag.mode {
                    1 => {
                        let left = (old.start_bar + delta).clamp(
                            old.start_bar,
                            (old.start_bar + old.length_bars - step).max(old.start_bar),
                        );
                        if left > old.start_bar {
                            if let Ok((_, right)) =
                                store::split(old, left, old.id.clone(), bpb, state.transport.tempo)
                            {
                                drag.current = right;
                            }
                        }
                    }
                    2 => drag.current.length_bars = (old.length_bars + delta).max(step),
                    _ => {
                        drag.current.start_bar = (old.start_bar + delta).max(0.0);
                        let index = ((pt.y - timeline_rect.top()) / ROW).floor().max(0.0) as usize;
                        if let Some(target) = state.tracks.get(index) {
                            let kind = match old.data {
                                ClipData::Midi { .. } => "midi",
                                _ => "audio",
                            };
                            if target.kind == kind {
                                drag.current.track_id = target.id.clone();
                            }
                        }
                    }
                }
            }
            if ui.input(|i| i.pointer.any_released()) {
                self.dispatch(Command::PutClip(drag.current));
            } else {
                self.clip_drag = Some(drag);
            }
        }
        ui.horizontal(|ui| {
            ui.add_space(HEADER);
            let max = (state.end_bar() + 16.0 - visible_bars).max(0.0);
            ui.add_sized(
                [ui.available_width() - 8.0, 18.0],
                egui::Slider::new(&mut self.scroll, 0.0..=max).show_value(false),
            );
        });
        if ui.rect_contains_pointer(ui.max_rect()) {
            let delta = ui.input(|i| i.raw_scroll_delta);
            if ui.input(|i| i.modifiers.command) {
                self.zoom = (self.zoom * (delta.y * 0.01).exp()).clamp(12.0, 480.0);
            } else if delta.x != 0.0 {
                self.scroll = (self.scroll - delta.x as f64 / self.zoom as f64).max(0.0);
            }
        }
    }
}
fn clip_rect(c: &Clip, x: f32, y: f32, zoom: f32, scroll: f64) -> Rect {
    Rect::from_min_size(
        pos2(x + ((c.start_bar - scroll) as f32) * zoom, y + 5.0),
        vec2((c.length_bars as f32 * zoom - 2.0).max(4.0), ROW - 10.0),
    )
}
