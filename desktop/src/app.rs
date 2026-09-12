use crate::theme::*;
use eframe::egui::{self, RichText};
use ondera_engine::{
    audio::{self, Library},
    device::{DeviceEngine, Message, Recorder},
    document,
    dsp::{EFFECTS, INSTRUMENTS},
    model::*,
    render::{self, Renderer},
    store::{self, Command, Store},
    Result,
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{atomic::Ordering, mpsc, Arc},
    time::Duration,
};

#[derive(Clone, Copy)]
pub enum Intent {
    New,
    Open,
    Demo,
    Quit,
}
enum JobResult {
    Prepared {
        renderer: Box<Renderer>,
        library: Library,
        revision: u64,
    },
    Loaded {
        session: Box<Session>,
        library: Library,
        path: PathBuf,
    },
    Saved {
        path: PathBuf,
        revision: u64,
    },
    Imported(Vec<(PathBuf, Arc<audio::AudioBuffer>)>),
    Bounced,
    Cancelled,
}
type Job = mpsc::Receiver<Result<JobResult>>;
pub struct Ondera {
    pub store: Store,
    pub library: Library,
    pub device: Option<DeviceEngine>,
    pub playing: bool,
    pub position: f64,
    pub record_enabled: bool,
    pub zoom: f32,
    pub scroll: f64,
    pub tool: usize,
    pub browser_tab: usize,
    pub error: Option<String>,
    pub status: String,
    pub path: Option<PathBuf>,
    pub clip_drag: Option<crate::timeline::ClipDrag>,
    pub note_drag: Option<crate::editor::NoteDrag>,
    pub editor_low: u8,
    pub editor_zoom: f32,
    job: Option<Job>,
    preparing: bool,
    sync_needed: bool,
    synced_revision: Option<u64>,
    recorder: Option<Recorder>,
    device_pending: Option<mpsc::Receiver<Result<DeviceEngine>>>,
    record_pending: Option<mpsc::Receiver<Result<Recorder>>>,
    recording_tracks: Vec<String>,
    record_start: f64,
    intent: Option<Intent>,
    after_save: Option<Intent>,
    closing: bool,
    pub screenshot: Option<PathBuf>,
    frames: usize,
    pub show_help: bool,
}
pub fn id(prefix: &str) -> String {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    format!(
        "{prefix}-{}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    )
}
impl Ondera {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        path: Option<PathBuf>,
        screenshot: Option<PathBuf>,
    ) -> Self {
        install(&cc.egui_ctx);
        let session = store::demo();
        let zoom = session.view.pixels_per_bar;
        let mut app = Self {
            store: Store::new(session).expect("Validated demo"),
            library: Library::new(),
            device: None,
            playing: false,
            position: 0.0,
            record_enabled: false,
            zoom,
            scroll: 0.0,
            tool: 0,
            browser_tab: 0,
            error: None,
            status: "Preparing audio…".into(),
            path: None,
            clip_drag: None,
            note_drag: None,
            editor_low: 36,
            editor_zoom: 1.0,
            job: None,
            preparing: false,
            sync_needed: true,
            synced_revision: None,
            recorder: None,
            device_pending: None,
            record_pending: None,
            recording_tracks: vec![],
            record_start: 0.0,
            intent: None,
            after_save: None,
            closing: false,
            screenshot,
            frames: 0,
            show_help: false,
        };
        app.connect();
        if let Some(path) = path {
            app.load_path(path);
        }
        app
    }
    pub fn dispatch(&mut self, command: Command) {
        match self.store.dispatch(command) {
            Ok(changed) => {
                if changed {
                    self.sync_needed = true;
                } else if let Some(device) = &mut self.device {
                    let index = self.store.session().tracks.iter().position(|t| {
                        Some(&t.id) == self.store.session().view.selected_track_id.as_ref()
                    });
                    if let Err(e) = device.send(Message::Select(index)) {
                        self.error = Some(e);
                    }
                }
            }
            Err(e) => self.error = Some(e),
        }
    }
    fn spawn(
        &mut self,
        status: &str,
        work: impl FnOnce() -> Result<JobResult> + std::marker::Send + 'static,
    ) {
        let (tx, rx) = mpsc::sync_channel(1);
        self.job = Some(rx);
        self.status = status.into();
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work))
                .unwrap_or_else(|_| {
                    Err("Background operation failed. Your open session is intact.".into())
                });
            let _ = tx.send(result);
        });
    }
    fn connect(&mut self) {
        if self.device_pending.is_some() {
            return;
        }
        self.stop();
        self.device = None;
        let empty = store::empty();
        let (tx, rx) = mpsc::sync_channel(1);
        self.device_pending = Some(rx);
        self.status = "Opening audio output…".into();
        std::thread::spawn(move || {
            let _ = tx.send(DeviceEngine::open(|rate| {
                Renderer::new(empty, &Library::new(), rate)
            }));
        });
    }
    fn poll(&mut self) {
        if let Some(result) = self
            .device_pending
            .as_ref()
            .and_then(|rx| match rx.try_recv() {
                Ok(v) => Some(v),
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err("Audio device worker stopped".into()))
                }
                Err(_) => None,
            })
        {
            self.device_pending = None;
            match result {
                Ok(device) => {
                    self.device = Some(device);
                    self.sync_needed = true;
                    self.synced_revision = None;
                }
                Err(e) => self.error = Some(e),
            }
        }
        if let Some(result) = self
            .record_pending
            .as_ref()
            .and_then(|rx| match rx.try_recv() {
                Ok(v) => Some(v),
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err("Microphone worker stopped".into()))
                }
                Err(_) => None,
            })
        {
            self.record_pending = None;
            match result {
                Ok(recorder) if self.playing && self.record_enabled => {
                    self.recorder = Some(recorder);
                    self.status = "Recording…".into();
                }
                Ok(recorder) => {
                    std::thread::spawn(move || drop(recorder));
                }
                Err(e) => {
                    self.error = Some(e);
                    self.record_enabled = false;
                }
            }
        }
        let result = self.job.as_ref().and_then(|rx| match rx.try_recv() {
            Ok(v) => Some(v),
            Err(mpsc::TryRecvError::Disconnected) => {
                Some(Err("Background worker disconnected".into()))
            }
            Err(_) => None,
        });
        if let Some(result) = result {
            self.job = None;
            self.preparing = false;
            match result {
                Ok(JobResult::Prepared {
                    renderer,
                    library,
                    revision,
                }) => {
                    self.library.extend(library);
                    if revision == self.store.revision {
                        if let Some(device) = &mut self.device {
                            if let Err(e) = device.send(Message::Replace(renderer)) {
                                self.error = Some(e);
                                self.sync_needed = true;
                            } else {
                                self.synced_revision = Some(revision);
                            }
                        }
                        self.status = "Ready".into();
                    } else {
                        self.sync_needed = true;
                    }
                }
                Ok(JobResult::Loaded {
                    session,
                    library,
                    path,
                }) => {
                    self.position = session.transport.position_beats;
                    self.zoom = session.view.pixels_per_bar.clamp(12.0, 480.0);
                    self.scroll = session.view.scroll_bars.max(0.0);
                    if let Err(e) = self.store.load(*session) {
                        self.error = Some(e);
                    } else {
                        self.library = library;
                        self.path = Some(path);
                        self.sync_needed = true;
                        self.synced_revision = None;
                        self.status = "Session opened".into();
                        self.locate(self.position);
                    }
                }
                Ok(JobResult::Saved { path, revision }) => {
                    self.path = Some(path);
                    self.store.mark_saved(revision);
                    self.status = "Session saved".into();
                    if !self.store.dirty() {
                        if let Some(intent) = self.after_save.take() {
                            self.execute(intent);
                        }
                    }
                }
                Ok(JobResult::Imported(files)) => {
                    for (path, buffer) in files {
                        self.import_buffer(
                            path.file_stem().and_then(|s| s.to_str()).unwrap_or("Audio"),
                            buffer,
                            None,
                        );
                    }
                    self.status = "Audio imported".into();
                }
                Ok(JobResult::Bounced) => self.status = "WAV export complete".into(),
                Ok(JobResult::Cancelled) => {
                    self.status = "Cancelled".into();
                    self.after_save = None;
                }
                Err(e) => {
                    self.error = Some(e);
                    self.status = "Operation failed".into();
                    self.after_save = None;
                }
            }
        }
        if self.sync_needed && self.job.is_none() {
            self.sync_needed = false;
            self.preparing = true;
            let session = self.store.snapshot();
            let mut library = self.library.clone();
            let revision = self.store.revision;
            let rate = self.device.as_ref().map_or(48000, |d| d.sample_rate);
            self.spawn("Updating audio…", move || {
                audio::prepare_sources(&session, &mut library)?;
                let renderer = Box::new(Renderer::new((*session).clone(), &library, rate)?);
                Ok(JobResult::Prepared {
                    renderer,
                    library,
                    revision,
                })
            });
        }
        if let Some(device) = &mut self.device {
            device.collect();
            if self.playing {
                self.position = device.telemetry.beats();
            }
            if device.telemetry.device_failed.load(Ordering::Relaxed) {
                self.stop();
                self.device = None;
                self.error=Some("Audio device disconnected. Use Audio > Reconnect output after reconnecting it.".into());
            }
        }
        if self
            .recorder
            .as_ref()
            .is_some_and(|r| r.failed.load(Ordering::Relaxed))
        {
            self.stop();
        }
    }
    pub fn locate(&mut self, beats: f64) {
        self.position = beats.max(0.0);
        if let Some(d) = &mut self.device {
            if let Err(e) = d.send(Message::Locate(self.position)) {
                self.error = Some(e);
            }
        }
    }
    pub fn play(&mut self) {
        if self.playing {
            self.stop();
            return;
        }
        if self.sync_needed || self.synced_revision != Some(self.store.revision) {
            self.status = "Audio is updating; press Play when ready".into();
            return;
        }
        let Some(d) = &mut self.device else {
            self.error = Some("No output device. Use Audio > Reconnect output.".into());
            return;
        };
        if let Err(e) = d.send(Message::Start(self.position)) {
            self.error = Some(e);
            return;
        }
        self.playing = true;
        if self.record_enabled {
            self.start_recording();
        }
    }
    fn start_recording(&mut self) {
        if self.recorder.is_some() || self.record_pending.is_some() {
            return;
        }
        let s = self.store.session();
        if s.transport.cycle {
            self.error = Some("Turn Cycle off before recording a linear take.".into());
            self.record_enabled = false;
            return;
        }
        self.recording_tracks = s
            .tracks
            .iter()
            .filter(|t| t.kind == "audio" && t.armed)
            .map(|t| t.id.clone())
            .collect();
        if self.recording_tracks.is_empty() {
            self.error = Some("Arm an audio track before recording.".into());
            return;
        }
        let Some(d) = &self.device else {
            return;
        };
        self.record_start = self.position;
        let telemetry = d.telemetry.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        self.record_pending = Some(rx);
        self.status = "Opening microphone — check system permission…".into();
        std::thread::spawn(move || {
            let _ = tx.send(Recorder::start(telemetry));
        });
    }
    pub fn stop(&mut self) {
        self.playing = false;
        self.record_pending.take();
        if let Some(d) = &mut self.device {
            if let Err(e) = d.send(Message::Stop) {
                self.error = Some(e);
            }
        }
        self.finish_recording();
    }
    fn finish_recording(&mut self) {
        if let Some(r) = self.recorder.take() {
            let first = f64::from_bits(r.first_beat.load(Ordering::Relaxed));
            let start = if first.is_finite() {
                first
            } else {
                self.record_start
            };
            match r.finish() {
                Ok(buffer) => self.import_buffer(
                    "Take",
                    Arc::new(buffer),
                    Some((start, self.recording_tracks.clone())),
                ),
                Err(e) => self.error = Some(e),
            }
        }
    }
    pub fn preview(&mut self, track: &str, pitch: u8, velocity: u8) {
        let index = self
            .store
            .session()
            .tracks
            .iter()
            .position(|t| t.id == track);
        if let (Some(index), Some(d)) = (index, self.device.as_mut()) {
            if let Err(e) = d.send(Message::Preview(index, pitch, velocity)) {
                self.error = Some(e);
            }
        }
    }
    pub fn add_track(&mut self, kind: &str) -> String {
        let index = self.store.session().tracks.len();
        let color = TRACKS[index % 8];
        let id = id("track");
        let track = Track {
            id: id.clone(),
            name: format!(
                "{} {}",
                if kind == "audio" {
                    "Audio"
                } else {
                    "Instrument"
                },
                index + 1
            ),
            color: format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b()),
            kind: kind.into(),
            volume: 0.75,
            pan: 0.0,
            mute: false,
            solo: false,
            armed: false,
            extra: HashMap::new(),
        };
        self.dispatch(Command::AddTrack(track));
        id
    }
    pub fn add_clip(&mut self, track: String, start_bar: f64, length_bars: f64) {
        let clip = Clip {
            id: id("clip"),
            name: "MIDI region".into(),
            agent: false,
            track_id: track.clone(),
            start_bar,
            length_bars,
            data: ClipData::Midi { notes: vec![] },
        };
        let clip_id = clip.id.clone();
        self.dispatch(Command::PutClip(clip));
        self.dispatch(Command::Select {
            track: Some(track),
            clip: Some(clip_id),
            note: None,
        });
    }
    pub fn duplicate_clip(&mut self) {
        let selected = self.store.session().view.selected_clip_id.as_ref();
        if let Some(c) = self
            .store
            .session()
            .clips
            .iter()
            .find(|c| Some(&c.id) == selected)
        {
            let mut c = c.clone();
            c.id = id("clip");
            c.start_bar += c.length_bars;
            self.dispatch(Command::PutClip(c));
        }
    }
    pub fn split_selected(&mut self, bar: f64) {
        let s = self.store.session();
        if let Some(clip) = s
            .clips
            .iter()
            .find(|c| Some(&c.id) == s.view.selected_clip_id.as_ref())
        {
            match store::split(clip, bar, id("clip"), s.beats_per_bar(), s.transport.tempo) {
                Ok((l, r)) => self.dispatch(Command::Batch(vec![
                    Command::PutClip(l),
                    Command::PutClip(r),
                ])),
                Err(e) => self.error = Some(e),
            }
        }
    }
    fn import_buffer(
        &mut self,
        name: &str,
        buffer: Arc<audio::AudioBuffer>,
        recorded: Option<(f64, Vec<String>)>,
    ) {
        let mut targets = recorded
            .as_ref()
            .map(|(_, ts)| ts.clone())
            .unwrap_or_default();
        if recorded.is_none() {
            if let Some(t) = self.store.session().tracks.iter().find(|t| {
                Some(&t.id) == self.store.session().view.selected_track_id.as_ref()
                    && t.kind == "audio"
            }) {
                targets.push(t.id.clone());
            }
            if targets.is_empty() {
                targets.push(self.add_track("audio"));
            }
        }
        let source_id = id("source");
        let s = self.store.session();
        let bpb = s.beats_per_bar();
        let start_bar = recorded.as_ref().map_or(self.position, |(beat, _)| *beat) / bpb;
        let length_bars = buffer.duration() * s.transport.tempo / 60.0 / bpb;
        let source = Source {
            id: source_id.clone(),
            name: name.into(),
            sample_rate: buffer.sample_rate,
            channels: 2,
            file_name: Some(format!("{name}.wav")),
            duration_seconds: buffer.duration(),
            origin: if recorded.is_some() {
                "recording"
            } else {
                "file"
            }
            .into(),
            seed: None,
            wave_kind: None,
        };
        let mut commands = vec![Command::PutSource(source)];
        for track in targets {
            commands.push(Command::PutClip(Clip {
                id: id("clip"),
                name: name.into(),
                agent: false,
                track_id: track,
                start_bar,
                length_bars,
                data: ClipData::Audio {
                    source_id: source_id.clone(),
                    offset_seconds: 0.0,
                },
            }));
        }
        self.library.insert(source_id, buffer);
        self.dispatch(Command::Batch(commands));
    }
    pub fn import(&mut self, paths: Option<Vec<PathBuf>>) {
        if self.job.is_some() {
            return;
        }
        self.spawn("Importing audio…", move || {
            let paths = paths.or_else(|| {
                rfd::FileDialog::new()
                    .add_filter(
                        "Audio",
                        &["wav", "aif", "aiff", "flac", "mp3", "ogg", "m4a", "aac"],
                    )
                    .pick_files()
            });
            let Some(paths) = paths else {
                return Ok(JobResult::Cancelled);
            };
            let mut files = vec![];
            for path in paths {
                if std::fs::metadata(&path).map_err(|e| e.to_string())?.len()
                    > audio::MAX_AUDIO_BYTES as u64
                {
                    return Err("Audio file exceeds 512 MiB".into());
                }
                let buffer = audio::decode(
                    std::fs::read(&path).map_err(|e| e.to_string())?,
                    path.extension().and_then(|s| s.to_str()),
                )?;
                files.push((path, Arc::new(buffer)));
            }
            Ok(JobResult::Imported(files))
        });
    }
    fn save(&mut self, save_as: bool) {
        if self.job.is_some() {
            self.status = "Wait for the current operation before saving".into();
            return;
        }
        self.stop();
        let mut session = (*self.store.snapshot()).clone();
        session.transport.position_beats = self.position;
        session.view.pixels_per_bar = self.zoom;
        session.view.scroll_bars = self.scroll;
        let library = self.library.clone();
        let revision = self.store.revision;
        let path = if save_as { None } else { self.path.clone() };
        self.spawn("Saving…", move || {
            let path = path.or_else(|| {
                rfd::FileDialog::new()
                    .add_filter("Ondera session", &["ondera"])
                    .set_file_name(&session.name)
                    .save_file()
            });
            let Some(mut path) = path else {
                return Ok(JobResult::Cancelled);
            };
            if path.extension().is_none() {
                path.set_extension("ondera");
            }
            document::save(&session, &library, &path)?;
            Ok(JobResult::Saved { path, revision })
        });
    }
    fn bounce(&mut self) {
        if self.job.is_some() {
            return;
        }
        self.stop();
        let session = self.store.snapshot();
        let library = self.library.clone();
        self.spawn("Exporting WAV…", move || {
            let path = rfd::FileDialog::new()
                .add_filter("WAV audio", &["wav"])
                .set_file_name(format!("{}.wav", session.name.trim_end_matches(".ondera")))
                .save_file();
            let Some(mut path) = path else {
                return Ok(JobResult::Cancelled);
            };
            if path.extension().is_none() {
                path.set_extension("wav");
            }
            render::bounce(&session, &library, &path, 48000)?;
            Ok(JobResult::Bounced)
        });
    }
    fn load_path(&mut self, path: PathBuf) {
        self.stop();
        self.spawn("Opening session…", move || {
            let (session, library) = document::load(&path)?;
            Ok(JobResult::Loaded {
                session: Box::new(session),
                library,
                path,
            })
        });
    }
    fn request(&mut self, intent: Intent) {
        if self.job.is_some() {
            return;
        }
        self.stop();
        if self.store.dirty() {
            self.intent = Some(intent);
        } else {
            self.execute(intent);
        }
    }
    fn execute(&mut self, intent: Intent) {
        match intent {
            Intent::Quit => self.closing = true,
            Intent::New | Intent::Demo => {
                let s = if matches!(intent, Intent::New) {
                    store::empty()
                } else {
                    store::demo()
                };
                if let Err(e) = self.store.load(s) {
                    self.error = Some(e);
                    return;
                }
                self.library.clear();
                self.path = None;
                self.position = 0.0;
                self.scroll = 0.0;
                self.sync_needed = true;
                self.synced_revision = None;
                self.locate(0.0);
            }
            Intent::Open => self.spawn("Opening session…", || {
                let Some(path) = rfd::FileDialog::new()
                    .add_filter("Ondera session", &["ondera"])
                    .pick_file()
                else {
                    return Ok(JobResult::Cancelled);
                };
                let (session, library) = document::load(&path)?;
                Ok(JobResult::Loaded {
                    session: Box::new(session),
                    library,
                    path,
                })
            }),
        }
    }
    fn keyboard(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() || self.intent.is_some() {
            return;
        }
        let pressed = |key| ctx.input(|i| i.key_pressed(key));
        let mods = ctx.input(|i| i.modifiers);
        if mods.command {
            if pressed(egui::Key::S) {
                self.save(mods.shift);
            }
            if pressed(egui::Key::O) {
                self.request(Intent::Open);
            }
            if pressed(egui::Key::N) {
                self.request(Intent::New);
            }
            if pressed(egui::Key::I) {
                self.import(None);
            }
            if pressed(egui::Key::B) {
                self.bounce();
            }
            if pressed(egui::Key::D) {
                self.duplicate_clip();
            }
            if pressed(egui::Key::T) {
                self.split_selected(self.position / self.store.session().beats_per_bar());
            }
            if pressed(egui::Key::Z) {
                self.dispatch(if mods.shift {
                    Command::Redo
                } else {
                    Command::Undo
                });
            }
            if pressed(egui::Key::Equals) || pressed(egui::Key::Plus) {
                self.zoom = (self.zoom * 1.25).min(480.0);
            }
            if pressed(egui::Key::Minus) {
                self.zoom = (self.zoom / 1.25).max(12.0);
            }
            return;
        }
        if pressed(egui::Key::Space) {
            self.play();
        }
        if pressed(egui::Key::Num0) {
            self.stop();
        }
        if pressed(egui::Key::Enter) {
            self.locate(0.0);
        }
        if pressed(egui::Key::C) {
            let mut t = self.store.session().transport.clone();
            t.cycle = !t.cycle;
            self.dispatch(Command::SetTransport(t));
        }
        if pressed(egui::Key::K) {
            let mut t = self.store.session().transport.clone();
            t.metronome = !t.metronome;
            self.dispatch(Command::SetTransport(t));
        }
        if pressed(egui::Key::R) {
            self.record_enabled = !self.record_enabled;
            if self.playing {
                if self.record_enabled {
                    self.start_recording();
                } else {
                    self.finish_recording();
                }
            }
        }
        for (key, tool) in [
            (egui::Key::Num1, 0),
            (egui::Key::Num2, 1),
            (egui::Key::Num3, 2),
        ] {
            if pressed(key) {
                self.tool = tool;
            }
        }
        if pressed(egui::Key::Backspace) || pressed(egui::Key::Delete) {
            self.delete_selected();
        }
        if pressed(egui::Key::F) {
            let mut v = self.store.session().view.clone();
            v.follow_playhead = !v.follow_playhead;
            self.dispatch(Command::SetView(v));
        }
        if pressed(egui::Key::Z) {
            self.zoom = (700.0 / self.store.session().end_bar() as f32).clamp(12.0, 480.0);
            self.scroll = 0.0;
        }
        if let Some(t) = self
            .store
            .session()
            .tracks
            .iter()
            .find(|t| Some(&t.id) == self.store.session().view.selected_track_id.as_ref())
        {
            let mut t = t.clone();
            let mut changed = false;
            if pressed(egui::Key::M) {
                t.mute = !t.mute;
                changed = true;
            }
            if pressed(egui::Key::S) {
                t.solo = !t.solo;
                changed = true;
            }
            if pressed(egui::Key::A) {
                t.armed = !t.armed;
                changed = true;
            }
            if changed {
                self.dispatch(Command::UpdateTrack(t));
            }
        }
    }
    pub fn delete_selected(&mut self) {
        let s = self.store.session();
        if let Some(c) = s
            .clips
            .iter()
            .find(|c| Some(&c.id) == s.view.selected_clip_id.as_ref())
        {
            if let Some(n) = &s.view.selected_note_id {
                let mut c = c.clone();
                if let ClipData::Midi { notes } = &mut c.data {
                    notes.retain(|note| &note.id != n);
                }
                self.dispatch(Command::PutClip(c));
            } else {
                self.dispatch(Command::RemoveClip(c.id.clone()));
            }
        }
    }
    fn menus(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu")
            .exact_height(32.0)
            .show(ctx, |ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    ui.label(RichText::new("ONDERA").strong().color(INK));
                    ui.separator();
                    ui.menu_button("File", |ui| {
                        for (label, intent) in [
                            ("New session", Intent::New),
                            ("Open…", Intent::Open),
                            ("Open demo", Intent::Demo),
                        ] {
                            if ui.button(label).clicked() {
                                self.request(intent);
                                ui.close();
                            }
                        }
                        ui.separator();
                        if ui.button("Save").clicked() {
                            self.save(false);
                            ui.close();
                        }
                        if ui.button("Save as…").clicked() {
                            self.save(true);
                            ui.close();
                        }
                        if ui.button("Import audio…").clicked() {
                            self.import(None);
                            ui.close();
                        }
                        if ui.button("Bounce mix to WAV…").clicked() {
                            self.bounce();
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Quit").clicked() {
                            self.request(Intent::Quit);
                            ui.close();
                        }
                    });
                    ui.menu_button("Edit", |ui| {
                        if ui
                            .add_enabled(self.store.can_undo(), egui::Button::new("Undo"))
                            .clicked()
                        {
                            self.dispatch(Command::Undo);
                            ui.close();
                        }
                        if ui
                            .add_enabled(self.store.can_redo(), egui::Button::new("Redo"))
                            .clicked()
                        {
                            self.dispatch(Command::Redo);
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Duplicate region").clicked() {
                            self.duplicate_clip();
                            ui.close();
                        }
                        if ui.button("Split at playhead").clicked() {
                            self.split_selected(
                                self.position / self.store.session().beats_per_bar(),
                            );
                            ui.close();
                        }
                        if ui.button("Delete selection").clicked() {
                            self.delete_selected();
                            ui.close();
                        }
                    });
                    ui.menu_button("Track", |ui| {
                        if ui.button("Add instrument track").clicked() {
                            self.add_track("midi");
                            ui.close();
                        }
                        if ui.button("Add audio track").clicked() {
                            self.add_track("audio");
                            ui.close();
                        }
                    });
                    ui.menu_button("Audio", |ui| {
                        if ui.button("Reconnect output").clicked() {
                            self.connect();
                            ui.close();
                        }
                        ui.separator();
                        ui.label("Uses the operating system's default input and output.");
                        ui.label("Change devices in system audio settings, then reconnect.");
                    });
                    if ui.button("Help").clicked() {
                        self.show_help = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!(
                                "{}{}",
                                self.store.session().name,
                                if self.store.dirty() { "  •" } else { "" }
                            ))
                            .color(DIM),
                        );
                    });
                });
            });
    }
    fn transport(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("transport")
            .exact_height(62.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui
                        .button("|◀")
                        .on_hover_text("Return to start · Enter")
                        .clicked()
                    {
                        self.locate(0.0);
                    }
                    if ui.button("■").on_hover_text("Stop · 0").clicked() {
                        self.stop();
                    }
                    if ui
                        .add(
                            egui::Button::new(if self.playing { "Pause" } else { "▶ Play" }).fill(
                                if self.playing {
                                    ACCENT.gamma_multiply(0.4)
                                } else {
                                    RAISED
                                },
                            ),
                        )
                        .clicked()
                    {
                        self.play();
                    }
                    if ui
                        .selectable_label(self.record_enabled, RichText::new("● Rec").color(RED))
                        .clicked()
                    {
                        self.record_enabled = !self.record_enabled;
                        if self.playing {
                            if self.record_enabled {
                                self.start_recording();
                            } else {
                                self.finish_recording();
                            }
                        }
                    }
                    ui.separator();
                    let bpb = self.store.session().beats_per_bar();
                    let bar = (self.position / bpb).floor();
                    let within = self.position - bar * bpb;
                    egui::Frame::new()
                        .fill(WELL)
                        .inner_margin(8)
                        .corner_radius(5)
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(format!(
                                    "{:03} . {} . {:03}",
                                    bar as u64 + 1,
                                    within.floor() as u64 + 1,
                                    (within.fract() * 960.0) as u64
                                ))
                                .monospace()
                                .size(DIGITS)
                                .color(INK),
                            );
                        });
                    let mut t = self.store.session().transport.clone();
                    let mut changed = false;
                    ui.label(RichText::new("BPM").small().color(FAINT));
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut t.tempo)
                                .speed(0.2)
                                .range(20.0..=400.0),
                        )
                        .changed();
                    changed |= ui
                        .add(egui::DragValue::new(&mut t.time_signature.numerator).range(1..=32))
                        .changed();
                    ui.label("/");
                    egui::ComboBox::from_id_salt("denominator")
                        .width(40.0)
                        .selected_text(t.time_signature.denominator.to_string())
                        .show_ui(ui, |ui| {
                            for d in [1, 2, 4, 8, 16, 32] {
                                changed |= ui
                                    .selectable_value(
                                        &mut t.time_signature.denominator,
                                        d,
                                        d.to_string(),
                                    )
                                    .changed();
                            }
                        });
                    if ui.selectable_label(t.cycle, "Cycle").clicked() {
                        t.cycle = !t.cycle;
                        changed = true;
                    }
                    if ui.selectable_label(t.metronome, "Click").clicked() {
                        t.metronome = !t.metronome;
                        changed = true;
                    }
                    if changed {
                        self.dispatch(Command::SetTransport(t));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (peaks, cpu) = self.device.as_ref().map_or(([0.0; 4], 0.0), |d| {
                            (d.telemetry.peaks(), d.telemetry.load())
                        });
                        ui.label(
                            RichText::new(format!("DSP {:2.0}%", cpu * 100.0))
                                .small()
                                .monospace()
                                .color(DIM),
                        );
                        ui.vertical(|ui| {
                            meter(ui, peaks[0], 90.0);
                            meter(ui, peaks[1], 90.0);
                        });
                    });
                });
            });
    }
    fn browser(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("browser")
            .default_width(210.0)
            .width_range(170.0..=300.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.heading("Library");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    for (i, name) in ["Sounds", "Loops", "Effects"].iter().enumerate() {
                        ui.selectable_value(&mut self.browser_tab, i, *name);
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let names: Vec<&str> = match self.browser_tab {
                        0 => INSTRUMENTS.to_vec(),
                        1 => vec![
                            "Boom Bap 92",
                            "Four Floor 124",
                            "Brushes Swing",
                            "Rhodes Comp Cm",
                            "Analog Pad Swell",
                            "Bass Pluck 120",
                            "Riser 1 bar",
                            "Reverse Cymbal",
                            "Vinyl Crackle",
                        ],
                        _ => EFFECTS.to_vec(),
                    };
                    ui.label(RichText::new("ONDERA").small().color(FAINT));
                    ui.add_space(6.0);
                    for (i, name) in names.iter().enumerate() {
                        let response = ui
                            .horizontal(|ui| {
                                ui.label(RichText::new("●").color(TRACKS[i % 8]));
                                ui.add_sized(
                                    [ui.available_width(), 28.0],
                                    egui::Button::new(*name).frame(false),
                                )
                            })
                            .inner;
                        if response.double_clicked() {
                            match self.browser_tab {
                                0 => self.instrument(name),
                                1 => self.add_loop(name),
                                _ => self.add_effect(name),
                            }
                        }
                    }
                    ui.add_space(22.0);
                    ui.label(RichText::new("Double-click to load").small().color(FAINT));
                    ui.separator();
                    ui.label(RichText::new("PROJECT AUDIO").small().color(FAINT));
                    let sources: Vec<_> = self
                        .store
                        .session()
                        .sources
                        .values()
                        .map(|s| (s.name.clone(), s.duration_seconds))
                        .collect();
                    for (name, duration) in sources {
                        ui.label(name);
                        ui.label(
                            RichText::new(format!("{duration:.1} s"))
                                .small()
                                .color(FAINT),
                        );
                    }
                    if ui.button("Import audio…").clicked() {
                        self.import(None);
                    }
                });
            });
    }
    fn instrument(&mut self, name: &str) {
        let selected = self
            .store
            .session()
            .tracks
            .iter()
            .find(|t| {
                Some(&t.id) == self.store.session().view.selected_track_id.as_ref()
                    && t.kind == "midi"
            })
            .map(|t| t.id.clone());
        let track = selected.unwrap_or_else(|| self.add_track("midi"));
        let mut strip = self
            .store
            .session()
            .strips
            .get(&track)
            .cloned()
            .unwrap_or_default();
        strip.instrument = name.into();
        self.dispatch(Command::SetStrip { track, strip });
    }
    fn add_effect(&mut self, name: &str) {
        let Some(track) = self.store.session().view.selected_track_id.clone() else {
            return;
        };
        let mut strip = self
            .store
            .session()
            .strips
            .get(&track)
            .cloned()
            .unwrap_or_default();
        let slot = Insert {
            name: name.into(),
            state: "active".into(),
            meta: String::new(),
        };
        if let Some(empty) = strip.inserts.iter_mut().find(|i| i.state == "empty") {
            *empty = slot;
        } else if strip.inserts.len() < 4 {
            strip.inserts.push(slot);
        } else {
            self.error = Some("The channel's four insert slots are occupied.".into());
            return;
        }
        self.dispatch(Command::SetStrip { track, strip });
    }
    fn add_loop(&mut self, name: &str) {
        let patterns: serde_json::Value =
            serde_json::from_str(include_str!("../../engine/tests/fixtures/loops.json"))
                .expect("Bundled loops");
        let Some(pattern) = patterns.get(name) else {
            return;
        };
        let Some(instrument) = pattern["instrument"].as_str() else {
            return;
        };
        self.instrument(instrument);
        let Some(track) = self.store.session().view.selected_track_id.clone() else {
            return;
        };
        let notes = pattern["notes"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|n| Note {
                id: id("note"),
                start: n["start"].as_f64().unwrap_or(0.0),
                length: n["length"].as_f64().unwrap_or(0.25),
                pitch: n["pitch"].as_u64().unwrap_or(60) as u8,
                velocity: n["velocity"].as_u64().unwrap_or(100) as u8,
                agent: false,
            })
            .collect();
        // Patterns are authored in 4/4; length is translated to current bars.
        let bpb = self.store.session().beats_per_bar();
        self.dispatch(Command::PutClip(Clip {
            id: id("clip"),
            name: name.into(),
            agent: false,
            track_id: track,
            start_bar: (self.position / bpb).floor(),
            length_bars: pattern["bars"].as_f64().unwrap_or(1.0) * 4.0 / bpb,
            data: ClipData::Midi { notes },
        }));
    }
    fn inspector(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("inspector")
            .default_width(220.0)
            .width_range(190.0..=320.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.heading("Inspector");
                ui.separator();
                let s = self.store.snapshot();
                let Some(t) = s
                    .tracks
                    .iter()
                    .find(|t| Some(&t.id) == s.view.selected_track_id.as_ref())
                else {
                    ui.label("Select a track");
                    return;
                };
                let mut track = t.clone();
                if ui.text_edit_singleline(&mut track.name).changed() && track.name != t.name {
                    self.dispatch(Command::UpdateTrack(track.clone()));
                }
                ui.label(
                    RichText::new(if track.kind == "audio" {
                        "AUDIO CHANNEL"
                    } else {
                        "INSTRUMENT CHANNEL"
                    })
                    .small()
                    .color(FAINT),
                );
                ui.add_space(10.0);
                let mut strip = s.strips.get(&track.id).cloned().unwrap_or_default();
                let mut strip_changed = false;
                if track.kind == "midi" {
                    egui::ComboBox::from_id_salt("instrument")
                        .width(ui.available_width() - 10.0)
                        .selected_text(if strip.instrument.is_empty() {
                            "Ondera Synth"
                        } else {
                            &strip.instrument
                        })
                        .show_ui(ui, |ui| {
                            for name in INSTRUMENTS {
                                strip_changed |= ui
                                    .selectable_value(&mut strip.instrument, name.into(), name)
                                    .changed();
                            }
                        });
                }
                ui.add_space(12.0);
                ui.label(RichText::new("INSERTS").small().color(FAINT));
                while strip.inserts.len() < 4 {
                    strip.inserts.push(Insert {
                        name: "Empty slot".into(),
                        state: "empty".into(),
                        meta: String::new(),
                    });
                }
                for i in 0..4 {
                    ui.horizontal(|ui| {
                        let slot = &mut strip.inserts[i];
                        let active = slot.state == "active";
                        if ui
                            .add_enabled(
                                slot.state != "empty",
                                egui::Button::new(RichText::new("●").color(if active {
                                    ACCENT
                                } else {
                                    FAINT
                                })),
                            )
                            .clicked()
                        {
                            slot.state = if active { "bypassed" } else { "active" }.into();
                            strip_changed = true;
                        }
                        egui::ComboBox::from_id_salt(("insert", i))
                            .width(142.0)
                            .selected_text(&slot.name)
                            .show_ui(ui, |ui| {
                                for name in std::iter::once("Empty slot").chain(EFFECTS) {
                                    if ui.selectable_label(slot.name == name, name).clicked() {
                                        slot.name = name.into();
                                        slot.state = if name == "Empty slot" {
                                            "empty"
                                        } else {
                                            "active"
                                        }
                                        .into();
                                        strip_changed = true;
                                    }
                                }
                            });
                    });
                }
                ui.add_space(14.0);
                ui.label(RichText::new("SENDS").small().color(FAINT));
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
                for (i, name) in ["A · Reverb", "B · Delay"].iter().enumerate() {
                    let mut db = strip.sends[i].level_db.unwrap_or(-100.0);
                    if ui
                        .add(
                            egui::Slider::new(&mut db, -100.0..=0.0)
                                .text(*name)
                                .suffix(" dB"),
                        )
                        .changed()
                    {
                        strip.sends[i].level_db = if db <= -99.0 { None } else { Some(db) };
                        strip_changed = true;
                    }
                }
                if strip_changed {
                    self.dispatch(Command::SetStrip {
                        track: track.id.clone(),
                        strip,
                    });
                }
                ui.add_space(14.0);
                ui.separator();
                if ui
                    .add(egui::Slider::new(&mut track.pan, -100.0..=100.0).text("Pan"))
                    .changed()
                {
                    self.dispatch(Command::UpdateTrack(track.clone()));
                }
                if ui
                    .add(egui::Slider::new(&mut track.volume, 0.0..=1.0).text("Volume"))
                    .changed()
                {
                    self.dispatch(Command::UpdateTrack(track.clone()));
                }
                let gain = fader_gain(track.volume);
                ui.label(
                    RichText::new(if gain > 0.0 {
                        format!("{:+.1} dB", 20.0 * gain.log10())
                    } else {
                        "−∞ dB".into()
                    })
                    .monospace(),
                );
                ui.horizontal(|ui| {
                    if ui.selectable_label(track.mute, "Mute").clicked() {
                        track.mute = !track.mute;
                        self.dispatch(Command::UpdateTrack(track.clone()));
                    }
                    if ui.selectable_label(track.solo, "Solo").clicked() {
                        track.solo = !track.solo;
                        self.dispatch(Command::UpdateTrack(track.clone()));
                    }
                    if ui
                        .selectable_label(track.armed, RichText::new("Arm").color(RED))
                        .clicked()
                    {
                        track.armed = !track.armed;
                        self.dispatch(Command::UpdateTrack(track.clone()));
                    }
                });
                let peaks = self
                    .device
                    .as_ref()
                    .map_or([0.0; 4], |d| d.telemetry.peaks());
                ui.add_space(12.0);
                meter(ui, peaks[2], ui.available_width());
                meter(ui, peaks[3], ui.available_width());
                ui.add_space(12.0);
                ui.label(RichText::new("Stereo Out").small().color(DIM));
                if let Some(c) = s
                    .clips
                    .iter()
                    .find(|c| Some(&c.id) == s.view.selected_clip_id.as_ref())
                {
                    ui.separator();
                    ui.label(RichText::new("REGION").small().color(FAINT));
                    let mut c = c.clone();
                    let old = c.name.clone();
                    if ui.text_edit_singleline(&mut c.name).changed() && c.name != old {
                        self.dispatch(Command::PutClip(c.clone()));
                    }
                    let mut start = c.start_bar + 1.0;
                    if ui
                        .add(
                            egui::DragValue::new(&mut start)
                                .range(1.0..=100000.0)
                                .prefix("Bar ")
                                .speed(0.25),
                        )
                        .changed()
                    {
                        c.start_bar = start - 1.0;
                        self.dispatch(Command::PutClip(c.clone()));
                    }
                    if ui
                        .add(
                            egui::DragValue::new(&mut c.length_bars)
                                .range(0.0625..=100000.0)
                                .prefix("Length ")
                                .speed(0.25),
                        )
                        .changed()
                    {
                        self.dispatch(Command::PutClip(c));
                    }
                }
            });
    }
    fn dialogs(&mut self, ctx: &egui::Context) {
        if let Some(intent) = self.intent {
            egui::Modal::new(egui::Id::new("unsaved")).show(ctx, |ui| {
                ui.heading("Save your changes?");
                ui.label("This session has unsaved changes.");
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        self.intent = None;
                        self.after_save = Some(intent);
                        self.save(false);
                    }
                    if ui.button("Discard").clicked() {
                        self.intent = None;
                        self.execute(intent);
                    }
                    if ui.button("Cancel").clicked() {
                        self.intent = None;
                    }
                });
            });
        }
        if let Some(message) = self.error.clone() {
            egui::Window::new("Ondera")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.set_max_width(480.0);
                    ui.label(message);
                    if ui.button("OK").clicked() {
                        self.error = None;
                    }
                });
        }
        if self.show_help {
            egui::Window::new("Working in Ondera").open(&mut self.show_help).show(ctx,|ui|{
            for text in ["Space: play / stop. Enter: return to start. R: record, A: arm selected track.","Double-click a MIDI lane to create a region. Draw notes in the piano roll.","Drag regions to move, drag edges to trim, right-click for options.","Tools 1 / 2 / 3: pointer, pencil, scissors. Alt disables snapping.","M / S: mute / solo. C: cycle. K: metronome. F: follow. Z: fit.","Cmd/Ctrl + S / O / I / B: save, open, import, bounce.","Cmd/Ctrl + Z / Shift+Z: undo / redo. Cmd/Ctrl + D / T: duplicate / split.","Drag across the ruler to set the cycle range. Drop audio files to import.","Built-in instruments and effects only. No external plugin hosting or agent connection."]{ui.label(text);}
        });
        }
    }
}
impl eframe::App for Ondera {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.store
            .set_gesture(ctx.input(|i| i.pointer.any_down()) || ctx.wants_keyboard_input());
        self.frames += 1;
        self.poll();
        self.keyboard(ctx);
        if ctx.input(|i| i.viewport().close_requested()) && !self.closing {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.request(Intent::Quit);
        }
        if self.closing {
            self.stop();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
            "{}{} — Ondera",
            self.store.session().name,
            if self.store.dirty() { " *" } else { "" }
        )));
        self.menus(ctx);
        self.transport(ctx);
        egui::TopBottomPanel::bottom("status")
            .exact_height(26.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if self.job.is_some() {
                        ui.spinner();
                    }
                    ui.label(RichText::new(&self.status).small().color(DIM));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some(d) = &self.device {
                            ui.label(
                                RichText::new(format!(
                                    "{}  ·  {} Hz  ·  24-bit WAV",
                                    d.device_name, d.sample_rate
                                ))
                                .small()
                                .color(FAINT),
                            );
                        } else {
                            ui.label("Audio offline");
                        }
                    });
                });
            });
        self.browser(ctx);
        self.inspector(ctx);
        egui::TopBottomPanel::bottom("editor")
            .default_height(290.0)
            .height_range(190.0..=500.0)
            .resizable(true)
            .show(ctx, |ui| {
                self.editor(ui);
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(LANE).inner_margin(0))
            .show(ctx, |ui| {
                self.arrangement(ui);
            });
        self.dialogs(ctx);
        let dropped: Vec<_> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        if !dropped.is_empty() {
            self.import(Some(dropped));
        }
        if self.playing || self.job.is_some() {
            ctx.request_repaint_after(Duration::from_millis(33));
        } else {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        if self.screenshot.is_some() && self.frames > 40 && self.job.is_none() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        for event in ctx.input(|i| i.events.clone()) {
            if let egui::Event::Screenshot { image, .. } = event {
                if let Some(path) = self.screenshot.take() {
                    let data: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
                    match image::save_buffer(
                        &path,
                        &data,
                        image.width() as u32,
                        image.height() as u32,
                        image::ColorType::Rgba8,
                    ) {
                        Ok(()) => self.closing = true,
                        Err(e) => self.error = Some(e.to_string()),
                    }
                }
            }
        }
    }
}
pub fn meter(ui: &mut egui::Ui, peak: f32, width: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 5.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 1, WELL);
    let level = if peak > 0.0 {
        ((20.0 * peak.log10() + 54.0) / 54.0).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let active = egui::Rect::from_min_size(rect.min, egui::vec2(width * level, rect.height()));
    ui.painter()
        .rect_filled(active, 1, if peak >= 0.95 { RED } else { ACCENT });
}
