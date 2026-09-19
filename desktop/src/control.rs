//! Live control: the window serves the shared command registry over the loopback socket, so
//! `ondera-cli` and `ondera-mcp` edit the same session a person is looking at. Requests are
//! answered on the interface thread between frames; nothing here touches the audio callback.

use crate::app::{Intent, Ondera};
use ondera_engine::{
    audio::{self, Library},
    control::{self, wire, Headless, Host},
    control_app, document, midi, recovery, render,
    session_file::SessionFileLock,
    settings::Settings,
    store::{Command, Store},
    Result,
};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::mpsc,
    time::Instant,
};

const TOOLS: [&str; 3] = ["pointer", "pencil", "scissors"];
const BROWSER_TABS: [&str; 4] = ["instruments", "loops", "plugins", "files"];

/// Who is waiting for a deferred command: a bridge client or the built-in agent.
pub(crate) enum Reply {
    Wire(wire::Request),
    Channel(mpsc::SyncSender<Result<Value>>),
}
impl Reply {
    pub(crate) fn respond(self, result: Result<Value>) {
        match self {
            Reply::Wire(request) => request.respond(result),
            Reply::Channel(sender) => {
                let _ = sender.send(result);
            }
        }
    }
}

pub(crate) struct ControlJob {
    receiver: mpsc::Receiver<Result<(Value, Headless, Option<SessionFileLock>)>>,
    reply: Option<Reply>,
    method: String,
    params: Value,
    source: String,
    revision: u64,
    undo_depth: usize,
}

/// Interface work that completes on a later frame: a screenshot, an update check or install.
pub(crate) enum LiveWait {
    /// Work on another thread (the network, an offline render) that answers when it is done.
    Worker(mpsc::Receiver<Result<Value>>),
    Screenshot {
        path: PathBuf,
        requested: bool,
    },
    UpdateCheck,
    UpdateInstall,
}
pub(crate) struct LiveJob {
    pub wait: LiveWait,
    pub reply: Option<Reply>,
    method: String,
    params: Value,
    source: String,
    revision: u64,
    undo_depth: usize,
    started: Instant,
}

/// Set by the Tauri window so a waiting CLI or MCP request is answered at once instead of on
/// the next 33 ms tick.
pub(crate) static CONTROL_WAKE: std::sync::OnceLock<Box<dyn Fn() + Send + Sync>> =
    std::sync::OnceLock::new();

impl Ondera {
    pub(crate) fn start_control(&mut self, ctx: &eframe::egui::Context) {
        let ctx = ctx.clone();
        match wire::Server::start(move || {
            ctx.request_repaint();
            if let Some(wake) = CONTROL_WAKE.get() {
                wake();
            }
        }) {
            Ok(server) => self.control = Some(server),
            Err(e) => self.status = format!("Live control unavailable: {e}"),
        }
    }
    /// Answer every queued request once per frame. Requests run with no gesture in progress,
    /// so each command is its own undo step and a drag that spans them is split around them;
    /// an idle server leaves the gesture untouched so a drag stays one undo step.
    pub(crate) fn serve_control(&mut self, gesture: bool) {
        let pending = self
            .control
            .as_ref()
            .map(wire::Server::drain)
            .unwrap_or_default();
        if pending.is_empty() {
            return;
        }
        self.store.set_gesture(false);
        for request in pending {
            let result = self.run_control_command(
                &request.method,
                &request.params,
                request.agent,
                if request.agent { "MCP / agent" } else { "CLI" },
            );
            let running = result
                .as_ref()
                .is_ok_and(|value| value["status"] == "running");
            if running {
                if let Err(Reply::Wire(request)) = self.attach_reply(Reply::Wire(request)) {
                    request.respond(result);
                }
            } else {
                request.respond(result);
            }
        }
        self.store.set_gesture(gesture);
    }
    /// Hand a waiting party to the job the last command started. Returns the reply when
    /// nothing is pending so the caller can answer immediately.
    pub(crate) fn attach_reply(&mut self, reply: Reply) -> std::result::Result<(), Reply> {
        if let Some(job) = self.control_job.as_mut().filter(|job| job.reply.is_none()) {
            job.reply = Some(reply);
            return Ok(());
        }
        if let Some(index) = self.attach_live.take() {
            if let Some(job) = self
                .live_jobs
                .get_mut(index)
                .filter(|job| job.reply.is_none())
            {
                job.reply = Some(reply);
                return Ok(());
            }
        }
        Err(reply)
    }
    pub(crate) fn run_control_command(
        &mut self,
        method: &str,
        params: &Value,
        agent: bool,
        source: &str,
    ) -> Result<Value> {
        if method == "session.batch" && !self.batching {
            return self.run_batch(params, agent, source);
        }
        let before = self.store.revision;
        let depth_before = self.store.undo_depth();
        self.attach_live = None;
        let result = (|| {
            control::validate_request(method, params)?;
            if method.starts_with("take.") && method != "take.list" && self.agents.runtime.running()
            {
                return Err("Stop the agent before switching or saving creative takes".into());
            }
            if agent {
                if method == "agent.configure"
                    || (matches!(method, "settings.set" | "settings.reset")
                        && params["path"].as_str().is_none_or(|p| {
                            p.trim() == "agent"
                                || p.trim().starts_with("agent.")
                                || p.trim() == "control"
                                || p.trim().starts_with("control.")
                        }))
                {
                    return Err("Agent connections and permissions must be changed by the person in Settings".into());
                }
                if let Some(denied) =
                    control_app::denied_for_agent(method, &self.settings.agent.permissions)
                {
                    return Err(denied);
                }
            }
            if matches!(method, "session.new" | "session.open") {
                self.can_replace_document()?;
            }
            if self.control_job.is_some()
                && control::COMMANDS
                    .iter()
                    .any(|s| s.name == method && s.mutates)
            {
                return Err(
                    "An agent file operation is in progress; retry when it finishes.".into(),
                );
            }
            if matches!(
                method,
                "session.open"
                    | "session.save"
                    | "session.bounce"
                    | "session.importAudio"
                    | "session.importMidi"
                    | "session.exportMidi"
                    | "session.exportAudio"
                    | "session.exportStems"
                    | "plugin.scan"
            ) {
                self.available()?;
                // Preparing a graph is routine and may be discarded safely. File operations
                // cannot interrupt a take or another operation.
                self.guarded(Ondera::stop)?;
                if matches!(
                    method,
                    "session.save"
                        | "session.bounce"
                        | "session.exportAudio"
                        | "session.exportStems"
                ) {
                    self.guarded(Ondera::capture_plugin_states)?;
                }
                let mut session = self.store.session().clone();
                session.transport.position_beats = self.position;
                session.view.pixels_per_bar = self.zoom;
                session.view.scroll_bars = self.scroll;
                let mut host = Headless {
                    store: Store::new(session)?,
                    library: self.library.clone(),
                    path: self.path.clone(),
                    position: self.position,
                    clipboard: None,
                    lane_width: self.lane_width,
                };
                let revision = self.store.revision;
                let (method_owned, mut params_owned) = (method.to_string(), params.clone());
                let ownership = if matches!(method, "session.open" | "session.save") {
                    let path = params
                        .get("path")
                        .and_then(Value::as_str)
                        .map(PathBuf::from)
                        .or_else(|| {
                            if method == "session.save" {
                                self.path.clone()
                            } else {
                                None
                            }
                        })
                        .ok_or("The session has no file yet: pass `path`.")?;
                    let ownership =
                        SessionFileLock::acquire_or_reuse(&path, self.session_file.as_ref())?;
                    if !params_owned.is_object() {
                        params_owned = json!({});
                    }
                    params_owned["path"] = json!(ownership.path());
                    Some(ownership)
                } else {
                    None
                };
                let (tx, rx) = mpsc::sync_channel(1);
                std::thread::spawn(move || {
                    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let result = control::call(&mut host, &method_owned, &params_owned, agent)?;
                        Ok((result, host, ownership))
                    }))
                    .unwrap_or_else(|_| {
                        Err("Agent file operation failed; the open document is intact.".into())
                    });
                    let _ = tx.send(outcome);
                });
                self.control_job = Some(ControlJob {
                    receiver: rx,
                    reply: None,
                    method: method.into(),
                    params: params.clone(),
                    source: source.into(),
                    revision,
                    undo_depth: self.store.undo_depth(),
                });
                self.status = format!("Running {method}…");
                return Ok(json!({"status":"running", "command":method}));
            }
            if method == "rhythm.preview" {
                // An offline render: seconds of work that must not hold the interface, and that
                // needs nothing from the open document but its tempo and meter.
                let mut scratch = Headless::new();
                scratch.store.dispatch(Command::SetTransport(
                    self.store.session().transport.clone(),
                ))?;
                let params_owned = params.clone();
                return Ok(self.start_worker(method, params, source, move || {
                    control::call(&mut scratch, "rhythm.preview", &params_owned, false)
                }));
            }
            if source != "Interface" && !self.batching {
                self.store.set_gesture(false);
            }
            let mut result = control::call(self, method, params, agent)?;
            if matches!(method, "transport.record" | "transport.stop") {
                result["pending"] =
                    json!(self.record_pending.is_some() || self.record_finishing.is_some());
            }
            Ok(result)
        })();
        // The entries of a batch share one undo step, so they are one change: `run_batch`
        // records it. A change per entry would offer Reverts that each undo the whole batch.
        if !self.batching {
            self.record_agent_activity(method, params, source, before, depth_before, &result);
        }
        result
    }
    /// `session.batch` in the window: every entry passes the same permission and busy checks
    /// as a command sent on its own, and the whole list lands as one undo step.
    fn run_batch(&mut self, params: &Value, agent: bool, source: &str) -> Result<Value> {
        use ondera_engine::control_edit;
        control::validate_request("session.batch", params)?;
        let entries = control_edit::batch_entries(params)?;
        let atomic = params["atomic"].as_bool().unwrap_or(true);
        let (before, depth_before) = (self.store.revision, self.store.undo_depth());
        self.store.set_gesture(true);
        self.batching = true;
        let mut results = Vec::with_capacity(entries.len());
        let mut failure = None;
        for (index, (command, params)) in entries.iter().enumerate() {
            match self.run_control_command(command, params, agent, source) {
                Ok(value) => results.push(value),
                Err(error) => {
                    failure = Some((index, command.as_str(), error));
                    break;
                }
            }
        }
        self.batching = false;
        let outcome = match failure {
            Some((index, command, error)) => {
                let rolled_back = atomic && self.store.cancel_gesture();
                Err(control_edit::batch_error(
                    index,
                    command,
                    &error,
                    rolled_back,
                ))
            }
            None => Ok(control_edit::batch_reply(results)),
        };
        self.store.set_gesture(false);
        self.record_agent_activity(
            "session.batch",
            params,
            source,
            before,
            depth_before,
            &outcome,
        );
        outcome
    }
    pub(crate) fn poll_control_job(&mut self) {
        let outcome = self
            .control_job
            .as_ref()
            .and_then(|job| match job.receiver.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Disconnected) => Some(Err(
                    "Agent worker stopped before completing the operation.".into(),
                )),
                Err(mpsc::TryRecvError::Empty) => None,
            });
        let Some(outcome) = outcome else { return };
        let mut job = self.control_job.take().unwrap();
        let result = outcome.and_then(|(mut value, host, ownership)| {
            match job.method.as_str() {
                "session.open" => {
                    self.loaded(
                        host.store.session().clone(),
                        host.library,
                        host.path.unwrap(),
                        ownership,
                    );
                    value = control::call(self, "session.info", &json!({}), false)?;
                }
                "session.save" => {
                    self.saved(
                        host.path.unwrap(),
                        job.revision,
                        ownership.expect("save acquired ownership"),
                    );
                    value["dirty"] = json!(self.store.dirty());
                }
                "session.importAudio" | "session.importMidi" => {
                    let prior = self.store.snapshot();
                    let next = host.store.session();
                    let mut commands = vec![];
                    for track in &next.tracks {
                        if !prior.tracks.iter().any(|t| t.id == track.id) {
                            commands.push(Command::AddTrack(track.clone()));
                        }
                    }
                    for source in next.sources.values() {
                        if !prior.sources.contains_key(&source.id) {
                            commands.push(Command::PutSource(source.clone()));
                        }
                    }
                    for clip in &next.clips {
                        if !prior.clips.iter().any(|c| c.id == clip.id) {
                            commands.push(Command::PutClip(clip.clone()));
                        }
                    }
                    for (track, strip) in &next.strips {
                        if !prior.strips.contains_key(track) {
                            commands.push(Command::SetStrip {
                                track: track.clone(),
                                strip: strip.clone(),
                            });
                        }
                    }
                    if job.method == "session.importMidi" {
                        commands.push(Command::SetTransport(next.transport.clone()));
                    }
                    self.try_dispatch(Command::Batch(commands))?;
                    self.library = host.library;
                }
                "plugin.scan" => {
                    self.catalog = ondera_engine::host::scan::installed();
                    self.plugins.failed.clear();
                }
                _ => {}
            }
            self.status = format!("{} complete", job.method);
            Ok(value)
        });
        self.export.completed(&job.method, &result);
        self.record_agent_activity(
            &job.method,
            &job.params,
            &job.source,
            job.revision,
            job.undo_depth,
            &result,
        );
        if let Err(error) = &result {
            self.error = Some(error.clone());
        }
        if let Some(reply) = job.reply.take() {
            reply.respond(result);
        }
    }
    /// Start a deferred interface job and mark it for the reply of the current command.
    fn start_live(&mut self, wait: LiveWait, method: &str, params: &Value, source: &str) -> Value {
        self.live_jobs.push(LiveJob {
            wait,
            reply: None,
            method: method.into(),
            params: params.clone(),
            source: source.into(),
            revision: self.store.revision,
            undo_depth: self.store.undo_depth(),
            started: Instant::now(),
        });
        self.attach_live = Some(self.live_jobs.len() - 1);
        json!({ "status": "running", "command": method })
    }
    /// Run `work` off the interface thread as a live job: the caller is told "running" and
    /// gets the result when it arrives, and the window never waits on the network.
    fn start_worker(
        &mut self,
        method: &str,
        params: &Value,
        source: &str,
        work: impl FnOnce() -> Result<Value> + Send + 'static,
    ) -> Value {
        let (tx, rx) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work))
                .unwrap_or_else(|_| Err("The background job failed".into()));
            let _ = tx.send(result);
            if let Some(wake) = CONTROL_WAKE.get() {
                wake();
            }
        });
        self.start_live(LiveWait::Worker(rx), method, params, source)
    }
    /// Answer the worker jobs that have finished.
    pub(crate) fn poll_workers(&mut self) {
        let mut index = 0;
        while index < self.live_jobs.len() {
            let outcome = match &self.live_jobs[index].wait {
                LiveWait::Worker(receiver) => match receiver.try_recv() {
                    Ok(result) => Some(result),
                    Err(mpsc::TryRecvError::Disconnected) => {
                        Some(Err("The background job stopped before it finished".into()))
                    }
                    Err(mpsc::TryRecvError::Empty) => None,
                },
                _ => None,
            };
            let Some(result) = outcome else {
                index += 1;
                continue;
            };
            let job = self.live_jobs.remove(index);
            if self.attach_live.is_some_and(|waiting| waiting >= index) {
                self.attach_live = None;
            }
            self.record_agent_activity(
                &job.method,
                &job.params,
                &job.source,
                job.revision,
                job.undo_depth,
                &result,
            );
            if let Some(reply) = job.reply {
                reply.respond(result);
            }
        }
    }
    /// Ask the viewport for pending screenshots; time out jobs nobody can finish.
    pub(crate) fn poll_live_jobs(&mut self, ctx: &eframe::egui::Context) {
        self.poll_workers();
        let mut request_capture = false;
        for job in &mut self.live_jobs {
            if let LiveWait::Screenshot { requested, .. } = &mut job.wait {
                if !*requested {
                    *requested = true;
                    request_capture = true;
                }
            }
        }
        if request_capture {
            ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Screenshot(Default::default()));
        }
        let expired: Vec<usize> = self
            .live_jobs
            .iter()
            .enumerate()
            .filter(|(_, job)| job.started.elapsed().as_secs() > 600)
            .map(|(i, _)| i)
            .collect();
        for index in expired.into_iter().rev() {
            let job = self.live_jobs.remove(index);
            let result = Err(format!("{} did not complete in time", job.method));
            self.record_agent_activity(
                &job.method,
                &job.params,
                &job.source,
                job.revision,
                job.undo_depth,
                &result,
            );
            if let Some(reply) = job.reply {
                reply.respond(result);
            }
        }
        if !self.live_jobs.is_empty() {
            ctx.request_repaint();
        }
    }
    /// Complete every pending live job of one kind with the same result.
    pub(crate) fn finish_live(&mut self, pick: impl Fn(&LiveWait) -> bool, result: Result<Value>) {
        let (done, rest): (Vec<LiveJob>, Vec<LiveJob>) = std::mem::take(&mut self.live_jobs)
            .into_iter()
            .partition(|job| pick(&job.wait));
        self.live_jobs = rest;
        for job in done {
            self.record_agent_activity(
                &job.method,
                &job.params,
                &job.source,
                job.revision,
                job.undo_depth,
                &result,
            );
            if let Some(reply) = job.reply {
                reply.respond(result.clone());
            }
        }
    }
    /// A finished window capture: write it where the live job asked.
    pub(crate) fn deliver_screenshot(&mut self, image: &eframe::egui::ColorImage) -> bool {
        let Some(index) = self
            .live_jobs
            .iter()
            .position(|job| matches!(job.wait, LiveWait::Screenshot { .. }))
        else {
            return false;
        };
        let path = match &self.live_jobs[index].wait {
            LiveWait::Screenshot { path, .. } => path.clone(),
            _ => unreachable!(),
        };
        let result = (|| {
            if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let data: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
            image::save_buffer(
                &path,
                &data,
                image.width() as u32,
                image.height() as u32,
                image::ColorType::Rgba8,
            )
            .map_err(|e| e.to_string())?;
            Ok(json!({
                "path": path,
                "width": image.width(),
                "height": image.height(),
                "bytes": std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0),
            }))
        })();
        let target = path.clone();
        self.finish_live(
            move |wait| matches!(wait, LiveWait::Screenshot { path, .. } if *path == target),
            result,
        );
        true
    }
    fn audio_status(&self) -> Value {
        let (peaks, cpu) = self.device.as_ref().map_or(([0.0; 4], 0.0), |d| {
            (d.telemetry.peaks(), d.telemetry.load())
        });
        json!({
            "device": self.device.as_ref().map(|d| d.device_name.clone()),
            "sampleRate": self.device.as_ref().map(|d| d.sample_rate),
            "bufferFrames": self.device.as_ref().map(|d| d.telemetry.output_frames.load(std::sync::atomic::Ordering::Relaxed)),
            "cpuLoad": cpu,
            "masterPeak": [peaks[0], peaks[1]],
            "selectedTrackPeak": [peaks[2], peaks[3]],
            "midiInput": self.midi.as_ref().map(|m| m.port_name.clone()),
            "outputDevice": self.output_device,
            "inputDevice": self.input_device,
            "playing": self.playing,
            "recording": self.midi_recording || self.recorder.is_some(),
            "recordEnabled": self.record_enabled,
            "musicalTyping": self.musical_typing,
            "monitoring": self.monitoring_status(),
        })
    }
    fn monitoring_status(&self) -> Value {
        use ondera_engine::device::Monitoring;
        use std::sync::atomic::Ordering::Relaxed;
        let (state, input, rate, reason) = match &self.monitoring {
            Monitoring::Off => ("off", None, None, None),
            Monitoring::On { input_name, input_rate } => {
                ("on", Some(input_name.clone()), Some(*input_rate), None)
            }
            Monitoring::FeedbackRisk { input_name } => (
                "blocked",
                Some(input_name.clone()),
                None,
                Some("The built-in microphone would feed back through the built-in speakers. Use headphones, or allow it with audio.allowSpeakerMonitoring.".to_string()),
            ),
            Monitoring::Failed(error) => ("failed", None, None, Some(error.clone())),
        };
        let mut value = json!({
            "state": state, "inputDevice": input, "inputRate": rate, "reason": reason,
            "speakersAllowed": self.monitor_speakers_ok,
        });
        if let Some(d) = self.device.as_ref().filter(|_| state == "on") {
            let t = &d.telemetry;
            value["inputFrames"] = json!(t.input_frames.load(Relaxed));
            value["outputFrames"] = json!(t.output_frames.load(Relaxed));
            value["ringFrames"] = json!(t.monitor_fill.load(Relaxed));
            value["latencyMs"] = json!(t
                .monitor_latency_ms(d.sample_rate)
                .map(|ms| (ms * 10.0).round() / 10.0));
            value["drops"] = json!(t.monitor_drops.load(Relaxed));
            value["underruns"] = json!(t.monitor_underruns.load(Relaxed));
        }
        value
    }
    fn ui_status(&self) -> Value {
        let session = self.store.session();
        json!({
            "frontendReady": self.frontend_ready,
            "agentPanel": self.agents.open,
            "automation": self.automation.open,
            "settings": self.settings_ui.open,
            "settingsSection": crate::settings::SECTION_KEYS[self.settings_ui.section.min(7)],
            "help": self.show_help,
            "mixer": self.show_mixer,
            "palette": self.show_palette,
            "tool": TOOLS[self.tool.min(2)],
            "musicalTyping": self.musical_typing,
            "pluginWindows": self.plugins.windows.keys().cloned().collect::<Vec<_>>(),
            "status": self.status,
            "error": self.error,
            "playing": self.playing,
            "recordEnabled": self.record_enabled,
            "pixelsPerBar": self.zoom,
            "scrollBar": self.scroll,
            "browserTab": session.view.browser_tab,
            "browserSelection": session.view.browser_selection,
            "selectedTrackId": session.view.selected_track_id,
            "selectedClipId": session.view.selected_clip_id,
            "busy": self.job.is_some() || self.control_job.is_some(),
            "prompt": self.intent.map(|intent| match intent {
                crate::app::Intent::New => "new",
                crate::app::Intent::Open => "open",
                crate::app::Intent::Recover => "recover",
                crate::app::Intent::Demo => "demo",
                crate::app::Intent::Quit => "quit",
                crate::app::Intent::Relaunch => "relaunch",
            }),
            "recoveredTake": self.unplaced_recording.is_some(),
            "monitorBlocked": matches!(
                self.monitoring,
                ondera_engine::device::Monitoring::FeedbackRisk { .. }
            ),
            "heldNotes": self.typing_down,
        })
    }
    fn available(&self) -> Result<()> {
        if (self.job.is_some() && !self.preparing) || self.control_job.is_some() {
            return Err("Ondera is busy with a file operation; retry in a moment".into());
        }
        if self.midi_recording
            || self.recorder.is_some()
            || self.record_pending.is_some()
            || self.record_finishing.is_some()
        {
            return Err("A recording is in progress; stop the transport first".into());
        }
        Ok(())
    }
    fn can_replace_document(&self) -> Result<()> {
        if self.unplaced_recording.is_some() {
            return Err("Save the recovered recording before replacing this session".into());
        }
        Ok(())
    }
    /// Run an interface action and report the error it would have shown, leaving any message
    /// the person was already reading in place.
    pub(crate) fn guarded(&mut self, action: impl FnOnce(&mut Self)) -> Result<()> {
        let shown = self.error.take();
        action(self);
        let result = self.error.take().map_or(Ok(()), Err);
        self.error = shown;
        result
    }
}

impl Host for Ondera {
    fn store(&self) -> &Store {
        &self.store
    }
    fn store_mut(&mut self) -> &mut Store {
        &mut self.store
    }
    fn library(&self) -> &Library {
        &self.library
    }
    fn library_mut(&mut self) -> &mut Library {
        &mut self.library
    }
    fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
    fn mode(&self) -> &'static str {
        "live"
    }
    fn playing(&self) -> bool {
        self.playing
    }
    fn recording(&self) -> bool {
        self.midi_recording || self.recorder.is_some()
    }
    fn record(&mut self) -> Result<()> {
        self.available()?;
        if !self.store.session().tracks.iter().any(|track| track.armed) {
            return Err("Arm an audio or MIDI track before recording.".into());
        }
        if self.store.session().transport.cycle {
            return Err("Disable cycle before recording a linear take.".into());
        }
        if self.sync_needed || self.synced_revision != Some(self.store.revision) {
            return Err("Audio is still updating; retry in a moment.".into());
        }
        self.record_enabled = true;
        if self.playing {
            self.guarded(Ondera::start_recording)
        } else {
            self.guarded(Ondera::play)
        }
    }
    fn plugin_parameters(&mut self, track: &str, slot: Option<usize>) -> Result<Value> {
        let session = self.store.session();
        if !session.tracks.iter().any(|t| t.id == track) && !ondera_engine::model::is_bus(track) {
            return Err("Track or bus not found".into());
        }
        let strip = session.strips.get(track).cloned().unwrap_or_default();
        let insert = if let Some(slot) = slot {
            strip
                .inserts
                .get(slot)
                .cloned()
                .filter(|i| !i.is_empty())
                .ok_or("Plugin slot is empty")?
        } else {
            if !session
                .tracks
                .iter()
                .any(|t| t.id == track && t.kind == "midi")
            {
                return Err("Instruments require a MIDI track".into());
            }
            strip.synth.clone().unwrap_or_else(|| {
                ondera_engine::model::Insert::new(
                    strip.synth_key(track),
                    &format!("stock:{}", strip.instrument),
                    &strip.instrument,
                )
            })
        };
        let entry = self
            .plugins
            .loaded
            .get(&insert.id)
            .filter(|entry| !entry.retiring)
            .ok_or("Plugin is still loading or failed to load; inspect the app status")?;
        Ok(json!({"pluginId":entry.plugin_id,
            "parameters":entry.editor.params().iter().map(|p| json!({"id":p.id,"name":p.name,
                "min":p.min,"max":p.max,"default":p.default,"value":insert.params.get(&p.id).copied()
                    .or_else(||entry.editor.value(p.id)).unwrap_or(p.default),"unit":p.unit,
                "steps":p.steps,"logarithmic":p.log,"labels":p.labels})).collect::<Vec<_>>() }))
    }
    fn position(&self) -> f64 {
        self.position
    }
    fn dispatch(&mut self, command: Command) -> Result<bool> {
        self.try_dispatch(command)
    }
    fn play(&mut self) -> Result<()> {
        if self.playing {
            return Ok(());
        }
        if self.sync_needed || self.synced_revision != Some(self.store.revision) {
            return Err("Audio is still updating after the last edit; retry in a moment".into());
        }
        self.guarded(Ondera::play)?;
        if self.playing {
            Ok(())
        } else {
            Err("Playback did not start".into())
        }
    }
    fn stop(&mut self) -> Result<()> {
        self.guarded(Ondera::stop)
    }
    fn locate(&mut self, beats: f64) -> Result<()> {
        self.guarded(|app| Ondera::locate(app, beats))
    }
    fn new_session(&mut self, demo: bool) -> Result<()> {
        self.available()?;
        self.can_replace_document()?;
        Ondera::stop(self);
        self.guarded(|app| app.execute(if demo { Intent::Demo } else { Intent::New }))
    }
    fn open(&mut self, path: &Path) -> Result<()> {
        self.available()?;
        self.can_replace_document()?;
        let ownership = SessionFileLock::acquire_or_reuse(path, self.session_file.as_ref())?;
        let path = ownership.path().to_path_buf();
        Ondera::stop(self);
        let (session, library) = document::load(&path)?;
        self.guarded(|app| app.loaded(session, library, path, Some(ownership)))
    }
    fn save(&mut self, path: Option<&Path>) -> Result<PathBuf> {
        self.available()?;
        Ondera::stop(self);
        self.guarded(Ondera::capture_plugin_states)?;
        let mut path = path
            .map(Path::to_path_buf)
            .or_else(|| self.path.clone())
            .ok_or("The session has no file yet: pass `path`.")?;
        if path.extension().is_none() {
            path.set_extension("ondera");
        }
        let ownership = SessionFileLock::acquire_or_reuse(&path, self.session_file.as_ref())?;
        let path = ownership.path().to_path_buf();
        let mut session = (*self.store.snapshot()).clone();
        session.transport.position_beats = self.position;
        session.view.pixels_per_bar = self.zoom;
        session.view.scroll_bars = self.scroll;
        let revision = self.store.revision;
        document::save(&session, &self.library, &path)?;
        self.saved(path.clone(), revision, ownership);
        Ok(path)
    }
    fn bounce(&mut self, path: &Path) -> Result<()> {
        self.available()?;
        Ondera::stop(self);
        self.guarded(Ondera::capture_plugin_states)?;
        let session = self.store.snapshot();
        let mut library = self.library.clone();
        audio::prepare_sources(&session, &mut library)?;
        render::bounce(&session, &library, path, 48000)?;
        self.status = "WAV export complete".into();
        Ok(())
    }
    fn view_state(&self) -> (f32, f64) {
        (self.zoom, self.scroll)
    }
    fn view_changed(&mut self) {
        let view = &self.store.session().view;
        self.zoom = view.pixels_per_bar.clamp(12.0, 480.0);
        self.scroll = view.scroll_bars.max(0.0);
        self.browser_tab = BROWSER_TABS
            .iter()
            .position(|tab| *tab == view.browser_tab)
            .unwrap_or(0);
    }
    fn lane_width(&self) -> f64 {
        self.lane_width
    }
    fn set_lane_width(&mut self, pixels: f64) {
        self.lane_width = pixels;
    }
    fn clipboard(&self) -> Option<&ondera_engine::model::Clip> {
        self.clipboard.as_ref()
    }
    fn set_clipboard(&mut self, clip: Option<ondera_engine::model::Clip>) {
        self.clipboard = clip;
    }
    fn capture_states(&mut self) -> Result<()> {
        self.guarded(Ondera::capture_plugin_states)
    }
    fn settings(&self) -> Settings {
        self.settings.clone()
    }
    fn update_settings(&mut self, settings: Settings) -> Result<()> {
        self.apply_settings(settings)
    }
    fn live(&mut self, action: &str, params: &Value) -> Result<Value> {
        let source = "live";
        match action {
            "audio.status" => Ok(self.audio_status()),
            "audio.allowSpeakerMonitoring" => {
                self.monitor_speakers_ok = params["allow"].as_bool().unwrap_or(false);
                self.poll_input_meter();
                Ok(self.audio_status())
            }
            "audio.setOutput" | "audio.setInput" => {
                let name = params["name"].as_str().map(str::to_string);
                if let Some(name) = &name {
                    let known = if action == "audio.setOutput" {
                        ondera_engine::device::output_devices()
                    } else {
                        ondera_engine::device::input_devices()
                    };
                    if !known.contains(name) {
                        return Err(format!("Unknown device `{name}`. See audio.devices."));
                    }
                }
                let mut settings = self.settings.clone();
                if action == "audio.setOutput" {
                    settings.audio.output_device = name;
                } else {
                    settings.audio.input_device = name;
                }
                self.apply_settings(settings)?;
                Ok(self.audio_status())
            }
            "audio.setMidiInput" => {
                let port = params["port"].as_str().map(str::to_string);
                if let Some(port) = &port {
                    if !midi::ports().contains(port) {
                        return Err(format!("Unknown MIDI port `{port}`. See audio.devices."));
                    }
                }
                let mut settings = self.settings.clone();
                settings.audio.midi_input = port;
                self.apply_settings(settings)?;
                Ok(self.audio_status())
            }
            "audio.reconnect" => {
                self.connect();
                Ok(json!({ "reconnecting": true }))
            }
            "note.preview" => {
                let track = params["trackId"]
                    .as_str()
                    .ok_or("note.preview needs `trackId`")?;
                if !self
                    .store
                    .session()
                    .tracks
                    .iter()
                    .any(|t| t.id == track && t.kind == "midi")
                {
                    return Err("Preview needs an instrument track".into());
                }
                let pitch = params["pitch"]
                    .as_i64()
                    .filter(|p| (0..=127).contains(p))
                    .ok_or("pitch must be 0-127")?;
                let velocity = params["velocity"].as_i64().unwrap_or(100);
                if !(1..=127).contains(&velocity) {
                    return Err("velocity must be 1-127".into());
                }
                let track = track.to_string();
                self.preview(&track, pitch as u8, velocity as u8);
                Ok(json!({ "trackId": track, "pitch": pitch, "velocity": velocity }))
            }
            "note.hold" => {
                let pitch = params["pitch"]
                    .as_i64()
                    .filter(|p| (0..=127).contains(p))
                    .ok_or("pitch must be 0-127")? as u8;
                let velocity = params["velocity"].as_i64().unwrap_or(100);
                if !(1..=127).contains(&velocity) {
                    return Err("velocity must be 1-127".into());
                }
                let on = params["on"].as_bool().ok_or("note.hold needs `on`")?;
                if on {
                    if self.midi_route.load(std::sync::atomic::Ordering::Relaxed)
                        == ondera_engine::midi::UNROUTED
                    {
                        return Err("Select an instrument track before holding a note".into());
                    }
                    if !self.typing_down.contains(&pitch) {
                        self.typing_down.push(pitch);
                        self.live_note(true, pitch, velocity as u8);
                    }
                } else {
                    self.typing_down.retain(|p| *p != pitch);
                    self.live_note(false, pitch, 0);
                }
                Ok(json!({ "pitch": pitch, "on": on, "held": self.typing_down }))
            }
            "note.releaseAll" => {
                self.release_typing();
                Ok(json!({ "held": [] }))
            }
            "transport.punch" => {
                let enabled = params["enabled"]
                    .as_bool()
                    .ok_or("transport.punch needs `enabled`")?;
                if self.record_enabled != enabled {
                    self.record_enabled = enabled;
                    if self.playing {
                        if enabled {
                            self.start_recording();
                        } else {
                            self.finish_recording();
                        }
                    }
                }
                if let Some(error) = self.error.clone().filter(|_| enabled && self.playing) {
                    return Err(error);
                }
                Ok(
                    json!({ "recordEnabled": self.record_enabled, "playing": self.playing,
                    "recording": self.midi_recording || self.recorder.is_some() }),
                )
            }
            "ui.closePluginWindow" => {
                let id = params["id"]
                    .as_str()
                    .ok_or("ui.closePluginWindow needs `id`")?;
                if !self.plugins.windows.contains_key(id) {
                    return Err(format!(
                        "No plugin window `{id}`; see ui.status pluginWindows"
                    ));
                }
                self.close_plugin_window(id);
                Ok(self.ui_status())
            }
            "ui.dismissError" => {
                let dismissed = self.error.take();
                Ok(json!({ "dismissed": dismissed }))
            }
            "app.confirm" => {
                let Some(intent) = self.intent.take() else {
                    return Err("The window is not asking anything right now".into());
                };
                match params["choice"].as_str().unwrap_or("") {
                    "cancel" => {}
                    "discard" => self.execute(intent),
                    "save" => {
                        self.after_save = Some(intent);
                        self.save(false);
                    }
                    other => {
                        self.intent = Some(intent);
                        return Err(format!(
                            "choice must be save, discard or cancel, not `{other}`"
                        ));
                    }
                }
                Ok(self.ui_status())
            }
            "app.openGuide" => match params["guide"].as_str().unwrap_or("") {
                "plugins" => {
                    let url =
                        "https://github.com/ludovic111/ondera/blob/main/docs/NATIVE_PLUGINS.md";
                    crate::settings::reveal(std::path::Path::new(url));
                    Ok(json!({ "opened": url }))
                }
                other => Err(format!("Unknown guide `{other}`. Guides: plugins.")),
            },
            "app.relaunch" => {
                self.request(crate::app::Intent::Relaunch);
                Ok(json!({ "prompt": self.intent.is_some() }))
            }
            "session.saveRecoveredTake" => {
                let path = PathBuf::from(params["path"].as_str().unwrap_or(""));
                if path
                    .extension()
                    .is_none_or(|e| !e.eq_ignore_ascii_case("wav"))
                {
                    return Err("The recovered take is written as .wav".into());
                }
                if self.recovered_recording_write.is_some() {
                    return Err("The recovered take is already being saved".into());
                }
                let buffer = self
                    .unplaced_recording
                    .clone()
                    .ok_or("There is no recovered take to save")?;
                ondera_engine::device::preserve_recording(&buffer, &path)?;
                self.unplaced_recording = None;
                self.status = format!("Recovered take saved to {}", path.display());
                Ok(json!({ "path": path }))
            }
            "agent.changes" => Ok(self.agents.changes_json(self.store.undo_depth())),
            "agent.revert" => {
                let sequence = params["sequence"]
                    .as_u64()
                    .ok_or("agent.revert needs `sequence`")?;
                if self.agents.runtime.running() {
                    return Err("Stop the agent before stepping through its changes".into());
                }
                if !self
                    .agents
                    .changes_json(0)
                    .as_array()
                    .is_some_and(|list| list.iter().any(|c| c["sequence"] == sequence))
                {
                    return Err(format!("No agent change {sequence}; see agent.changes"));
                }
                if params["redo"].as_bool().unwrap_or(false) {
                    self.redo_activity(sequence);
                } else {
                    self.revert_activity(sequence);
                }
                Ok(self.agents.changes_json(self.store.undo_depth()))
            }
            "ui.screenshot" => {
                let path = match params["path"].as_str() {
                    Some(p) if !p.trim().is_empty() => PathBuf::from(p),
                    _ => control_app::default_screenshot_path(),
                };
                if path
                    .extension()
                    .is_none_or(|e| !e.eq_ignore_ascii_case("png"))
                {
                    return Err("Screenshots are written as .png".into());
                }
                if self
                    .live_jobs
                    .iter()
                    .any(|j| matches!(j.wait, LiveWait::Screenshot { .. }))
                {
                    return Err("A screenshot is already being captured; retry in a moment".into());
                }
                let mut result = self.start_live(
                    LiveWait::Screenshot {
                        path: path.clone(),
                        requested: false,
                    },
                    action,
                    params,
                    source,
                );
                result["path"] = json!(path);
                Ok(result)
            }
            "ui.showPanel" => {
                let panel = params["panel"].as_str().unwrap_or("");
                let visible = params["visible"].as_bool().unwrap_or(true);
                match panel {
                    "agent" => self.agents.open = visible,
                    "automation" => self.automation.open = visible,
                    "settings" => {
                        let section = params["section"].as_str().map(|key| {
                            crate::settings::SECTION_KEYS.iter().position(|candidate| *candidate == key)
                                .ok_or_else(|| format!("Unknown settings section `{key}`"))
                        }).transpose()?;
                        if visible {
                            self.open_settings(section);
                        } else {
                            self.settings_ui.open = false;
                        }
                    }
                    "help" => self.show_help = visible,
                    "mixer" => self.show_mixer = visible,
                    "palette" => self.show_palette = visible,
                    "export" => {
                        if visible {
                            self.open_export_dialog();
                        } else {
                            self.export.close();
                        }
                    }
                    "recovery" => {
                        if visible {
                            self.open_recovery();
                        } else {
                            self.recovery.close();
                        }
                    }
                    "master" | "bus-a" | "bus-b" => {
                        self.try_dispatch(Command::Select {
                            track: Some(panel.into()),
                            clip: None,
                            note: None,
                        })?;
                    }
                    other => {
                        return Err(format!(
                            "Unknown panel `{other}`. Panels: agent, automation, mixer, palette, settings, help, export, recovery, master, bus-a, bus-b."
                        ))
                    }
                }
                Ok(self.ui_status())
            }
            "ui.openPluginWindow" => {
                let track = params["trackId"]
                    .as_str()
                    .ok_or("ui.openPluginWindow needs `trackId`")?;
                let slot = params["slot"].as_u64().map(|s| s as usize);
                if slot.is_some_and(|s| s >= ondera_engine::model::MAX_INSERTS) {
                    return Err("Insert slot must be 0-7".into());
                }
                let insert = control::selected_plugin(self.store.session(), track, slot)?;
                let key = insert.id.clone();
                self.open_plugin_window(&key);
                if params["native"].as_bool().unwrap_or(false) {
                    if !self
                        .plugins
                        .loaded
                        .get(&key)
                        .is_some_and(|l| l.editor.has_gui())
                    {
                        return Err(
                            "This plugin has no native editor; its parameter panel is open".into(),
                        );
                    }
                    self.toggle_native_window_public(&key);
                }
                Ok(json!({ "opened": key, "plugin": insert.name }))
            }
            "ui.closePluginWindows" => {
                let keys: Vec<String> = self.plugins.windows.keys().cloned().collect();
                for key in &keys {
                    self.close_plugin_window(key);
                }
                Ok(json!({ "closed": keys.len() }))
            }
            "ui.musicalTyping" => {
                let enabled = params["enabled"]
                    .as_bool()
                    .ok_or("enabled must be true or false")?;
                if enabled != self.musical_typing {
                    self.toggle_musical_typing();
                }
                Ok(json!({ "musicalTyping": self.musical_typing }))
            }
            "ui.setTool" => {
                self.tool = match params["tool"].as_str().unwrap_or("") {
                    "pointer" => 0,
                    "pencil" => 1,
                    "scissors" => 2,
                    other => {
                        return Err(format!(
                            "Unknown tool `{other}`: pointer, pencil or scissors"
                        ))
                    }
                };
                Ok(self.ui_status())
            }
            "ui.status" => Ok(self.ui_status()),
            "app.status" => Ok(json!({
                "device": self.device.as_ref().map(|d| d.device_name.clone()),
                "sampleRate": self.device.as_ref().map(|d| d.sample_rate),
                "bridgePort": self.control.as_ref().map(|s| s.port()),
                "updateAvailable": self.updates.available.as_ref().map(|r| r.version.clone()),
                "updateInstalled": self.updates.installed.as_ref().map(|p| p.display().to_string()),
                "agentProvider": self.settings.agent.provider.key(),
                "dirty": self.store.dirty(),
            })),
            "app.checkUpdates" => {
                if self.updates.busy() {
                    return Err("An update check or install is already running".into());
                }
                if let Some(release) = &self.updates.available {
                    return Ok(
                        json!({ "available": release_json(release), "current": crate::update::current_version() }),
                    );
                }
                self.check_for_updates(true);
                Ok(self.start_live(LiveWait::UpdateCheck, action, params, source))
            }
            "app.installUpdate" => {
                if self.updates.installed.is_some() {
                    return Err("An update is installed; relaunch Ondera to use it".into());
                }
                if self.updates.busy() {
                    return Err("An update check or install is already running".into());
                }
                if self.updates.available.is_none() {
                    return Err("No update is known; run app.checkUpdates first".into());
                }
                self.install_update();
                Ok(self.start_live(LiveWait::UpdateInstall, action, params, source))
            }
            "app.quit" => {
                if params["discard"].as_bool().unwrap_or(false) {
                    self.intent = None;
                    self.store.mark_saved(self.store.revision);
                    self.execute(Intent::Quit);
                } else {
                    self.request(Intent::Quit);
                }
                Ok(json!({ "quitting": true, "prompted": self.intent.is_some() }))
            }
            "session.restoreSnapshot" => {
                let path = PathBuf::from(
                    params["path"]
                        .as_str()
                        .ok_or("session.restoreSnapshot needs `path`")?,
                );
                recovery::ensure_generated(&recovery::directory(), &path)?;
                self.can_replace_document()?;
                self.recovery.select(path);
                self.request(Intent::Recover);
                Ok(json!({ "status": "requested", "prompted": self.intent.is_some() }))
            }
            "agent.configure" => {
                if self.agents.runtime.running() {
                    return Err("Stop the agent before changing its model".into());
                }
                let mut next = self.settings.clone();
                next.agent.provider = serde_json::from_value(params["provider"].clone())
                    .map_err(|e| format!("Invalid provider: {e}"))?;
                next.agent.model = params["model"]
                    .as_str()
                    .ok_or("Missing model")?
                    .trim()
                    .into();
                next.agent.reasoning_effort = params["reasoningEffort"]
                    .as_str()
                    .ok_or("Missing effort")?
                    .into();
                self.apply_settings(next)?;
                Ok(self.agents.status_json(&self.settings))
            }
            "agent.status" => Ok(self.agents.status_json(&self.settings)),
            "agent.providers" => Ok(crate::agent::providers_json(&self.settings)),
            "agent.models" => {
                let settings = self.settings.clone();
                Ok(self.start_worker(action, params, source, move || {
                    serde_json::to_value(crate::agent::catalog::discover(&settings))
                        .map_err(|e| e.to_string())
                }))
            }
            "agent.connection" => {
                let settings = self.settings.clone();
                Ok(self.start_worker(action, params, source, move || {
                    serde_json::to_value(crate::agent::connection::check(&settings))
                        .map_err(|e| e.to_string())
                }))
            }
            "agent.send" => {
                let prompt = params["prompt"].as_str().unwrap_or("").trim().to_string();
                if prompt.is_empty() {
                    return Err("agent.send needs a non-empty `prompt`".into());
                }
                self.agents.set_prompt(&prompt);
                self.agents.open = true;
                self.start_agent_task_now()?;
                Ok(self.agents.status_json(&self.settings))
            }
            "agent.stop" => {
                self.agents.stop_runner();
                Ok(self.agents.status_json(&self.settings))
            }
            "agent.transcript" => {
                let limit = params["limit"].as_u64().unwrap_or(40).clamp(1, 500) as usize;
                Ok(self.agents.transcript_json(limit))
            }
            "agent.clear" => {
                if self.agents.runner_busy() {
                    return Err("Stop the running task before clearing the conversation".into());
                }
                self.agents.clear_transcript();
                Ok(json!({ "cleared": true }))
            }
            other => Err(format!("{other} is not available in this window")),
        }
    }
}

fn release_json(release: &crate::update::Release) -> Value {
    json!({
        "version": release.version,
        "tag": release.tag,
        "asset": release.asset,
        "size": release.size,
        "signed": release.signature_url.is_some(),
        "notes": release.notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ondera_engine::{audio::AudioBuffer, model::*, store};
    use std::time::{Duration, Instant};

    fn finish(app: &mut Ondera) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while app.control_job.is_some() {
            app.poll_control_job();
            assert!(Instant::now() < deadline, "control worker stalled");
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(app.error.is_none(), "{:?}", app.error);
    }

    #[test]
    fn session_lock_filemode_probe() {
        let Some(path) = std::env::var_os("ONDERA_TEST_LOCK_TARGET") else {
            return;
        };
        let denied = std::env::var_os("ONDERA_TEST_LOCK_DENIED").is_some();
        let result = ondera_tools::Backend::headless(Some(Path::new(&path)), false);
        if denied {
            assert!(result
                .err()
                .is_some_and(|error| error.contains("already in use")));
        } else {
            assert!(
                result.is_ok(),
                "file mode must be available after the window releases ownership"
            );
        }
    }

    fn probe_file_mode(path: &Path, denied: bool) {
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "control::tests::session_lock_filemode_probe",
                "--nocapture",
            ])
            .env("ONDERA_TEST_LOCK_TARGET", path)
            .env_remove("ONDERA_TEST_LOCK_DENIED");
        if denied {
            command.env("ONDERA_TEST_LOCK_DENIED", "1");
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn finish_native(app: &mut Ondera) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while app.job.is_some() {
            app.poll();
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn window_file_ownership_survives_native_save_and_async_path_transitions() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.ondera");
        let b = dir.path().join("b.ondera");
        let c = dir.path().join("c.ondera");
        document::save(&store::empty(), &Library::new(), &a).unwrap();
        document::save(&store::empty(), &Library::new(), &b).unwrap();
        let mut app = Ondera::from_session(store::empty(), None);
        app.load_path(a.clone());
        finish_native(&mut app);
        assert!(app.error.is_none(), "{:?}", app.error);
        probe_file_mode(&a, true);
        assert!(
            document::load(&a).is_ok(),
            "read-only validation remains allowed"
        );
        app.try_dispatch(Command::Rename("Window owns A".into()))
            .unwrap();
        Ondera::save(&mut app, false);
        finish_native(&mut app);
        assert!(app.error.is_none(), "{:?}", app.error);
        probe_file_mode(&a, true);
        let other = ondera_tools::Backend::headless(Some(&b), false).unwrap();
        for method in ["session.open", "session.save"] {
            assert!(app
                .run_control_command(method, &json!({"path":b}), false, "test")
                .unwrap_err()
                .contains("already in use"));
            assert_eq!(app.store.session().name, "Window owns A");
            assert!(app.control_job.is_none());
            probe_file_mode(&a, true);
        }
        app.run_control_command("session.save", &json!({"path":c}), false, "test")
            .unwrap();
        probe_file_mode(&c, true);
        probe_file_mode(&a, true);
        finish(&mut app);
        probe_file_mode(&a, false);
        probe_file_mode(&c, true);
        drop(other);
        app.run_control_command("session.open", &json!({"path":b}), false, "test")
            .unwrap();
        finish(&mut app);
        probe_file_mode(&c, false);
        probe_file_mode(&b, true);
        app.execute(Intent::New);
        probe_file_mode(&b, false);
    }

    #[test]
    fn failed_window_load_keeps_its_previous_lease_and_document() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.ondera");
        let corrupt = dir.path().join("corrupt.ondera");
        document::save(&store::empty(), &Library::new(), &a).unwrap();
        std::fs::write(&corrupt, b"invalid session").unwrap();
        let mut app = Ondera::from_session(store::empty(), None);
        Host::open(&mut app, &a).unwrap();
        let before = app.store.snapshot();
        app.run_control_command("session.open", &json!({"path":corrupt}), false, "test")
            .unwrap();
        while app.control_job.is_some() {
            app.poll_control_job();
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(app.error.take().is_some());
        assert_eq!(json!(app.store.session()), json!(&*before));
        probe_file_mode(&a, true);
        assert!(SessionFileLock::acquire(&corrupt).is_ok());
        let directory = dir.path().join("cannot-replace-directory.ondera");
        std::fs::create_dir(&directory).unwrap();
        app.run_control_command("session.save", &json!({"path":directory}), false, "test")
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        while app.control_job.is_some() {
            app.poll_control_job();
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(app.error.take().is_some());
        assert!(directory.is_dir());
        assert!(
            SessionFileLock::acquire(&directory).is_ok(),
            "failed save releases destination lease"
        );
        probe_file_mode(&a, true);
        drop(app);
        probe_file_mode(&a, false);
    }

    #[test]
    fn agent_file_work_is_deferred_validated_and_preserves_undo() {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("take.wav");
        let buffer = AudioBuffer::new(
            48000,
            (0..4800)
                .map(|i| {
                    let v = (i as f32 * 0.03).sin() * 0.2;
                    [v, v]
                })
                .collect(),
        )
        .unwrap();
        std::fs::write(&input, audio::encode_wav(&buffer).unwrap()).unwrap();
        let mut app = Ondera::from_session(store::empty(), None);
        app.preparing = false;
        app.sync_needed = false;
        let before = app.store.session().clips.len();
        let result = app
            .run_control_command(
                "session.importAudio",
                &json!({"path":input,"startBar":0}),
                true,
                "test",
            )
            .unwrap();
        assert_eq!(result["status"], "running");
        assert_eq!(app.store.session().clips.len(), before);
        assert!(app
            .try_dispatch(Command::Rename("concurrent change".into()))
            .is_err());
        finish(&mut app);
        assert_eq!(app.store.session().clips.len(), before + 1);
        assert!(app.store.session().clips.last().unwrap().agent);
        assert!(!app.library.is_empty());
        app.try_dispatch(Command::Undo).unwrap();
        assert_eq!(app.store.session().clips.len(), before);
        app.try_dispatch(Command::Redo).unwrap();

        let project = temp.path().join("song.ondera");
        app.run_control_command("session.save", &json!({"path":project}), true, "test")
            .unwrap();
        finish(&mut app);
        assert!(!app.store.dirty());
        let (saved, library) = document::load(&project).unwrap();
        assert_eq!(saved.clips.len(), before + 1);
        assert!(!library.is_empty());
        let wav = temp.path().join("mix.wav");
        app.run_control_command("session.bounce", &json!({"path":wav}), true, "test")
            .unwrap();
        finish(&mut app);
        let mix = control::decode_file(&wav).unwrap();
        assert!(mix.frames.iter().any(|f| f[0].abs() > 0.001));
        assert!(mix.frames.iter().all(|f| f.iter().all(|v| v.is_finite())));
    }

    #[test]
    fn malformed_agent_file_command_does_not_start_work() {
        let mut app = Ondera::from_session(store::empty(), None);
        assert!(app
            .run_control_command("session.save", &json!({"path":3}), true, "test")
            .is_err());
        assert!(app.control_job.is_none());
        assert!(!app.store.dirty());
    }

    #[test]
    fn a_groove_preview_is_a_job_that_answers_later_and_leaves_the_song_alone() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("groove.wav");
        let mut app = Ondera::from_session(store::demo(), None);
        app.preparing = false;
        app.sync_needed = false;
        let revision = app.store.revision;
        let tracks = app.store.session().tracks.len();
        let lanes = json!([{"steps":16,"pulses":4,"rotation":0,"pitch":36,"velocity":110}]);
        let result = app
            .run_control_command(
                "rhythm.preview",
                &json!({"lanes":lanes,"bars":1,"path":path}),
                false,
                "test",
            )
            .unwrap();
        assert_eq!(result["status"], "running");
        let (tx, rx) = mpsc::sync_channel(1);
        assert!(app.attach_reply(Reply::Channel(tx)).is_ok());
        // Unlike a file job, a preview does not lock the document while it renders.
        app.try_dispatch(Command::Rename("still editable".into()))
            .unwrap();
        let answer = loop {
            app.poll_workers();
            if let Ok(answer) = rx.try_recv() {
                break answer.unwrap();
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        assert!(app.live_jobs.is_empty());
        assert_eq!(answer["bars"], 1);
        let decoded = audio::decode(std::fs::read(&path).unwrap(), Some("wav")).unwrap();
        assert!(
            decoded.frames.iter().flatten().any(|v| v.abs() > 0.01),
            "the kick is audible"
        );
        assert_eq!(
            app.store.session().tracks.len(),
            tracks,
            "nothing was created"
        );
        assert_eq!(app.store.revision, revision + 1, "only the rename happened");
        // Bad input is refused by the job, not by a panic.
        app.run_control_command(
            "rhythm.preview",
            &json!({"lanes":lanes,"bars":9}),
            false,
            "test",
        )
        .unwrap();
        let (tx, rx) = mpsc::sync_channel(1);
        assert!(app.attach_reply(Reply::Channel(tx)).is_ok());
        let refused = loop {
            app.poll_workers();
            if let Ok(answer) = rx.try_recv() {
                break answer;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        assert!(refused.unwrap_err().contains("1 to 4 bars"));
    }

    #[test]
    fn a_batch_is_one_change_because_it_is_one_undo_step() {
        let mut app = Ondera::from_session(store::demo(), None);
        app.preparing = false;
        app.sync_needed = false;
        let clips = app.store.session().clips.len();
        let batch = json!({"commands":[
            {"command":"clip.create","params":{"trackId":"bass","startBar":40,"lengthBars":1}},
            {"command":"clip.create","params":{"trackId":"bass","startBar":41,"lengthBars":1}},
            {"command":"track.setVolume","params":{"trackId":"bass","volume":0.5}},
        ]});
        app.run_control_command("session.batch", &batch, true, "MCP / agent")
            .unwrap();
        assert_eq!(app.store.session().clips.len(), clips + 2);
        let changes = app.agents.changes_json(app.store.undo_depth());
        let list = changes.as_array().unwrap();
        assert_eq!(list.len(), 1, "{changes}");
        assert!(list[0]["title"]
            .as_str()
            .unwrap()
            .starts_with("Batch · 3 commands · clip.create"));
        // Reverting that one change takes the whole batch back, which is what it says.
        let sequence = list[0]["sequence"].clone();
        app.run_control_command("agent.revert", &json!({"sequence":sequence}), false, "test")
            .unwrap();
        assert_eq!(app.store.session().clips.len(), clips);
        // A batch that fails and rolls back is still one entry, marked as not having succeeded.
        let bad = json!({"commands":[
            {"command":"clip.create","params":{"trackId":"bass","startBar":50,"lengthBars":1}},
            {"command":"clip.remove","params":{"clipId":"no-such-clip"}},
        ]});
        assert!(app
            .run_control_command("session.batch", &bad, true, "MCP / agent")
            .is_err());
        assert_eq!(app.store.session().clips.len(), clips);
        let after = app.agents.changes_json(app.store.undo_depth());
        let after = after.as_array().unwrap();
        // The revert above is itself on the list; the failed batch added exactly one more.
        let failed: Vec<_> = after
            .iter()
            .filter(|c| c["title"].as_str().unwrap().starts_with("Batch"))
            .collect();
        assert_eq!(failed.len(), 2);
        let outcomes: Vec<bool> = failed
            .iter()
            .map(|c| c["succeeded"].as_bool().unwrap())
            .collect();
        assert!(
            outcomes.contains(&true) && outcomes.contains(&false),
            "{outcomes:?}"
        );
        assert!(after
            .iter()
            .all(|c| !c["title"].as_str().unwrap().starts_with("Clip")));
    }

    #[test]
    fn midi_take_blocks_nested_timing_edits_and_undo() {
        let mut app = Ondera::from_session(store::empty(), None);
        app.midi_recording = true;
        let mut t: Transport = app.store.session().transport.clone();
        t.tempo = 99.0;
        assert!(app
            .try_dispatch(Command::Batch(vec![Command::SetTransport(t)]))
            .is_err());
        assert!(app.try_dispatch(Command::Undo).is_err());
        assert_eq!(app.store.session().transport.tempo, 120.0);
        assert!(app
            .try_dispatch(Command::Rename("Take session".into()))
            .is_ok());
    }
}
