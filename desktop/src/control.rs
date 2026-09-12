//! Live control: the window serves the shared command registry over the loopback socket, so
//! `ondera-cli` and `ondera-mcp` edit the same session a person is looking at. Requests are
//! answered on the interface thread between frames; nothing here touches the audio callback.

use crate::app::{Intent, Ondera};
use ondera_engine::{
    audio::{self, Library},
    control::{wire, Host},
    document, render,
    store::{Command, Store},
    Result,
};
use std::path::{Path, PathBuf};

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
            wire::serve(self, request);
        }
        self.store.set_gesture(gesture);
    }
    fn available(&self) -> Result<()> {
        if self.job.is_some() {
            return Err("Ondera is busy with a file operation; retry in a moment".into());
        }
        if self.recorder.is_some()
            || self.record_pending.is_some()
            || self.record_finishing.is_some()
        {
            return Err("A recording is in progress; stop the transport first".into());
        }
        Ok(())
    }
    /// Run an interface action and report the error it would have shown, leaving any message
    /// the person was already reading in place.
    fn guarded(&mut self, action: impl FnOnce(&mut Self)) -> Result<()> {
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
        Ondera::stop(self);
        self.guarded(|app| app.execute(if demo { Intent::Demo } else { Intent::New }))
    }
    fn open(&mut self, path: &Path) -> Result<()> {
        self.available()?;
        Ondera::stop(self);
        let (session, library) = document::load(path)?;
        self.guarded(|app| app.loaded(session, library, path.to_path_buf()))
    }
    fn save(&mut self, path: Option<&Path>) -> Result<PathBuf> {
        self.available()?;
        Ondera::stop(self);
        let mut path = path
            .map(Path::to_path_buf)
            .or_else(|| self.path.clone())
            .ok_or("The session has no file yet: pass `path`.")?;
        if path.extension().is_none() {
            path.set_extension("ondera");
        }
        let mut session = (*self.store.snapshot()).clone();
        session.transport.position_beats = self.position;
        session.view.pixels_per_bar = self.zoom;
        session.view.scroll_bars = self.scroll;
        let revision = self.store.revision;
        document::save(&session, &self.library, &path)?;
        self.saved(path.clone(), revision);
        Ok(path)
    }
    fn bounce(&mut self, path: &Path) -> Result<()> {
        self.available()?;
        Ondera::stop(self);
        let session = self.store.snapshot();
        let mut library = self.library.clone();
        audio::prepare_sources(&session, &mut library)?;
        render::bounce(&session, &library, path, 48000)?;
        self.status = "WAV export complete".into();
        Ok(())
    }
}
