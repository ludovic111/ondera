//! A native task runner and workbench for the shared CLI/MCP command registry.
//! The installed Codex CLI supplies the model; every command runs against the document
//! open in this window and is recorded in the same activity history.

use crate::{app::Ondera, theme::*};
use eframe::egui::{self, vec2, Id, Sense};
use ondera_engine::{
    control::{self, Kind, Spec},
    model::Session,
    Result,
};
use serde_json::{json, Map, Value};
use std::{collections::VecDeque, path::Path, time::Instant};

const HISTORY_LIMIT: usize = 64;
const DETAIL_LIMIT: usize = 24_000;

pub(crate) struct AgentPanel {
    pub open: bool,
    runner: crate::agent_runner::Runner,
    filter: String,
    method: String,
    arguments: String,
    history: VecDeque<Activity>,
    sequence: u64,
    last_request: Option<Instant>,
    copied: Option<(&'static str, Instant)>,
}

impl Default for AgentPanel {
    fn default() -> Self {
        Self {
            open: false,
            runner: Default::default(),
            filter: String::new(),
            method: "session.info".into(),
            arguments: "{}".into(),
            history: VecDeque::new(),
            sequence: 0,
            last_request: None,
            copied: None,
        }
    }
}

struct Activity {
    sequence: u64,
    method: String,
    source: String,
    parameters: String,
    detail: String,
    succeeded: bool,
    running: bool,
    expanded: bool,
    before: u64,
    after: u64,
}

struct Connection {
    port: Option<u16>,
    discovery: std::path::PathBuf,
}

enum Action {
    ToggleBridge,
    Run(String, Value),
    StartTask,
    StopTask,
}

impl AgentPanel {
    fn select(&mut self, method: &str, session: &Session) {
        self.method = method.to_owned();
        self.arguments = control::spec(method)
            .map(|spec| pretty(&example_arguments(spec, session)))
            .unwrap_or_else(|| "{}".into());
    }

    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        connection: &Connection,
        session: &Session,
    ) -> Option<Action> {
        let mut action = None;
        ui.spacing_mut().item_spacing = vec2(GAP, GAP);
        ui.add_space(GAP);
        if segmented(
            ui,
            &["Library", "Agents"],
            1,
            (ui.available_width() - GAP) / 2.0,
        ) == Some(0)
        {
            self.open = false;
        }
        ui.add_space(GAP);
        ui.label(text("Agents", FS_PANEL_TITLE, Weight::SemiBold, INK_BRIGHT));
        ui.label(text(
            "Ask Codex to compose, arrange and mix this session. Its commands share the app's undo history.",
            FS_SECONDARY,
            Weight::Medium,
            DIM,
        ));
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(GAP, GAP), Sense::hover());
            ui.painter().circle_filled(
                rect.center(),
                GAP / 3.0,
                if connection.port.is_some() {
                    ACCENT
                } else {
                    NEUTRAL_DOT
                },
            );
            ui.label(mono(
                connection.port.map_or_else(
                    || "Local bridge disabled".into(),
                    |port| format!("Listening · 127.0.0.1:{port}"),
                ),
                FS_SECONDARY,
                INK,
            ));
        });
        ui.horizontal(|ui| {
            if text_button(
                ui,
                if connection.port.is_some() {
                    "Disable bridge"
                } else {
                    "Enable bridge"
                },
                Face::Raised,
            )
            .clicked()
            {
                action = Some(Action::ToggleBridge);
            }
            if let Some(seen) = self.last_request {
                let seconds = seen.elapsed().as_secs();
                ui.label(mono(
                    if seconds < 60 {
                        format!("Last request {seconds}s ago")
                    } else {
                        format!("Last request {}m ago", seconds / 60)
                    },
                    FS_SMALL,
                    FAINT,
                ));
            }
        });
        egui::ScrollArea::vertical()
            .id_salt("agents-content")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.label(caps("Music assistant"));
                ui.label(text(
                    "Uses your installed Codex CLI and its signed-in account. Requests and tool results are sent to your configured model provider.",
                    FS_SECONDARY, Weight::Medium, DIM,
                ));
                ui.add_enabled_ui(!self.runner.running(), |ui| {
                    ui.add(egui::TextEdit::multiline(&mut self.runner.prompt)
                        .id(Id::new("agents-task-prompt"))
                        .font(font(FS_LIST, Weight::Medium))
                        .hint_text("Add a warm bass line and soft drums for an eight-bar chorus…")
                        .char_limit(8000)
                        .desired_width(f32::INFINITY).desired_rows(4));
                });
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(connection.port.is_some() && !self.runner.running() && !self.runner.prompt.trim().is_empty(), |ui| {
                        let response = text_button(ui, "Run task", Face::Lit);
                        ui.ctx().data_mut(|data| data.insert_temp(Id::new("agents-task-run-rect"), response.rect));
                        if response.clicked() { action = Some(Action::StartTask); }
                    });
                    if self.runner.running() && text_button(ui, "Stop", Face::Raised).clicked() {
                        action = Some(Action::StopTask);
                    }
                });
                if !self.runner.status.is_empty() {
                    ui.label(text(&self.runner.status, FS_SECONDARY, Weight::Medium, INK));
                }
                if let Some(error) = &self.runner.error {
                    ui.label(text(error, FS_SECONDARY, Weight::Medium, INK_BRIGHT));
                }
                if !self.runner.response.is_empty() {
                    ui.label(text(&self.runner.response, FS_SECONDARY, Weight::Medium, INK));
                    if text_button(ui, "Copy response", Face::Raised).clicked() { ui.ctx().copy_text(self.runner.response.clone()); }
                }
                if !self.runner.events.is_empty() {
                    egui::CollapsingHeader::new("Task progress").id_salt("agents-task-progress").show(ui, |ui| {
                        for event in &self.runner.events { ui.label(mono(event, FS_SMALL, DIM)); }
                    });
                }
                egui::CollapsingHeader::new("Codex settings").id_salt("agents-codex-settings").show(ui, |ui| {
                    ui.add_enabled_ui(!self.runner.running(), |ui| {
                        ui.label(text("CLI executable · blank finds Codex automatically", FS_SMALL, Weight::Medium, DIM));
                        ui.add(egui::TextEdit::singleline(&mut self.runner.executable).hint_text("/path/to/codex").desired_width(f32::INFINITY));
                        ui.label(text("Model · blank uses your account default", FS_SMALL, Weight::Medium, DIM));
                        ui.add(egui::TextEdit::singleline(&mut self.runner.model).hint_text("Default").desired_width(f32::INFINITY));
                    });
                    ui.label(text("Install or update Codex CLI and run codex login in a terminal. This task exposes only this window's Ondera tools; shell, web, apps and other plugins are disabled. Stop preserves completed edits in Undo history.", FS_SMALL, Weight::Medium, FAINT));
                });
                ui.separator();
                egui::CollapsingHeader::new(text("Connect a client", FS_BODY, Weight::SemiBold, INK))
                    .id_salt("agents-setup")
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.label(text(
                            "Connect another MCP client using the installed companions. Audio processing stays in Ondera; your client controls which session data it sends to its model.",
                            FS_SECONDARY,
                            Weight::Medium,
                            DIM,
                        ));
                        ui.horizontal_wrapped(|ui| {
                            if text_button(ui, "Copy MCP config", Face::Raised).clicked() {
                                ui.ctx().copy_text(mcp_config(&connection.discovery));
                                self.copied = Some(("MCP configuration copied", Instant::now()));
                            }
                            if text_button(ui, "Copy CLI check", Face::Raised).clicked() {
                                ui.ctx().copy_text(cli_check(&connection.discovery));
                                self.copied = Some(("CLI command copied", Instant::now()));
                            }
                        });
                        if let Some((message, _)) = self.copied.filter(|(_, when)| when.elapsed().as_secs() < 5) {
                            ui.label(text(message, FS_SMALL, Weight::Medium, ACCENT));
                        }
                        ui.label(mono(
                            format!("Discovery: {}", connection.discovery.display()),
                            FS_SMALL,
                            FAINT,
                        ));
                        ui.label(text(
                            "Clients authenticate with the private discovery file. Disabling the bridge disconnects control from this session.",
                            FS_SMALL,
                            Weight::Medium,
                            FAINT,
                        ));
                    });
                ui.separator();
                ui.label(caps("Command workbench"));
                ui.horizontal_wrapped(|ui| {
                    for (label, method) in [
                        ("Inspect", "session.info"),
                        ("Add track", "track.add"),
                        ("Add loop", "clip.addLoop"),
                        ("Plugins", "plugin.list"),
                    ] {
                        if control::spec(method).is_some()
                            && text_button(ui, label, Face::Raised).clicked()
                        {
                            self.select(method, session);
                        }
                    }
                });
                ui.add(
                    egui::TextEdit::singleline(&mut self.filter)
                        .id(Id::new("agents-command-search"))
                        .font(font(FS_LIST, Weight::Medium))
                        .hint_text("Find a command…")
                        .desired_width(f32::INFINITY),
                );
                let filter = self.filter.trim().to_lowercase();
                let matching: Vec<&Spec> = control::COMMANDS
                    .iter()
                    .filter(|spec| filter.is_empty()
                        || spec.name.to_lowercase().contains(&filter)
                        || spec.doc.to_lowercase().contains(&filter))
                    .collect();
                let mut chosen = None;
                egui::ComboBox::from_id_salt("agents-command")
                    .width(ui.available_width())
                    .selected_text(mono(&self.method, FS_LIST, INK))
                    .show_ui(ui, |ui| {
                        for spec in &matching {
                            if ui.selectable_label(self.method == spec.name, spec.name).clicked() {
                                chosen = Some(spec.name);
                            }
                        }
                        if matching.is_empty() {
                            ui.label("No matching commands");
                        }
                    });
                if let Some(method) = chosen {
                    self.select(method, session);
                }
                ui.label(mono(
                    format!("{} of {} commands", matching.len(), control::COMMANDS.len()),
                    FS_SMALL,
                    FAINT,
                ));
                if let Some(spec) = control::spec(&self.method) {
                    ui.label(text(spec.doc, FS_SECONDARY, Weight::Medium, DIM));
                    ui.label(caps(if spec.mutates { "Changes session, transport or files" } else { "Read only" }));
                    if !spec.params.is_empty() {
                        egui::CollapsingHeader::new("Parameters")
                            .id_salt(("agents-parameters", spec.name))
                            .show(ui, |ui| {
                                for param in spec.params {
                                    ui.label(mono(
                                        format!("{} · {}{}", param.name, param.kind.schema_type(), if param.required { " · required" } else { "" }),
                                        FS_SMALL,
                                        INK,
                                    ));
                                    ui.label(text(param.doc, FS_SMALL, Weight::Medium, DIM));
                                }
                            });
                    }
                }
                ui.add(
                    egui::TextEdit::multiline(&mut self.arguments)
                        .id(Id::new("agents-arguments"))
                        .font(mono_font(FS_SECONDARY))
                        .desired_width(f32::INFINITY)
                        .desired_rows(5),
                );
                let parsed = parse_arguments(&self.arguments).and_then(|params| {
                    control::validate_request(&self.method, &params)?;
                    Ok(params)
                });
                let mut clicked = false;
                ui.add_enabled_ui(parsed.is_ok(), |ui| {
                    let response = text_button(ui, "Run command", Face::Raised);
                    ui.ctx().data_mut(|data| data.insert_temp(Id::new("agents-run-rect"), response.rect));
                    clicked = response.clicked();
                });
                if clicked {
                    if let Ok(params) = parsed.as_ref() {
                        action = Some(Action::Run(self.method.clone(), params.clone()));
                    }
                }
                if let Err(message) = parsed {
                    ui.label(text(message, FS_SMALL, Weight::Medium, INK_DIM));
                }
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(caps("Activity"));
                    ui.label(mono(self.history.len().to_string(), FS_SMALL, FAINT));
                    if !self.history.is_empty()
                        && text_button(ui, "Clear", Face::Raised).clicked()
                    {
                        self.history.clear();
                    }
                });
                if self.history.is_empty() {
                    ui.label(text(
                        "No commands yet. Select Inspect and Run command, or connect your MCP client. Results and errors appear here.",
                        FS_SECONDARY,
                        Weight::Medium,
                        DIM,
                    ));
                }
                for entry in &mut self.history {
                    let response = egui::CollapsingHeader::new(mono(
                        format!("{} · {}", if entry.running { "Running" } else if entry.succeeded { "OK" } else { "Error" }, entry.method),
                        FS_SECONDARY,
                        if entry.succeeded { INK } else { INK_BRIGHT },
                    ))
                    .id_salt(("agent-activity", entry.sequence))
                    .open(Some(entry.expanded))
                    .show(ui, |ui| {
                        ui.label(mono(
                            format!("{} · revision {} → {}", entry.source, entry.before, entry.after),
                            FS_SMALL,
                            FAINT,
                        ));
                        if entry.parameters != "{}" {
                            ui.label(mono(&entry.parameters, FS_SMALL, DIM));
                        }
                        ui.add(egui::Label::new(mono(&entry.detail, FS_SMALL, INK)).wrap());
                        if text_button(ui, "Copy result", Face::Raised).clicked() {
                            ui.ctx().copy_text(entry.detail.clone());
                        }
                    });
                    if response.header_response.clicked() {
                        entry.expanded = !entry.expanded;
                    }
                }
            });
        action
    }
}

impl Ondera {
    pub(crate) fn agents_panel(&mut self, ctx: &egui::Context) {
        let connection = Connection {
            port: self.control.as_ref().map(control::wire::Server::port),
            discovery: self
                .control
                .as_ref()
                .map_or_else(control::wire::discovery_path, |server| {
                    server.path().to_path_buf()
                }),
        };
        let session = self.store.snapshot();
        let mut action = None;
        egui::SidePanel::left("browser")
            .exact_width(BROWSER + HEADER)
            .resizable(false)
            .frame(egui::Frame::new().fill(PANEL).inner_margin(GAP))
            .show(ctx, |ui| action = self.agents.ui(ui, &connection, &session));
        match action {
            Some(Action::ToggleBridge) => {
                if self.control.take().is_some() {
                    self.agents.runner.stop();
                    self.status = "Local agent bridge disabled".into();
                } else {
                    self.start_control(ctx);
                }
            }
            Some(Action::Run(method, params)) => {
                let _ = self.run_control_command(&method, &params, true, "Agents tab");
            }
            Some(Action::StartTask) => {
                if self.control.is_some() {
                    self.agents
                        .runner
                        .start(&connection.discovery, &companion("ondera-mcp"));
                }
            }
            Some(Action::StopTask) => self.agents.runner.stop(),
            None => {}
        }
    }

    pub(crate) fn record_agent_activity(
        &mut self,
        method: &str,
        params: &Value,
        source: &str,
        revision_before: u64,
        result: &Result<Value>,
    ) {
        self.agents.sequence += 1;
        self.agents.last_request = Some(Instant::now());
        for entry in &mut self.agents.history {
            entry.expanded = false;
        }
        self.agents.history.push_front(Activity {
            sequence: self.agents.sequence,
            method: method.into(),
            source: source.into(),
            parameters: bounded(pretty(params)),
            detail: bounded(match result {
                Ok(value) => pretty(value),
                Err(message) => message.clone(),
            }),
            succeeded: result.is_ok(),
            running: result
                .as_ref()
                .is_ok_and(|value| value["status"] == "running"),
            expanded: true,
            before: revision_before,
            after: self.store.revision,
        });
        self.agents.history.truncate(HISTORY_LIMIT);
    }
}

impl AgentPanel {
    #[cfg(test)]
    pub(crate) fn mock_running_task(&mut self) -> impl FnOnce() + use<> {
        self.runner.mock_running_task()
    }

    pub(crate) fn runner_busy(&self) -> bool {
        self.runner.running()
    }

    pub(crate) fn stop_runner(&mut self) {
        self.runner.stop();
    }

    pub(crate) fn poll_runner(&mut self, ctx: &egui::Context) {
        self.runner.poll();
        if self.runner.running() {
            ctx.request_repaint_after(std::time::Duration::from_millis(40));
        }
    }
}

fn parse_arguments(text: &str) -> Result<Value> {
    match serde_json::from_str::<Value>(text) {
        Ok(value @ Value::Object(_)) => Ok(value),
        Ok(_) => Err("Parameters must be a JSON object, for example {}.".into()),
        Err(error) => Err(format!("Check JSON: {error}")),
    }
}

fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_default()
}

fn bounded(text: String) -> String {
    if text.len() <= DETAIL_LIMIT {
        text
    } else {
        let mut end = DETAIL_LIMIT;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        format!(
            "{}\n… Display truncated; query with the CLI for the full response.",
            &text[..end]
        )
    }
}

fn example_arguments(spec: &Spec, session: &Session) -> Value {
    let track = session
        .view
        .selected_track_id
        .as_deref()
        .or_else(|| session.tracks.first().map(|track| track.id.as_str()))
        .unwrap_or("");
    let clip = session
        .view
        .selected_clip_id
        .as_deref()
        .or_else(|| session.clips.first().map(|clip| clip.id.as_str()))
        .unwrap_or("");
    match spec.name {
        "track.add" => return json!({"kind":"midi","name":"New instrument"}),
        "clip.addLoop" => return json!({"name":"Boom Bap 92","startBar":0}),
        "transport.setTempo" => return json!({"bpm":session.transport.tempo}),
        _ => {}
    }
    let mut args = Map::new();
    for param in spec.params.iter().filter(|param| param.required) {
        let value = match param.name {
            "trackId" => json!(track),
            "clipId" => json!(clip),
            "name" => json!("New name"),
            "length" | "lengthBars" => json!(1),
            "pitch" => json!(60),
            "velocity" => json!(100),
            _ => match param.kind {
                Kind::String => json!(""),
                Kind::Number | Kind::Integer => json!(0),
                Kind::Boolean => json!(false),
                Kind::Array => json!([]),
                Kind::Object => json!({}),
            },
        };
        args.insert(param.name.into(), value);
    }
    Value::Object(args)
}

fn companion(name: &str) -> String {
    let executable = format!("{name}{}", std::env::consts::EXE_SUFFIX);
    if let Some(path) = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join(&executable)))
    {
        if path.is_file() {
            return path.to_string_lossy().into_owned();
        }
    }
    executable
}

fn mcp_config(discovery: &Path) -> String {
    pretty(&json!({
        "mcpServers": {
            "ondera": {
                "command": companion("ondera-mcp"),
                "args": ["--live"],
                "env": { "ONDERA_CONTROL": discovery.to_string_lossy() }
            }
        }
    }))
}

fn cli_check(discovery: &Path) -> String {
    let path = discovery.to_string_lossy();
    let executable = companion("ondera-cli");
    if cfg!(windows) {
        format!(
            "$env:ONDERA_CONTROL = '{}'; & '{}' --live session.info",
            path.replace('\'', "''"),
            executable.replace('\'', "''")
        )
    } else {
        format!(
            "ONDERA_CONTROL='{}' '{}' --live session.info",
            path.replace('\'', "'\\''"),
            executable.replace('\'', "'\\''")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ondera_engine::store;

    #[test]
    fn templates_use_real_selection_and_registry() {
        let session = store::demo();
        let args = example_arguments(control::spec("clip.get").unwrap(), &session);
        let clip_id = args["clipId"].as_str().unwrap();
        assert!(session.clips.iter().any(|clip| clip.id == clip_id));
        for spec in control::COMMANDS.iter() {
            let params = example_arguments(spec, &session);
            assert!(params.is_object());
            for required in spec.params.iter().filter(|param| param.required) {
                assert!(
                    params.get(required.name).is_some(),
                    "{} needs {}",
                    spec.name,
                    required.name
                );
            }
        }
    }

    #[test]
    fn malformed_json_and_non_objects_cannot_execute() {
        assert!(parse_arguments("{}").is_ok());
        assert!(parse_arguments("{broken").is_err());
        assert!(parse_arguments("[]").is_err());
        assert!(parse_arguments("null").is_err());
    }

    #[test]
    fn stopping_stays_busy_until_worker_cleanup_has_completed() {
        let mut panel = AgentPanel::default();
        let complete = panel.mock_running_task();
        panel.stop_runner();
        panel.poll_runner(&egui::Context::default());
        assert!(panel.runner_busy());
        complete();
        panel.poll_runner(&egui::Context::default());
        assert!(!panel.runner_busy());
        assert!(panel.runner.status.starts_with("Stopped"));
    }

    #[test]
    fn natural_language_run_requires_a_prompt_and_connected_bridge() {
        for (port, prompt, should_run) in [
            (Some(12345), "Write a bass line", true),
            (None, "Write a bass line", false),
            (Some(12345), "", false),
        ] {
            let ctx = egui::Context::default();
            install(&ctx);
            let mut panel = AgentPanel::default();
            panel.runner.prompt = prompt.into();
            let connection = Connection {
                port,
                discovery: "/tmp/control.json".into(),
            };
            let session = store::demo();
            let input = |events| egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    vec2(520.0, 1400.0),
                )),
                events,
                focused: true,
                ..Default::default()
            };
            let _ = ctx.run(input(vec![]), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    panel.ui(ui, &connection, &session);
                });
            });
            let rect = ctx
                .data(|data| data.get_temp::<egui::Rect>(Id::new("agents-task-run-rect")))
                .unwrap();
            let mut action = None;
            for pressed in [true, false] {
                let _ = ctx.run(
                    input(vec![
                        egui::Event::PointerMoved(rect.center()),
                        egui::Event::PointerButton {
                            pos: rect.center(),
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ]),
                    |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| {
                            action = panel.ui(ui, &connection, &session);
                        });
                    },
                );
            }
            assert_eq!(matches!(action, Some(Action::StartTask)), should_run);
        }
    }

    #[test]
    fn copied_configuration_targets_this_window_without_copying_secrets() {
        let path = Path::new("/tmp/Ondera user's session/control.json");
        let config: Value = serde_json::from_str(&mcp_config(path)).unwrap();
        assert_eq!(config["mcpServers"]["ondera"]["args"], json!(["--live"]));
        assert_eq!(
            config["mcpServers"]["ondera"]["env"]["ONDERA_CONTROL"],
            path.to_string_lossy().as_ref()
        );
        assert!(!mcp_config(path).contains("token"));
        let check = cli_check(path);
        assert!(check.contains("--live session.info"));
        assert!(check.contains("ONDERA_CONTROL"));
    }

    #[test]
    fn activity_records_success_errors_and_shared_history() {
        let mut app = Ondera::from_session(store::demo(), None);
        let before = app.store.revision;
        let params = json!({"name":"Agent session"});
        let result = control::call(&mut app, "session.rename", &params, true);
        app.record_agent_activity("session.rename", &params, "test", before, &result);
        assert!(app.agents.history[0].succeeded);
        assert!(app.agents.history[0].after > before);
        assert_eq!(app.store.session().name, "Agent session");
        control::call(&mut app, "history.undo", &json!({}), true).unwrap();
        assert_ne!(app.store.session().name, "Agent session");
        let before = app.store.revision;
        let result = control::call(
            &mut app,
            "track.remove",
            &json!({"trackId":"missing"}),
            true,
        );
        app.record_agent_activity("track.remove", &json!({}), "test", before, &result);
        assert!(!app.agents.history[0].succeeded);
        assert_eq!(app.store.revision, before);
        for _ in 0..HISTORY_LIMIT + 1 {
            app.record_agent_activity("session.info", &json!({}), "test", before, &Ok(json!({})));
        }
        assert_eq!(app.agents.history.len(), HISTORY_LIMIT);
    }

    #[test]
    fn native_workbench_run_button_returns_validated_object() {
        let ctx = egui::Context::default();
        install(&ctx);
        let mut panel = AgentPanel {
            open: true,
            ..Default::default()
        };
        let connection = Connection {
            port: Some(12345),
            discovery: "/tmp/control.json".into(),
        };
        let session = store::demo();
        let input = |events| egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                vec2(520.0, 1400.0),
            )),
            events,
            focused: true,
            ..Default::default()
        };
        let _ = ctx.run(input(vec![]), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                panel.ui(ui, &connection, &session);
            });
        });
        let rect = ctx
            .data(|data| data.get_temp::<egui::Rect>(Id::new("agents-run-rect")))
            .unwrap();
        let mut action = None;
        for pressed in [true, false] {
            let events = vec![
                egui::Event::PointerMoved(rect.center()),
                egui::Event::PointerButton {
                    pos: rect.center(),
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ];
            let _ = ctx.run(input(events), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    action = panel.ui(ui, &connection, &session);
                });
            });
        }
        assert!(
            matches!(action, Some(Action::Run(method, params)) if method == "session.info" && params == json!({}))
        );
    }

    #[test]
    fn large_activity_is_bounded_on_a_utf8_boundary() {
        let output = bounded("🎹".repeat(DETAIL_LIMIT));
        assert!(output.len() < DETAIL_LIMIT + 100);
        assert!(output.contains("Display truncated"));
    }
}
