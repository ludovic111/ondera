use crate::theme::*;
use eframe::egui;
use ondera_engine::{
    audio::{self, Library},
    device::{DeviceEngine, Message, Recorder},
    document,
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
pub(crate) enum AfterTake {
    Save(bool),
    Bounce,
    Request(Intent),
}
pub(crate) enum JobResult {
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
    pub browser_filter: String,
    pub browser_selected: Option<String>,
    pub error: Option<String>,
    pub status: String,
    pub path: Option<PathBuf>,
    pub clip_drag: Option<crate::timeline::ClipDrag>,
    pub note_drag: Option<crate::editor::NoteDrag>,
    pub ruler_anchor: Option<f64>,
    pub draw_clip_anchor: Option<(String, f64)>,
    pub draw_note_anchor: Option<f64>,
    pub(crate) pending_preview: Option<(String, u8, u8)>,
    pub editor_low: u8,
    pub editor_zoom: f32,
    pub(crate) job: Option<Job>,
    pub(crate) preparing: bool,
    pub(crate) sync_needed: bool,
    pub(crate) synced_revision: Option<u64>,
    pub(crate) recorder: Option<Recorder>,
    pub(crate) device_pending: Option<mpsc::Receiver<Result<DeviceEngine>>>,
    pub(crate) record_pending: Option<mpsc::Receiver<Result<Recorder>>>,
    pub(crate) record_finishing: Option<mpsc::Receiver<Result<audio::AudioBuffer>>>,
    pub(crate) after_take: Option<AfterTake>,
    pub(crate) recording_tracks: Vec<String>,
    pub(crate) record_start: f64,
    pub(crate) intent: Option<Intent>,
    pub(crate) after_save: Option<Intent>,
    pub(crate) closing: bool,
    pub screenshot: Option<PathBuf>,
    pub(crate) frames: usize,
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
        let mut app = Self::from_session(store::demo(), screenshot);
        app.connect();
        if let Some(path) = path {
            app.load_path(path);
        }
        app
    }
    pub(crate) fn from_session(session: Session, screenshot: Option<PathBuf>) -> Self {
        let zoom = session.view.pixels_per_bar;
        Self {
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
            browser_filter: String::new(),
            browser_selected: None,
            error: None,
            status: "Preparing audio…".into(),
            path: None,
            clip_drag: None,
            note_drag: None,
            ruler_anchor: None,
            draw_clip_anchor: None,
            draw_note_anchor: None,
            pending_preview: None,
            editor_low: 36,
            editor_zoom: 1.0,
            job: None,
            preparing: false,
            sync_needed: true,
            synced_revision: None,
            recorder: None,
            device_pending: None,
            record_pending: None,
            record_finishing: None,
            after_take: None,
            recording_tracks: vec![],
            record_start: 0.0,
            intent: None,
            after_save: None,
            closing: false,
            screenshot,
            frames: 0,
            show_help: false,
        }
    }
    pub fn dispatch(&mut self, command: Command) {
        if self.recorder.is_some()
            || self.record_pending.is_some()
            || self.record_finishing.is_some()
        {
            if let Command::SetTransport(t) = &command {
                let old = &self.store.session().transport;
                if t.tempo != old.tempo
                    || t.time_signature.numerator != old.time_signature.numerator
                    || t.time_signature.denominator != old.time_signature.denominator
                    || t.cycle
                {
                    self.error = Some(
                        "Stop recording before changing tempo, time signature or cycle.".into(),
                    );
                    return;
                }
            }
        }
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
    pub(crate) fn connect(&mut self) {
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
    pub(crate) fn poll(&mut self) {
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
                    if revision == self.store.revision
                        && self
                            .device
                            .as_ref()
                            .is_none_or(|d| d.sample_rate == renderer.rate())
                    {
                        if let Some(device) = &mut self.device {
                            if let Err(e) = device.send(Message::Replace(renderer)) {
                                self.error = Some(e);
                                self.sync_needed = true;
                            } else {
                                self.synced_revision = Some(revision);
                                let selected = self.store.session().tracks.iter().position(|t| {
                                    Some(&t.id)
                                        == self.store.session().view.selected_track_id.as_ref()
                                });
                                if let Err(e) = device.send(Message::Select(selected)) {
                                    self.error = Some(e);
                                }
                            }
                        }
                        if let Some((track, pitch, velocity)) = self.pending_preview.take() {
                            self.preview(&track, pitch, velocity);
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
                    let position = self.position;
                    for (path, buffer) in files {
                        let duration = buffer.duration();
                        self.import_buffer(
                            path.file_stem().and_then(|s| s.to_str()).unwrap_or("Audio"),
                            buffer,
                            None,
                        );
                        let s = self.store.session();
                        self.position += (duration * s.transport.tempo / 60.0 / s.beats_per_bar())
                            .ceil()
                            * s.beats_per_bar();
                    }
                    self.position = position;
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
        if let Some(result) = self
            .record_finishing
            .as_ref()
            .and_then(|rx| match rx.try_recv() {
                Ok(value) => Some(value),
                Err(mpsc::TryRecvError::Disconnected) => Some(Err(
                    "Recording worker stopped before delivering the take".into(),
                )),
                Err(mpsc::TryRecvError::Empty) => None,
            })
        {
            self.record_finishing = None;
            match result {
                Ok(buffer) => {
                    let clips = self.store.session().clips.len();
                    self.import_buffer(
                        "Take",
                        Arc::new(buffer),
                        Some((self.record_start, self.recording_tracks.clone())),
                    );
                    if self.store.session().clips.len() == clips {
                        self.after_take = None;
                    }
                }
                Err(error) => {
                    self.error = Some(error);
                    self.after_take = None;
                }
            }
        }
        if self.record_finishing.is_none() && self.job.is_none() {
            if let Some(action) = self.after_take.take() {
                match action {
                    AfterTake::Save(save_as) => self.save(save_as),
                    AfterTake::Bounce => self.bounce(),
                    AfterTake::Request(intent) => self.request(intent),
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
    pub(crate) fn start_recording(&mut self) {
        if self.recorder.is_some()
            || self.record_pending.is_some()
            || self.record_finishing.is_some()
        {
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
    pub(crate) fn finish_recording(&mut self) {
        if let Some(r) = self.recorder.take() {
            let first = f64::from_bits(r.first_beat.load(Ordering::Relaxed));
            self.record_start = if first.is_finite() {
                first
            } else {
                self.record_start
            };
            let (tx, rx) = mpsc::sync_channel(1);
            self.record_finishing = Some(rx);
            self.status = "Finishing take…".into();
            std::thread::spawn(move || {
                let _ = tx.send(r.finish());
            });
        }
    }
    pub fn preview(&mut self, track: &str, pitch: u8, velocity: u8) {
        if self.sync_needed || self.synced_revision != Some(self.store.revision) {
            self.pending_preview = Some((track.into(), pitch, velocity));
            return;
        }
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
        if audio::library_bytes(&self.library).saturating_add(buffer.frames.len() * 8)
            > audio::MAX_LIBRARY_BYTES
        {
            self.error = Some(
                "Decoded audio library exceeds 1 GiB. Save and reopen to release unused sources."
                    .into(),
            );
            return;
        }
        let mut targets = recorded
            .as_ref()
            .map(|(_, ts)| ts.clone())
            .unwrap_or_default();
        targets.retain(|id| {
            self.store
                .session()
                .tracks
                .iter()
                .any(|t| &t.id == id && t.kind == "audio")
        });
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
        if targets.is_empty() {
            targets.push(self.add_track("audio"));
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
            let mut decoded_bytes = 0usize;
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
                decoded_bytes = decoded_bytes.saturating_add(buffer.frames.len() * 8);
                if decoded_bytes > audio::MAX_LIBRARY_BYTES {
                    return Err("Imported batch exceeds 1 GiB of decoded audio".into());
                }
                files.push((path, Arc::new(buffer)));
            }
            Ok(JobResult::Imported(files))
        });
    }
    pub(crate) fn save(&mut self, save_as: bool) {
        if self.job.is_some() {
            self.status = "Wait for the current operation before saving".into();
            return;
        }
        self.stop();
        if self.record_finishing.is_some() {
            self.after_take = Some(AfterTake::Save(save_as));
            return;
        }
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
    pub(crate) fn bounce(&mut self) {
        if self.job.is_some() {
            return;
        }
        self.stop();
        if self.record_finishing.is_some() {
            self.after_take = Some(AfterTake::Bounce);
            return;
        }
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
    pub(crate) fn load_path(&mut self, path: PathBuf) {
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
    pub(crate) fn request(&mut self, intent: Intent) {
        if self.job.is_some() {
            return;
        }
        self.stop();
        if self.record_finishing.is_some() {
            self.after_take = Some(AfterTake::Request(intent));
            return;
        }
        if self.store.dirty() {
            self.intent = Some(intent);
        } else {
            self.execute(intent);
        }
    }
    pub(crate) fn execute(&mut self, intent: Intent) {
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
    pub(crate) fn keyboard(&mut self, ctx: &egui::Context) {
        let editing_text = ctx
            .memory(|m| m.focused())
            .is_some_and(|id| egui::TextEdit::load_state(ctx, id).is_some());
        if editing_text || self.intent.is_some() {
            return;
        }
        let mods = ctx.input(|i| i.modifiers);
        let pressed = |key| ctx.input_mut(|i| i.consume_key(mods, key));
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

    pub(crate) fn instrument(&mut self, name: &str) {
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
        self.dispatch(Command::SetStrip {
            track: track.clone(),
            strip,
        });
        self.preview(&track, 60, 95);
    }
    pub(crate) fn add_effect(&mut self, name: &str) {
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
    pub(crate) fn add_loop(&mut self, name: &str) {
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
    pub(crate) fn dialogs(&mut self, ctx: &egui::Context) {
        if let Some(intent) = self.intent {
            egui::Modal::new(egui::Id::new("unsaved")).show(ctx, |ui| {
                ui.label(text(
                    "Save your changes?",
                    FS_PANEL_TITLE,
                    Weight::Bold,
                    INK,
                ));
                ui.add_space(6.0);
                ui.label(text(
                    "This session has unsaved changes.",
                    FS_BODY,
                    Weight::Medium,
                    DIM,
                ));
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    if text_button(ui, "Save", Face::Raised).clicked() {
                        self.intent = None;
                        self.after_save = Some(intent);
                        self.save(false);
                    }
                    if text_button(ui, "Discard", Face::Raised).clicked() {
                        self.intent = None;
                        self.execute(intent);
                    }
                    if text_button(ui, "Cancel", Face::Raised).clicked() {
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
                    ui.label(text(message, FS_BODY, Weight::Medium, INK));
                    ui.add_space(10.0);
                    if text_button(ui, "OK", Face::Raised).clicked() {
                        self.error = None;
                    }
                });
        }
        if self.show_help {
            egui::Window::new("Working in Ondera")
                .open(&mut self.show_help)
                .show(ctx, |ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;
                    for line in [
                        "Space: play / stop. Enter: return to start. R: record, A: arm selected track.",
                        "Double-click a MIDI lane to create a region. Draw notes in the piano roll.",
                        "Drag regions to move, drag edges to trim, right-click for options.",
                        "Tools 1 / 2 / 3: pointer, pencil, scissors. Alt disables snapping.",
                        "M / S: mute / solo. C: cycle. K: metronome. F: follow. Z: fit.",
                        "Cmd/Ctrl + S / O / I / B: save, open, import, bounce.",
                        "Cmd/Ctrl + Z / Shift+Z: undo / redo. Cmd/Ctrl + D / T: duplicate / split.",
                        "Drag across the ruler to set the cycle range. Drop audio files to import.",
                        "Drag the tempo readout, click the signature or key to change them.",
                        "Built-in instruments and effects only. No external plugin hosting or agent connection.",
                    ] {
                        ui.label(text(line, FS_BODY, Weight::Medium, INK_CONTROL));
                    }
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
        self.title_bar(ctx);
        self.transport(ctx);
        self.browser(ctx);
        self.inspector(ctx);
        egui::TopBottomPanel::bottom("editor")
            .default_height(300.0)
            .height_range(200.0..=560.0)
            .resizable(true)
            .frame(egui::Frame::new().fill(EDITOR))
            .show(ctx, |ui| {
                self.editor(ui);
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(TIMELINE))
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

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{vec2, Event, Id, Modifiers, PointerButton, Pos2, RawInput, Rect};

    fn frame(app: &mut Ondera, ctx: &egui::Context, events: Vec<Event>, time: f64, editor: bool) {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1200.0, 800.0))),
            events,
            time: Some(time),
            focused: true,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                if editor {
                    app.editor(ui);
                } else {
                    app.arrangement(ui);
                }
            });
        });
    }
    fn pointer(pos: Pos2, pressed: bool) -> Vec<Event> {
        vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            },
        ]
    }
    fn setup() -> (Ondera, egui::Context) {
        let app = Ondera::from_session(store::demo(), None);
        let ctx = egui::Context::default();
        install(&ctx);
        (app, ctx)
    }
    fn notes(app: &Ondera) -> &Vec<Note> {
        let clip = app
            .store
            .session()
            .clips
            .iter()
            .find(|c| c.id == "bass-2")
            .unwrap();
        let ClipData::Midi { notes } = &clip.data else {
            panic!()
        };
        notes
    }

    #[test]
    fn piano_click_draws_note_and_undo_restores_it() {
        let (mut app, ctx) = setup();
        frame(&mut app, &ctx, vec![], 0.0, true);
        let rect = ctx
            .read_response(Id::new(("piano-grid", "bass-2")))
            .unwrap()
            .rect;
        let p = rect.min + vec2(100.0, 25.0);
        let count = notes(&app).len();
        frame(&mut app, &ctx, pointer(p, true), 0.1, true);
        frame(&mut app, &ctx, pointer(p, false), 0.2, true);
        assert_eq!(notes(&app).len(), count + 1);
        assert!(app.error.is_none());
        app.dispatch(Command::Undo);
        assert_eq!(notes(&app).len(), count);
    }
    #[test]
    fn piano_drag_creates_requested_length() {
        let (mut app, ctx) = setup();
        frame(&mut app, &ctx, vec![], 0.0, true);
        let rect = ctx
            .read_response(Id::new(("piano-grid", "bass-2")))
            .unwrap()
            .rect;
        let p = rect.min + vec2(100.0, 25.0);
        frame(&mut app, &ctx, pointer(p, true), 0.1, true);
        frame(
            &mut app,
            &ctx,
            vec![Event::PointerMoved(p + vec2(120.0, 0.0))],
            0.2,
            true,
        );
        frame(
            &mut app,
            &ctx,
            pointer(p + vec2(120.0, 0.0), false),
            0.3,
            true,
        );
        assert!(
            notes(&app).last().unwrap().length > 2.0,
            "Drag should preserve its start across frames"
        );
    }
    #[test]
    fn dragging_region_body_moves_without_resizing() {
        let (mut app, ctx) = setup();
        frame(&mut app, &ctx, vec![], 0.0, false);
        let rect = ctx.read_response(Id::new(("clip", "bass-1"))).unwrap().rect;
        let p = rect.center();
        frame(&mut app, &ctx, pointer(p, true), 0.1, false);
        frame(
            &mut app,
            &ctx,
            vec![Event::PointerMoved(p + vec2(240.0, 0.0))],
            0.2,
            false,
        );
        frame(
            &mut app,
            &ctx,
            pointer(p + vec2(240.0, 0.0), false),
            0.3,
            false,
        );
        let c = app
            .store
            .session()
            .clips
            .iter()
            .find(|c| c.id == "bass-1")
            .unwrap();
        assert_eq!(c.length_bars, 4.0);
        assert_eq!(c.start_bar, 5.0);
    }
    #[test]
    fn ruler_drag_sets_full_cycle_range() {
        let (mut app, ctx) = setup();
        frame(&mut app, &ctx, vec![], 0.0, false);
        let rect = ctx.read_response(Id::new("ruler-drag")).unwrap().rect;
        let p = rect.left_center() + vec2(48.0, 0.0);
        frame(&mut app, &ctx, pointer(p, true), 0.1, false);
        frame(
            &mut app,
            &ctx,
            vec![Event::PointerMoved(p + vec2(144.0, 0.0))],
            0.2,
            false,
        );
        frame(
            &mut app,
            &ctx,
            pointer(p + vec2(144.0, 0.0), false),
            0.3,
            false,
        );
        let t = &app.store.session().transport;
        assert_eq!(t.cycle_start_bar, 1.0);
        assert_eq!(t.cycle_end_bar, 4.0);
    }
    #[test]
    fn ui_callbacks_can_use_sendable_device_handles() {
        fn send<T: std::marker::Send>() {}
        send::<DeviceEngine>();
        send::<Recorder>();
    }
    #[test]
    fn undo_shortcut_works_after_focusing_a_non_text_control() {
        let (mut app, ctx) = setup();
        let original = app.store.session().name.clone();
        app.dispatch(Command::Rename("Edited".into()));
        ctx.memory_mut(|m| m.request_focus(Id::new("combo")));
        let mods = Modifiers {
            command: true,
            ctrl: true,
            ..Default::default()
        };
        let input = RawInput {
            modifiers: mods,
            events: vec![Event::Key {
                key: egui::Key::Z,
                physical_key: Some(egui::Key::Z),
                pressed: true,
                repeat: false,
                modifiers: mods,
            }],
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| app.keyboard(ctx));
        assert_eq!(app.store.session().name, original);
    }
    #[test]
    fn quit_waits_for_final_take_then_offers_to_save_it() {
        let mut app = Ondera::from_session(store::empty(), None);
        app.sync_needed = false;
        app.record_start = 8.0;
        let (tx, rx) = mpsc::sync_channel(1);
        app.record_finishing = Some(rx);
        app.request(Intent::Quit);
        app.poll();
        assert!(!app.closing);
        assert!(app.intent.is_none());
        assert!(app.store.session().clips.is_empty());
        tx.send(Ok(
            audio::AudioBuffer::new(48000, vec![[0.1; 2]; 4800]).unwrap()
        ))
        .unwrap();
        app.poll();
        assert!(!app.closing);
        assert!(matches!(app.intent, Some(Intent::Quit)));
        assert!(app.store.dirty());
        let clip = &app.store.session().clips[0];
        assert_eq!(clip.start_bar, 2.0);
        assert!(clip.length_bars > 0.0);
    }
    #[test]
    fn failed_take_cancels_deferred_quit_and_preserves_session() {
        let mut app = Ondera::from_session(store::empty(), None);
        app.sync_needed = false;
        let before = serde_json::to_value(app.store.session()).unwrap();
        let (tx, rx) = mpsc::sync_channel(1);
        app.record_finishing = Some(rx);
        app.request(Intent::Quit);
        tx.send(Err("Input disconnected".into())).unwrap();
        app.poll();
        assert!(!app.closing);
        assert!(app.intent.is_none());
        assert_eq!(app.error.as_deref(), Some("Input disconnected"));
        assert_eq!(serde_json::to_value(app.store.session()).unwrap(), before);
    }
}
