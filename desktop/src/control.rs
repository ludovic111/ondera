//! Live control: the window serves the shared command registry over the loopback socket, so
//! `ondera-cli` and `ondera-mcp` edit the same session a person is looking at. Requests are
//! answered on the interface thread between frames; nothing here touches the audio callback.

use crate::app::{Intent, Ondera};
use ondera_engine::{
    audio::{self, Library},
    control::{self, wire, Headless, Host},
    document, render,
    session_file::SessionFileLock,
    store::{Command, Store},
    Result,
};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::mpsc,
};

pub(crate) struct ControlJob {
    receiver: mpsc::Receiver<Result<(Value, Headless, Option<SessionFileLock>)>>,
    request: Option<wire::Request>,
    method: String,
    params: Value,
    source: String,
    revision: u64,
}

impl Ondera {
    pub(crate) fn start_control(&mut self, ctx: &eframe::egui::Context) {
        let ctx = ctx.clone();
        match wire::Server::start(move || ctx.request_repaint()) {
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
            let idle = self.control_job.is_none();
            let result = self.run_control_command(
                &request.method,
                &request.params,
                request.agent,
                if request.agent { "MCP / agent" } else { "CLI" },
            );
            if idle && result.is_ok() && self.control_job.is_some() {
                self.control_job.as_mut().unwrap().request = Some(request);
            } else {
                request.respond(result);
            }
        }
        self.store.set_gesture(gesture);
    }
    pub(crate) fn run_control_command(
        &mut self,
        method: &str,
        params: &Value,
        agent: bool,
        source: &str,
    ) -> Result<Value> {
        let before = self.store.revision;
        let result = (|| {
            control::validate_request(method, params)?;
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
                    request: None,
                    method: method.into(),
                    params: params.clone(),
                    source: source.into(),
                    revision,
                });
                self.status = format!("Running {method}…");
                return Ok(json!({"status":"running", "command":method}));
            }
            self.store.set_gesture(false);
            let mut result = control::call(self, method, params, agent)?;
            if matches!(method, "transport.record" | "transport.stop") {
                result["pending"] =
                    json!(self.record_pending.is_some() || self.record_finishing.is_some());
            }
            Ok(result)
        })();
        self.record_agent_activity(method, params, source, before, &result);
        result
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
        self.record_agent_activity(&job.method, &job.params, &job.source, job.revision, &result);
        if let Err(error) = &result {
            self.error = Some(error.clone());
        }
        if let Some(request) = job.request.take() {
            request.respond(result);
        }
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
