//! Window chrome: title bar, transport, browser and inspector.
//!
//! Layout follows `design/Ondera Arrangement.dc.html`. Every persistent edit
//! goes through `store::Command`; the widgets here only paint and interact.

use crate::{
    app::{Intent, Ondera},
    theme::*,
};
use eframe::egui::{self, pos2, vec2, Align2, Rect, Sense, Vec2};
use ondera_engine::{
    dsp::{EFFECTS, INSTRUMENTS},
    model::*,
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
type BrowserItem = (String, String, egui::Color32);
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
                ui.horizontal_centered(|ui| {
                    ui.add_space(14.0);
                    ui.spacing_mut().item_spacing.x = 6.0;
                    ui.spacing_mut().button_padding = vec2(5.0, 3.0);
                    ui.style_mut()
                        .text_styles
                        .insert(egui::TextStyle::Button, font(FS_LIST, Weight::Medium));
                    ui.visuals_mut().widgets.inactive.fg_stroke.color = DIM;
                    egui::MenuBar::new().ui(ui, |ui| self.menus(ui));
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
            if ui.button("Bounce mix to WAV…").clicked() {
                self.bounce();
            }
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
        });
        ui.menu_button("Track", |ui| {
            if ui.button("Add instrument track").clicked() {
                self.add_track("midi");
            }
            if ui.button("Add audio track").clicked() {
                self.add_track("audio");
            }
        });
        ui.menu_button("Audio", |ui| {
            if ui.button("Reconnect output").clicked() {
                self.connect();
            }
            ui.separator();
            ui.label(text(
                "Uses the operating system's default input and output.",
                FS_SECONDARY,
                Weight::Medium,
                DIM,
            ));
            ui.label(text(
                "Change devices in system audio settings, then reconnect.",
                FS_SECONDARY,
                Weight::Medium,
                DIM,
            ));
        });
        ui.menu_button("View", |ui| {
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
        if ui.button("Help").clicked() {
            self.show_help = true;
        }
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
                        ui.label(caps("Master"));
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
                        &["Instr", "Loops", "Effects", "Files"],
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
                    2 => "Search effects",
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
                let mut load: Option<(usize, String)> = None;
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
                        let groups: Vec<(&str, Vec<BrowserItem>)> = match tab {
                            0 => vec![(
                                "Ondera",
                                INSTRUMENTS
                                    .iter()
                                    .enumerate()
                                    .map(|(i, n)| (n.to_string(), String::new(), TRACKS[i % 8]))
                                    .collect(),
                            )],
                            1 => vec![(
                                "Loops",
                                LOOPS
                                    .iter()
                                    .enumerate()
                                    .map(|(i, n)| (n.to_string(), "midi".into(), TRACKS[i % 8]))
                                    .collect(),
                            )],
                            2 => vec![(
                                "Ondera",
                                EFFECTS
                                    .iter()
                                    .map(|n| (n.to_string(), String::new(), NEUTRAL_DOT))
                                    .collect(),
                            )],
                            _ => vec![(
                                "Project audio",
                                sources
                                    .into_iter()
                                    .map(|(n, d)| (n, d, TRACKS[4]))
                                    .collect(),
                            )],
                        };
                        for (group, items) in groups {
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                ui.add_space(14.0);
                                ui.label(caps(group));
                            });
                            ui.add_space(4.0);
                            let mut shown = 0;
                            for (name, meta, dot) in items {
                                if !filter.is_empty() && !name.to_lowercase().contains(&filter) {
                                    continue;
                                }
                                shown += 1;
                                let (row, response) = ui.allocate_exact_size(
                                    vec2(BROWSER - 12.0, 24.0),
                                    Sense::click(),
                                );
                                let row = Rect::from_min_max(
                                    pos2(row.left() + 6.0, row.top()),
                                    pos2(row.right() - 6.0, row.bottom()),
                                );
                                let selected = self.browser_selected.as_deref() == Some(&name);
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
                                    dot,
                                );
                                p.text(
                                    pos2(row.left() + 25.0, row.center().y),
                                    Align2::LEFT_CENTER,
                                    &name,
                                    font(FS_LIST, Weight::Medium),
                                    INK_CONTROL,
                                );
                                p.text(
                                    pos2(row.right() - 8.0, row.center().y),
                                    Align2::RIGHT_CENTER,
                                    &meta,
                                    mono_font(FS_SMALL),
                                    FAINT,
                                );
                                if response.clicked() {
                                    self.browser_selected = Some(name.clone());
                                }
                                if response.double_clicked() {
                                    load = Some((tab, name.clone()));
                                }
                            }
                            if shown == 0 {
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
                        }
                        ui.add_space(14.0);
                        ui.horizontal(|ui| {
                            ui.add_space(14.0);
                            ui.label(text(
                                if tab == 3 {
                                    "Drop audio files anywhere to import"
                                } else {
                                    "Double-click to load onto the selected track"
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
                    });
                if let Some((tab, name)) = load {
                    match tab {
                        0 => self.instrument(&name),
                        1 => self.add_loop(&name),
                        2 => self.add_effect(&name),
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
                                    ui.label(mono(
                                        self.browser_selected.clone().unwrap_or_default(),
                                        FS_VALUE,
                                        FAINT,
                                    ));
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
                let s = self.store.snapshot();
                let Some((index, t)) = s
                    .tracks
                    .iter()
                    .enumerate()
                    .find(|(_, t)| Some(&t.id) == s.view.selected_track_id.as_ref())
                else {
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        ui.add_space(14.0);
                        ui.label(text("Select a track", FS_BODY, Weight::SemiBold, DIM));
                    });
                    return;
                };
                let color = track_color(&t.color, index);
                let mut track = t.clone();
                let is_midi = track.kind == "midi";
                let divider = |ui: &mut egui::Ui| {
                    let y = ui.cursor().top();
                    hline(ui.painter(), rect.left() + 1.0, rect.right(), y, black(0.5));
                    ui.add_space(1.0);
                };
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
                divider(ui);
                // Routing rows.
                let mut strip = s.strips.get(&track.id).cloned().unwrap_or_default();
                let mut strip_changed = false;
                ui.add_space(10.0);
                let row = |ui: &mut egui::Ui,
                           label: &str,
                           value: &str,
                           clickable: bool|
                 -> egui::Response {
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
                    p.text(
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
                if is_midi {
                    let name = if strip.instrument.is_empty() {
                        INSTRUMENTS[0].to_string()
                    } else {
                        strip.instrument.clone()
                    };
                    let response = row(ui, "Instrument", &name, true);
                    egui::Popup::menu(&response).show(|ui| {
                        for n in INSTRUMENTS {
                            if ui.selectable_label(name == n, n).clicked() {
                                strip.instrument = n.into();
                                strip_changed = true;
                            }
                        }
                    });
                    row(ui, "Input", "All MIDI", false);
                } else {
                    row(ui, "Input", "Default input", false);
                }
                row(ui, "Output", "Stereo Out", false);
                ui.add_space(10.0);
                divider(ui);
                // Channel EQ.
                let eq = strip.inserts.iter().find(|i| i.name == "Channel EQ");
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
                let (well, _) = ui.allocate_exact_size(vec2(INSPECTOR, 74.0), Sense::hover());
                let well = Rect::from_min_max(
                    pos2(well.left() + 14.0, well.top()),
                    pos2(well.right() - 14.0, well.bottom()),
                );
                eq_well(
                    ui.painter(),
                    well,
                    color,
                    eq.is_some_and(|i| i.state == "active"),
                );
                ui.add_space(8.0);
                divider(ui);
                // Inserts.
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    ui.label(caps("Inserts"));
                });
                ui.add_space(7.0);
                while strip.inserts.len() < 4 {
                    strip.inserts.push(Insert {
                        name: "Empty slot".into(),
                        state: "empty".into(),
                        meta: String::new(),
                    });
                }
                for i in 0..4 {
                    let (slot_name, state) = (
                        strip.inserts[i].name.clone(),
                        strip.inserts[i].state.clone(),
                    );
                    let (r, response) =
                        ui.allocate_exact_size(vec2(INSPECTOR, 24.0), Sense::click());
                    let r = Rect::from_min_max(
                        pos2(r.left() + 14.0, r.top()),
                        pos2(r.right() - 14.0, r.bottom()),
                    );
                    let p = ui.painter();
                    let empty = state == "empty";
                    if empty {
                        groove_send(p, r, R_MD);
                    } else {
                        insert_face(p, r, R_MD);
                    }
                    let led_center = pos2(r.left() + 12.0, r.center().y);
                    let led_rect = Rect::from_center_size(led_center, Vec2::splat(12.0));
                    match state.as_str() {
                        "active" => {
                            p.circle_filled(led_center, 6.0, LED_GLOW.gamma_multiply(0.18));
                            p.circle_filled(led_center, 3.5, LED);
                        }
                        "bypassed" => {
                            p.circle_filled(led_center, 3.5, KNOB_LO);
                            p.circle_stroke(led_center, 3.5, egui::Stroke::new(1.0, black(0.6)));
                        }
                        _ => {
                            p.circle_stroke(led_center, 3.5, egui::Stroke::new(1.0, white(0.12)));
                        }
                    }
                    p.text(
                        pos2(r.left() + 24.0, r.center().y),
                        Align2::LEFT_CENTER,
                        &slot_name,
                        font(FS_SECONDARY, Weight::SemiBold),
                        match state.as_str() {
                            "active" => INK,
                            "bypassed" => DIM,
                            _ => FAINT,
                        },
                    );
                    p.text(
                        pos2(r.right() - 8.0, r.center().y),
                        Align2::RIGHT_CENTER,
                        match state.as_str() {
                            "bypassed" => "bypassed",
                            "active" => "on",
                            _ => "",
                        },
                        mono_font(FS_CAPS),
                        FAINT,
                    );
                    let led_response =
                        ui.interact(led_rect, ui.id().with(("insert-led", i)), Sense::click());
                    if led_response.clicked() && !empty {
                        strip.inserts[i].state = if state == "active" {
                            "bypassed"
                        } else {
                            "active"
                        }
                        .into();
                        strip_changed = true;
                    } else {
                        egui::Popup::menu(&response).show(|ui| {
                            for n in std::iter::once("Empty slot").chain(EFFECTS) {
                                if ui.selectable_label(slot_name == n, n).clicked() {
                                    strip.inserts[i].name = n.into();
                                    strip.inserts[i].state =
                                        if n == "Empty slot" { "empty" } else { "active" }.into();
                                    strip_changed = true;
                                }
                            }
                        });
                    }
                    if i < 3 {
                        ui.add_space(4.0);
                    }
                }
                ui.add_space(8.0);
                divider(ui);
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
                    for (i, name) in ["A · Reverb", "B · Delay"].iter().enumerate() {
                        let (card, _) = ui.allocate_exact_size(vec2(card_w, 40.0), Sense::hover());
                        groove_send(ui.painter(), card, R_CONTROL);
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
                                        strip.sends[i].level_db =
                                            if db <= -59.0 { None } else { Some(db) };
                                        strip_changed = true;
                                    }
                                    ui.vertical(|ui| {
                                        ui.spacing_mut().item_spacing.y = 1.0;
                                        ui.add(
                                            egui::Label::new(text(
                                                *name,
                                                FS_VALUE,
                                                Weight::SemiBold,
                                                INK,
                                            ))
                                            .truncate(),
                                        );
                                        ui.add(
                                            egui::Label::new(mono(
                                                match strip.sends[i].level_db {
                                                    Some(v) => {
                                                        format!("{v:.1} dB").replace('-', "−")
                                                    }
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
                ui.add_space(10.0);
                divider(ui);
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
                            ui.add_space(
                                FADER_H - 36.0 - 4.0 - 12.0 - 12.0 - 8.0 - 26.0 - 8.0 - 20.0,
                            );
                            ui.label(caps("Vol"));
                            let (well, _) =
                                ui.allocate_exact_size(vec2(44.0, 18.0), Sense::hover());
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
                            let (scale, _) =
                                ui.allocate_exact_size(vec2(18.0, FADER_H), Sense::hover());
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
                    divider(ui);
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
            });
    }
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
/// The channel EQ well. The built-in Channel EQ is a fixed three-band tone:
/// +1.5 dB below 120 Hz, −2 dB through the mids and +2 dB above 6 kHz.
fn eq_well(p: &egui::Painter, well: Rect, color: egui::Color32, active: bool) {
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
    let db_at = |t: f32| -> f32 {
        // t is 0..1 across 20 Hz .. 20 kHz on a log axis.
        let hz = 20.0 * (1000.0_f32).powf(t);
        let low = 1.0 / (1.0 + (hz / 120.0).powi(2));
        let high = 1.0 - 1.0 / (1.0 + (hz / 6000.0).powi(2));
        1.5 * low - 2.0 * (1.0 - low - high) + 2.0 * high
    };
    let n = 64;
    let points: Vec<egui::Pos2> = (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let db = if active { db_at(t) } else { 0.0 };
            pos2(
                inner.left() + t * inner.width(),
                zero - db * (inner.height() / 12.0),
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
        for t in [0.33, 0.75] {
            let i = (t * n as f32) as usize;
            p.circle(
                points[i],
                3.0,
                WELL_DEEP,
                egui::Stroke::new(1.5, color.lerp_to_gamma(egui::Color32::WHITE, 0.3)),
            );
        }
    }
}
