//! The Settings window: every preference Ondera keeps, edited in place and applied at once.
//! The document of record is `engine::settings::Settings`; the window holds the live copy
//! the rest of the app reads and writes it back through `apply_settings`, which is also
//! what `settings.set` from the CLI, MCP and agents ends up calling.

use crate::{app::Ondera, theme::*};
use eframe::egui::{self, pos2, vec2, Align2, Rect, Sense, Vec2};
use ondera_engine::{
    device, midi,
    plugin::Format,
    settings::{Provider, Settings},
    Result,
};
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

const SECTIONS: [&str; 8] = [
    "General",
    "Audio & MIDI",
    "Interface",
    "Agent",
    "Plugins",
    "Control",
    "Updates",
    "About",
];
const SIDEBAR: f32 = 168.0;
const WINDOW: Vec2 = vec2(820.0, 560.0);

#[derive(Default)]
pub(crate) struct SettingsWindow {
    pub open: bool,
    pub section: usize,
    dirty: bool,
    notice: Option<(String, Instant)>,
    error: Option<String>,
    job: Option<CliJob>,
    new_path: String,
    pub pending_scale: Option<f32>,
}
/// A vendor CLI run for sign-in or status checks; output shows in the pane.
struct CliJob {
    label: String,
    started: Instant,
    receiver: mpsc::Receiver<Result<String>>,
}

impl Ondera {
    pub(crate) fn open_settings(&mut self, section: Option<usize>) {
        self.settings_ui.open = true;
        if let Some(section) = section {
            self.settings_ui.section = section.min(SECTIONS.len() - 1);
        }
    }

    /// Apply a new document: side effects for what changed, then persist. Errors leave the
    /// previous settings in force.
    pub(crate) fn apply_settings(&mut self, next: Settings) -> Result<()> {
        next.validate()?;
        let previous = self.settings.clone();
        next.save()?;
        self.settings = next.clone();
        if previous.audio.output_device != next.audio.output_device {
            self.output_device = next.audio.output_device.clone();
            self.connect();
        }
        if previous.audio.input_device != next.audio.input_device {
            self.input_device = next.audio.input_device.clone();
        }
        if previous.audio.midi_input != next.audio.midi_input {
            match &next.audio.midi_input {
                Some(port) => self.connect_midi(Some(port.clone())),
                None => {
                    self.midi = None;
                    self.midi_port = None;
                }
            }
        }
        if previous.control.enable_bridge != next.control.enable_bridge {
            if next.control.enable_bridge {
                self.bridge_wanted = true;
            } else if self.control.take().is_some() {
                self.agents.stop_runner();
                self.status = "Local agent bridge disabled".into();
            }
        }
        if previous.interface.scale != next.interface.scale {
            self.settings_ui.pending_scale = Some(next.interface.scale);
        }
        Ok(())
    }

    pub(crate) fn settings_window(&mut self, ctx: &egui::Context) {
        if let Some(scale) = self.settings_ui.pending_scale.take() {
            ctx.set_zoom_factor(scale);
        }
        if self.bridge_wanted && self.control.is_none() {
            self.bridge_wanted = false;
            self.start_control(ctx);
        }
        self.poll_settings_job();
        if !self.settings_ui.open {
            return;
        }
        let mut open = true;
        let mut draft = self.settings.clone();
        let mut changed = false;
        egui::Window::new("Settings")
            .id(egui::Id::new("settings-window"))
            .open(&mut open)
            .title_bar(false)
            .frame(window_frame())
            .fixed_size(WINDOW)
            .default_pos(ctx.screen_rect().center() - WINDOW / 2.0)
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                faceplate(ui.painter(), rect, R_CARD);
                let sidebar = Rect::from_min_size(rect.min, vec2(SIDEBAR, rect.height()));
                {
                    let p = ui.painter();
                    p.rect_filled(
                        sidebar,
                        egui::CornerRadius {
                            nw: R_CARD as u8,
                            sw: R_CARD as u8,
                            ne: 0,
                            se: 0,
                        },
                        black(0.18),
                    );
                    vline(
                        p,
                        sidebar.right(),
                        rect.top() + 8.0,
                        rect.bottom() - 8.0,
                        black(0.55),
                    );
                    vline(
                        p,
                        sidebar.right() + 1.0,
                        rect.top() + 8.0,
                        rect.bottom() - 8.0,
                        white(0.05),
                    );
                    p.text(
                        pos2(sidebar.left() + 22.0, rect.top() + 26.0),
                        Align2::LEFT_CENTER,
                        "Settings",
                        font(FS_PANEL_TITLE, Weight::Bold),
                        INK,
                    );
                }
                ui.scope_builder(
                    egui::UiBuilder::new().max_rect(Rect::from_min_max(
                        pos2(sidebar.left() + 14.0, rect.top() + 48.0),
                        pos2(sidebar.right() - 14.0, rect.bottom() - 14.0),
                    )),
                    |ui| {
                        ui.spacing_mut().item_spacing = vec2(0.0, 6.0);
                        for (index, name) in SECTIONS.iter().enumerate() {
                            let active = self.settings_ui.section == index;
                            let galley = ui.painter().layout_no_wrap(
                                name.to_string(),
                                font(FS_LIST, Weight::SemiBold),
                                INK,
                            );
                            let width = ui.available_width();
                            if button(
                                ui,
                                vec2(width, 26.0),
                                Face::from_flag(active),
                                R_CONTROL,
                                |p, r, ink| {
                                    p.galley(
                                        pos2(r.left() + 12.0, r.center().y - galley.size().y / 2.0),
                                        galley.clone(),
                                        ink,
                                    );
                                    if active {
                                        accent_dot(
                                            p,
                                            pos2(r.right() - 12.0, r.center().y),
                                            2.5,
                                            true,
                                        );
                                    }
                                },
                            )
                            .clicked()
                            {
                                self.settings_ui.section = index;
                            }
                        }
                    },
                );
                let pane = Rect::from_min_max(
                    pos2(sidebar.right() + 20.0, rect.top() + 16.0),
                    pos2(rect.right() - 18.0, rect.bottom() - 52.0),
                );
                ui.scope_builder(egui::UiBuilder::new().max_rect(pane), |ui| {
                    ui.set_clip_rect(pane);
                    ui.spacing_mut().item_spacing = vec2(GAP, 8.0);
                    ui.label(text(
                        SECTIONS[self.settings_ui.section],
                        FS_PANEL_TITLE,
                        Weight::Bold,
                        INK_BRIGHT,
                    ));
                    ui.add_space(4.0);
                    egui::ScrollArea::vertical()
                        .id_salt("settings-pane")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.set_min_width(pane.width() - 12.0);
                            changed |= self.settings_section(ui, &mut draft);
                            ui.add_space(12.0);
                        });
                });
                // Footer: status, reset, close.
                let footer = Rect::from_min_max(
                    pos2(sidebar.right() + 20.0, rect.bottom() - 44.0),
                    pos2(rect.right() - 18.0, rect.bottom() - 12.0),
                );
                hline(
                    ui.painter(),
                    footer.left(),
                    footer.right(),
                    footer.top() - 4.0,
                    black(0.5),
                );
                hline(
                    ui.painter(),
                    footer.left(),
                    footer.right(),
                    footer.top() - 3.0,
                    white(0.05),
                );
                ui.scope_builder(egui::UiBuilder::new().max_rect(footer), |ui| {
                    ui.horizontal_centered(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        let message = match (&self.settings_ui.error, &self.settings_ui.notice) {
                            (Some(error), _) => (error.clone(), ACCENT),
                            (None, Some((notice, when))) if when.elapsed().as_secs() < 6 => {
                                (notice.clone(), DIM)
                            }
                            _ => (format!("Saved to {}", Settings::path().display()), FAINT),
                        };
                        ui.add(egui::Label::new(mono(message.0, FS_SMALL, message.1)).truncate());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if text_button(ui, "Close", Face::Raised).clicked() {
                                self.settings_ui.open = false;
                            }
                            if text_button(ui, "Reset section", Face::Raised)
                                .on_hover_text("Restore this section's defaults")
                                .clicked()
                            {
                                let section = [
                                    "general",
                                    "audio",
                                    "interface",
                                    "agent",
                                    "plugins",
                                    "control",
                                    "general",
                                    "",
                                ][self.settings_ui.section];
                                if !section.is_empty() {
                                    if let Err(e) = draft.reset(Some(section)) {
                                        self.settings_ui.error = Some(e);
                                    } else {
                                        changed = true;
                                    }
                                }
                            }
                        });
                    });
                });
            });
        if changed {
            self.settings_ui.dirty = true;
        }
        // Apply once typing pauses: text fields keep focus while edited; combo boxes,
        // switches and sliders apply on the next frame.
        let typing = ctx
            .memory(|m| m.focused())
            .is_some_and(|id| egui::TextEdit::load_state(ctx, id).is_some());
        if self.settings_ui.dirty && !typing {
            self.settings_ui.dirty = false;
            match self.apply_settings(draft) {
                Ok(()) => {
                    self.settings_ui.error = None;
                    self.settings_ui.notice = Some(("Settings applied".into(), Instant::now()));
                }
                Err(e) => self.settings_ui.error = Some(e),
            }
        } else if changed {
            // Keep the draft alive across frames while a field is focused.
            self.settings = draft;
        }
        if !open {
            self.settings_ui.open = false;
        }
        if self.settings_ui.job.is_some() {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
    }

    fn settings_section(&mut self, ui: &mut egui::Ui, draft: &mut Settings) -> bool {
        let mut changed = false;
        match self.settings_ui.section {
            0 => {
                section_header(ui, "Sessions");
                changed |= row_switch(
                    ui,
                    "Reopen the last session at start",
                    "Otherwise Ondera opens the demo arrangement.",
                    &mut draft.general.reopen_last_session,
                );
                changed |= row_switch(
                    ui,
                    "Ask before quitting",
                    "Confirm even when nothing is unsaved.",
                    &mut draft.general.confirm_before_quit,
                );
                section_header(ui, "Recovery");
                let mut seconds = draft.general.recovery_interval_seconds as f32;
                if row_slider(
                    ui,
                    "Snapshot interval",
                    "Seconds between recovery copies while an edited session is idle.",
                    &mut seconds,
                    10.0..=600.0,
                    "s",
                ) {
                    draft.general.recovery_interval_seconds = seconds.round() as u32;
                    changed = true;
                }
                row_note(
                    ui,
                    format!(
                        "Snapshots live in {}",
                        ondera_engine::recovery::directory().display()
                    ),
                );
                if !draft.general.recent_sessions.is_empty() {
                    section_header(ui, "Recent sessions");
                    let mut open_path = None;
                    for path in draft.general.recent_sessions.clone() {
                        ui.horizontal(|ui| {
                            if text_button(ui, "Open", Face::Raised).clicked() {
                                open_path = Some(PathBuf::from(&path));
                            }
                            ui.add(egui::Label::new(mono(path, FS_SMALL, DIM)).truncate());
                        });
                    }
                    if ui.small_button("Clear list").clicked() {
                        draft.general.recent_sessions.clear();
                        changed = true;
                    }
                    if let Some(path) = open_path {
                        self.settings_ui.open = false;
                        self.load_path(path);
                    }
                }
            }
            1 => {
                section_header(ui, "Output");
                let outputs = device::output_devices();
                changed |= row_choice(
                    ui,
                    "Output device",
                    "Reconnects when changed.",
                    &mut draft.audio.output_device,
                    &outputs,
                    "System default",
                );
                if let Some(d) = &self.device {
                    row_note(
                        ui,
                        format!(
                            "Playing through {} at {} kHz",
                            d.device_name,
                            d.sample_rate / 1000
                        ),
                    );
                } else {
                    row_note(ui, "Audio output is not open.".to_string());
                }
                ui.horizontal(|ui| {
                    if text_button(ui, "Reconnect output", Face::Raised).clicked() {
                        self.connect();
                    }
                });
                section_header(ui, "Input");
                let inputs = device::input_devices();
                changed |= row_choice(
                    ui,
                    "Recording input",
                    "Used when an audio track is armed.",
                    &mut draft.audio.input_device,
                    &inputs,
                    "System default",
                );
                section_header(ui, "MIDI");
                let ports = midi::ports();
                changed |= row_choice(
                    ui,
                    "MIDI input",
                    "Live notes go straight to the audio thread.",
                    &mut draft.audio.midi_input,
                    &ports,
                    "None",
                );
                changed |= row_switch(
                    ui,
                    "Connect at start",
                    "Reconnect the chosen port when Ondera opens.",
                    &mut draft.audio.connect_midi_on_start,
                );
                if ports.is_empty() {
                    row_note(ui, "No MIDI ports found.".to_string());
                }
            }
            2 => {
                section_header(ui, "Appearance");
                changed |= row_slider(
                    ui,
                    "Interface scale",
                    "Zoom for every panel; applies immediately.",
                    &mut draft.interface.scale,
                    0.75..=1.75,
                    "×",
                );
                changed |= row_switch(
                    ui,
                    "Show tooltips",
                    "Hover hints on controls.",
                    &mut draft.interface.show_tooltips,
                );
                section_header(ui, "Start-up");
                changed |= row_switch(
                    ui,
                    "Open the agent panel",
                    "Show the agent conversation when Ondera starts.",
                    &mut draft.interface.agent_panel_open_on_start,
                );
                changed |= row_switch(
                    ui,
                    "Follow the playhead",
                    "Default for new sessions.",
                    &mut draft.interface.follow_playhead,
                );
            }
            3 => changed |= self.agent_settings(ui, draft),
            4 => {
                section_header(ui, "Scanning");
                changed |= row_switch(ui, "Scan at start", "Look for new CLAP, VST3, Audio Unit and Ondera native plugins when the app opens.", &mut draft.plugins.scan_on_start);
                ui.horizontal(|ui| {
                    if text_button(
                        ui,
                        if self.scan_job.is_some() {
                            "Scanning…"
                        } else {
                            "Scan now"
                        },
                        Face::from_flag(self.scan_job.is_some()),
                    )
                    .clicked()
                        && self.scan_job.is_none()
                    {
                        self.scan_plugins();
                    }
                    let external = self
                        .catalog
                        .iter()
                        .filter(|d| d.format != Format::Stock)
                        .count();
                    ui.label(mono(
                        format!("{external} external plugins in the cache"),
                        FS_SMALL,
                        FAINT,
                    ));
                });
                for (title, format, paths) in [
                    (
                        "Ondera native plugin folders",
                        Format::Native,
                        &mut draft.plugins.extra_native_paths,
                    ),
                    (
                        "CLAP folders",
                        Format::Clap,
                        &mut draft.plugins.extra_clap_paths,
                    ),
                    (
                        "VST3 folders",
                        Format::Vst3,
                        &mut draft.plugins.extra_vst3_paths,
                    ),
                ] {
                    section_header(ui, title);
                    let standard = ondera_engine::host::scan::directories(format);
                    for dir in standard
                        .iter()
                        .filter(|d| !paths.iter().any(|p| Path::new(p) == d.as_path()))
                    {
                        row_note(ui, dir.display().to_string());
                    }
                    let mut remove = None;
                    for (index, path) in paths.iter().enumerate() {
                        ui.horizontal(|ui| {
                            if text_button(ui, "Remove", Face::Raised).clicked() {
                                remove = Some(index);
                            }
                            ui.add(egui::Label::new(mono(path, FS_SMALL, INK)).truncate());
                        });
                    }
                    if let Some(index) = remove {
                        paths.remove(index);
                        changed = true;
                    }
                    ui.horizontal(|ui| {
                        let id = ("plugin-path", format.prefix());
                        field(
                            ui,
                            id,
                            &mut self.settings_ui.new_path,
                            "Add a folder…",
                            false,
                            360.0,
                        );
                        if text_button(ui, "Add", Face::Raised).clicked()
                            && !self.settings_ui.new_path.trim().is_empty()
                        {
                            paths.push(self.settings_ui.new_path.trim().to_string());
                            self.settings_ui.new_path.clear();
                            changed = true;
                        }
                        if text_button(ui, "Choose…", Face::Raised).clicked() {
                            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                                paths.push(folder.to_string_lossy().into_owned());
                                changed = true;
                            }
                        }
                    });
                }
                section_header(ui, "Native plugins");
                row_note(ui, format!("Ondera native plugins are Rust libraries built with the ondera-plugin SDK (ABI {}). Drop an .onplug bundle or a bare library into a folder above and scan.", ondera_plugin::ABI_VERSION));
                ui.horizontal(|ui| {
                    if text_button(ui, "Open plugin folder", Face::Raised).clicked() {
                        let dir = ondera_engine::host::scan::data_dir().join("plugins");
                        let _ = std::fs::create_dir_all(&dir);
                        reveal(&dir);
                    }
                });
            }
            5 => {
                section_header(ui, "Local bridge");
                changed |= row_switch(ui, "Serve the CLI and MCP bridge", "ondera-cli and ondera-mcp control this window over 127.0.0.1 with a private token.", &mut draft.control.enable_bridge);
                let connection = self
                    .control
                    .as_ref()
                    .map(|s| (s.port(), s.path().to_path_buf()));
                row_note(
                    ui,
                    match &connection {
                        Some((port, path)) => format!(
                            "Listening on 127.0.0.1:{port} · discovery {}",
                            path.display()
                        ),
                        None => "Bridge off: clients cannot reach this window.".into(),
                    },
                );
                ui.horizontal(|ui| {
                    if text_button(ui, "Copy MCP config", Face::Raised).clicked() {
                        ui.ctx()
                            .copy_text(crate::agents::mcp_config_text(&self.discovery_path()));
                        self.settings_ui.notice =
                            Some(("MCP configuration copied".into(), Instant::now()));
                    }
                    if text_button(ui, "Copy CLI check", Face::Raised).clicked() {
                        ui.ctx()
                            .copy_text(crate::agents::cli_check_text(&self.discovery_path()));
                        self.settings_ui.notice =
                            Some(("CLI command copied".into(), Instant::now()));
                    }
                });
                section_header(ui, "Registry");
                row_note(ui, format!("{} commands are shared by the window, ondera-cli, ondera-mcp and the built-in agent. Run `ondera-cli commands` to list them.", ondera_engine::control::COMMANDS.len()));
            }
            6 => {
                section_header(ui, "Releases");
                changed |= row_switch(
                    ui,
                    "Check for updates at start",
                    "Compares the latest GitHub release with this build.",
                    &mut draft.general.check_updates_on_start,
                );
                changed |= row_switch(ui, "Install updates automatically", "Download and verify a newer release as soon as it is found; relaunch is still confirmed.", &mut draft.general.install_updates_automatically);
                ui.horizontal(|ui| {
                    if text_button(ui, "Check now", Face::Raised).clicked() {
                        self.check_for_updates(true);
                    }
                    if let Some(release) = &self.updates.available {
                        if text_button(ui, &format!("Install {}", release.version), Face::Lit)
                            .clicked()
                        {
                            self.install_update();
                        }
                    }
                });
                row_note(
                    ui,
                    format!(
                        "Ondera {} · {}",
                        crate::update::current_version(),
                        crate::update::signing_summary()
                    ),
                );
                row_note(ui, "Downloads are verified against SHA256SUMS and the release signature before the installed copy is replaced; the previous copy is kept until the new one starts.".to_string());
            }
            _ => {
                section_header(ui, "Ondera");
                row_note(
                    ui,
                    format!(
                        "Version {} · plugin ABI {} · {} {}",
                        crate::update::current_version(),
                        ondera_plugin::ABI_VERSION,
                        std::env::consts::OS,
                        std::env::consts::ARCH
                    ),
                );
                row_note(
                    ui,
                    format!("Data: {}", ondera_engine::host::scan::data_dir().display()),
                );
                row_note(ui, format!("Settings: {}", Settings::path().display()));
                row_note(
                    ui,
                    format!("Presets: {}", ondera_engine::preset::directory().display()),
                );
                ui.horizontal(|ui| {
                    if text_button(ui, "Open data folder", Face::Raised).clicked() {
                        reveal(&ondera_engine::host::scan::data_dir());
                    }
                    if text_button(ui, "Working in Ondera", Face::Raised).clicked() {
                        self.show_help = true;
                    }
                });
                row_note(ui, "MIT licensed. Native Rust: no web view, no runtime, one command registry for people and agents.".to_string());
            }
        }
        changed
    }

    fn agent_settings(&mut self, ui: &mut egui::Ui, draft: &mut Settings) -> bool {
        let mut changed = false;
        section_header(ui, "Provider");
        let mut provider = draft.agent.provider;
        ui.horizontal(|ui| {
            ui.label(text(
                "How the agent thinks",
                FS_SECONDARY,
                Weight::Medium,
                DIM,
            ));
            egui::ComboBox::from_id_salt("agent-provider")
                .width(300.0)
                .selected_text(provider.label())
                .show_ui(ui, |ui| {
                    for p in Provider::ALL {
                        ui.selectable_value(&mut provider, p, p.label());
                    }
                });
        });
        if provider != draft.agent.provider {
            draft.agent.provider = provider;
            changed = true;
        }
        changed |= row_text(
            ui,
            "Model",
            &format!(
                "Blank uses the default ({}).",
                if provider.default_model().is_empty() {
                    "the account's default"
                } else {
                    provider.default_model()
                }
            ),
            &mut draft.agent.model,
            false,
        );
        match provider {
            Provider::Codex => {
                changed |= row_text(
                    ui,
                    "Codex executable",
                    "Blank finds `codex` on PATH or in the usual places.",
                    &mut draft.agent.codex_executable,
                    false,
                );
                self.sign_in_row(
                    ui,
                    "Codex",
                    crate::agent::discover_codex(&draft.agent.codex_executable),
                    &["login"],
                    &["login", "status"],
                );
                row_note(ui, "Uses your ChatGPT sign-in through the Codex CLI. Prompts and tool results go to OpenAI; the CLI keeps the credentials.".to_string());
            }
            Provider::Claude => {
                changed |= row_text(
                    ui,
                    "Claude executable",
                    "Blank finds `claude` on PATH or in the usual places.",
                    &mut draft.agent.claude_executable,
                    false,
                );
                self.sign_in_row(
                    ui,
                    "Claude",
                    crate::agent::discover_claude(&draft.agent.claude_executable),
                    &["auth", "login"],
                    &["auth", "status"],
                );
                row_note(ui, "Uses your Anthropic sign-in through the Claude Code CLI. Prompts and tool results go to Anthropic; the CLI keeps the credentials.".to_string());
            }
            Provider::Anthropic => {
                changed |= row_text(ui, "API key", "Stored in settings.json, readable only by you. ANTHROPIC_API_KEY is used when blank.", &mut draft.agent.anthropic_api_key, true);
                row_note(ui, "Talks to api.anthropic.com directly from this app with streaming and tool use.".to_string());
            }
            Provider::OpenAi => {
                changed |= row_text(ui, "API key", "Stored in settings.json, readable only by you. OPENAI_API_KEY is used when blank.", &mut draft.agent.openai_api_key, true);
                row_note(ui, "Talks to api.openai.com directly from this app with streaming and function calling.".to_string());
            }
            Provider::Compatible => {
                changed |= row_text(
                    ui,
                    "Base URL",
                    "For example http://localhost:11434/v1 or https://openrouter.ai/api/v1.",
                    &mut draft.agent.compatible_base_url,
                    false,
                );
                changed |= row_text(
                    ui,
                    "API key",
                    "Optional for local servers.",
                    &mut draft.agent.compatible_api_key,
                    true,
                );
                row_note(
                    ui,
                    "Any server that speaks the OpenAI chat-completions API with tools."
                        .to_string(),
                );
            }
        }
        section_header(ui, "Behaviour");
        let mut tokens = draft.agent.max_output_tokens as f32;
        if row_slider(
            ui,
            "Max reply tokens",
            "Longest single reply the model may write.",
            &mut tokens,
            256.0..=32000.0,
            "",
        ) {
            draft.agent.max_output_tokens = tokens.round() as u32;
            changed = true;
        }
        let mut rounds = draft.agent.max_tool_rounds as f32;
        if row_slider(
            ui,
            "Max tool rounds",
            "Tool calls allowed in one task before the agent has to answer.",
            &mut rounds,
            1.0..=200.0,
            "",
        ) {
            draft.agent.max_tool_rounds = rounds.round() as u32;
            changed = true;
        }
        ui.label(text(
            "Standing instructions",
            FS_SECONDARY,
            Weight::Medium,
            DIM,
        ));
        let (well, _) =
            ui.allocate_exact_size(vec2(ui.available_width() - 4.0, 84.0), Sense::hover());
        well_input(ui.painter(), well, R_MD);
        ui.scope_builder(egui::UiBuilder::new().max_rect(well.shrink2(vec2(6.0, 4.0))), |ui| {
            if ui
                .add(
                    egui::TextEdit::multiline(&mut draft.agent.instructions)
                        .id_salt("agent-instructions")
                        .frame(false)
                        .font(font(FS_LIST, Weight::Medium))
                        .text_color(INK)
                        .hint_text(text("Preferences the agent should always follow, e.g. “keep everything in A minor, never touch the master chain”.", FS_LIST, Weight::Medium, FAINT))
                        .desired_width(f32::INFINITY)
                        .desired_rows(4),
                )
                .changed()
            {
                changed = true;
            }
        });
        section_header(ui, "Permissions");
        let permissions = &mut draft.agent.permissions;
        changed |= row_switch(
            ui,
            "File operations",
            "Save, open, import, export, bounce, scan plugins, save presets.",
            &mut permissions.file_operations,
        );
        changed |= row_switch(
            ui,
            "Transport",
            "Play, stop, record, locate and audition notes.",
            &mut permissions.transport,
        );
        changed |= row_switch(
            ui,
            "Replace the session",
            "session.new, session.open and snapshot restore.",
            &mut permissions.replace_session,
        );
        changed |= row_switch(
            ui,
            "Change settings",
            "settings.set, settings.reset and audio device changes.",
            &mut permissions.settings,
        );
        changed |= row_switch(
            ui,
            "Application control",
            "Quit and install updates.",
            &mut permissions.app_control,
        );
        row_note(ui, "Applies to the built-in agent, MCP clients and `ondera-cli --agent`. Plain document edits are always allowed and always undoable.".to_string());
        changed
    }

    fn sign_in_row(
        &mut self,
        ui: &mut egui::Ui,
        vendor: &str,
        exe: PathBuf,
        login: &[&str],
        status: &[&str],
    ) {
        ui.horizontal(|ui| {
            let busy = self.settings_ui.job.is_some();
            ui.add_enabled_ui(!busy, |ui| {
                if text_button(ui, &format!("Sign in with {vendor}"), Face::Raised)
                    .on_hover_text("Runs the CLI's login flow; a browser window opens")
                    .clicked()
                {
                    self.start_settings_job(format!("{vendor} sign-in"), exe.clone(), login);
                }
                if text_button(ui, "Check sign-in", Face::Raised).clicked() {
                    self.start_settings_job(format!("{vendor} status"), exe.clone(), status);
                }
            });
            ui.add(egui::Label::new(mono(exe.display().to_string(), FS_SMALL, FAINT)).truncate());
        });
        if let Some(job) = &self.settings_ui.job {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(mono(
                    format!("{} · {}s", job.label, job.started.elapsed().as_secs()),
                    FS_SMALL,
                    DIM,
                ));
            });
        }
    }

    fn start_settings_job(&mut self, label: String, exe: PathBuf, args: &[&str]) {
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        let (tx, receiver) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let result = run_cli(&exe, &args, Duration::from_secs(240));
            let _ = tx.send(result);
        });
        self.settings_ui.job = Some(CliJob {
            label,
            started: Instant::now(),
            receiver,
        });
    }

    fn poll_settings_job(&mut self) {
        let Some(job) = &self.settings_ui.job else {
            return;
        };
        let outcome = match job.receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => Err("The command stopped unexpectedly".into()),
        };
        let label = job.label.clone();
        self.settings_ui.job = None;
        match outcome {
            Ok(output) => {
                self.settings_ui.error = None;
                self.settings_ui.notice =
                    Some((format!("{label}: {}", one_line(&output)), Instant::now()));
            }
            Err(e) => self.settings_ui.error = Some(format!("{label}: {}", one_line(&e))),
        }
    }
}

/// Run a vendor CLI to completion with a deadline, returning its combined output.
fn run_cli(exe: &Path, args: &[String], timeout: Duration) -> Result<String> {
    let mut child = Command::new(exe)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not start {}: {e}", exe.display()))?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let out = std::thread::spawn(move || read_all(stdout));
    let err = std::thread::spawn(move || read_all(stderr));
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < timeout => {
                std::thread::sleep(Duration::from_millis(50))
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Timed out waiting for the command; finish the sign-in in the browser and check again".into());
            }
            Err(e) => return Err(e.to_string()),
        }
    };
    let output = format!(
        "{}\n{}",
        out.join().unwrap_or_default(),
        err.join().unwrap_or_default()
    )
    .trim()
    .to_string();
    if status.success() {
        Ok(if output.is_empty() {
            "done".into()
        } else {
            output
        })
    } else {
        Err(if output.is_empty() {
            format!("exited with {status}")
        } else {
            output
        })
    }
}
fn read_all(stream: Option<impl std::io::Read>) -> String {
    use std::io::Read;
    let mut text = String::new();
    if let Some(stream) = stream {
        let _ = stream.take(64 * 1024).read_to_string(&mut text);
    }
    text
}
fn one_line(text: &str) -> String {
    let line: String = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
    if line.chars().count() > 160 {
        line.chars().take(157).collect::<String>() + "…"
    } else {
        line
    }
}
/// Show a folder in the system file manager.
pub(crate) fn reveal(path: &Path) {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    let _ = Command::new(program).arg(path).spawn();
}

// Row helpers: label and description at the left, control at the right.
fn row<R>(
    ui: &mut egui::Ui,
    label: &str,
    description: &str,
    control: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let mut out = None;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_width(ui.available_width() - 230.0);
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.label(text(label, FS_BODY, Weight::SemiBold, INK));
            if !description.is_empty() {
                ui.add(
                    egui::Label::new(text(description, FS_SECONDARY, Weight::Medium, DIM)).wrap(),
                );
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            out = Some(control(ui));
        });
    });
    ui.add_space(4.0);
    out.expect("row control ran")
}
fn row_switch(ui: &mut egui::Ui, label: &str, description: &str, value: &mut bool) -> bool {
    row(ui, label, description, |ui| {
        toggle_switch(ui, value).changed()
    })
}
fn row_text(
    ui: &mut egui::Ui,
    label: &str,
    description: &str,
    value: &mut String,
    secret: bool,
) -> bool {
    row(ui, label, description, |ui| {
        field(ui, ("settings-field", label), value, "", secret, 220.0).changed()
    })
}
fn row_slider(
    ui: &mut egui::Ui,
    label: &str,
    description: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    unit: &str,
) -> bool {
    row(ui, label, description, |ui| {
        let shown = if unit == "×" {
            format!("{value:.2}×")
        } else if unit.is_empty() {
            format!("{}", value.round() as i64)
        } else {
            format!("{} {unit}", value.round() as i64)
        };
        ui.label(mono(shown, FS_VALUE, INK));
        hslider(ui, value, range, 130.0).changed()
    })
}
fn row_choice(
    ui: &mut egui::Ui,
    label: &str,
    description: &str,
    value: &mut Option<String>,
    options: &[String],
    none: &str,
) -> bool {
    row(ui, label, description, |ui| {
        let mut changed = false;
        egui::ComboBox::from_id_salt(("settings-choice", label))
            .width(220.0)
            .selected_text(value.clone().unwrap_or_else(|| none.to_string()))
            .show_ui(ui, |ui| {
                if ui.selectable_label(value.is_none(), none).clicked() && value.is_some() {
                    *value = None;
                    changed = true;
                }
                for option in options {
                    if ui
                        .selectable_label(value.as_deref() == Some(option), option)
                        .clicked()
                        && value.as_deref() != Some(option)
                    {
                        *value = Some(option.clone());
                        changed = true;
                    }
                }
            });
        changed
    })
}
fn row_note(ui: &mut egui::Ui, note: String) {
    ui.add(egui::Label::new(text(note, FS_SECONDARY, Weight::Medium, FAINT)).wrap());
}
