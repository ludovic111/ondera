//! Window chrome: title bar, transport, browser and inspector.
//!
//! Layout follows `design/Ondera Arrangement.dc.html`. Every persistent edit
//! goes through `store::Command`; the widgets here only paint and interact.

use crate::{
    app::{Intent, Ondera},
    plugins::{format_icon, strip_label, truncate},
    theme::*,
};
use eframe::egui::{self, pos2, vec2, Align2, Rect, Sense, Vec2};
use ondera_engine::{
    device, midi,
    model::*,
    plugin::{Descriptor, Format},
    store::Command,
};

pub const LOOPS: [&str; 9] = [
    "Boom Bap 92",
    "Four Floor 124",
    "Brushes Swing",
    "Rhodes Comp Cm",
    "Analog Pad Swell",
    "Bass Pluck 120",
    "Riser 1 bar",
    "Reverse Cymbal",
    "Vinyl Crackle",
];
const KEYS: [&str; 24] = [
    "C maj", "C min", "Db maj", "C# min", "D maj", "D min", "Eb maj", "Eb min", "E maj", "E min",
    "F maj", "F min", "F# maj", "F# min", "G maj", "G min", "Ab maj", "G# min", "A maj", "A min",
    "Bb maj", "Bb min", "B maj", "B min",
];
/// name, meta, swatch colour, plugin id
type BrowserItem = (String, String, egui::Color32, String);
const SIGNATURES: [(u32, u32); 7] = [(4, 4), (3, 4), (2, 4), (6, 8), (5, 4), (7, 8), (12, 8)];

fn db_text(gain: f32) -> String {
    if gain > 0.0 {
        format!("{:.1}", 20.0 * gain.log10()).replace('-', "−")
    } else {
        "−∞".into()
    }
}
pub fn position_parts(beats: f64, bpb: f64) -> (u64, u64, u64, u64) {
    let bar = (beats / bpb).floor();
    let within = beats - bar * bpb;
    let beat = within.floor();
    let frac = within - beat;
    let sixteenth = (frac * 4.0).floor();
    let ticks = ((frac * 4.0 - sixteenth) * 240.0).floor();
    (
        bar as u64 + 1,
        beat as u64 + 1,
        sixteenth as u64 + 1,
        ticks as u64,
    )
}

impl Ondera {
    pub fn title_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("title")
            .exact_height(TITLE_BAR)
            .frame(egui::Frame::new().fill(PANEL))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let p = ui.painter();
                hline(p, rect.left(), rect.right(), rect.top(), white(0.05));
                hline(
                    p,
                    rect.left(),
                    rect.right(),
                    rect.bottom() - 1.0,
                    black(0.5),
                );
                let name = format!(
                    "{}{}",
                    self.store.session().name,
                    if self.store.dirty() { " •" } else { "" }
                );
                p.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    name,
                    font(FS_LIST, Weight::Medium),
                    INK,
                );
                let right = match &self.device {
                    Some(d) => format!(
                        "{} · {} · {} kHz · 24-bit",
                        self.status,
                        d.device_name,
                        d.sample_rate / 1000
                    ),
                    None => format!("{} · audio offline", self.status),
                };
                p.text(
                    pos2(rect.right() - 14.0, rect.center().y),
                    Align2::RIGHT_CENTER,
                    right,
                    mono_font(FS_SMALL),
                    FAINT,
                );
                // The bar moves the window; a double-click zooms it. Menus added after
                // this interaction sit on top of it and keep their clicks.
                let drag = ui.interact(rect, ui.id().with("drag"), Sense::click_and_drag());
                if drag.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if drag.double_clicked() {
                    let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                }
                ui.horizontal_centered(|ui| {
                    ui.add_space(if cfg!(target_os = "macos") {
                        TRAFFIC_LIGHTS + 14.0
                    } else {
                        14.0
                    });
                    ui.spacing_mut().item_spacing.x = 6.0;
                    ui.spacing_mut().button_padding = vec2(5.0, 3.0);
                    ui.style_mut()
                        .text_styles
                        .insert(egui::TextStyle::Button, font(FS_LIST, Weight::Medium));
                    ui.visuals_mut().widgets.inactive.fg_stroke.color = DIM;
                    egui::MenuBar::new().ui(ui, |ui| self.menus(ui));
                    ui.add_space(8.0);
                    self.update_button(ui);
                });
            });
    }
    fn menus(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("File", |ui| {
            for (label, intent) in [
                ("New session", Intent::New),
                ("Open…", Intent::Open),
                ("Open demo", Intent::Demo),
            ] {
                if ui.button(label).clicked() {
                    self.request(intent);
                }
            }
            ui.separator();
            if ui.button("Save").clicked() {
                self.save(false);
            }
            if ui.button("Save as…").clicked() {
                self.save(true);
            }
            if ui.button("Import audio…").clicked() {
                self.import(None);
            }
            if ui.button("Import MIDI…").clicked() {
                self.import_midi_dialog();
            }
            if ui.button("Export audio…").clicked() {
                self.bounce();
            }
            if ui.button("Export MIDI…").clicked() {
                self.export_midi_dialog();
            }
            ui.separator();
            if ui.button("Recover session…").clicked() {
                self.open_recovery();
            }
            ui.label(mono(self.recovery.status(), FS_SMALL, DIM));
            ui.separator();
            if ui.button("Quit").clicked() {
                self.request(Intent::Quit);
            }
        });
        ui.menu_button("Edit", |ui| {
            if ui
                .add_enabled(self.store.can_undo(), egui::Button::new("Undo"))
                .clicked()
            {
                self.dispatch(Command::Undo);
            }
            if ui
                .add_enabled(self.store.can_redo(), egui::Button::new("Redo"))
                .clicked()
            {
                self.dispatch(Command::Redo);
            }
            ui.separator();
            if ui.button("Duplicate region").clicked() {
                self.duplicate_clip();
            }
            if ui.button("Split at playhead").clicked() {
                self.split_selected(self.position / self.store.session().beats_per_bar());
            }
            if ui.button("Delete selection").clicked() {
                self.delete_selected();
            }
            ui.separator();
            if ui.button("Quantize region notes").clicked() {
                self.quantize_selected();
            }
            if ui.button("Transpose region up").clicked() {
                self.transpose_selected(1);
            }
            if ui.button("Transpose region down").clicked() {
                self.transpose_selected(-1);
            }
            if ui.button("Transpose region up an octave").clicked() {
                self.transpose_selected(12);
            }
            if ui.button("Transpose region down an octave").clicked() {
                self.transpose_selected(-12);
            }
        });
        ui.menu_button("Track", |ui| {
            if ui.button("Add instrument track").clicked() {
                self.add_track("midi");
            }
            if ui.button("Add audio track").clicked() {
                self.add_track("audio");
            }
            ui.separator();
            for (label, bus) in [
                ("Show master strip", MASTER),
                ("Show reverb bus (A)", BUS_A),
                ("Show delay bus (B)", BUS_B),
            ] {
                if ui.button(label).clicked() {
                    self.dispatch(Command::Select {
                        track: Some(bus.into()),
                        clip: None,
                        note: None,
                    });
                }
            }
        });
        ui.menu_button("Mix", |ui| {
            if self.unplaced_recording.is_some() && ui.button("Save recovered take…").clicked() {
                self.save_recovered_take();
                ui.close();
            }
            if ui.button("Reconnect output").clicked() {
                self.connect();
            }
            ui.menu_button("Output device", |ui| {
                let mut chosen: Option<Option<String>> = None;
                if ui
                    .radio(self.output_device.is_none(), "System default")
                    .clicked()
                {
                    chosen = Some(None);
                }
                for name in device::output_devices() {
                    if ui
                        .radio(self.output_device.as_deref() == Some(&name), &name)
                        .clicked()
                    {
                        chosen = Some(Some(name.clone()));
                    }
                }
                if let Some(choice) = chosen {
                    self.output_device = choice;
                    self.connect();
                    ui.close();
                }
            });
            ui.menu_button("Input device", |ui| {
                if ui
                    .radio(self.input_device.is_none(), "System default")
                    .clicked()
                {
                    self.input_device = None;
                    ui.close();
                }
                for name in device::input_devices() {
                    if ui
                        .radio(self.input_device.as_deref() == Some(&name), &name)
                        .clicked()
                    {
                        self.input_device = Some(name.clone());
                        ui.close();
                    }
                }
            });
            ui.menu_button("MIDI input", |ui| {
                if ui.radio(self.midi.is_none(), "None").clicked() {
                    self.midi = None;
                    self.midi_port = None;
                    ui.close();
                }
                let ports = midi::ports();
                if ports.is_empty() {
                    ui.label(text(
                        "No MIDI ports found",
                        FS_SECONDARY,
                        Weight::Medium,
                        DIM,
                    ));
                }
                for name in ports {
                    let active = self.midi.is_some() && self.midi_port.as_deref() == Some(&name);
                    if ui.radio(active, &name).clicked() {
                        self.connect_midi(Some(name.clone()));
                        ui.close();
                    }
                }
            });
            if ui
                .checkbox(&mut self.musical_typing.clone(), "Musical typing")
                .on_hover_text(
                    "Play the selected instrument with the computer keyboard · Cmd/Ctrl+K",
                )
                .clicked()
            {
                self.toggle_musical_typing();
            }
            ui.separator();
            let external = self
                .catalog
                .iter()
                .filter(|d| d.format != Format::Stock)
                .count();
            if ui
                .add_enabled(
                    self.scan_job.is_none(),
                    egui::Button::new(if self.scan_job.is_some() {
                        "Scanning plugins…".to_string()
                    } else {
                        "Rescan plugins".to_string()
                    }),
                )
                .clicked()
            {
                self.scan_plugins();
            }
            ui.label(text(
                format!("{external} external plugins (CLAP, VST3, AU)"),
                FS_SECONDARY,
                Weight::Medium,
                DIM,
            ));
        });
        ui.menu_button("Agent", |ui| self.agent_menu(ui));
        ui.menu_button("View", |ui| {
            if ui.button("Automation").clicked() {
                self.automation.open = true;
            }
            ui.separator();
            let view = self.store.session().view.clone();
            if ui
                .button(if view.follow_playhead {
                    "Stop following playhead"
                } else {
                    "Follow playhead"
                })
                .clicked()
            {
                let mut v = view;
                v.follow_playhead = !v.follow_playhead;
                self.dispatch(Command::SetView(v));
            }
            if ui.button("Fit session").clicked() {
                self.zoom = (700.0 / self.store.session().end_bar() as f32).clamp(12.0, 480.0);
                self.scroll = 0.0;
            }
            if ui.button("Zoom in").clicked() {
                self.zoom = (self.zoom * 1.25).min(480.0);
            }
            if ui.button("Zoom out").clicked() {
                self.zoom = (self.zoom / 1.25).max(12.0);
            }
        });
        ui.menu_button("Help", |ui| {
            if ui.button("Working in Ondera").clicked() {
                self.show_help = true;
            }
            if ui.button("Check for updates…").clicked() {
                self.check_for_updates(true);
            }
            ui.separator();
            ui.label(text(
                format!("Ondera {}", crate::update::current_version()),
                FS_SECONDARY,
                Weight::Medium,
                DIM,
            ));
        });
    }

    pub fn transport(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("transport")
            .exact_height(TRANSPORT)
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                transport_bar(ui.painter(), rect);
                let state = self.store.snapshot();
                let bpb = state.beats_per_bar();
                ui.horizontal_centered(|ui| {
                    ui.add_space(16.0);
                    ui.spacing_mut().item_spacing.x = 6.0;
                    if icon_button(ui, BUTTON, Face::Raised, Icon::Return)
                        .on_hover_text("Return to start · Enter")
                        .clicked()
                    {
                        self.locate(0.0);
                    }
                    if icon_button(ui, BUTTON, Face::Raised, Icon::Rewind)
                        .on_hover_text("Back one bar")
                        .clicked()
                    {
                        self.locate(((self.position / bpb).ceil() - 1.0).max(0.0) * bpb);
                    }
                    if icon_button(ui, BUTTON, Face::Raised, Icon::Forward)
                        .on_hover_text("Forward one bar")
                        .clicked()
                    {
                        self.locate(((self.position / bpb).floor() + 1.0) * bpb);
                    }
                    ui.add_space(8.0);
                    if icon_button(ui, PLAY_BUTTON, Face::from_flag(self.playing), Icon::Play)
                        .on_hover_text("Play / pause · Space")
                        .clicked()
                    {
                        self.play();
                    }
                    if icon_button(ui, BUTTON, Face::Raised, Icon::Stop)
                        .on_hover_text("Stop · 0")
                        .clicked()
                    {
                        self.stop();
                    }
                    if icon_button(
                        ui,
                        BUTTON,
                        Face::lit_flag(self.record_enabled),
                        Icon::Record,
                    )
                    .on_hover_text("Record · R")
                    .clicked()
                    {
                        self.toggle_record();
                    }
                    let mut t = state.transport.clone();
                    let mut changed = false;
                    if icon_button(ui, BUTTON, Face::from_flag(t.cycle), Icon::Cycle)
                        .on_hover_text("Cycle · C")
                        .clicked()
                    {
                        t.cycle = !t.cycle;
                        changed = true;
                    }
                    ui.add_space(8.0);
                    changed |= self.time_display(ui, &mut t, bpb);
                    ui.add_space(8.0);
                    if text_button(ui, "Click", Face::from_flag(t.metronome))
                        .on_hover_text("Metronome · K")
                        .clicked()
                    {
                        t.metronome = !t.metronome;
                        changed = true;
                    }
                    let snap =
                        text_button(ui, &format!("Snap 1/{}", t.snap_division), Face::Raised);
                    egui::Popup::menu(&snap).show(|ui| {
                        for d in [1, 2, 4, 8, 16, 32, 64] {
                            if ui
                                .selectable_label(t.snap_division == d, format!("1/{d}"))
                                .clicked()
                            {
                                t.snap_division = d;
                                changed = true;
                            }
                        }
                    });
                    if changed {
                        self.dispatch(Command::SetTransport(t));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(16.0);
                        ui.spacing_mut().item_spacing.x = 10.0;
                        let working = self.agents.runner_busy();
                        let label = ui.painter().layout_no_wrap(
                            "Agent".into(),
                            font(FS_SECONDARY, Weight::SemiBold),
                            INK,
                        );
                        if button(
                            ui,
                            vec2(11.0 + 7.0 + 7.0 + label.size().x + 11.0, BUTTON.y),
                            Face::from_flag(self.agents.open),
                            R_CONTROL,
                            |p, r, ink| {
                                accent_dot(p, pos2(r.left() + 14.5, r.center().y), 3.5, true);
                                p.galley(
                                    pos2(r.left() + 25.0, r.center().y - label.size().y / 2.0),
                                    label.clone(),
                                    ink,
                                );
                            },
                        )
                        .on_hover_text(if working {
                            "Agent panel · working"
                        } else {
                            "Agent panel"
                        })
                        .clicked()
                        {
                            self.agents.open = !self.agents.open;
                        }
                        ui.add_space(4.0);
                        let (peaks, cpu) = self.device.as_ref().map_or(([0.0; 4], 0.0), |d| {
                            (d.telemetry.peaks(), d.telemetry.load())
                        });
                        ui.label(mono(format!("{:2.0}%", cpu * 100.0), FS_SECONDARY, DIM));
                        let (well, _) = ui.allocate_exact_size(
                            vec2(12.0 * 6.0 + 5.0, LED_CPU_H + 6.0),
                            Sense::hover(),
                        );
                        well_meter(ui.painter(), well, R_BUTTON);
                        led_strip(
                            ui.painter(),
                            well.min + vec2(3.0, 3.0),
                            12,
                            vec2(5.0, LED_CPU_H),
                            false,
                            cpu.clamp(0.0, 1.0),
                        );
                        ui.label(caps("CPU"));
                        ui.add_space(4.0);
                        let master = peaks[0].max(peaks[1]);
                        ui.add_sized(
                            [34.0, 16.0],
                            egui::Label::new(mono(db_text(master), FS_SECONDARY, DIM)),
                        );
                        let (well, _) = ui.allocate_exact_size(
                            vec2(22.0 * 6.0 + 5.0, LED_SEG.y * 2.0 + 8.0),
                            Sense::hover(),
                        );
                        well_meter(ui.painter(), well, R_BUTTON);
                        led_strip(
                            ui.painter(),
                            well.min + vec2(3.0, 3.0),
                            22,
                            LED_SEG,
                            false,
                            level(peaks[0]),
                        );
                        led_strip(
                            ui.painter(),
                            well.min + vec2(3.0, 3.0 + LED_SEG.y + 2.0),
                            22,
                            LED_SEG,
                            false,
                            level(peaks[1]),
                        );
                        let master = ui.label(caps("Master"));
                        if ui
                            .interact(master.rect, ui.id().with("master-strip"), Sense::click())
                            .on_hover_text("Show the master strip")
                            .clicked()
                        {
                            self.dispatch(Command::Select {
                                track: Some(MASTER.into()),
                                clip: None,
                                note: None,
                            });
                        }
                    });
                });
            });
    }
    /// The position / SMPTE / tempo / signature / key readout. Returns whether
    /// the transport changed.
    fn time_display(&mut self, ui: &mut egui::Ui, t: &mut Transport, bpb: f64) -> bool {
        let mut changed = false;
        let (bar, beat, sixteenth, ticks) = position_parts(self.position, bpb);
        let seconds = self.position * 60.0 / t.tempo.max(1.0);
        let smpte = format!(
            "{:02}:{:02}:{:02}:{:02}",
            (seconds / 3600.0) as u64,
            (seconds / 60.0) as u64 % 60,
            seconds as u64 % 60,
            (seconds.fract() * 30.0) as u64
        );
        let tempo = format!("{:.2}", t.tempo);
        let sig = format!(
            "{}/{}",
            t.time_signature.numerator, t.time_signature.denominator
        );
        let key = if t.key.is_empty() {
            "—".to_string()
        } else {
            t.key.clone()
        };
        let position = format!("{bar:03} · {beat} · {sixteenth} · {ticks:03}");
        let columns: [(&str, &str); 5] = [
            ("Position", &position),
            ("SMPTE", &smpte),
            ("Tempo", &tempo),
            ("Sig", &sig),
            ("Key", &key),
        ];
        let widths: Vec<f32> = columns
            .iter()
            .map(|(label, value)| {
                let v = ui
                    .painter()
                    .layout_no_wrap(value.to_string(), mono_font(FS_TRANSPORT), INK)
                    .size()
                    .x;
                let l = label.len() as f32 * FS_CAPS * 0.75;
                v.max(l) + 20.0
            })
            .collect();
        let total: f32 = widths.iter().sum::<f32>() + 4.0;
        let (rect, _) = ui.allocate_exact_size(vec2(total, TIME_DISPLAY), Sense::hover());
        let p = ui.painter();
        well_deep(p, rect, R_LG);
        let mut x = rect.left() + 2.0;
        for (i, ((label, value), w)) in columns.iter().zip(&widths).enumerate() {
            let col = Rect::from_min_size(pos2(x, rect.top()), vec2(*w, TIME_DISPLAY));
            caps_at(
                p,
                pos2(x + 10.0, rect.top() + 6.0),
                Align2::LEFT_TOP,
                label,
                FAINT,
            );
            let value_pos = pos2(x + 10.0, rect.top() + 16.0);
            match i {
                0 => {
                    // Digits with dim separators, ticks in ink-300.
                    let mut cx = value_pos.x;
                    for (j, part) in [
                        format!("{bar:03}"),
                        "·".into(),
                        beat.to_string(),
                        "·".into(),
                        sixteenth.to_string(),
                        "·".into(),
                        format!("{ticks:03}"),
                    ]
                    .iter()
                    .enumerate()
                    {
                        let color = match j {
                            1 | 3 | 5 => SEPARATOR,
                            6 => DIM,
                            _ => INK_BRIGHT,
                        };
                        let galley = p.layout_no_wrap(part.clone(), mono_font(FS_TRANSPORT), color);
                        p.galley(pos2(cx, value_pos.y), galley.clone(), color);
                        cx += galley.size().x + 6.0;
                    }
                }
                1 => {
                    p.text(
                        value_pos,
                        Align2::LEFT_TOP,
                        value,
                        mono_font(FS_TRANSPORT),
                        INK_DIM,
                    );
                }
                _ => {
                    p.text(
                        value_pos,
                        Align2::LEFT_TOP,
                        value,
                        mono_font(FS_TRANSPORT),
                        INK_BRIGHT,
                    );
                }
            }
            if i + 1 < columns.len() {
                vline(
                    p,
                    col.right(),
                    rect.top() + 4.0,
                    rect.bottom() - 4.0,
                    white(0.06),
                );
            }
            match i {
                2 => {
                    let response = ui
                        .interact(col, ui.id().with("tempo"), Sense::click_and_drag())
                        .on_hover_text("Drag to change the tempo. Double-click resets 120.");
                    if response.double_clicked() {
                        t.tempo = 120.0;
                        changed = true;
                    } else if response.dragged() {
                        let delta = response.drag_delta();
                        let step = if ui.input(|i| i.modifiers.shift) {
                            0.01
                        } else {
                            0.1
                        };
                        let next = (t.tempo + (delta.x - delta.y) as f64 * step).clamp(20.0, 400.0);
                        if next != t.tempo {
                            t.tempo = next;
                            changed = true;
                        }
                    }
                }
                3 => {
                    let response = ui.interact(col, ui.id().with("signature"), Sense::click());
                    egui::Popup::menu(&response).show(|ui| {
                        for (n, d) in SIGNATURES {
                            let current = t.time_signature.numerator == n
                                && t.time_signature.denominator == d;
                            if ui.selectable_label(current, format!("{n}/{d}")).clicked() {
                                t.time_signature = TimeSignature {
                                    numerator: n,
                                    denominator: d,
                                };
                                changed = true;
                            }
                        }
                    });
                }
                4 => {
                    let response = ui.interact(col, ui.id().with("key"), Sense::click());
                    egui::Popup::menu(&response).show(|ui| {
                        for k in KEYS {
                            if ui.selectable_label(t.key == k, k).clicked() {
                                t.key = k.into();
                                changed = true;
                            }
                        }
                    });
                }
                _ => {}
            }
            x += w;
        }
        changed
    }
    pub fn toggle_record(&mut self) {
        self.record_enabled = !self.record_enabled;
        if self.playing {
            if self.record_enabled {
                self.start_recording();
            } else {
                self.finish_recording();
            }
        }
    }

    pub fn browser(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("browser")
            .exact_width(BROWSER)
            .resizable(false)
            .frame(egui::Frame::new().fill(PANEL))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                {
                    let p = ui.painter();
                    inset(
                        p,
                        Rect::from_min_max(pos2(rect.right() - 6.0, rect.top()), rect.max),
                        0.0,
                        Side::Right,
                        6.0,
                        black(0.25),
                    );
                    vline(p, rect.right() - 1.0, rect.top(), rect.bottom(), black(0.6));
                }
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.add_space(10.0);
                    if let Some(tab) = segmented(
                        ui,
                        &["Instr", "Loops", "Plugins", "Files"],
                        self.browser_tab,
                        (BROWSER - 24.0) / 4.0,
                    ) {
                        self.browser_tab = tab;
                        self.browser_selected = None;
                    }
                });
                ui.add_space(8.0);
                let (search, _) = ui.allocate_exact_size(vec2(BROWSER, 26.0), Sense::hover());
                let search = search.shrink2(vec2(10.0, 0.0));
                groove_shallow(ui.painter(), search, R_CONTROL);
                icon(
                    ui.painter(),
                    Rect::from_min_size(search.min + vec2(8.0, 7.0), Vec2::splat(12.0)),
                    Icon::Search,
                    FAINT,
                );
                let hint = match self.browser_tab {
                    0 => "Search instruments",
                    1 => "Search loops",
                    2 => "Search plugins",
                    _ => "Search files",
                };
                ui.scope_builder(
                    egui::UiBuilder::new().max_rect(Rect::from_min_max(
                        search.min + vec2(24.0, 3.0),
                        search.max - vec2(4.0, 3.0),
                    )),
                    |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.browser_filter)
                                .frame(false)
                                .font(font(FS_LIST, Weight::Medium))
                                .text_color(INK)
                                .hint_text(text(hint, FS_LIST, Weight::Medium, FAINT))
                                .desired_width(f32::INFINITY),
                        );
                    },
                );
                ui.add_space(10.0);
                let filter = self.browser_filter.to_lowercase();
                let bottom_h = 42.0;
                let list_h = (ui.available_height() - bottom_h).max(40.0);
                let mut load: Option<(usize, String, String)> = None;
                let mut scan = false;
                egui::ScrollArea::vertical()
                    .max_height(list_h)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_min_width(BROWSER - 12.0);
                        let tab = self.browser_tab;
                        let sources: Vec<(String, String)> = self
                            .store
                            .session()
                            .sources
                            .values()
                            .map(|s| (s.name.clone(), format!("{:.1} s", s.duration_seconds)))
                            .collect();
                        let groups: Vec<(String, Vec<BrowserItem>)> = match tab {
                            0 => plugin_groups(&self.catalog_entries(true), true),
                            1 => vec![(
                                "Loops".into(),
                                LOOPS
                                    .iter()
                                    .enumerate()
                                    .map(|(i, n)| {
                                        (n.to_string(), "midi".into(), TRACKS[i % 8], String::new())
                                    })
                                    .collect(),
                            )],
                            2 => plugin_groups(&self.catalog_entries(false), false),
                            _ => vec![(
                                "Project audio".into(),
                                sources
                                    .into_iter()
                                    .map(|(n, d)| (n, d, TRACKS[4], String::new()))
                                    .collect(),
                            )],
                        };
                        let mut any = 0;
                        for (group, items) in groups {
                            let visible: Vec<&BrowserItem> = items
                                .iter()
                                .filter(|(name, meta, _, _)| {
                                    filter.is_empty()
                                        || name.to_lowercase().contains(&filter)
                                        || meta.to_lowercase().contains(&filter)
                                })
                                .collect();
                            if visible.is_empty() {
                                continue;
                            }
                            any += visible.len();
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                ui.add_space(14.0);
                                ui.label(caps(&group));
                            });
                            ui.add_space(4.0);
                            for (name, meta, dot, plugin_id) in visible {
                                let (row, response) = ui.allocate_exact_size(
                                    vec2(BROWSER - 12.0, 24.0),
                                    Sense::click(),
                                );
                                let row = Rect::from_min_max(
                                    pos2(row.left() + 6.0, row.top()),
                                    pos2(row.right() - 6.0, row.bottom()),
                                );
                                let key = if plugin_id.is_empty() {
                                    name.clone()
                                } else {
                                    plugin_id.clone()
                                };
                                let selected = self.browser_selected.as_deref() == Some(&key);
                                let p = ui.painter();
                                if selected || response.hovered() {
                                    p.rect_filled(
                                        row,
                                        R_MD,
                                        white(if selected { 0.07 } else { 0.04 }),
                                    );
                                }
                                swatch(
                                    p,
                                    Rect::from_center_size(
                                        pos2(row.left() + 12.0, row.center().y),
                                        Vec2::splat(8.0),
                                    ),
                                    *dot,
                                );
                                let meta_galley =
                                    p.layout_no_wrap(meta.clone(), mono_font(FS_SMALL), FAINT);
                                let name_clip = Rect::from_min_max(
                                    pos2(row.left() + 25.0, row.top()),
                                    pos2(row.right() - 12.0 - meta_galley.size().x, row.bottom()),
                                );
                                p.with_clip_rect(name_clip).text(
                                    pos2(row.left() + 25.0, row.center().y),
                                    Align2::LEFT_CENTER,
                                    name,
                                    font(FS_LIST, Weight::Medium),
                                    INK_CONTROL,
                                );
                                p.text(
                                    pos2(row.right() - 8.0, row.center().y),
                                    Align2::RIGHT_CENTER,
                                    meta,
                                    mono_font(FS_SMALL),
                                    FAINT,
                                );
                                if response.clicked() {
                                    self.browser_selected = Some(key.clone());
                                }
                                if response.double_clicked() {
                                    load = Some((tab, plugin_id.clone(), name.clone()));
                                }
                                response.on_hover_text(if plugin_id.is_empty() {
                                    name.clone()
                                } else {
                                    format!("{name}\n{plugin_id}")
                                });
                            }
                        }
                        if any == 0 {
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                ui.add_space(14.0);
                                ui.label(text(
                                    if tab == 3 {
                                        "No audio yet"
                                    } else {
                                        "Nothing matches"
                                    },
                                    FS_SECONDARY,
                                    Weight::Medium,
                                    FAINT,
                                ));
                            });
                        }
                        ui.add_space(14.0);
                        ui.horizontal(|ui| {
                            ui.add_space(14.0);
                            ui.label(text(
                                match tab {
                                    3 => "Drop audio files anywhere to import",
                                    2 => "Double-click to insert on the selected strip",
                                    _ => "Double-click to load onto the selected track",
                                },
                                FS_SMALL,
                                Weight::Medium,
                                FAINT,
                            ));
                        });
                        if tab == 3 {
                            ui.add_space(10.0);
                            ui.horizontal(|ui| {
                                ui.add_space(14.0);
                                if text_button(ui, "Import audio…", Face::Raised).clicked() {
                                    self.import(None);
                                }
                            });
                        }
                        if tab == 0 || tab == 2 {
                            ui.add_space(10.0);
                            ui.horizontal(|ui| {
                                ui.add_space(14.0);
                                let label = if self.scan_job.is_some() {
                                    "Scanning…"
                                } else {
                                    "Scan plugins"
                                };
                                if text_button(ui, label, Face::from_flag(self.scan_job.is_some()))
                                    .on_hover_text("Find CLAP, VST3 and Audio Unit plugins")
                                    .clicked()
                                    && self.scan_job.is_none()
                                {
                                    scan = true;
                                }
                                let external = self
                                    .catalog
                                    .iter()
                                    .filter(|d| d.format != Format::Stock)
                                    .count();
                                ui.add_space(8.0);
                                ui.label(mono(format!("{external} external"), FS_SMALL, FAINT));
                            });
                        }
                    });
                if scan {
                    self.scan_plugins();
                }
                if let Some((tab, plugin_id, name)) = load {
                    match tab {
                        0 => self.instrument(&plugin_id, &name),
                        1 => self.add_loop(&name),
                        2 => self.add_effect(&plugin_id, &name),
                        _ => {}
                    }
                }
                // Preview bar.
                let bar = Rect::from_min_max(pos2(rect.left(), rect.bottom() - bottom_h), rect.max);
                let p = ui.painter();
                hline(p, bar.left(), bar.right() - 1.0, bar.top(), black(0.5));
                hline(
                    p,
                    bar.left(),
                    bar.right() - 1.0,
                    bar.top() + 1.0,
                    white(0.04),
                );
                ui.scope_builder(
                    egui::UiBuilder::new().max_rect(bar.shrink2(vec2(10.0, 10.0))),
                    |ui| {
                        ui.horizontal_centered(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            if button(ui, vec2(26.0, 22.0), Face::Raised, R_MD, |p, r, ink| {
                                let c = r.center();
                                p.add(egui::Shape::convex_polygon(
                                    vec![
                                        pos2(c.x - 3.5, c.y - 4.5),
                                        pos2(c.x + 4.5, c.y),
                                        pos2(c.x - 3.5, c.y + 4.5),
                                    ],
                                    ink,
                                    egui::Stroke::NONE,
                                ));
                            })
                            .on_hover_text("Preview on the selected instrument track")
                            .clicked()
                            {
                                self.preview_selection();
                            }
                            ui.label(text("Preview", FS_SECONDARY, Weight::Medium, DIM));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let label = self
                                        .browser_selected
                                        .clone()
                                        .map(|k| {
                                            self.catalog
                                                .iter()
                                                .find(|d| d.id == k)
                                                .map_or(k, |d| d.name.clone())
                                        })
                                        .unwrap_or_default();
                                    ui.label(mono(truncate(&label, 18), FS_VALUE, FAINT));
                                },
                            );
                        });
                    },
                );
            });
    }
    fn preview_selection(&mut self) {
        let Some(track) = self
            .store
            .session()
            .tracks
            .iter()
            .find(|t| Some(&t.id) == self.store.session().view.selected_track_id.as_ref())
            .filter(|t| t.kind == "midi")
            .map(|t| t.id.clone())
        else {
            self.status = "Select an instrument track to preview".into();
            return;
        };
        self.preview(&track, 60, 100);
    }
    /// Snap the selected region's notes to the grid.
    pub(crate) fn quantize_selected(&mut self) {
        let s = self.store.session();
        let step = 4.0 / s.transport.snap_division as f64;
        let Some(clip) = s
            .clips
            .iter()
            .find(|c| Some(&c.id) == s.view.selected_clip_id.as_ref())
        else {
            return;
        };
        let mut clip = clip.clone();
        if let ClipData::Midi { notes } = &mut clip.data {
            for n in notes.iter_mut() {
                n.start = (n.start / step).round() * step;
            }
            self.dispatch(Command::PutClip(clip));
        }
    }
    pub(crate) fn transpose_selected(&mut self, semitones: i32) {
        let s = self.store.session();
        let Some(clip) = s
            .clips
            .iter()
            .find(|c| Some(&c.id) == s.view.selected_clip_id.as_ref())
        else {
            return;
        };
        let mut clip = clip.clone();
        if let ClipData::Midi { notes } = &mut clip.data {
            for n in notes.iter_mut() {
                n.pitch = (n.pitch as i32 + semitones).clamp(0, 127) as u8;
            }
            self.dispatch(Command::PutClip(clip));
        }
    }

    pub fn inspector(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("inspector")
            .exact_width(INSPECTOR)
            .resizable(false)
            .frame(egui::Frame::new().fill(PANEL))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                {
                    let p = ui.painter();
                    inset(
                        p,
                        Rect::from_min_max(rect.min, pos2(rect.left() + 6.0, rect.bottom())),
                        0.0,
                        Side::Left,
                        6.0,
                        black(0.25),
                    );
                    vline(p, rect.left(), rect.top(), rect.bottom(), black(0.6));
                }
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_min_width(INSPECTOR);
                        let s = self.store.snapshot();
                        let selected = s.view.selected_track_id.clone();
                        if let Some(bus) = selected.as_deref().filter(|id| is_bus(id)) {
                            self.bus_strip(ui, rect, &s, bus);
                            return;
                        }
                        let Some((index, t)) = s
                            .tracks
                            .iter()
                            .enumerate()
                            .find(|(_, t)| Some(&t.id) == selected.as_ref())
                        else {
                            ui.add_space(14.0);
                            ui.horizontal(|ui| {
                                ui.add_space(14.0);
                                ui.label(text("Select a track", FS_BODY, Weight::SemiBold, DIM));
                            });
                            return;
                        };
                        self.track_strip(ui, rect, &s, index, t);
                    });
            });
    }
    fn divider(&self, ui: &mut egui::Ui, rect: Rect) {
        let y = ui.cursor().top();
        hline(ui.painter(), rect.left() + 1.0, rect.right(), y, black(0.5));
        ui.add_space(1.0);
    }
    fn bus_strip(&mut self, ui: &mut egui::Ui, rect: Rect, s: &Session, bus: &str) {
        let mut strip = s.strips.get(bus).cloned().unwrap_or_default();
        let mut changed = false;
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            let (sw, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
            swatch(
                ui.painter(),
                sw,
                if bus == MASTER { ACCENT } else { NEUTRAL_DOT },
            );
            ui.add_space(9.0);
            ui.label(text(bus_name(bus), FS_PANEL_TITLE, Weight::Bold, INK));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(14.0);
                ui.label(mono(
                    if bus == MASTER { "MASTER" } else { "AUX BUS" },
                    FS_SMALL,
                    FAINT,
                ));
            });
        });
        ui.add_space(10.0);
        self.divider(ui, rect);
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label(text(
                match bus {
                    MASTER => "Everything sums here before the output.",
                    BUS_A => "Fed by each track's send A.",
                    _ => "Fed by each track's send B.",
                },
                FS_SECONDARY,
                Weight::Medium,
                DIM,
            ));
        });
        ui.add_space(10.0);
        self.divider(ui, rect);
        self.inserts_section(ui, rect, bus, &mut strip, &mut changed);
        if changed {
            self.dispatch(Command::SetStrip {
                track: bus.into(),
                strip,
            });
        }
        if bus == MASTER {
            ui.add_space(8.0);
            self.divider(ui, rect);
            ui.add_space(10.0);
            let peaks = self
                .device
                .as_ref()
                .map_or([0.0; 4], |d| d.telemetry.peaks());
            let mut volume = s.master_volume;
            let mut changed = false;
            ui.horizontal_top(|ui| {
                ui.add_space(14.0);
                ui.spacing_mut().item_spacing = vec2(14.0, 8.0);
                ui.vertical(|ui| {
                    ui.set_width(44.0);
                    ui.vertical_centered(|ui| {
                        ui.label(caps("Vol"));
                        let (well, _) = ui.allocate_exact_size(vec2(44.0, 18.0), Sense::hover());
                        well_value(ui.painter(), well, 3.0);
                        ui.painter().text(
                            well.center(),
                            Align2::CENTER_CENTER,
                            db_text(fader_gain(volume)),
                            mono_font(FS_SECONDARY),
                            INK,
                        );
                    });
                });
                ui.add_space(6.0);
                ui.vertical(|ui| {
                    ui.add_space(6.0);
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 14.0;
                        if fader(ui, &mut volume, FADER_H)
                            .on_hover_text("Master fader")
                            .changed()
                        {
                            changed = true;
                        }
                        let (well, _) = ui.allocate_exact_size(
                            vec2(5.0 * 2.0 + 2.0 + 6.0, FADER_H),
                            Sense::hover(),
                        );
                        well_meter(ui.painter(), well, R_BUTTON);
                        let seg = vec2(5.0, (FADER_H - 6.0 - 19.0) / 20.0);
                        led_strip(
                            ui.painter(),
                            pos2(well.left() + 3.0, well.bottom() - 3.0),
                            20,
                            seg,
                            true,
                            level(peaks[0]),
                        );
                        led_strip(
                            ui.painter(),
                            pos2(well.left() + 9.0, well.bottom() - 3.0),
                            20,
                            seg,
                            true,
                            level(peaks[1]),
                        );
                    });
                });
            });
            if changed {
                self.dispatch(Command::SetMasterVolume(volume));
            }
        }
        ui.add_space(12.0);
    }
    /// The insert chain of any strip: eight slots, click to edit, LED to bypass.
    fn inserts_section(
        &mut self,
        ui: &mut egui::Ui,
        _rect: Rect,
        strip_id: &str,
        strip: &mut Strip,
        changed: &mut bool,
    ) {
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label(caps("Inserts"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(14.0);
                let used = strip.inserts.iter().filter(|i| !i.is_empty()).count();
                ui.label(mono(format!("{used} / {MAX_INSERTS}"), FS_CAPS, DIM));
            });
        });
        ui.add_space(7.0);
        strip.inserts.retain(|i| !i.is_empty());
        let count = strip.inserts.len();
        let mut open: Option<String> = None;
        let mut native: Option<String> = None;
        let mut remove: Option<usize> = None;
        let mut swap: Option<(usize, usize)> = None;
        let mut add: Option<(String, String)> = None;
        for i in 0..count {
            let insert = strip.inserts[i].clone();
            let (r, response) = ui.allocate_exact_size(vec2(INSPECTOR, 24.0), Sense::click());
            let r = Rect::from_min_max(
                pos2(r.left() + 14.0, r.top()),
                pos2(r.right() - 14.0, r.bottom()),
            );
            let p = ui.painter();
            insert_face(p, r, R_MD);
            let active = insert.state == "active";
            let led_center = pos2(r.left() + 12.0, r.center().y);
            let led_rect = Rect::from_center_size(led_center, Vec2::splat(12.0));
            if active {
                p.circle_filled(led_center, 6.0, LED_GLOW.gamma_multiply(0.18));
                p.circle_filled(led_center, 3.5, LED);
            } else {
                p.circle_filled(led_center, 3.5, KNOB_LO);
                p.circle_stroke(led_center, 3.5, egui::Stroke::new(1.0, black(0.6)));
            }
            let loaded = self.plugins.loaded.contains_key(&insert.id);
            let failed = self.plugins.failed.get(&insert.id).cloned();
            let format_tag = Format::parse(&insert.plugin_id())
                .map(|(f, _)| format_icon(f))
                .unwrap_or("?");
            let tag = if failed.is_some() {
                "failed".to_string()
            } else if !loaded {
                "loading".to_string()
            } else if active {
                format_tag.to_string()
            } else {
                "bypassed".to_string()
            };
            let tag_galley = p.layout_no_wrap(tag.clone(), mono_font(FS_CAPS), FAINT);
            p.with_clip_rect(Rect::from_min_max(
                pos2(r.left() + 24.0, r.top()),
                pos2(r.right() - 12.0 - tag_galley.size().x, r.bottom()),
            ))
            .text(
                pos2(r.left() + 24.0, r.center().y),
                Align2::LEFT_CENTER,
                &insert.name,
                font(FS_SECONDARY, Weight::SemiBold),
                if failed.is_some() {
                    FAINT
                } else if active {
                    INK
                } else {
                    DIM
                },
            );
            p.text(
                pos2(r.right() - 8.0, r.center().y),
                Align2::RIGHT_CENTER,
                tag,
                mono_font(FS_CAPS),
                if failed.is_some() { ACCENT } else { FAINT },
            );
            let led_response =
                ui.interact(led_rect, ui.id().with(("insert-led", i)), Sense::click());
            if led_response.on_hover_text("Bypass").clicked() {
                strip.inserts[i].state = if active { "bypassed" } else { "active" }.into();
                *changed = true;
            } else if response.clicked() {
                open = Some(insert.id.clone());
            }
            let response = if let Some(e) = &failed {
                response.on_hover_text(e)
            } else {
                response.on_hover_text("Click for parameters · right-click for options")
            };
            response.context_menu(|ui| {
                if ui.button("Parameters").clicked() {
                    open = Some(insert.id.clone());
                    ui.close();
                }
                let has_gui = self
                    .plugins
                    .loaded
                    .get(&insert.id)
                    .is_some_and(|l| l.editor.has_gui());
                if has_gui && ui.button("Open plugin window").clicked() {
                    native = Some(insert.id.clone());
                    ui.close();
                }
                ui.separator();
                if i > 0 && ui.button("Move up").clicked() {
                    swap = Some((i, i - 1));
                    ui.close();
                }
                if i + 1 < count && ui.button("Move down").clicked() {
                    swap = Some((i, i + 1));
                    ui.close();
                }
                if ui.button("Remove").clicked() {
                    remove = Some(i);
                    ui.close();
                }
            });
            ui.add_space(4.0);
        }
        if count < MAX_INSERTS {
            let (r, response) = ui.allocate_exact_size(vec2(INSPECTOR, 24.0), Sense::click());
            let r = Rect::from_min_max(
                pos2(r.left() + 14.0, r.top()),
                pos2(r.right() - 14.0, r.bottom()),
            );
            let p = ui.painter();
            groove_send(p, r, R_MD);
            p.circle_stroke(
                pos2(r.left() + 12.0, r.center().y),
                3.5,
                egui::Stroke::new(1.0, white(0.12)),
            );
            p.text(
                pos2(r.left() + 24.0, r.center().y),
                Align2::LEFT_CENTER,
                "Add effect…",
                font(FS_SECONDARY, Weight::SemiBold),
                if response.hovered() { INK } else { FAINT },
            );
            let effects = self.catalog_entries(false);
            egui::Popup::menu(&response).show(|ui| {
                ui.set_min_width(220.0);
                for (group, items) in plugin_groups(&effects, false) {
                    ui.menu_button(group, |ui| {
                        for (name, meta, _, plugin_id) in items {
                            if ui
                                .button(if meta.is_empty() {
                                    name.clone()
                                } else {
                                    format!("{name}  ·  {meta}")
                                })
                                .clicked()
                            {
                                add = Some((plugin_id, name));
                                ui.close();
                            }
                        }
                    });
                }
            });
        }
        if let Some((a, b)) = swap {
            strip.inserts.swap(a, b);
            *changed = true;
        }
        if let Some(i) = remove {
            let removed = strip.inserts.remove(i);
            self.plugins.windows.remove(&removed.id);
            *changed = true;
        }
        if let Some((plugin_id, name)) = add {
            let insert = Insert::new(crate::app::id("insert"), &plugin_id, &name);
            open = Some(insert.id.clone());
            strip.inserts.push(insert);
            *changed = true;
        }
        if let Some(key) = open {
            self.open_plugin_window(&key);
        }
        if let Some(key) = native {
            self.open_plugin_window(&key);
            self.toggle_native_window_public(&key);
        }
        let _ = strip_id;
    }
    fn track_strip(&mut self, ui: &mut egui::Ui, rect: Rect, s: &Session, index: usize, t: &Track) {
        let color = track_color(&t.color, index);
        let mut track = t.clone();
        let is_midi = track.kind == "midi";
        // Header: swatch, name, kind.
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            let (sw, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
            swatch(ui.painter(), sw, color);
            ui.add_space(9.0);
            let width = INSPECTOR - 14.0 - 12.0 - 9.0 - 74.0 - 14.0;
            if inline_edit(
                ui,
                "track-name",
                &mut track.name,
                font(FS_PANEL_TITLE, Weight::Bold),
                INK,
                width,
            )
            .changed()
                && track.name != t.name
            {
                self.dispatch(Command::UpdateTrack(track.clone()));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(14.0);
                ui.label(mono(
                    if is_midi {
                        format!("MIDI · Ch {}", index + 1)
                    } else {
                        "AUDIO".into()
                    },
                    FS_SMALL,
                    FAINT,
                ));
            });
        });
        ui.add_space(10.0);
        self.divider(ui, rect);
        // Routing rows.
        let mut strip = s.strips.get(&track.id).cloned().unwrap_or_default();
        let mut strip_changed = false;
        ui.add_space(10.0);
        let row =
            |ui: &mut egui::Ui, label: &str, value: &str, clickable: bool| -> egui::Response {
                let (r, response) = ui.allocate_exact_size(
                    vec2(INSPECTOR, 17.0),
                    if clickable {
                        Sense::click()
                    } else {
                        Sense::hover()
                    },
                );
                let p = ui.painter();
                p.text(
                    pos2(r.left() + 14.0, r.center().y),
                    Align2::LEFT_CENTER,
                    label,
                    font(FS_SECONDARY, Weight::Medium),
                    FAINT,
                );
                let value_clip = Rect::from_min_max(
                    pos2(r.left() + 80.0, r.top()),
                    pos2(r.right() - 14.0, r.bottom()),
                );
                p.with_clip_rect(value_clip).text(
                    pos2(r.right() - 14.0, r.center().y),
                    Align2::RIGHT_CENTER,
                    value,
                    font(FS_SECONDARY, Weight::SemiBold),
                    if clickable && response.hovered() {
                        ACCENT
                    } else {
                        INK
                    },
                );
                response
            };
        let mut synth_open: Option<String> = None;
        let mut synth_native: Option<String> = None;
        if is_midi {
            let name = strip.instrument_name();
            let current_id = strip
                .synth
                .as_ref()
                .map(|s| s.plugin_id())
                .unwrap_or_else(|| format!("stock:{}", strip.instrument));
            let response = row(ui, "Instrument", &name, true);
            let instruments = self.catalog_entries(true);
            egui::Popup::menu(&response).show(|ui| {
                ui.set_min_width(220.0);
                for (group, items) in plugin_groups(&instruments, true) {
                    ui.menu_button(group, |ui| {
                        for (label, meta, _, plugin_id) in items {
                            if ui
                                .selectable_label(
                                    current_id == plugin_id,
                                    if meta.is_empty() {
                                        label.clone()
                                    } else {
                                        format!("{label}  ·  {meta}")
                                    },
                                )
                                .clicked()
                            {
                                self.set_instrument(&track.id, &plugin_id, &label);
                                ui.close();
                            }
                        }
                    });
                }
            });
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                ui.spacing_mut().item_spacing.x = 6.0;
                let key = strip.synth_key(&track.id);
                if text_button(ui, "Instrument parameters", Face::Raised).clicked() {
                    synth_open = Some(key.clone());
                }
                let has_gui = self
                    .plugins
                    .loaded
                    .get(&key)
                    .is_some_and(|l| l.editor.has_gui());
                if has_gui && text_button(ui, "Window", Face::Raised).clicked() {
                    synth_native = Some(key);
                }
            });
            ui.add_space(4.0);
            row(
                ui,
                "Input",
                if self.midi.is_some() {
                    "MIDI + typing"
                } else {
                    "Musical typing"
                },
                false,
            );
        } else {
            row(
                ui,
                "Input",
                self.input_device.as_deref().unwrap_or("Default input"),
                false,
            );
        }
        row(ui, "Output", "Stereo Out", false);
        ui.add_space(10.0);
        self.divider(ui, rect);
        // Channel EQ overview drawn from the first Channel EQ insert's parameters.
        let eq = strip
            .inserts
            .iter()
            .find(|i| i.plugin_id() == "stock:Channel EQ" && !i.is_empty());
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label(caps("Channel EQ"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(14.0);
                ui.label(mono(
                    match eq.map(|i| i.state.as_str()) {
                        Some("active") => "3 bands",
                        Some(_) => "bypassed",
                        None => "not inserted",
                    },
                    FS_CAPS,
                    DIM,
                ));
            });
        });
        ui.add_space(6.0);
        let (well, well_response) = ui.allocate_exact_size(vec2(INSPECTOR, 74.0), Sense::click());
        let well = Rect::from_min_max(
            pos2(well.left() + 14.0, well.top()),
            pos2(well.right() - 14.0, well.bottom()),
        );
        let defaults = [1.5, 120.0, -2.0, 1000.0, 0.8, 2.0, 6000.0];
        let values: [f64; 7] = std::array::from_fn(|i| {
            eq.and_then(|e| e.params.get(&(i as u32)).copied())
                .unwrap_or(defaults[i])
        });
        eq_well(
            ui.painter(),
            well,
            color,
            eq.is_some_and(|i| i.state == "active"),
            &values,
        );
        if well_response
            .on_hover_text("Click to edit the Channel EQ")
            .clicked()
        {
            match eq {
                Some(e) => self.open_plugin_window(&e.id.clone()),
                None => self.add_effect_to(&track.id, "stock:Channel EQ", "Channel EQ"),
            }
        }
        ui.add_space(8.0);
        self.divider(ui, rect);
        self.inserts_section(ui, rect, &track.id, &mut strip, &mut strip_changed);
        ui.add_space(8.0);
        self.divider(ui, rect);
        // Sends.
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label(caps("Sends"));
        });
        ui.add_space(8.0);
        while strip.sends.len() < 2 {
            strip.sends.push(Send {
                level_db: None,
                name: if strip.sends.is_empty() {
                    "A · Reverb"
                } else {
                    "B · Delay"
                }
                .into(),
            });
        }
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.spacing_mut().item_spacing.x = 10.0;
            let card_w = (INSPECTOR - 28.0 - 10.0) / 2.0;
            for (i, (name, bus)) in [("A · Reverb", BUS_A), ("B · Delay", BUS_B)]
                .iter()
                .enumerate()
            {
                let (card, card_response) =
                    ui.allocate_exact_size(vec2(card_w, 40.0), Sense::click());
                groove_send(ui.painter(), card, R_CONTROL);
                if card_response
                    .on_hover_text("Click the name to open the bus strip")
                    .double_clicked()
                {
                    self.dispatch(Command::Select {
                        track: Some((*bus).into()),
                        clip: None,
                        note: None,
                    });
                }
                let mut db = strip.sends[i].level_db.unwrap_or(-60.0);
                ui.scope_builder(
                    egui::UiBuilder::new().max_rect(card.shrink2(vec2(5.0, 6.0))),
                    |ui| {
                        ui.horizontal_centered(|ui| {
                            ui.spacing_mut().item_spacing.x = 5.0;
                            if knob_widget(ui, &mut db, -60.0..=0.0, -60.0, KNOB_MD)
                                .on_hover_text("Drag to set the send level")
                                .changed()
                            {
                                strip.sends[i].level_db = if db <= -59.0 { None } else { Some(db) };
                                strip_changed = true;
                            }
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 1.0;
                                ui.add(
                                    egui::Label::new(text(*name, FS_VALUE, Weight::SemiBold, INK))
                                        .truncate(),
                                );
                                ui.add(
                                    egui::Label::new(mono(
                                        match strip.sends[i].level_db {
                                            Some(v) => format!("{v:.1} dB").replace('-', "−"),
                                            None => "−∞ dB".into(),
                                        },
                                        FS_CAPS,
                                        FAINT,
                                    ))
                                    .truncate(),
                                );
                            });
                        });
                    },
                );
            }
        });
        if strip_changed {
            self.dispatch(Command::SetStrip {
                track: track.id.clone(),
                strip,
            });
        }
        if let Some(key) = synth_open {
            self.open_plugin_window(&key);
        }
        if let Some(key) = synth_native {
            self.open_plugin_window(&key);
            self.toggle_native_window_public(&key);
        }
        ui.add_space(10.0);
        self.divider(ui, rect);
        // Pan, volume, fader and meters.
        ui.add_space(10.0);
        let peaks = self
            .device
            .as_ref()
            .map_or([0.0; 4], |d| d.telemetry.peaks());
        let mut track_changed = false;
        ui.horizontal_top(|ui| {
            ui.add_space(14.0);
            ui.spacing_mut().item_spacing = vec2(14.0, 8.0);
            ui.vertical(|ui| {
                ui.set_width(44.0);
                ui.spacing_mut().item_spacing.y = 8.0;
                ui.vertical_centered(|ui| {
                    ui.label(caps("Pan"));
                    let mut pan = track.pan;
                    if knob_widget(ui, &mut pan, -100.0..=100.0, 0.0, KNOB_LG).changed() {
                        track.pan = pan.round();
                        track_changed = true;
                    }
                    let pan_text = if track.pan.abs() < 0.5 {
                        "C".to_string()
                    } else if track.pan < 0.0 {
                        format!("L {}", track.pan.abs().round())
                    } else {
                        format!("R {}", track.pan.round())
                    };
                    ui.label(mono(pan_text, FS_SMALL, DIM));
                    ui.add_space(FADER_H - 36.0 - 4.0 - 12.0 - 12.0 - 8.0 - 26.0 - 8.0 - 20.0);
                    ui.label(caps("Vol"));
                    let (well, _) = ui.allocate_exact_size(vec2(44.0, 18.0), Sense::hover());
                    well_value(ui.painter(), well, 3.0);
                    ui.painter().text(
                        well.center(),
                        Align2::CENTER_CENTER,
                        db_text(fader_gain(track.volume)),
                        mono_font(FS_SECONDARY),
                        INK,
                    );
                });
            });
            ui.add_space(6.0);
            ui.vertical(|ui| {
                ui.add_space(6.0);
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 14.0;
                    let mut volume = track.volume;
                    if fader(ui, &mut volume, FADER_H)
                        .on_hover_text("Channel fader")
                        .changed()
                    {
                        track.volume = volume;
                        track_changed = true;
                    }
                    let (well, _) = ui
                        .allocate_exact_size(vec2(5.0 * 2.0 + 2.0 + 6.0, FADER_H), Sense::hover());
                    well_meter(ui.painter(), well, R_BUTTON);
                    let seg = vec2(5.0, (FADER_H - 6.0 - 19.0) / 20.0);
                    led_strip(
                        ui.painter(),
                        pos2(well.left() + 3.0, well.bottom() - 3.0),
                        20,
                        seg,
                        true,
                        level(peaks[2]),
                    );
                    led_strip(
                        ui.painter(),
                        pos2(well.left() + 9.0, well.bottom() - 3.0),
                        20,
                        seg,
                        true,
                        level(peaks[3]),
                    );
                    let (scale, _) = ui.allocate_exact_size(vec2(18.0, FADER_H), Sense::hover());
                    let labels = ["+6", "0", "−6", "−12", "−24", "−∞"];
                    for (i, l) in labels.iter().enumerate() {
                        let y = scale.top()
                            + 4.0
                            + (scale.height() - 8.0) * i as f32 / (labels.len() - 1) as f32;
                        ui.painter().text(
                            pos2(scale.left(), y),
                            Align2::LEFT_CENTER,
                            *l,
                            mono_font(FS_MICRO),
                            FAINT,
                        );
                    }
                });
            });
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.spacing_mut().item_spacing.x = 3.0;
            if toggle_small(ui, "M", track.mute, false)
                .on_hover_text("Mute · M")
                .clicked()
            {
                track.mute = !track.mute;
                track_changed = true;
            }
            if toggle_small(ui, "S", track.solo, false)
                .on_hover_text("Solo · S")
                .clicked()
            {
                track.solo = !track.solo;
                track_changed = true;
            }
            if toggle_small(ui, "R", track.armed, true)
                .on_hover_text("Record arm · A")
                .clicked()
            {
                track.armed = !track.armed;
                track_changed = true;
            }
            ui.add_space(8.0);
            ui.label(text("Stereo Out", FS_SECONDARY, Weight::Medium, FAINT));
        });
        if track_changed {
            self.dispatch(Command::UpdateTrack(track.clone()));
        }
        // Region.
        if let Some(c) = s
            .clips
            .iter()
            .find(|c| Some(&c.id) == s.view.selected_clip_id.as_ref())
        {
            ui.add_space(12.0);
            self.divider(ui, rect);
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                ui.label(caps("Region"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(14.0);
                    ui.label(mono(
                        match &c.data {
                            ClipData::Midi { notes } => format!("{} notes", notes.len()),
                            ClipData::Audio { .. } => "audio".into(),
                        },
                        FS_CAPS,
                        FAINT,
                    ));
                });
            });
            ui.add_space(6.0);
            let mut clip = c.clone();
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                let (well, _) =
                    ui.allocate_exact_size(vec2(INSPECTOR - 28.0, 24.0), Sense::hover());
                well_input(ui.painter(), well, R_MD);
                ui.scope_builder(
                    egui::UiBuilder::new().max_rect(well.shrink2(vec2(4.0, 2.0))),
                    |ui| {
                        if inline_edit(
                            ui,
                            "clip-name",
                            &mut clip.name,
                            font(FS_LIST, Weight::SemiBold),
                            INK,
                            well.width() - 8.0,
                        )
                        .changed()
                            && clip.name != c.name
                        {
                            self.dispatch(Command::PutClip(clip.clone()));
                        }
                    },
                );
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                ui.spacing_mut().item_spacing.x = 10.0;
                let mut start = clip.start_bar + 1.0;
                if value_drag(ui, "Bar", &mut start, 1.0, 1.0..=100000.0) {
                    clip.start_bar = start - 1.0;
                    self.dispatch(Command::PutClip(clip.clone()));
                }
                let mut length = clip.length_bars;
                if value_drag(ui, "Length", &mut length, 0.25, 0.0625..=100000.0) {
                    clip.length_bars = length;
                    self.dispatch(Command::PutClip(clip.clone()));
                }
            });
        }
        ui.add_space(12.0);
        let _ = strip_label;
    }
}

/// Group plugins for menus and the browser: Ondera first (by category for
/// effects), then each external format sorted by vendor and name.
pub fn plugin_groups(entries: &[Descriptor], instruments: bool) -> Vec<(String, Vec<BrowserItem>)> {
    let mut groups: Vec<(String, Vec<BrowserItem>)> = vec![];
    let mut push = |group: String, item: BrowserItem| {
        if let Some((_, items)) = groups.iter_mut().find(|(g, _)| *g == group) {
            items.push(item);
        } else {
            groups.push((group, vec![item]));
        }
    };
    for (stock_index, d) in entries
        .iter()
        .filter(|d| d.format == Format::Stock)
        .enumerate()
    {
        let group = if instruments {
            "Ondera".to_string()
        } else {
            format!("Ondera · {}", d.category)
        };
        push(
            group,
            (
                d.name.clone(),
                String::new(),
                if instruments {
                    TRACKS[stock_index % 8]
                } else {
                    NEUTRAL_DOT
                },
                d.id.clone(),
            ),
        );
    }
    let mut external: Vec<&Descriptor> = entries
        .iter()
        .filter(|d| d.format != Format::Stock)
        .collect();
    external.sort_by(|a, b| {
        (
            a.format.label(),
            a.vendor.to_lowercase(),
            a.name.to_lowercase(),
        )
            .cmp(&(
                b.format.label(),
                b.vendor.to_lowercase(),
                b.name.to_lowercase(),
            ))
    });
    for d in external {
        let group = match d.format {
            Format::Clap => "CLAP",
            Format::Vst3 => "VST3",
            Format::AudioUnit => "Audio Units",
            Format::Stock => "Ondera",
        };
        push(
            group.to_string(),
            (
                d.name.clone(),
                truncate(&d.vendor, 14),
                match d.format {
                    Format::Clap => TRACKS[2],
                    Format::Vst3 => TRACKS[1],
                    _ => TRACKS[5],
                },
                d.id.clone(),
            ),
        );
    }
    groups
}

/// A labelled numeric well that changes by dragging horizontally.
fn value_drag(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f64,
    step: f64,
    range: std::ops::RangeInclusive<f64>,
) -> bool {
    let (r, response) = ui.allocate_exact_size(vec2(96.0, 24.0), Sense::click_and_drag());
    let p = ui.painter();
    well_value(p, r, R_MD);
    p.text(
        pos2(r.left() + 8.0, r.center().y),
        Align2::LEFT_CENTER,
        label,
        font(FS_SECONDARY, Weight::Medium),
        FAINT,
    );
    p.text(
        pos2(r.right() - 8.0, r.center().y),
        Align2::RIGHT_CENTER,
        format!("{value:.2}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string(),
        mono_font(FS_VALUE),
        INK,
    );
    if response.dragged() {
        let delta = (response.drag_delta().x / 12.0) as f64 * step;
        if delta != 0.0 {
            let next = ((*value + delta) / step).round() * step;
            let next = next.clamp(*range.start(), *range.end());
            if next != *value {
                *value = next;
                return true;
            }
        }
    }
    false
}
/// The channel EQ well, drawn from the Channel EQ parameters: low shelf,
/// mid peak and high shelf gains and frequencies.
fn eq_well(p: &egui::Painter, well: Rect, color: egui::Color32, active: bool, v: &[f64; 7]) {
    well_deep(p, well, R_MD);
    let inner = well.shrink(1.0);
    let clip = p.with_clip_rect(inner);
    let mut x = inner.left() + 42.0;
    while x < inner.right() {
        vline(&clip, x, inner.top(), inner.bottom(), white(0.05));
        x += 42.0;
    }
    let zero = inner.top() + inner.height() * 0.5;
    hline(&clip, inner.left(), inner.right(), zero, white(0.08));
    let (low_gain, low_f, mid_gain, mid_f, mid_q, high_gain, high_f) =
        (v[0], v[1], v[2], v[3], v[4].max(0.1), v[5], v[6]);
    let db_at = |t: f32| -> f32 {
        // t is 0..1 across 20 Hz .. 20 kHz on a log axis.
        let hz = 20.0 * (1000.0_f64).powf(t as f64);
        let low = 1.0 / (1.0 + (hz / low_f).powi(2));
        let high = 1.0 - 1.0 / (1.0 + (hz / high_f).powi(2));
        let octaves = (hz / mid_f).log2();
        let mid = (-(octaves * mid_q).powi(2) * 1.5).exp();
        (low_gain * low + mid_gain * mid + high_gain * high) as f32
    };
    let n = 64;
    let points: Vec<egui::Pos2> = (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let db = if active { db_at(t) } else { 0.0 };
            pos2(
                inner.left() + t * inner.width(),
                zero - db.clamp(-15.0, 15.0) * (inner.height() / 32.0),
            )
        })
        .collect();
    if active {
        for w in points.windows(2) {
            let (a, b) = (w[0], w[1]);
            let top = a.y.min(b.y);
            let fill = Rect::from_min_max(pos2(a.x, top), pos2(b.x + 0.5, zero.max(a.y.max(b.y))));
            shade_rect(&clip, fill, 0.0, move |q| {
                let t = ((q.y - top) / (zero - top).max(1.0)).clamp(0.0, 1.0);
                color.gamma_multiply(0.35 * (1.0 - t))
            });
        }
    }
    clip.add(egui::Shape::line(
        points.clone(),
        egui::Stroke::new(1.5, if active { color } else { FAINT }),
    ));
    if active {
        for hz in [low_f, mid_f, high_f] {
            let t = ((hz / 20.0).log(1000.0)).clamp(0.0, 1.0) as f32;
            let i = ((t * n as f32) as usize).min(n);
            p.circle(
                points[i],
                3.0,
                WELL_DEEP,
                egui::Stroke::new(1.5, color.lerp_to_gamma(egui::Color32::WHITE, 0.3)),
            );
        }
    }
}
