//! Native read-automation editor. Every edit is a Store command and can be undone.
use crate::{
    app::{id, Ondera},
    theme::*,
};
use eframe::egui::{self, pos2, vec2, Align2, Rect, Sense, Stroke};
use ondera_engine::{
    automation::{AutomationLane, AutomationPoint, AutomationTarget, Interpolation},
    model::{Strip, BUS_A, BUS_B, MASTER},
    store::Command,
};
use serde_json::json;

pub struct AutomationUi {
    pub open: bool,
    lane: Option<String>,
    point: Option<String>,
    track: String,
    kind: usize,
    plugin: String,
    parameter: u32,
    filter: String,
    start: f64,
    length: f64,
    snap: bool,
    drag: Option<(AutomationLane, String)>,
    #[cfg(test)]
    graph: Option<Rect>,
}
impl Default for AutomationUi {
    fn default() -> Self {
        Self {
            open: false,
            lane: None,
            point: None,
            track: MASTER.into(),
            kind: 0,
            plugin: String::new(),
            parameter: 0,
            filter: String::new(),
            start: 0.0,
            length: 32.0,
            snap: true,
            drag: None,
            #[cfg(test)]
            graph: None,
        }
    }
}
impl Ondera {
    pub(crate) fn automation_window(&mut self, ctx: &egui::Context) {
        if !self.automation.open {
            return;
        }
        let mut state = std::mem::take(&mut self.automation);
        let mut open = state.open;
        egui::Window::new("Automation")
            .id(egui::Id::new("automation-editor"))
            .open(&mut open)
            .default_size(vec2(920.0, 640.0))
            .min_size(vec2(640.0, 460.0))
            .show(ctx, |ui| self.automation_editor(ui, &mut state));
        state.open = open;
        self.automation = state;
    }
    fn automation_editor(&mut self, ui: &mut egui::Ui, state: &mut AutomationUi) {
        ui.label(text(
            "Read automation · absolute beats",
            FS_BODY,
            Weight::SemiBold,
            INK,
        ));
        ui.label(text(
            "Volume / pan: per sample. Plugin parameters: at most 256 frames between updates.",
            FS_SECONDARY,
            Weight::Medium,
            FAINT,
        ));
        ui.separator();
        let session = self.store.snapshot();
        if !session
            .automation
            .iter()
            .any(|lane| Some(&lane.id) == state.lane.as_ref())
        {
            state.lane = session.automation.first().map(|lane| lane.id.clone());
            state.point = None;
        }
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("automation-lane")
                .width(290.0)
                .selected_text(
                    session
                        .automation
                        .iter()
                        .find(|lane| Some(&lane.id) == state.lane.as_ref())
                        .map_or("Choose a lane", |lane| lane.name.as_str()),
                )
                .show_ui(ui, |ui| {
                    for lane in &session.automation {
                        if ui
                            .selectable_value(&mut state.lane, Some(lane.id.clone()), &lane.name)
                            .changed()
                        {
                            state.point = None;
                        }
                    }
                });
            if text_button(ui, "Fit song", Face::Raised).clicked() {
                state.start = 0.0;
                state.length = (session.end_bar() * session.beats_per_bar()).max(4.0);
            }
            ui.checkbox(&mut state.snap, "¼-beat snap");
        });
        ui.collapsing("Add automation lane", |ui| {
            ui.horizontal(|ui| {
                ui.label("Track");
                egui::ComboBox::from_id_salt("automation-track")
                    .selected_text(
                        session
                            .tracks
                            .iter()
                            .find(|track| track.id == state.track)
                            .map_or_else(
                                || ondera_engine::model::bus_name(&state.track).to_string(),
                                |track| track.name.clone(),
                            ),
                    )
                    .show_ui(ui, |ui| {
                        for (id, label) in
                            [(MASTER, "Stereo Out"), (BUS_A, "Bus A"), (BUS_B, "Bus B")]
                        {
                            ui.selectable_value(&mut state.track, id.into(), label);
                        }
                        for track in &session.tracks {
                            ui.selectable_value(&mut state.track, track.id.clone(), &track.name);
                        }
                    });
                if state.track == BUS_A || state.track == BUS_B {
                    state.kind = 2;
                }
                if state.track == MASTER && state.kind == 1 {
                    state.kind = 0;
                }
                egui::ComboBox::from_id_salt("automation-kind")
                    .selected_text(["Volume", "Pan", "Plugin parameter"][state.kind])
                    .show_ui(ui, |ui| {
                        if state.track != BUS_A && state.track != BUS_B {
                            ui.selectable_value(&mut state.kind, 0, "Volume");
                        }
                        if !ondera_engine::model::is_bus(&state.track) {
                            ui.selectable_value(&mut state.kind, 1, "Pan");
                        }
                        ui.selectable_value(&mut state.kind, 2, "Plugin parameter");
                    });
            });
            let strip = session
                .strips
                .get(&state.track)
                .cloned()
                .unwrap_or_else(Strip::default);
            let mut choices: Vec<(String, String, Option<usize>)> = strip
                .inserts
                .iter()
                .enumerate()
                .filter(|(_, insert)| !insert.is_empty())
                .map(|(slot, insert)| {
                    (
                        insert.id.clone(),
                        format!("{} · {}", slot + 1, insert.name),
                        Some(slot),
                    )
                })
                .collect();
            if session
                .tracks
                .iter()
                .any(|track| track.id == state.track && track.kind == "midi")
            {
                choices.insert(
                    0,
                    (
                        strip.synth_key(&state.track),
                        format!("Instrument · {}", strip.instrument_name()),
                        None,
                    ),
                );
            }
            let mut parameter_name = None;
            if state.kind == 2 {
                if !choices.iter().any(|(key, _, _)| key == &state.plugin) {
                    state.plugin = choices
                        .first()
                        .map_or_else(String::new, |choice| choice.0.clone());
                }
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("automation-plugin")
                        .width(280.0)
                        .selected_text(
                            choices
                                .iter()
                                .find(|choice| choice.0 == state.plugin)
                                .map_or("Load an instrument or effect", |choice| choice.1.as_str()),
                        )
                        .show_ui(ui, |ui| {
                            for (key, name, _) in &choices {
                                ui.selectable_value(&mut state.plugin, key.clone(), name);
                            }
                        });
                    ui.add(
                        egui::TextEdit::singleline(&mut state.filter)
                            .hint_text("Find parameter")
                            .desired_width(180.0),
                    );
                });
                if let Some(entry) = self
                    .plugins
                    .loaded
                    .get(&state.plugin)
                    .filter(|entry| !entry.retiring)
                {
                    let params = entry.editor.params();
                    if !params
                        .iter()
                        .any(|parameter| parameter.id == state.parameter)
                    {
                        state.parameter = params.first().map_or(0, |parameter| parameter.id);
                    }
                    parameter_name = params
                        .iter()
                        .find(|parameter| parameter.id == state.parameter)
                        .map(|parameter| parameter.name.clone());
                    egui::ComboBox::from_id_salt("automation-parameter")
                        .width(360.0)
                        .selected_text(parameter_name.as_deref().unwrap_or("Parameter unavailable"))
                        .show_ui(ui, |ui| {
                            let filter = state.filter.to_lowercase();
                            for parameter in params.iter().filter(|parameter| {
                                parameter.max > parameter.min
                                    && parameter.name.to_lowercase().contains(&filter)
                            }) {
                                ui.selectable_value(
                                    &mut state.parameter,
                                    parameter.id,
                                    &parameter.name,
                                );
                            }
                        });
                } else {
                    ui.label(text(
                        "Plugin is loading or unavailable.",
                        FS_SECONDARY,
                        Weight::Medium,
                        FAINT,
                    ));
                }
            }
            let create = ui.add_enabled(
                state.kind != 2 || parameter_name.is_some(),
                egui::Button::new("Create lane"),
            );
            if create.clicked() {
                let target = match state.kind {
                    0 if state.track == MASTER => "masterVolume",
                    0 => "trackVolume",
                    1 => "trackPan",
                    _ => "pluginParameter",
                };
                let slot = choices
                    .iter()
                    .find(|choice| choice.0 == state.plugin)
                    .and_then(|choice| choice.2);
                let track_name = session
                    .tracks
                    .iter()
                    .find(|track| track.id == state.track)
                    .map_or_else(
                        || ondera_engine::model::bus_name(&state.track).to_string(),
                        |track| track.name.clone(),
                    );
                let name = format!(
                    "{} · {}",
                    track_name,
                    parameter_name.as_deref().unwrap_or(if state.kind == 1 {
                        "Pan"
                    } else {
                        "Volume"
                    })
                );
                let mut params = json!({"target":target,"trackId":state.track,"name":name});
                if state.kind == 2 {
                    params["parameterId"] = json!(state.parameter);
                    if let Some(slot) = slot {
                        params["slot"] = json!(slot);
                    }
                }
                match self.run_control_command(
                    "automation.create",
                    &params,
                    false,
                    "Automation editor",
                ) {
                    Ok(value) => {
                        state.lane = value["lane"]["id"].as_str().map(str::to_string);
                        state.point = None;
                    }
                    Err(error) => self.error = Some(error),
                }
            }
        });
        let session = self.store.snapshot();
        let Some(original) = session
            .automation
            .iter()
            .find(|lane| Some(&lane.id) == state.lane.as_ref())
        else {
            ui.add_space(20.0);
            ui.label("Add a lane, then double-click its graph to draw a fade or parameter change.");
            return;
        };
        let mut lane = original.clone();
        let mut changed = false;
        let mut remove = false;
        ui.horizontal(|ui| {
            changed |= ui.checkbox(&mut lane.enabled, "Read enabled").changed();
            egui::ComboBox::from_id_salt("automation-interpolation")
                .selected_text(if lane.interpolation == Interpolation::Linear {
                    "Linear"
                } else {
                    "Step"
                })
                .show_ui(ui, |ui| {
                    changed |= ui
                        .selectable_value(&mut lane.interpolation, Interpolation::Linear, "Linear")
                        .changed();
                    changed |= ui
                        .selectable_value(&mut lane.interpolation, Interpolation::Step, "Step")
                        .changed();
                });
            if text_button(ui, "Point at playhead", Face::Raised).clicked() {
                let beat = self.position.max(0.0);
                let value = lane.value_at(beat).unwrap_or((lane.min + lane.max) * 0.5);
                put_point(&mut lane, state, beat, value);
                changed = true;
            }
            remove = text_button(ui, "Delete lane", Face::Raised).clicked();
        });
        ui.horizontal(|ui| {
            ui.label("Start beat");
            ui.add(
                egui::DragValue::new(&mut state.start)
                    .speed(0.25)
                    .range(0.0..=1_000_000.0),
            );
            ui.label("Visible beats");
            ui.add(
                egui::DragValue::new(&mut state.length)
                    .speed(1.0)
                    .range(1.0..=1_000_000.0),
            );
            ui.label(format!(
                "{} points · {:.3} to {:.3}",
                lane.points.len(),
                lane.min,
                lane.max
            ));
        });
        let (bounds, response) = ui.allocate_exact_size(
            vec2(
                ui.available_width(),
                ui.available_height().max(230.0) - 78.0,
            ),
            Sense::click(),
        );
        well_deep(ui.painter(), bounds, 5.0);
        let graph =
            Rect::from_min_max(bounds.min + vec2(62.0, 18.0), bounds.max - vec2(18.0, 27.0));
        #[cfg(test)]
        {
            state.graph = Some(graph);
        }
        let at = |beat: f64, value: f64| {
            pos2(
                graph.left() + ((beat - state.start) / state.length) as f32 * graph.width(),
                graph.bottom()
                    - ((value - lane.min) / (lane.max - lane.min)) as f32 * graph.height(),
            )
        };
        for row in 0..=4 {
            let value = lane.min + (lane.max - lane.min) * row as f64 / 4.0;
            let y = at(state.start, value).y;
            hline(
                ui.painter(),
                graph.left(),
                graph.right(),
                y,
                SEPARATOR.gamma_multiply(0.35),
            );
            ui.painter().text(
                pos2(graph.left() - 8.0, y),
                Align2::RIGHT_CENTER,
                format!("{value:.2}"),
                font(FS_SECONDARY, Weight::Medium),
                FAINT,
            );
        }
        for col in 0..=8 {
            let beat = state.start + state.length * col as f64 / 8.0;
            let x = at(beat, lane.min).x;
            vline(
                ui.painter(),
                x,
                graph.top(),
                graph.bottom(),
                SEPARATOR.gamma_multiply(0.35),
            );
            ui.painter().text(
                pos2(x, graph.bottom() + 7.0),
                Align2::CENTER_TOP,
                format!("{beat:.1}"),
                font(FS_SECONDARY, Weight::Medium),
                FAINT,
            );
        }
        let color = match &lane.target {
            AutomationTarget::TrackVolume { track_id }
            | AutomationTarget::TrackPan { track_id }
            | AutomationTarget::PluginParameter { track_id, .. } => session
                .tracks
                .iter()
                .enumerate()
                .find(|(_, track)| &track.id == track_id)
                .map_or(INK_DIM, |(index, track)| track_color(&track.color, index)),
            AutomationTarget::MasterVolume => INK_DIM,
        };
        let painter = ui.painter().with_clip_rect(graph);
        if !lane.points.is_empty() {
            let mut previous = at(
                state.start,
                lane.value_at(state.start).unwrap_or(lane.points[0].value),
            );
            for point in lane
                .points
                .iter()
                .filter(|point| point.beat > state.start && point.beat < state.start + state.length)
            {
                let next = at(point.beat, point.value);
                if lane.interpolation == Interpolation::Step {
                    painter.line_segment(
                        [previous, pos2(next.x, previous.y)],
                        Stroke::new(1.8, color),
                    );
                    painter.line_segment([pos2(next.x, previous.y), next], Stroke::new(1.8, color));
                } else {
                    painter.line_segment([previous, next], Stroke::new(1.8, color));
                }
                previous = next;
            }
            let end = at(
                state.start + state.length,
                lane.value_at(state.start + state.length)
                    .unwrap_or(lane.points.last().unwrap().value),
            );
            painter.line_segment([previous, end], Stroke::new(1.8, color));
        }
        let playhead = at(self.position, lane.min).x;
        if graph.x_range().contains(playhead) {
            vline(&painter, playhead, graph.top(), graph.bottom(), ACCENT);
        }
        let mut dragged = None;
        let mut drag_stopped = false;
        let mut delete_point = None;
        for point in &lane.points {
            if !(state.start..=state.start + state.length).contains(&point.beat) {
                continue;
            }
            let position = at(point.beat, point.value);
            let response = ui.interact(
                Rect::from_center_size(position, vec2(14.0, 14.0)),
                egui::Id::new(("automation-point", &lane.id, &point.id)),
                Sense::click_and_drag(),
            );
            painter.circle_filled(
                position,
                if Some(&point.id) == state.point.as_ref() {
                    5.0
                } else {
                    3.5
                },
                color,
            );
            if response.clicked() {
                state.point = Some(point.id.clone());
            }
            if response.secondary_clicked() {
                delete_point = Some(point.id.clone());
            }
            if response.drag_started() {
                state.drag = Some((lane.clone(), point.id.clone()));
                state.point = Some(point.id.clone());
                self.store.set_gesture(false);
                self.store.set_gesture(true);
            }
            if response.dragged() || response.drag_stopped() {
                dragged = response.interact_pointer_pos();
            }
            if response.drag_stopped() {
                drag_stopped = true;
            }
        }
        let from_pointer = |position: egui::Pos2| {
            let beat = state.start
                + ((position.x - graph.left()) / graph.width()).clamp(0.0, 1.0) as f64
                    * state.length;
            let beat = if state.snap {
                (beat * 4.0).round() / 4.0
            } else {
                beat
            };
            let value = lane.min
                + ((graph.bottom() - position.y) / graph.height()).clamp(0.0, 1.0) as f64
                    * (lane.max - lane.min);
            (beat, value)
        };
        if let Some(position) = dragged {
            if let Some((original, point_id)) = &state.drag {
                let (beat, value) = from_pointer(position);
                let index = original
                    .points
                    .iter()
                    .position(|point| &point.id == point_id)
                    .unwrap();
                let low = index
                    .checked_sub(1)
                    .map_or(0.0, |index| original.points[index].beat + 1e-6);
                let high = original
                    .points
                    .get(index + 1)
                    .map_or(1_000_000.0, |point| point.beat - 1e-6);
                lane = original.clone();
                lane.points[index].beat = beat.clamp(low, high);
                lane.points[index].value = value;
                changed = true;
            }
        } else if response.double_clicked() {
            if let Some(position) = response
                .interact_pointer_pos()
                .filter(|position| graph.contains(*position))
            {
                let (beat, value) = from_pointer(position);
                put_point(&mut lane, state, beat, value);
                changed = true;
            }
        }
        if let Some(id) = delete_point {
            lane.points.retain(|point| point.id != id);
            changed = true;
        }
        if let Some(index) = lane
            .points
            .iter()
            .position(|point| Some(&point.id) == state.point.as_ref())
        {
            ui.horizontal(|ui| {
                ui.label("Selected point");
                let low = index
                    .checked_sub(1)
                    .map_or(0.0, |index| lane.points[index].beat + 1e-6);
                let high = lane
                    .points
                    .get(index + 1)
                    .map_or(1_000_000.0, |point| point.beat - 1e-6);
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut lane.points[index].beat)
                            .prefix("Beat ")
                            .speed(0.25)
                            .range(low..=high),
                    )
                    .changed();
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut lane.points[index].value)
                            .prefix("Value ")
                            .speed((lane.max - lane.min) / 500.0)
                            .range(lane.min..=lane.max),
                    )
                    .changed();
                if text_button(ui, "Delete point", Face::Raised).clicked() {
                    lane.points.remove(index);
                    state.point = None;
                    changed = true;
                }
            });
        }
        ui.label(text("Double-click to add · drag to move · right-click a point to delete · Undo / Redo restores edits",FS_SECONDARY,Weight::Medium,FAINT));
        if remove {
            self.dispatch(Command::RemoveAutomation(lane.id.clone()));
            state.lane = None;
        } else if changed {
            self.dispatch(Command::PutAutomation(lane));
        }
        if drag_stopped {
            state.drag = None;
            self.store.set_gesture(false);
        }
    }
}
fn put_point(lane: &mut AutomationLane, state: &mut AutomationUi, beat: f64, value: f64) {
    if let Some(point) = lane
        .points
        .iter_mut()
        .find(|point| (point.beat - beat).abs() < 1e-8)
    {
        point.value = value;
        state.point = Some(point.id.clone());
    } else {
        let point = AutomationPoint {
            id: id("automation-point"),
            beat,
            value,
        };
        state.point = Some(point.id.clone());
        lane.points.push(point);
        lane.points.sort_by(|a, b| a.beat.total_cmp(&b.beat));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(app: &mut Ondera, ctx: &egui::Context, time: f64, events: Vec<egui::Event>) {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1200.0, 900.0))),
                time: Some(time),
                events,
                focused: true,
                ..Default::default()
            },
            |ctx| app.automation_window(ctx),
        );
    }
    fn button(position: egui::Pos2, pressed: bool) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(position),
            egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    }
    #[test]
    fn graph_double_click_adds_and_drag_moves_a_point_with_undo() {
        let mut app = Ondera::from_session(ondera_engine::store::empty(), None);
        app.run_control_command(
            "automation.create",
            &json!({"target":"masterVolume"}),
            false,
            "test",
        )
        .unwrap();
        app.automation.open = true;
        let ctx = egui::Context::default();
        install(&ctx);
        frame(&mut app, &ctx, 0.0, vec![]);
        frame(&mut app, &ctx, 0.1, vec![]);
        let graph = app.automation.graph.expect("automation graph");
        let center = graph.center();
        frame(&mut app, &ctx, 0.2, button(center, true));
        frame(&mut app, &ctx, 0.25, button(center, false));
        frame(&mut app, &ctx, 0.3, button(center, true));
        frame(&mut app, &ctx, 0.35, button(center, false));
        assert_eq!(app.store.session().automation[0].points.len(), 1);
        let original = app.store.session().automation[0].points[0].clone();
        frame(&mut app, &ctx, 0.45, vec![]);
        let current = app.automation.graph.unwrap();
        let point_position = pos2(
            current.left() + (original.beat / 32.0) as f32 * current.width(),
            current.bottom() - original.value as f32 * current.height(),
        );
        frame(&mut app, &ctx, 0.6, button(point_position, true));
        let moved = point_position + vec2(60.0, -25.0);
        frame(&mut app, &ctx, 0.7, vec![egui::Event::PointerMoved(moved)]);
        frame(&mut app, &ctx, 0.75, vec![egui::Event::PointerMoved(moved)]);
        let released = moved + vec2(20.0, -10.0);
        frame(&mut app, &ctx, 0.8, button(released, false));
        let point = &app.store.session().automation[0].points[0];
        assert!(point.beat > original.beat);
        assert!(point.value > original.value);
        assert_eq!(point.id, original.id);
        let expected_beat =
            (((released.x - current.left()) / current.width()) as f64 * 32.0 * 4.0).round() / 4.0;
        assert_eq!(
            point.beat, expected_beat,
            "release position must be committed even without another held frame"
        );
        app.store.dispatch(Command::Undo).unwrap();
        assert_eq!(app.store.session().automation[0].points[0], original);
    }
}
