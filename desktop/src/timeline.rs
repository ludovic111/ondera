//! Arrangement: toolbar, bar ruler, track headers and lanes.
//!
//! Drag previews stay local (`ClipDrag`); the store receives one `PutClip`
//! when the pointer is released, so undo restores the whole gesture.

use crate::{
    app::{id, Ondera},
    theme::*,
};
use eframe::egui::{self, pos2, vec2, Align2, Pos2, Rect, Sense, Stroke, Vec2};
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
const ZOOM_MIN: f32 = 12.0;
const ZOOM_MAX: f32 = 480.0;
fn zoom_to_t(zoom: f32) -> f32 {
    ((zoom / ZOOM_MIN).ln() / (ZOOM_MAX / ZOOM_MIN).ln()).clamp(0.0, 1.0)
}
fn t_to_zoom(t: f32) -> f32 {
    ZOOM_MIN * (ZOOM_MAX / ZOOM_MIN).powf(t.clamp(0.0, 1.0))
}

impl Ondera {
    pub fn arrangement(&mut self, ui: &mut egui::Ui) {
        let state = self.store.snapshot();
        let bpb = state.beats_per_bar();
        let full = ui.max_rect();
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        self.arrangement_toolbar(ui, &state);
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
        let first = self.scroll.floor() as u64;
        let last = first + (visible_bars.ceil() as u64) + 1;
        let (zoom, scroll) = (self.zoom, self.scroll);
        let bar_x = move |bar: u64| lane_x + ((bar as f64 - scroll) as f32) * zoom;

        // Ruler corner: "+" and TRACKS.
        let corner = Rect::from_min_size(ruler_rect.min, vec2(HEADER, RULER));
        {
            let p = ui.painter();
            p.rect_filled(corner, 0.0, PANEL);
            vline(
                p,
                corner.right() - 1.0,
                corner.top(),
                corner.bottom(),
                black(0.6),
            );
            hline(
                p,
                corner.left(),
                corner.right(),
                corner.bottom() - 1.0,
                black(0.6),
            );
            caps_at(
                p,
                pos2(corner.left() + 40.0, corner.center().y),
                Align2::LEFT_CENTER,
                "Tracks",
                FAINT,
            );
        }
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(Rect::from_min_size(
                corner.min + vec2(10.0, 5.0),
                vec2(22.0, 18.0),
            )),
            |ui| {
                let add = button(ui, vec2(22.0, 18.0), Face::Raised, R_BUTTON, |p, r, ink| {
                    icon(p, r, Icon::Plus, ink)
                })
                .on_hover_text("Add a track");
                egui::Popup::menu(&add).show(|ui| {
                    if ui.button("Instrument track").clicked() {
                        self.add_track("midi");
                    }
                    if ui.button("Audio track").clicked() {
                        self.add_track("audio");
                    }
                });
            },
        );
        // Ruler.
        let ruler = Rect::from_min_max(pos2(lane_x, ruler_y), ruler_rect.max);
        let painter = ui.painter_at(ruler);
        painter.rect_filled(ruler, 0.0, RULER_BG);
        if state.transport.cycle {
            let a = lane_x + ((state.transport.cycle_start_bar - self.scroll) as f32) * self.zoom;
            let b = lane_x + ((state.transport.cycle_end_bar - self.scroll) as f32) * self.zoom;
            let cycle = Rect::from_min_max(
                pos2(a.max(lane_x), ruler_y),
                pos2(b.min(ruler.right()), ruler.bottom()),
            );
            if cycle.is_positive() {
                painter.rect_filled(cycle, 0.0, white(0.09));
                vline(&painter, a, ruler.top(), ruler.bottom(), white(0.2));
                vline(&painter, b - 1.0, ruler.top(), ruler.bottom(), white(0.2));
            }
        }
        for bar in first..=last {
            let x = bar_x(bar);
            if x < lane_x - 1.0 {
                continue;
            }
            vline(&painter, x, ruler.top(), ruler.bottom(), white(0.16));
            painter.text(
                pos2(x + 5.0, ruler.top() + 4.0),
                Align2::LEFT_TOP,
                format!("{}", bar + 1),
                mono_font(FS_VALUE),
                DIM,
            );
            if self.zoom >= 24.0 {
                for beat in 1..bpb.round() as u64 {
                    let bx = x + beat as f32 / bpb as f32 * self.zoom;
                    vline(
                        &painter,
                        bx,
                        ruler.bottom() - 6.0,
                        ruler.bottom(),
                        white(0.07),
                    );
                }
            }
        }
        hline(
            &painter,
            ruler.left(),
            ruler.right(),
            ruler.bottom() - 1.0,
            black(0.6),
        );
        let ruler_hit = ui.interact(ruler, egui::Id::new("ruler-drag"), Sense::click_and_drag());
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
        // Playhead in the ruler: line, flag and a glass readout.
        {
            let x = lane_x + ((self.position / bpb - self.scroll) as f32) * self.zoom;
            if x >= lane_x && x <= ruler.right() {
                playhead(&painter, x, ruler.top(), ruler.bottom());
                playhead_flag(&painter, x, ruler.top());
                let (bar, beat, sixteenth, _) = crate::chrome::position_parts(self.position, bpb);
                let label = format!("{bar}.{beat}.{sixteenth}");
                let galley = painter.layout_no_wrap(label, mono_font(FS_VALUE), INK_BRIGHT);
                let bubble = Rect::from_min_size(
                    pos2(x + 9.0, ruler.top() + 4.0),
                    galley.size() + vec2(14.0, 4.0),
                );
                glass(&painter, bubble, R_BUTTON);
                painter.galley(bubble.min + vec2(7.0, 2.0), galley, INK_BRIGHT);
            }
        }

        // Tracks.
        let scroll_h = (ui.available_height() - 20.0).max(ROW);
        let mut timeline_rect = Rect::NOTHING;
        egui::ScrollArea::vertical()
            .max_height(scroll_h)
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
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                for (index, track) in state.tracks.iter().enumerate() {
                    let (row, _) = ui.allocate_exact_size(vec2(width, ROW), Sense::hover());
                    let selected = Some(&track.id) == state.view.selected_track_id.as_ref();
                    let color = track_color(&track.color, index);
                    let header = Rect::from_min_size(row.min, vec2(HEADER, ROW));
                    let lane = Rect::from_min_max(pos2(lane_x, row.top()), row.max);
                    let p = ui.painter();
                    // Header material.
                    let (top, bottom) = if selected {
                        (HEADER_SELECTED_TOP, HEADER_SELECTED_BOTTOM)
                    } else {
                        (HEADER_TOP, HEADER_BOTTOM)
                    };
                    shade_rect(p, header, 0.0, vertical(header, top, bottom));
                    hline(p, header.left(), header.right(), header.top(), white(0.04));
                    hline(
                        p,
                        header.left(),
                        header.right(),
                        header.bottom() - 1.0,
                        black(0.55),
                    );
                    vline(
                        p,
                        header.right() - 1.0,
                        header.top(),
                        header.bottom(),
                        black(0.6),
                    );
                    let strip = Rect::from_min_size(header.min, vec2(COLOR_STRIP, ROW));
                    p.rect_filled(strip, 0.0, color);
                    vline(p, strip.left(), strip.top(), strip.bottom(), white(0.15));
                    vline(
                        p,
                        strip.right() - 1.0,
                        strip.top(),
                        strip.bottom(),
                        black(0.4),
                    );
                    let title = Rect::from_min_max(
                        pos2(header.left() + 16.0, header.top() + 10.0),
                        pos2(header.right() - 8.0, header.top() + 32.0),
                    );
                    let kind = if track.kind == "audio" { "AUD" } else { "MIDI" };
                    let kind_w = p
                        .layout_no_wrap(kind.into(), mono_font(FS_KIND), FAINT)
                        .size()
                        .x;
                    p.text(
                        pos2(header.right() - 10.0, title.center().y),
                        Align2::RIGHT_CENTER,
                        kind,
                        mono_font(FS_KIND),
                        FAINT,
                    );
                    let name_clip = Rect::from_min_max(
                        title.min,
                        pos2(header.right() - 17.0 - kind_w, title.bottom()),
                    );
                    p.with_clip_rect(name_clip).text(
                        title.left_center(),
                        Align2::LEFT_CENTER,
                        &track.name,
                        font(FS_BODY, Weight::SemiBold),
                        INK,
                    );
                    let response =
                        ui.interact(title, egui::Id::new(("track", &track.id)), Sense::click());
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
                            ui.horizontal(|ui| {
                                for swatch_color in TRACKS {
                                    let (r, resp) =
                                        ui.allocate_exact_size(Vec2::splat(18.0), Sense::click());
                                    swatch(ui.painter(), r.shrink(2.0), swatch_color);
                                    if resp.clicked() {
                                        let mut t = track.clone();
                                        t.color = format!(
                                            "#{:02x}{:02x}{:02x}",
                                            swatch_color.r(),
                                            swatch_color.g(),
                                            swatch_color.b()
                                        );
                                        self.dispatch(Command::UpdateTrack(t));
                                        ui.close();
                                    }
                                }
                            });
                        });
                        if ui.button("Delete track").clicked() {
                            self.dispatch(Command::RemoveTrack(track.id.clone()));
                            ui.close();
                        }
                    });
                    // Header controls: M S R, volume, pan.
                    ui.scope_builder(
                        egui::UiBuilder::new().max_rect(Rect::from_min_size(
                            header.min + vec2(16.0, 34.0),
                            vec2(HEADER - 26.0, 26.0),
                        )),
                        |ui| {
                            ui.spacing_mut().item_spacing.x = 3.0;
                            ui.horizontal_centered(|ui| {
                                let mut t = track.clone();
                                let mut changed = false;
                                if toggle_small(ui, "M", t.mute, false).clicked() {
                                    t.mute = !t.mute;
                                    changed = true;
                                }
                                if toggle_small(ui, "S", t.solo, false).clicked() {
                                    t.solo = !t.solo;
                                    changed = true;
                                }
                                if toggle_small(ui, "R", t.armed, true).clicked() {
                                    t.armed = !t.armed;
                                    changed = true;
                                }
                                ui.add_space(5.0);
                                let mut volume = t.volume;
                                if hslider(ui, &mut volume, 0.0..=1.0, 56.0).changed() {
                                    t.volume = volume;
                                    changed = true;
                                }
                                ui.add_space(5.0);
                                let mut pan = t.pan;
                                if knob_widget(ui, &mut pan, -100.0..=100.0, 0.0, KNOB_SM)
                                    .on_hover_text("Pan")
                                    .changed()
                                {
                                    t.pan = pan.round();
                                    changed = true;
                                }
                                if changed {
                                    self.dispatch(Command::UpdateTrack(t));
                                }
                            });
                        },
                    );
                    // Lane.
                    let p = ui.painter_at(lane);
                    p.rect_filled(
                        lane,
                        0.0,
                        if selected {
                            TIMELINE_SELECTED
                        } else {
                            TIMELINE
                        },
                    );
                    if state.transport.cycle {
                        let a = lane_x
                            + ((state.transport.cycle_start_bar - self.scroll) as f32) * self.zoom;
                        let b = lane_x
                            + ((state.transport.cycle_end_bar - self.scroll) as f32) * self.zoom;
                        let cycle = Rect::from_min_max(
                            pos2(a.max(lane_x), lane.top()),
                            pos2(b.min(lane.right()), lane.bottom()),
                        );
                        if cycle.is_positive() {
                            p.rect_filled(cycle, 0.0, white(0.025));
                        }
                    }
                    for bar in first..=last {
                        let x = bar_x(bar);
                        vline(&p, x, lane.top(), lane.bottom(), white(0.075));
                        if self.zoom >= 24.0 {
                            for beat in 1..bpb.round() as u64 {
                                let bx = x + beat as f32 / bpb as f32 * self.zoom;
                                vline(&p, bx, lane.top(), lane.bottom(), white(0.025));
                            }
                        }
                    }
                    hline(&p, lane.left(), lane.right(), lane.top(), white(0.02));
                    hline(
                        &p,
                        lane.left(),
                        lane.right(),
                        lane.bottom() - 1.0,
                        black(0.45),
                    );
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
                    if self.tool == 1 && self.clip_drag.is_none() {
                        if let (Some((anchor_track, start)), Some(pt)) =
                            (&self.draw_clip_anchor, pointer)
                        {
                            if anchor_track == &track.id && hit.dragged() {
                                let end = self.scroll + ((pt.x - lane_x) / self.zoom) as f64;
                                let a =
                                    lane_x + ((start.min(end) - self.scroll) as f32) * self.zoom;
                                let b =
                                    lane_x + ((start.max(end) - self.scroll) as f32) * self.zoom;
                                let preview = Rect::from_min_max(
                                    pos2(a, lane.top() + CLIP_INSET),
                                    pos2(b.max(a + 4.0), lane.bottom() - CLIP_INSET),
                                );
                                p.rect_filled(preview, R_CLIP, accent(0.25));
                                p.rect_stroke(
                                    preview,
                                    R_CLIP,
                                    Stroke::new(1.0, accent(0.9)),
                                    egui::StrokeKind::Inside,
                                );
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
                        let strip = clip_slab(&p, rect, color, chosen, c.agent);
                        let text_rect = Rect::from_min_max(
                            strip.min + vec2(6.0, 0.0),
                            strip.max - vec2(4.0, 0.0),
                        );
                        if text_rect.width() > 10.0 {
                            let name_painter = p.with_clip_rect(text_rect.intersect(lane));
                            name_painter.text(
                                text_rect.left_center(),
                                Align2::LEFT_CENTER,
                                &c.name,
                                font(FS_SMALL, Weight::SemiBold),
                                white(0.88),
                            );
                            if c.agent && text_rect.width() > 80.0 {
                                name_painter.text(
                                    text_rect.right_center(),
                                    Align2::RIGHT_CENTER,
                                    "AGENT",
                                    mono_font(FS_KIND),
                                    ACCENT,
                                );
                            }
                        }
                        let body = Rect::from_min_max(
                            pos2(rect.left() + 3.0, strip.bottom() + 4.0),
                            rect.max - vec2(3.0, 4.0),
                        );
                        match &c.data {
                            ClipData::Midi { notes } => {
                                for n in notes {
                                    let x = rect.left() + (n.start / bpb) as f32 * self.zoom;
                                    let w = (n.length / bpb) as f32 * self.zoom;
                                    let y = body.bottom()
                                        - (n.pitch as f32 - 24.0) / 84.0 * body.height();
                                    let r = Rect::from_min_size(
                                        pos2(x, y - 1.5),
                                        vec2((w - 1.0).max(3.0), 3.0),
                                    )
                                    .intersect(body);
                                    if r.is_positive() {
                                        if n.agent {
                                            p.rect_filled(r.expand(1.5), 2.0, accent(0.35));
                                            p.rect_filled(r, 1.0, ACCENT);
                                        } else {
                                            p.rect_filled(r, 1.0, white(0.78));
                                        }
                                    }
                                }
                            }
                            ClipData::Audio {
                                source_id,
                                offset_seconds,
                                ..
                            } => {
                                if let Some(buffer) = self.library.get(source_id) {
                                    let visible = body.intersect(lane).intersect(ui.clip_rect());
                                    hline(
                                        &p,
                                        visible.left(),
                                        visible.right(),
                                        body.center().y,
                                        white(0.35),
                                    );
                                    let start_x = (visible.left() - body.left()).max(0.0) as usize;
                                    let end_x = (visible.right() - body.left()).max(0.0) as usize;
                                    for x in start_x..end_x {
                                        let seconds = *offset_seconds
                                            + x as f64 / self.zoom as f64 * bpb * 60.0
                                                / state.transport.tempo;
                                        let index = (seconds * buffer.sample_rate as f64
                                            / (buffer.sample_rate / 400).max(1) as f64)
                                            as usize;
                                        let peak = buffer.peaks.get(index).copied().unwrap_or(0.0);
                                        let height =
                                            (peak.min(1.0) * body.height() * 0.46).max(0.4);
                                        p.line_segment(
                                            [
                                                pos2(
                                                    body.left() + x as f32 + 0.5,
                                                    body.center().y - height,
                                                ),
                                                pos2(
                                                    body.left() + x as f32 + 0.5,
                                                    body.center().y + height,
                                                ),
                                            ],
                                            Stroke::new(1.0, white(0.72)),
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
                        if self.tool == 2 && response.hovered() {
                            if let Some(pt) = pointer {
                                vline(&p, pt.x, rect.top(), rect.bottom(), accent(0.9));
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
                                let mode = if anchor.x - rect.left() < CLIP_EDGE_GRIP {
                                    1
                                } else if rect.right() - anchor.x < CLIP_EDGE_GRIP {
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
                // Below the last track.
                let used = state.tracks.len() as f32 * ROW;
                let rest_h = (scroll_h - used).max(0.0);
                if rest_h > 0.0 {
                    let (rest, _) = ui.allocate_exact_size(vec2(width, rest_h), Sense::hover());
                    let p = ui.painter();
                    p.rect_filled(rest, 0.0, TIMELINE_EMPTY);
                    let column = Rect::from_min_size(rest.min, vec2(HEADER, rest_h));
                    p.rect_filled(column, 0.0, EDITOR);
                    vline(
                        p,
                        column.right() - 1.0,
                        column.top(),
                        column.bottom(),
                        black(0.6),
                    );
                    if state.tracks.is_empty() {
                        p.text(
                            pos2(rest.left() + HEADER + 16.0, rest.top() + 24.0),
                            Align2::LEFT_CENTER,
                            "Add a track with + or drop audio files here.",
                            font(FS_SECONDARY, Weight::Medium),
                            FAINT,
                        );
                    }
                }
                let x = lane_x + ((self.position / bpb - self.scroll) as f32) * self.zoom;
                if x >= lane_x {
                    playhead(
                        &ui.painter_at(timeline_rect),
                        x,
                        row_top,
                        timeline_rect.bottom(),
                    );
                }
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
        // Horizontal scroll rail.
        ui.horizontal(|ui| {
            ui.add_space(HEADER + 4.0);
            let max = (state.end_bar() + 16.0 - visible_bars).max(0.0) as f32;
            let mut scroll = self.scroll as f32;
            let rail_w = (ui.available_width() - 8.0).max(40.0);
            if hslider(ui, &mut scroll, 0.0..=max.max(0.001), rail_w).changed() {
                self.scroll = scroll.max(0.0) as f64;
            }
        });
        // The editor pane below casts a shadow up onto the arrangement.
        inset(
            ui.painter(),
            Rect::from_min_max(pos2(full.left(), full.bottom() - 10.0), full.max),
            0.0,
            Side::Bottom,
            10.0,
            black(0.5),
        );
        if ui.rect_contains_pointer(ui.max_rect()) {
            let delta = ui.input(|i| i.raw_scroll_delta);
            if ui.input(|i| i.modifiers.command) {
                self.zoom = (self.zoom * (delta.y * 0.01).exp()).clamp(ZOOM_MIN, ZOOM_MAX);
            } else if delta.x != 0.0 {
                self.scroll = (self.scroll - delta.x as f64 / self.zoom as f64).max(0.0);
            }
        }
    }
    fn arrangement_toolbar(&mut self, ui: &mut egui::Ui, state: &Session) {
        let (bar, _) = ui.allocate_exact_size(vec2(ui.available_width(), TOOLBAR), Sense::hover());
        toolbar_bar(ui.painter(), bar);
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(bar.shrink2(vec2(10.0, 0.0))),
            |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    if let Some(tool) = segmented_icons(
                        ui,
                        &[
                            (Icon::Pointer, "Pointer · 1"),
                            (Icon::Pencil, "Pencil · 2"),
                            (Icon::Scissors, "Scissors · 3"),
                        ],
                        self.tool,
                    ) {
                        self.tool = tool;
                    }
                    ui.add_space(4.0);
                    let t = &state.transport;
                    let grid = ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.label(text("Grid", FS_SECONDARY, Weight::Medium, FAINT));
                        ui.label(text(
                            format!("1/{}", t.snap_division),
                            FS_SECONDARY,
                            Weight::Medium,
                            DIM,
                        ));
                    });
                    let grid_hit =
                        ui.interact(grid.response.rect, ui.id().with("grid"), Sense::click());
                    egui::Popup::menu(&grid_hit).show(|ui| {
                        for d in [1, 2, 4, 8, 16, 32, 64] {
                            if ui
                                .selectable_label(t.snap_division == d, format!("1/{d}"))
                                .clicked()
                            {
                                let mut t = t.clone();
                                t.snap_division = d;
                                self.dispatch(Command::SetTransport(t));
                            }
                        }
                    });
                    ui.add_space(4.0);
                    let cycle = ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.label(text("Cycle", FS_SECONDARY, Weight::Medium, FAINT));
                        ui.label(text(
                            if t.cycle {
                                format!(
                                    "{} – {}",
                                    t.cycle_start_bar as u64 + 1,
                                    t.cycle_end_bar as u64 + 1
                                )
                            } else {
                                "off".into()
                            },
                            FS_SECONDARY,
                            Weight::Medium,
                            DIM,
                        ));
                    });
                    if ui
                        .interact(cycle.response.rect, ui.id().with("cycle"), Sense::click())
                        .on_hover_text("Toggle cycle · C")
                        .clicked()
                    {
                        let mut t = t.clone();
                        t.cycle = !t.cycle;
                        self.dispatch(Command::SetTransport(t));
                    }
                    ui.add_space(4.0);
                    let follow = ui.label(text(
                        if state.view.follow_playhead {
                            "Follow"
                        } else {
                            "Free"
                        },
                        FS_SECONDARY,
                        Weight::Medium,
                        DIM,
                    ));
                    if ui
                        .interact(follow.rect, ui.id().with("follow"), Sense::click())
                        .on_hover_text("Follow the playhead while playing · F")
                        .clicked()
                    {
                        let mut v = state.view.clone();
                        v.follow_playhead = !v.follow_playhead;
                        self.dispatch(Command::SetView(v));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        let mut t = zoom_to_t(self.zoom);
                        if hslider(ui, &mut t, 0.0..=1.0, ZOOM_RAIL_W).changed() {
                            self.zoom = t_to_zoom(t);
                        }
                        ui.label(text("Zoom", FS_SECONDARY, Weight::Medium, FAINT));
                    });
                });
            },
        );
    }
}
fn clip_rect(c: &Clip, x: f32, y: f32, zoom: f32, scroll: f64) -> Rect {
    Rect::from_min_size(
        pos2(
            x + ((c.start_bar - scroll) as f32) * zoom + 1.0,
            y + CLIP_INSET,
        ),
        vec2(
            (c.length_bars as f32 * zoom - 2.0).max(4.0),
            ROW - 2.0 * CLIP_INSET,
        ),
    )
}
