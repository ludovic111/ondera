use crate::{
    plugins::{Bank, ScanEvent},
    theme::*,
};
use eframe::egui;
use ondera_engine::{
    audio::{self, Library},
    device::{DeviceEngine, LiveInput, Message, RecordedAudio, Recorder},
    document, host, midi,
    model::*,
    plugin::Descriptor,
    render::Renderer,
    session_file::SessionFileLock,
    settings::Settings,
    store::{self, Command, Store},
    Result,
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc,
    },
    time::Duration,
};

/// A note captured from MIDI input or musical typing while recording.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordedNote {
    pub start: f64,
    pub end: Option<f64>,
    pub pitch: u8,
    pub velocity: u8,
    pub channel: u8,
}

#[derive(Clone, Copy)]
pub enum Intent {
    New,
    Open,
    Recover,
    Demo,
    Quit,
    /// Close this copy and start the freshly installed one.
    Relaunch,
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
        ownership: SessionFileLock,
    },
    Saved {
        path: PathBuf,
        revision: u64,
        ownership: SessionFileLock,
    },
    Imported(Vec<(PathBuf, Arc<audio::AudioBuffer>)>),
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
    pub(crate) session_file: Option<SessionFileLock>,
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
    /// A take waiting for the input to open, with its byte limit.
    pub(crate) record_pending: Option<usize>,
    pub(crate) record_finishing: Option<mpsc::Receiver<Result<RecordedAudio>>>,
    pub(crate) after_take: Option<AfterTake>,
    pub(crate) recording_tracks: Vec<String>,
    // Last-resort ownership when both placement and source-file writing fail.
    pub(crate) unplaced_recording: Option<Arc<audio::AudioBuffer>>,
    pub(crate) recovered_recording_write: Option<mpsc::Receiver<Result<Option<PathBuf>>>>,
    pub(crate) record_start: f64,
    pub(crate) intent: Option<Intent>,
    pub(crate) after_save: Option<Intent>,
    after_agent: Option<Intent>,
    pub(crate) closing: bool,
    pub screenshot: Option<PathBuf>,
    pub(crate) frames: usize,
    pub(crate) frontend_ready: bool,
    pub show_help: bool,
    /// The web window shows the mixer in place of the region editor.
    pub show_mixer: bool,
    /// The region editor shows the controller lane under the piano roll.
    pub show_controllers: bool,
    /// The command palette is open.
    pub show_palette: bool,
    /// The copied clip, shared by the window, the CLI and agents.
    pub(crate) clipboard: Option<Clip>,
    /// Width of the arrangement lanes as the window last reported it.
    pub(crate) lane_width: f64,
    pub plugins: Bank,
    pub catalog: Vec<Descriptor>,
    pub(crate) scan_job: Option<mpsc::Receiver<ScanEvent>>,
    pub(crate) midi: Option<midi::MidiInput>,
    pub midi_port: Option<String>,
    pub(crate) midi_route: Arc<AtomicUsize>,
    pub musical_typing: bool,
    pub typing_octave: i32,
    pub(crate) typing_down: Vec<u8>,
    typing_owners: midi::NoteOwners,
    pub(crate) midi_take: Vec<RecordedNote>,
    /// Control changes, bend and pressure of the MIDI take, at absolute beats.
    pub(crate) midi_controls: Vec<(f64, ondera_engine::plugin::Event)>,
    pub(crate) midi_recording: bool,
    pub(crate) recording_midi_tracks: Vec<String>,
    pub output_device: Option<String>,
    pub input_device: Option<String>,
    pub(crate) control: Option<ondera_engine::control::wire::Server>,
    pub(crate) agents: crate::agents::AgentPanel,
    pub(crate) automation: crate::automation::AutomationUi,
    pub(crate) export: crate::export::ExportDialog,
    pub(crate) recovery: crate::recovery::Recovery,
    pub(crate) control_job: Option<crate::control::ControlJob>,
    pub(crate) updates: crate::update::Updates,
    pub(crate) settings: Settings,
    pub(crate) settings_ui: crate::settings::SettingsWindow,
    pub(crate) live_jobs: Vec<crate::control::LiveJob>,
    pub(crate) attach_live: Option<usize>,
    /// The last command started `control_job`, so a waiting caller belongs to it.
    pub(crate) attach_control: bool,
    /// A `session.batch` is running: its commands share one undo step.
    pub(crate) batching: bool,
    /// The one input stream: level meter, monitoring and takes. Open while an audio track is
    /// armed (and the meter setting allows it), a track monitors the input, or a take runs.
    input: Option<LiveInput>,
    input_pending: Option<mpsc::Receiver<Result<LiveInput>>>,
    /// The device and buffer size the input was opened with; a change reopens it between takes.
    input_key: (Option<String>, Option<u32>),
    /// The input failed to open for this key; do not retry until something changes.
    input_failed: bool,
    /// What the open input was last told about monitoring: (monitoring wanted, speakers
    /// allowed). `None` after the input or the output changed, so the tap is made again.
    monitor_key: Option<(bool, bool)>,
    /// How the open input stream answered the request to monitor.
    pub(crate) monitoring: ondera_engine::device::Monitoring,
    /// The user accepted built-in microphone to built-in speakers, until the app closes.
    pub(crate) monitor_speakers_ok: bool,
    pub(crate) bridge_wanted: bool,
}
pub fn id(prefix: &str) -> String {
    ondera_engine::control::new_id(prefix)
}
impl Ondera {
    pub(crate) fn release_typing(&mut self) {
        let pitches = std::mem::take(&mut self.typing_down);
        for pitch in pitches {
            self.live_note(false, pitch, 0);
        }
    }

    pub fn new(
        ctx: &egui::Context,
        path: Option<PathBuf>,
        screenshot: Option<PathBuf>,
        control: bool,
        check_updates: bool,
    ) -> Self {
        install(ctx);
        let screenshot_run = screenshot.is_some();
        let settings = Settings::load();
        let mut app = Self::from_session(store::demo(), screenshot);
        app.settings = settings.clone();
        app.output_device = settings.audio.output_device.clone();
        app.input_device = settings.audio.input_device.clone();
        if settings.audio.connect_midi_on_start {
            app.midi_port = settings.audio.midi_input.clone();
        }
        app.agents.open = settings.interface.agent_panel_open_on_start;
        if (settings.interface.scale - 1.0).abs() > f32::EPSILON {
            ctx.set_zoom_factor(settings.interface.scale);
        }
        app.catalog = host::scan::installed();
        app.connect();
        if control && settings.control.enable_bridge {
            app.start_control(ctx);
        }
        if check_updates && settings.general.check_updates_on_start && !screenshot_run {
            app.check_for_updates(false);
        }
        if settings.plugins.scan_on_start && !screenshot_run {
            app.scan_plugins();
        }
        let path = path.or_else(|| {
            settings
                .general
                .reopen_last_session
                .then(|| settings.general.last_session.as_ref().map(PathBuf::from))
                .flatten()
                .filter(|p| p.is_file())
        });
        if let Some(path) = path {
            app.load_path(path);
        }
        app
    }
    /// Remember a session file in the recent list and as the last one opened.
    /// Keep the audio a finished "Updating audio" job generated, but only what the open
    /// document uses: a New, Open or Recover while it ran cleared the library, and adding the
    /// previous session's buffers back would hold them in memory and count them against the
    /// 1 GiB limit that later imports and recordings hit.
    pub(crate) fn adopt_prepared_library(&mut self, library: Library) {
        let sources = &self.store.session().sources;
        for (id, buffer) in library {
            if sources.contains_key(&id) {
                self.library.entry(id).or_insert(buffer);
            }
        }
    }
    pub(crate) fn remember_session(&mut self, path: &std::path::Path) {
        let text = path.to_string_lossy().into_owned();
        let recent = &mut self.settings.general.recent_sessions;
        recent.retain(|p| p != &text);
        recent.insert(0, text.clone());
        recent.truncate(10);
        self.settings.general.last_session = Some(text);
        // Unit tests open temporary files; they must not touch the person's settings.
        if !cfg!(test) {
            let _ = self.settings.save();
        }
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
            session_file: None,
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
            unplaced_recording: None,
            recovered_recording_write: None,
            record_start: 0.0,
            intent: None,
            after_save: None,
            after_agent: None,
            closing: false,
            screenshot,
            frames: 0,
            frontend_ready: false,
            show_help: false,
            show_mixer: false,
            show_controllers: false,
            show_palette: false,
            clipboard: None,
            lane_width: 960.0,
            plugins: Bank::default(),
            catalog: vec![],
            scan_job: None,
            midi: None,
            midi_port: None,
            midi_route: Arc::new(AtomicUsize::new(midi::UNROUTED)),
            musical_typing: false,
            typing_octave: 0,
            typing_down: vec![],
            typing_owners: Default::default(),
            midi_take: vec![],
            midi_controls: vec![],
            midi_recording: false,
            recording_midi_tracks: vec![],
            output_device: None,
            input_device: None,
            control: None,
            agents: Default::default(),
            automation: Default::default(),
            export: Default::default(),
            recovery: Default::default(),
            control_job: None,
            updates: Default::default(),
            settings: Settings::default(),
            settings_ui: Default::default(),
            live_jobs: vec![],
            attach_live: None,
            attach_control: false,
            batching: false,
            input: None,
            input_pending: None,
            input_key: (None, None),
            input_failed: false,
            monitor_key: None,
            monitoring: ondera_engine::device::Monitoring::Off,
            monitor_speakers_ok: false,
            bridge_wanted: false,
        }
    }
    pub fn dispatch(&mut self, command: Command) {
        if let Err(e) = self.try_dispatch(command) {
            self.error = Some(e);
        }
    }
    /// Apply a command; a change schedules an audio graph update, a selection updates the
    /// device's preview track.
    pub(crate) fn try_dispatch(&mut self, command: Command) -> Result<bool> {
        if self.control_job.is_some()
            && !matches!(command, Command::Select { .. } | Command::SetView(_))
        {
            return Err("Wait for the current agent file operation before editing.".into());
        }
        if self.midi_recording
            || self.recorder.is_some()
            || self.record_pending.is_some()
            || self.record_finishing.is_some()
        {
            fn changes_recording(command: &Command, old: &Transport) -> bool {
                match command {
                    Command::Undo
                    | Command::Redo
                    | Command::RemoveTrack(_)
                    | Command::RestoreTake(_) => true,
                    Command::SetTransport(t) => {
                        t.tempo != old.tempo
                            || t.time_signature.numerator != old.time_signature.numerator
                            || t.time_signature.denominator != old.time_signature.denominator
                            || t.cycle
                    }
                    Command::Batch(commands) => commands.iter().any(|c| changes_recording(c, old)),
                    _ => false,
                }
            }
            if changes_recording(&command, &self.store.session().transport) {
                return Err(
                    "Stop recording before changing timing, undoing, or removing a track.".into(),
                );
            }
        }
        let changed = self.store.dispatch(command)?;
        if changed {
            self.sync_needed = true;
        } else if let Some(device) = &mut self.device {
            let index =
                self.store.session().tracks.iter().position(|t| {
                    Some(&t.id) == self.store.session().view.selected_track_id.as_ref()
                });
            device.send(Message::Select(index))?;
        }
        Ok(changed)
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
        self.unload_plugins();
        self.device = None;
        let empty = store::empty();
        let name = self.output_device.clone();
        let buffer = self.settings.audio.buffer_frames;
        let (tx, rx) = mpsc::sync_channel(1);
        self.device_pending = Some(rx);
        self.status = "Opening audio output…".into();
        std::thread::spawn(move || {
            let _ = tx.send(DeviceEngine::open(name, buffer, |rate| {
                Renderer::new(empty, &Library::new(), rate, &HashMap::new())
            }));
        });
    }
    /// Connect a MIDI input port; `None` picks the first one. Live notes reach
    /// the audio thread directly, so this is redone whenever the device changes.
    pub(crate) fn connect_midi(&mut self, port: Option<String>) {
        if self.midi.is_some() {
            self.stop();
        }
        self.midi = None;
        let Some(device) = &self.device else {
            self.midi_port = port;
            return;
        };
        match midi::connect(
            port.as_deref(),
            device.telemetry.clone(),
            device.sender(),
            self.midi_route.clone(),
        ) {
            Ok(input) => {
                self.midi_port = Some(input.port_name.clone());
                self.midi = Some(input);
            }
            Err(e) => {
                if port.is_some() {
                    self.error = Some(e);
                }
                self.midi_port = None;
            }
        }
    }
    /// Which track receives live notes: an armed instrument track while
    /// recording, else the selected instrument track.
    fn update_midi_route(&mut self) {
        let s = self.store.session();
        let armed = s
            .tracks
            .iter()
            .position(|t| t.kind == "midi" && t.armed)
            .filter(|_| self.record_enabled);
        let selected = s
            .tracks
            .iter()
            .position(|t| t.kind == "midi" && Some(&t.id) == s.view.selected_track_id.as_ref());
        self.midi_route.store(
            armed
                .or(selected)
                .map_or(midi::UNROUTED, |index| midi::route_id(&s.tracks[index].id)),
            Ordering::Relaxed,
        );
    }
    /// A note from musical typing: play it live and record it when armed.
    pub(crate) fn live_note(&mut self, on: bool, pitch: u8, velocity: u8) {
        let (prior, track) =
            self.typing_owners
                .event(on, pitch, 0, self.midi_route.load(Ordering::Relaxed));
        if let (Some(route), Some(device)) = (prior, self.device.as_mut()) {
            if device
                .send(Message::RoutedNote {
                    route,
                    on: false,
                    pitch,
                    velocity: 0,
                })
                .is_err()
            {
                device
                    .telemetry
                    .input_overflow
                    .store(true, Ordering::Release);
            }
        }
        let Some(track) = track else {
            if on {
                self.status = "Select an instrument track to play".into();
            }
            return;
        };
        if let Some(d) = &mut self.device {
            if let Err(e) = d.send(Message::RoutedNote {
                route: track,
                on,
                pitch,
                velocity,
            }) {
                d.telemetry.input_overflow.store(true, Ordering::Release);
                self.stop();
                self.error = Some(e);
                return;
            }
        }
        let beats = self.position;
        self.record_note(on, pitch, velocity, beats);
    }
    fn record_note(&mut self, on: bool, pitch: u8, velocity: u8, beats: f64) {
        self.record_note_channel(on, pitch, velocity, beats, 0);
    }
    fn record_note_channel(&mut self, on: bool, pitch: u8, velocity: u8, beats: f64, channel: u8) {
        if !self.midi_recording {
            return;
        }
        if on {
            if let Some(previous) =
                self.midi_take.iter_mut().rev().find(|note| {
                    note.pitch == pitch && note.channel == channel && note.end.is_none()
                })
            {
                previous.end = Some(beats.max(previous.start));
            }
            self.midi_take.push(RecordedNote {
                start: beats.max(self.record_start),
                end: None,
                pitch,
                velocity: velocity.max(1),
                channel,
            });
        } else if let Some(note) = self
            .midi_take
            .iter_mut()
            .rev()
            .find(|n| n.pitch == pitch && n.channel == channel && n.end.is_none())
        {
            note.end = Some(beats.max(note.start));
        }
    }
    pub(crate) fn record_control(&mut self, event: ondera_engine::plugin::Event, beats: f64) {
        if self.midi_recording {
            self.midi_controls
                .push((beats.max(self.record_start), event));
        }
    }
    fn poll_midi(&mut self) {
        let (events, failed, disconnected) = match &mut self.midi {
            Some(input) => {
                input.check_connection();
                (
                    input.drain(),
                    input.failed.load(Ordering::Relaxed),
                    input.disconnected,
                )
            }
            None => return,
        };
        for e in events {
            let beats = if e.playing { e.beats } else { self.position };
            match e.control {
                Some(control) => self.record_control(control, beats),
                None => self.record_note_channel(e.on, e.pitch, e.velocity, beats, e.channel),
            }
        }
        if failed {
            self.stop();
            self.midi = None;
            self.error = Some(if disconnected { "MIDI input disconnected and playback was stopped. Captured notes were retained; reconnect the MIDI input before continuing." } else { "MIDI input exceeded its event queue capacity and was stopped. Captured notes were retained; reconnect the MIDI input before continuing." }.into());
        }
    }
    /// Turn the captured notes into one region per armed instrument track.
    fn commit_midi_take(&mut self) {
        if !self.midi_recording {
            return;
        }
        self.midi_recording = false;
        // An audio take reports its own progress from here; a MIDI take is over.
        if self.recorder.is_none() && self.record_pending.is_none() {
            self.status = if self.midi_take.is_empty() && self.midi_controls.is_empty() {
                "Ready".into()
            } else {
                "MIDI take recorded".into()
            };
        }
        let end_position = self.position;
        // Key down/up can arrive in the same UI frame. Keep a one-millisecond
        // tap instead of silently deleting it because both saw one playhead time.
        let minimum_length = self.store.session().transport.tempo / 60.0 * 0.001;
        let mut take = std::mem::take(&mut self.midi_take);
        let controls = std::mem::take(&mut self.midi_controls);
        for note in &mut take {
            note.end = Some(
                note.end
                    .unwrap_or(end_position)
                    .max(note.start + minimum_length),
            );
        }
        take.retain(|n| n.end.is_some_and(|e| e > n.start + 1e-6));
        if take.is_empty() && controls.is_empty() {
            return;
        }
        let s = self.store.session();
        let bpb = s.beats_per_bar();
        let start_bar = (self.record_start / bpb).floor();
        let last = take
            .iter()
            .map(|n| n.end.unwrap_or(n.start))
            .chain(controls.iter().map(|(beats, _)| beats + 1e-6))
            .fold(0.0, f64::max);
        let length_bars = ((last - start_bar * bpb) / bpb).ceil().max(1.0);
        let tracks: Vec<String> = self
            .recording_midi_tracks
            .iter()
            .filter(|id| s.tracks.iter().any(|t| &t.id == *id && t.kind == "midi"))
            .cloned()
            .collect();
        let mut commands = vec![];
        for track in tracks {
            let notes = take
                .iter()
                .map(|n| Note {
                    id: id("note"),
                    start: n.start - start_bar * bpb,
                    length: n.end.unwrap_or(n.start) - n.start,
                    pitch: n.pitch,
                    velocity: n.velocity,
                    agent: false,
                })
                .collect();
            let controllers =
                ondera_engine::controllers::recorded(&controls, start_bar * bpb, false, || {
                    id("ctl")
                });
            commands.push(Command::PutClip(Clip {
                id: id("clip"),
                name: "Take".into(),
                agent: false,
                track_id: track,
                start_bar,
                length_bars,
                data: ClipData::Midi { notes, controllers },
            }));
        }
        if !commands.is_empty() {
            self.dispatch(Command::Batch(commands));
        }
    }
    /// Release the rack and the device in the right order before quitting.
    pub(crate) fn shutdown_audio(&mut self) {
        self.stop();
        self.midi = None;
        self.unload_plugins();
        self.device = None;
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
                    // The monitor tap belonged to the previous output callback: make a new
                    // one; the input stream itself stays open.
                    self.monitor_key = None;
                    self.sync_needed = true;
                    self.synced_revision = None;
                    self.connect_midi(self.midi_port.clone());
                }
                Err(e) => self.error = Some(e),
            }
        }
        self.refresh_recording_status();
        self.poll_scan();
        self.collect_retired();
        self.poll_midi();
        self.update_midi_route();
        self.reconcile_plugins();
        self.idle_plugins();
        if self.record_pending.is_some() {
            self.poll_input();
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
                    self.adopt_prepared_library(library);
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
                    ownership,
                }) => self.loaded(*session, library, path, Some(ownership)),
                Ok(JobResult::Saved {
                    path,
                    revision,
                    ownership,
                }) => self.saved(path, revision, ownership),
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
                Ok(take) => {
                    let clips = self.store.session().clips.len();
                    let buffer = Arc::new(take.buffer);
                    self.import_buffer(
                        "Take",
                        buffer.clone(),
                        Some((self.record_start, self.recording_tracks.clone())),
                    );
                    if self.store.session().clips.len() == clips {
                        self.after_take = None;
                        let reason = self.error.take().unwrap_or_else(|| {
                            "The recorded take could not be placed in this session".into()
                        });
                        if let Some(path) = take.recovery_path {
                            self.error = Some(format!("{reason}. The complete captured audio is preserved at {}. Import that WAV into a session with available capacity.", path.display()));
                        } else {
                            self.unplaced_recording = Some(buffer);
                            self.error = Some(format!("{reason}. Source-file backup also failed; the take remains in memory. Free disk space and use Audio > Save recovered take before closing."));
                        }
                    } else if let Some(warning) = take.warning {
                        self.error = Some(warning);
                        self.status = "Recorded take retained with warning".into();
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
        if let Some(result) =
            self.recovered_recording_write
                .as_ref()
                .and_then(|rx| match rx.try_recv() {
                    Ok(value) => Some(value),
                    Err(mpsc::TryRecvError::Disconnected) => Some(Err(
                        "Recovered-take writer stopped; audio remains in memory".into(),
                    )),
                    Err(mpsc::TryRecvError::Empty) => None,
                })
        {
            self.recovered_recording_write = None;
            match result {
                Ok(Some(path)) => { self.unplaced_recording = None; self.error = None; self.status = format!("Recovered take saved to {}", path.display()); }
                Ok(None) => self.status = "Recovered take remains in memory".into(),
                Err(error) => self.error = Some(format!("{error}. Recovered audio remains in memory; retry Audio > Save recovered take.")),
            }
        }
        if self.sync_needed && self.job.is_none() {
            self.sync_needed = false;
            self.preparing = true;
            let session = self.store.snapshot();
            let mut library = self.library.clone();
            let revision = self.store.revision;
            let rate = self.device.as_ref().map_or(48000, |d| d.sample_rate);
            let slots = self.plugins.slots.clone();
            let latencies = self
                .plugins
                .loaded
                .values()
                .map(|entry| (entry.slot, entry.editor.latency()))
                .collect();
            self.spawn("Updating audio…", move || {
                audio::prepare_sources(&session, &mut library)?;
                let mut renderer =
                    Box::new(Renderer::new((*session).clone(), &library, rate, &slots)?);
                renderer.set_latencies(&latencies)?;
                Ok(JobResult::Prepared {
                    renderer,
                    library,
                    revision,
                })
            });
        }
        if let Some(device) = &mut self.device {
            if self.playing {
                self.position = device.telemetry.beats();
            }
            if device.telemetry.device_failed.load(Ordering::Relaxed) {
                self.stop();
                self.device = None;
                self.error=Some("Audio device disconnected. Use Audio > Reconnect output after reconnecting it.".into());
            }
        }
        if self.recorder.as_ref().is_some_and(|r| r.failed()) {
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
        // Recording from a standstill gets a count-in: the song stays parked while the
        // click runs, the microphone opens in the meantime, and the take starts on the beat.
        let session = self.store.session();
        let count_in = if self.record_enabled
            && !session.transport.cycle
            && session.tracks.iter().any(|t| t.armed)
        {
            f64::from(self.settings.audio.count_in_bars.min(4)) * session.beats_per_bar()
        } else {
            0.0
        };
        let message = if count_in > 0.0 {
            d.telemetry.counting_in.store(true, Ordering::Relaxed);
            Message::CountIn(self.position, count_in)
        } else {
            Message::Start(self.position)
        };
        if let Err(e) = d.send(message) {
            d.telemetry.counting_in.store(false, Ordering::Relaxed);
            self.error = Some(e);
            return;
        }
        self.playing = true;
        if self.record_enabled {
            self.start_recording();
        }
    }
    /// Some audio track routes the live input to its strip right now.
    pub(crate) fn monitor_wanted(&self) -> bool {
        use ondera_engine::model::Monitor;
        self.store.session().tracks.iter().any(|t| {
            t.kind == "audio"
                && match t.monitor {
                    Monitor::Off => false,
                    Monitor::Auto => t.armed,
                    Monitor::On => true,
                }
        })
    }
    /// Where an input worker sends the monitor tap, when some track wants to hear the input.
    fn monitor_link(&self) -> Option<ondera_engine::device::MonitorLink> {
        let device = self.device.as_ref()?;
        self.monitor_wanted()
            .then(|| ondera_engine::device::MonitorLink {
                sender: device.sender(),
                output_rate: device.sample_rate,
                output_name: device.device_name.clone(),
                allow_speakers: self.monitor_speakers_ok,
            })
    }
    /// Keep the one input stream open exactly while it is useful: an audio track is armed and
    /// the setting allows the meter, a track monitors the input, or a take runs. Monitoring and
    /// takes attach to the open stream, so neither reopens the device.
    pub(crate) fn poll_input(&mut self) {
        use ondera_engine::device::Monitoring;
        if let Some(result) = self
            .input_pending
            .as_ref()
            .and_then(|rx| match rx.try_recv() {
                Ok(v) => Some(v),
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err("Microphone worker stopped".into()))
                }
                Err(_) => None,
            })
        {
            self.input_pending = None;
            match result {
                Ok(input) => {
                    self.input = Some(input);
                    self.monitor_key = None;
                }
                Err(error) => {
                    self.input_failed = true;
                    if self.record_pending.take().is_some() {
                        self.error = Some(error.clone());
                        self.record_enabled = false;
                    }
                    self.monitoring = if self.monitor_wanted() {
                        Monitoring::Failed(error)
                    } else {
                        Monitoring::Off
                    };
                }
            }
        }
        let taking = self.recorder.is_some() || self.record_pending.is_some();
        // A new device or buffer size reopens the input, but never under a running take.
        let key = (self.input_device.clone(), self.settings.audio.buffer_frames);
        if self.input_key != key && !taking {
            self.input = None;
            self.input_pending = None;
            self.input_failed = false;
            self.input_key = key;
        }
        if !taking && self.input.as_ref().is_some_and(|input| input.failed()) {
            self.input = None;
            self.input_failed = true;
            if self.monitor_wanted() {
                self.monitoring = Monitoring::Failed(
                    "The input device stopped; reconnect it or choose another in Settings > Audio"
                        .into(),
                );
            }
        }
        let monitor = self.monitor_wanted();
        let metering = self.settings.audio.meter_input_when_armed
            && self
                .store
                .session()
                .tracks
                .iter()
                .any(|t| t.kind == "audio" && t.armed);
        let wanted = (metering || monitor || taking) && self.device.is_some();
        if !wanted {
            self.input = None;
            self.input_pending = None;
            self.input_failed = false;
            self.monitor_key = None;
            self.monitoring = Monitoring::Off;
            return;
        }
        if self.input.is_some() {
            let monitor_key = (monitor, self.monitor_speakers_ok);
            if self.monitor_key != Some(monitor_key) {
                let link = self.monitor_link();
                if let Some(input) = self.input.as_mut() {
                    self.monitoring = input.monitor(link.as_ref());
                }
                self.monitor_key = Some(monitor_key);
            }
            if let (Some(limit), Some(input)) = (self.record_pending.take(), self.input.as_mut()) {
                match input.record(limit) {
                    Ok(recorder) => {
                        self.recorder = Some(recorder);
                        self.status = "Recording…".into();
                    }
                    Err(error) => {
                        self.error = Some(error);
                        self.record_enabled = false;
                    }
                }
            }
            return;
        }
        if self.input_pending.is_some() || self.input_failed {
            return;
        }
        let Some(telemetry) = self.device.as_ref().map(|d| d.telemetry.clone()) else {
            return;
        };
        let input = self.input_device.clone();
        let buffer = self.settings.audio.buffer_frames;
        let (tx, rx) = mpsc::sync_channel(1);
        self.input_pending = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(LiveInput::open(telemetry, input, buffer));
        });
    }
    /// What the status line says while a take runs; `None` leaves it alone (the microphone
    /// is still opening and its permission hint matters more).
    pub(crate) fn recording_status(
        counting_in: bool,
        audio: bool,
        midi: bool,
        opening: bool,
    ) -> Option<&'static str> {
        match (counting_in, audio, midi, opening) {
            (_, false, false, _) | (false, false, _, true) => None,
            (true, ..) => Some("Count-in…"),
            (false, true, ..) => Some("Recording…"),
            (false, false, true, false) => Some("Recording MIDI…"),
        }
    }
    fn refresh_recording_status(&mut self) {
        if !self.playing {
            return;
        }
        let counting_in = self
            .device
            .as_ref()
            .is_some_and(|d| d.telemetry.counting_in.load(Ordering::Relaxed));
        if let Some(status) = Self::recording_status(
            counting_in,
            self.recorder.is_some(),
            self.midi_recording,
            self.record_pending.is_some(),
        ) {
            if self.status != status {
                self.status = status.into();
            }
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
        self.recording_midi_tracks = s
            .tracks
            .iter()
            .filter(|t| t.kind == "midi" && t.armed)
            .map(|t| t.id.clone())
            .collect();
        let mut byte_limit = 0;
        if !self.recording_tracks.is_empty() {
            let embedded_bytes: usize = s
                .sources
                .values()
                .filter(|source| source.origin != "generated")
                .filter_map(|source| self.library.get(&source.id))
                .map(|buffer| (buffer.frames.len() * 8 + 128).div_ceil(3) * 4)
                .sum();
            byte_limit = audio::MAX_LIBRARY_BYTES
                .saturating_sub(audio::library_bytes(&self.library))
                .min(
                    (700usize * 1024 * 1024)
                        .saturating_sub(embedded_bytes)
                        .saturating_mul(3)
                        / 4,
                )
                .saturating_sub(4096);
            if byte_limit < 8192
                || s.sources.len() >= 10_000
                || s.clips.len() + self.recording_tracks.len() > 50_000
                || self.unplaced_recording.is_some()
            {
                self.error = Some("No capacity remains for another audio take. Save recovered audio or start a new session first.".into());
                self.record_enabled = false;
                return;
            }
        }
        if self.recording_tracks.is_empty() && self.recording_midi_tracks.is_empty() {
            self.error = Some("Arm an audio or instrument track before recording.".into());
            return;
        }
        let Some(d) = &mut self.device else {
            return;
        };
        self.record_start = self.position;
        let _ = d.send(Message::SetRecording(true));
        if !self.recording_midi_tracks.is_empty() {
            self.midi_recording = true;
            self.midi_take.clear();
            self.midi_controls.clear();
            self.status = "Recording MIDI…".into();
        }
        if self.recording_tracks.is_empty() {
            return;
        }
        // The take joins the open input stream (opening it first if needed), so what a
        // monitoring singer hears does not drop out when the take starts.
        self.record_pending = Some(byte_limit);
        self.status = "Opening microphone — check system permission…".into();
        self.poll_input();
    }
    pub fn stop(&mut self) {
        let events = self
            .midi
            .as_mut()
            .map(|input| {
                let events = input.drain();
                input.reset_notes();
                events
            })
            .unwrap_or_default();
        for event in events {
            match event.control {
                Some(control) => self.record_control(control, event.beats),
                None => self.record_note_channel(
                    event.on,
                    event.pitch,
                    event.velocity,
                    event.beats,
                    event.channel,
                ),
            }
        }
        self.playing = false;
        self.record_pending.take();
        if let Some(d) = &mut self.device {
            d.telemetry.counting_in.store(false, Ordering::Relaxed);
            if let Err(e) = d.send(Message::Stop) {
                self.error = Some(e);
            }
        }
        self.finish_recording();
    }
    pub(crate) fn finish_recording(&mut self) {
        self.commit_midi_take();
        if let Some(d) = &mut self.device {
            let _ = d.send(Message::SetRecording(false));
        }
        self.record_pending = None;
        if let Some(r) = self.recorder.take() {
            r.end();
            self.record_start = r.first_beat().unwrap_or(self.record_start);
            let (tx, rx) = mpsc::sync_channel(1);
            self.record_finishing = Some(rx);
            self.status = "Finishing take…".into();
            std::thread::spawn(move || {
                let result = r.finish().map(|mut take| {
                    let path = host::scan::data_dir().join("recordings").join(format!("{}.wav", id("take")));
                    if let Err(error) = take.preserve(&path) {
                        let detail = format!("Source audio backup failed at {}: {error}. Keep this session open until you save the take.", path.display());
                        take.warning = Some(take.warning.map_or(detail.clone(), |warning| format!("{warning} {detail}")));
                    }
                    take
                });
                let _ = tx.send(result);
            });
        }
    }
    pub(crate) fn save_recovered_take(&mut self) {
        if self.recovered_recording_write.is_some() {
            return;
        }
        let Some(buffer) = self.unplaced_recording.clone() else {
            return;
        };
        let (tx, rx) = mpsc::sync_channel(1);
        self.recovered_recording_write = Some(rx);
        std::thread::spawn(move || {
            let result = (|| {
                let Some(path) = rfd::FileDialog::new()
                    .set_file_name("Recovered take.wav")
                    .add_filter("WAV", &["wav"])
                    .save_file()
                else {
                    return Ok(None);
                };
                ondera_engine::device::preserve_recording(&buffer, &path)?;
                Ok(Some(path))
            })();
            let _ = tx.send(result);
        });
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
            monitor: Default::default(),
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
            data: ClipData::Midi {
                notes: vec![],
                controllers: vec![],
            },
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
                data: ClipData::audio(source_id.clone(), 0.0),
            }));
        }
        self.library.insert(source_id, buffer);
        self.dispatch(Command::Batch(commands));
    }
    pub fn import(&mut self, paths: Option<Vec<PathBuf>>) {
        if self.job.is_some() || self.control_job.is_some() {
            return;
        }
        self.spawn("Importing audio…", move || {
            let paths = paths.or_else(|| {
                rfd::FileDialog::new()
                    .add_filter("Audio", audio::IMPORT_EXTENSIONS)
                    .pick_files()
            });
            let Some(paths) = paths else {
                return Ok(JobResult::Cancelled);
            };
            let mut files = vec![];
            let mut decoded_bytes = 0usize;
            for path in paths {
                let buffer = ondera_engine::control::decode_file(&path)?;
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
        // A save that does not start must not leave "then quit" (or New, Open…) waiting for
        // the next ordinary save to run it.
        if self.job.is_some() || self.control_job.is_some() {
            self.status = "Wait for the current operation before saving".into();
            self.after_save = None;
            return;
        }
        self.stop();
        if self.record_finishing.is_some() {
            self.after_take = Some(AfterTake::Save(save_as));
            return;
        }
        if let Err(error) = self.guarded(Ondera::capture_plugin_states) {
            self.error = Some(error);
            self.after_save = None;
            return;
        }
        let mut session = (*self.store.snapshot()).clone();
        session.transport.position_beats = self.position;
        session.view.pixels_per_bar = self.zoom;
        session.view.scroll_bars = self.scroll;
        let library = self.library.clone();
        let revision = self.store.revision;
        let path = if save_as { None } else { self.path.clone() };
        let current = self.session_file.clone();
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
            let ownership = SessionFileLock::acquire_or_reuse(&path, current.as_ref())?;
            let path = ownership.path().to_path_buf();
            document::save(&session, &library, &path)?;
            Ok(JobResult::Saved {
                path,
                revision,
                ownership,
            })
        });
    }
    pub(crate) fn bounce(&mut self) {
        if self.job.is_some() || self.control_job.is_some() {
            return;
        }
        self.stop();
        if self.record_finishing.is_some() {
            self.after_take = Some(AfterTake::Bounce);
            return;
        }
        self.open_export_dialog();
    }
    /// Install a decoded session and its audio, whether a job or a live client opened it.
    pub(crate) fn loaded(
        &mut self,
        session: Session,
        library: Library,
        path: PathBuf,
        ownership: Option<SessionFileLock>,
    ) {
        self.load_document(session, library, path, ownership, true);
    }
    /// `remember` files the path under Recent sessions and as the session to reopen; a
    /// recovery snapshot is neither.
    pub(crate) fn load_document(
        &mut self,
        session: Session,
        library: Library,
        path: PathBuf,
        ownership: Option<SessionFileLock>,
        remember: bool,
    ) {
        self.unload_plugins();
        self.position = session.transport.position_beats;
        self.zoom = session.view.pixels_per_bar.clamp(12.0, 480.0);
        self.scroll = session.view.scroll_bars.max(0.0);
        if let Err(e) = self.store.load(session) {
            self.error = Some(e);
        } else {
            self.reset_agent_history();
            self.recovery.new_document();
            self.library = library;
            if remember {
                self.remember_session(&path);
            }
            self.path = Some(path);
            self.session_file = ownership;
            self.sync_needed = true;
            self.synced_revision = None;
            self.status = "Session opened".into();
            self.locate(self.position);
        }
    }
    pub(crate) fn saved(&mut self, path: PathBuf, revision: u64, ownership: SessionFileLock) {
        self.remember_session(&path);
        self.path = Some(path);
        self.session_file = Some(ownership);
        self.store.mark_saved(revision);
        self.status = "Session saved".into();
        if let Some(intent) = self.after_save.take() {
            if self.store.dirty() {
                // Edited while saving: ask again rather than quit over the new changes, or
                // keep the request around for an unrelated save later.
                self.intent = Some(intent);
            } else {
                self.execute(intent);
            }
        }
    }
    pub(crate) fn load_path(&mut self, path: PathBuf) {
        if self.unplaced_recording.is_some() {
            self.error = Some("Use Audio > Save recovered take before opening another session; recorded audio remains in memory.".into());
            return;
        }
        self.stop();
        let current = self.session_file.clone();
        self.spawn("Opening session…", move || {
            let ownership = SessionFileLock::acquire_or_reuse(&path, current.as_ref())?;
            let path = ownership.path().to_path_buf();
            let (session, library) = document::load(&path)?;
            Ok(JobResult::Loaded {
                session: Box::new(session),
                library,
                path,
                ownership,
            })
        });
    }
    pub(crate) fn poll_agent(&mut self, ctx: &egui::Context) {
        self.run_agent_tools(ctx);
        if !self.agents.runner_busy() && self.job.is_none() && self.control_job.is_none() {
            if let Some(intent) = self.after_agent.take() {
                // The runner joins its MCP children before becoming idle. Reject commands
                // they queued before cancellation so none can reach the replacement session.
                if let Some(server) = &self.control {
                    for request in server.drain() {
                        request.respond(Err("Agent stopped before switching sessions; retry for the current session.".into()));
                    }
                }
                self.request(intent);
            }
        }
    }
    pub(crate) fn request(&mut self, intent: Intent) {
        if self.agents.runner_busy() {
            self.agents.stop_runner();
            self.after_agent = Some(intent);
            self.status = "Stopping agent before switching sessions…".into();
            return;
        }
        if self.unplaced_recording.is_some() {
            self.error = Some("Use Audio > Save recovered take before closing or replacing this session; recorded audio remains in memory.".into());
            return;
        }
        if self.job.is_some() || self.control_job.is_some() {
            return;
        }
        self.stop();
        if self.record_finishing.is_some() {
            self.after_take = Some(AfterTake::Request(intent));
            return;
        }
        let previous_error = self.error.take();
        self.capture_plugin_states();
        if self.error.is_some() {
            return;
        }
        self.error = previous_error;
        let confirm_quit =
            matches!(intent, Intent::Quit) && self.settings.general.confirm_before_quit;
        if self.store.dirty() || confirm_quit {
            self.intent = Some(intent);
        } else {
            self.execute(intent);
        }
    }
    pub(crate) fn execute(&mut self, intent: Intent) {
        match intent {
            Intent::Quit => {
                self.shutdown_audio();
                self.closing = true;
            }
            Intent::Relaunch => {
                // Without an installed update, Relaunch starts this copy again: quitting
                // without a new window would look like a crash.
                let target = crate::update::relaunch_target(self.updates.installed.take());
                if let Err(e) = target.and_then(|target| crate::update::relaunch(&target)) {
                    self.error = Some(e);
                    return;
                }
                self.shutdown_audio();
                self.closing = true;
            }
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
                self.reset_agent_history();
                self.library.clear();
                self.recovery.new_document();
                self.path = None;
                self.session_file = None;
                self.position = 0.0;
                self.scroll = 0.0;
                self.sync_needed = true;
                self.synced_revision = None;
                self.locate(0.0);
            }
            Intent::Recover => self.restore_recovery(),
            Intent::Open => {
                let current = self.session_file.clone();
                self.spawn("Opening session…", move || {
                    let Some(path) = rfd::FileDialog::new()
                        .add_filter("Ondera session", &["ondera"])
                        .pick_file()
                    else {
                        return Ok(JobResult::Cancelled);
                    };
                    let ownership = SessionFileLock::acquire_or_reuse(&path, current.as_ref())?;
                    let path = ownership.path().to_path_buf();
                    let (session, library) = document::load(&path)?;
                    Ok(JobResult::Loaded {
                        session: Box::new(session),
                        library,
                        path,
                        ownership,
                    })
                })
            }
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
            if pressed(egui::Key::Comma) {
                self.open_settings(None);
            }
            if pressed(egui::Key::K) {
                self.toggle_musical_typing();
            }
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
        if pressed(egui::Key::Backspace) || pressed(egui::Key::Delete) {
            self.delete_selected();
        }
        if self.musical_typing {
            self.musical_typing_keys(ctx);
            return;
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
    pub(crate) fn toggle_musical_typing(&mut self) {
        self.musical_typing = !self.musical_typing;
        if !self.musical_typing {
            for pitch in std::mem::take(&mut self.typing_down) {
                self.live_note(false, pitch, 0);
            }
            self.status = "Musical typing off".into();
        } else {
            self.status = "Musical typing on: A–L play, Z / X shift octave".into();
        }
    }
    /// The computer keyboard as a two-octave piano (Cmd/Ctrl+K).
    fn musical_typing_keys(&mut self, ctx: &egui::Context) {
        use egui::Key;
        let base = 60 + self.typing_octave * 12;
        let semitone = |key: Key| -> Option<i32> {
            Some(match key {
                Key::A => 0,
                Key::W => 1,
                Key::S => 2,
                Key::E => 3,
                Key::D => 4,
                Key::F => 5,
                Key::T => 6,
                Key::G => 7,
                Key::Y => 8,
                Key::H => 9,
                Key::U => 10,
                Key::J => 11,
                Key::K => 12,
                Key::O => 13,
                Key::L => 14,
                Key::P => 15,
                Key::Semicolon => 16,
                _ => return None,
            })
        };
        let events = ctx.input(|i| i.events.clone());
        for event in events {
            let egui::Event::Key {
                key,
                pressed,
                repeat,
                modifiers,
                ..
            } = event
            else {
                continue;
            };
            if modifiers.command || repeat {
                continue;
            }
            match key {
                Key::Z if pressed => self.typing_octave = (self.typing_octave - 1).max(-3),
                Key::X if pressed => self.typing_octave = (self.typing_octave + 1).min(3),
                _ => {
                    if let Some(offset) = semitone(key) {
                        let pitch = (base + offset).clamp(0, 127) as u8;
                        if pressed && !self.typing_down.contains(&pitch) {
                            self.typing_down.push(pitch);
                            self.live_note(true, pitch, 100);
                        } else if !pressed {
                            // Release whichever pitch this key started, even after an octave change.
                            let candidates: Vec<u8> = self
                                .typing_down
                                .iter()
                                .copied()
                                .filter(|p| {
                                    (*p as i32 - offset).rem_euclid(12) == base.rem_euclid(12)
                                })
                                .collect();
                            for p in candidates {
                                self.typing_down.retain(|q| *q != p);
                                self.live_note(false, p, 0);
                            }
                        }
                    }
                }
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
                if let ClipData::Midi { notes, .. } = &mut c.data {
                    notes.retain(|note| &note.id != n);
                }
                self.dispatch(Command::PutClip(c));
            } else {
                self.dispatch(Command::RemoveClip(c.id.clone()));
            }
        }
    }

    /// Load an instrument (stock or external) onto the selected instrument
    /// track, adding one when needed.
    pub(crate) fn instrument(&mut self, plugin_id: &str, name: &str) {
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
        self.set_instrument(&track, plugin_id, name);
        self.preview(&track, 60, 95);
    }
    /// Insert an effect on the selected track, bus or master strip.
    pub(crate) fn add_effect(&mut self, plugin_id: &str, name: &str) {
        let Some(target) = self.store.session().view.selected_track_id.clone() else {
            self.error = Some("Select a track, a bus or the master strip first.".into());
            return;
        };
        self.add_effect_to(&target, plugin_id, name);
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
        self.instrument(&format!("stock:{instrument}"), instrument);
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
            data: ClipData::Midi {
                notes,
                controllers: vec![],
            },
        }));
    }
    pub(crate) fn dialogs(&mut self, ctx: &egui::Context) {
        if let Some(intent) = self.intent {
            let dirty = self.store.dirty();
            egui::Modal::new(egui::Id::new("unsaved"))
                .frame(dialog_frame())
                .show(ctx, |ui| {
                    ui.set_max_width(400.0);
                    ui.label(text(
                        if dirty {
                            "Save your changes?"
                        } else {
                            "Quit Ondera?"
                        },
                        FS_PANEL_TITLE,
                        Weight::Bold,
                        INK,
                    ));
                    ui.add_space(6.0);
                    ui.label(text(
                        if dirty {
                            "This session has unsaved changes."
                        } else {
                            "Everything is saved. Settings > General turns this question off."
                        },
                        FS_BODY,
                        Weight::Medium,
                        DIM,
                    ));
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        if dirty {
                            if text_button(ui, "Save", Face::Lit).clicked() {
                                self.intent = None;
                                self.after_save = Some(intent);
                                self.save(false);
                            }
                            if text_button(ui, "Discard", Face::Raised).clicked() {
                                self.intent = None;
                                self.execute(intent);
                            }
                        } else if text_button(ui, "Quit", Face::Lit).clicked() {
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
            egui::Modal::new(egui::Id::new("error"))
                .frame(dialog_frame())
                .show(ctx, |ui| {
                    ui.set_max_width(480.0);
                    ui.horizontal(|ui| {
                        let (dot, _) =
                            ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                        accent_dot(ui.painter(), dot.center(), 3.5, true);
                        ui.label(text("Ondera", FS_PANEL_TITLE, Weight::Bold, INK));
                    });
                    ui.add_space(6.0);
                    ui.add(
                        egui::Label::new(text(message, FS_BODY, Weight::Medium, INK_CONTROL))
                            .wrap(),
                    );
                    ui.add_space(12.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if text_button(ui, "OK", Face::Raised).clicked() {
                            self.error = None;
                        }
                    });
                });
        }
        if self.show_help {
            egui::Window::new("Working in Ondera")
                .open(&mut self.show_help)
                .frame(window_frame())
                .show(ctx, |ui| plate(ui, "help-plate", |ui| {
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
                        "Cmd/Ctrl + K: musical typing (A–L play notes, Z / X change octave).",
                        "Audio > MIDI input picks a keyboard; arm an instrument track and record to capture notes.",
                        "Effects tab: Ondera plugins plus scanned CLAP, VST3 and Audio Unit plugins.",
                        "Click an insert to open its parameters; Open plugin window shows the native editor.",
                        "Master and A / B buses have their own insert chains in the inspector.",
                        "Cmd/Ctrl + , opens Settings: audio devices, the agent provider, plugin folders and updates.",
                        "The Agent panel (right edge) talks to Codex, Claude Code or an API key; every edit it makes can be reverted.",
                    ] {
                        ui.label(text(line, FS_BODY, Weight::Medium, INK_CONTROL));
                    }
                }));
        }
    }
}
impl eframe::App for Ondera {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let gesture = ctx.input(|i| i.pointer.any_down()) || ctx.wants_keyboard_input();
        self.store.set_gesture(gesture);
        self.frames += 1;
        self.poll();
        self.poll_control_job();
        self.poll_recovery(ctx);
        self.poll_agent(ctx);
        self.serve_control(gesture);
        self.poll_updates();
        self.poll_live_jobs(ctx);
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
        self.agent_panel(ctx);
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
        self.plugin_windows(ctx);
        self.automation_window(ctx);
        self.export_dialog(ctx);
        self.settings_window(ctx);
        self.update_dialog(ctx);
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
        if self.playing
            || self.job.is_some()
            || self.scan_job.is_some()
            || self.midi_recording
            || self.updates.busy()
            || !self.live_jobs.is_empty()
        {
            ctx.request_repaint_after(Duration::from_millis(33));
        } else {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        if self.screenshot.is_some() && self.frames > 40 && self.job.is_none() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        for event in ctx.input(|i| i.events.clone()) {
            if let egui::Event::Screenshot { image, .. } = event {
                if self.deliver_screenshot(&image) {
                    continue;
                }
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

    #[test]
    fn audio_prepared_for_a_replaced_session_is_not_kept() {
        let mut app = Ondera::from_session(store::demo(), None);
        let current: Vec<String> = app.store.session().sources.keys().cloned().collect();
        let buffer = || {
            Arc::new(ondera_engine::audio::AudioBuffer {
                frames: vec![[0.0; 2]; 4],
                sample_rate: 48000,
                peaks: vec![],
            })
        };
        let mut prepared = Library::new();
        prepared.insert("from-the-old-session".into(), buffer());
        for id in &current {
            prepared.insert(id.clone(), buffer());
        }
        app.library.clear();
        app.adopt_prepared_library(prepared);
        assert!(!app.library.contains_key("from-the-old-session"));
        assert_eq!(app.library.len(), current.len());
    }

    #[test]
    fn a_save_that_does_not_happen_forgets_what_was_to_follow() {
        let mut app = Ondera::from_session(store::empty(), None);
        app.dispatch(Command::Rename("Edited".into()));
        app.after_save = Some(Intent::Quit);
        let (_busy, receiver) = mpsc::sync_channel(1);
        app.job = Some(receiver);
        app.save(false);
        assert!(app.after_save.is_none(), "a later Cmd+S must not quit");
        app.job = None;

        // Edits made while the save ran: ask again instead of quitting over them.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("song.ondera");
        let lock = SessionFileLock::acquire_or_reuse(&path, None).unwrap();
        let saved_revision = app.store.revision;
        app.dispatch(Command::Rename("Edited again".into()));
        app.after_save = Some(Intent::Quit);
        app.saved(path, saved_revision, lock);
        assert!(!app.closing);
        assert!(matches!(app.intent, Some(Intent::Quit)));
        assert!(app.after_save.is_none());
    }

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
        let ClipData::Midi { notes, .. } = &clip.data else {
            panic!()
        };
        notes
    }

    #[test]
    fn the_status_line_follows_the_take_and_lets_go_of_it_afterwards() {
        let status = Ondera::recording_status;
        assert_eq!(status(true, false, true, false), Some("Count-in…"));
        assert_eq!(status(true, true, false, false), Some("Count-in…"));
        assert_eq!(status(false, true, true, false), Some("Recording…"));
        assert_eq!(status(false, false, true, false), Some("Recording MIDI…"));
        // Still opening the microphone: keep the permission hint, but never hide the count.
        assert_eq!(status(false, false, true, true), None);
        assert_eq!(status(true, false, true, true), Some("Count-in…"));
        assert_eq!(status(false, false, false, false), None);

        let (mut app, _) = setup();
        app.midi_recording = true;
        app.status = "Recording MIDI…".into();
        app.finish_recording();
        assert_eq!(
            app.status, "Ready",
            "an empty MIDI take leaves nothing behind"
        );
        app.midi_recording = true;
        app.recording_midi_tracks = vec!["bass".into()];
        app.midi_take.push(RecordedNote {
            pitch: 60,
            velocity: 100,
            start: 0.0,
            end: Some(1.0),
            channel: 0,
        });
        app.status = "Recording MIDI…".into();
        app.finish_recording();
        assert_eq!(app.status, "MIDI take recorded");
    }

    #[test]
    fn switching_sessions_waits_for_agent_cleanup_and_rejects_queued_edits() {
        use ondera_engine::control::wire;
        let (mut app, ctx) = setup();
        app.store.mark_unsaved();
        let before = serde_json::to_value(app.store.session()).unwrap();
        let directory = std::env::temp_dir().join(id("ondera-agent-stop"));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("control.json");
        let (wake_tx, wake_rx) = mpsc::channel();
        app.control = Some(
            wire::Server::start_at(path.clone(), move || {
                let _ = wake_tx.send(());
            })
            .unwrap(),
        );
        let complete = app.agents.mock_running_task();
        app.request(Intent::New);
        assert!(app.agents.runner_busy());
        assert!(app.intent.is_none());
        let client = std::thread::spawn(move || {
            let mut client = wire::Client::connect_at(&path).unwrap();
            client.call(
                "track.add",
                &serde_json::json!({"kind":"midi", "name":"Late agent edit"}),
                true,
            )
        });
        wake_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        app.poll_agent(&ctx);
        assert!(app.intent.is_none());
        complete();
        app.poll_agent(&ctx);
        assert!(matches!(app.intent, Some(Intent::New)));
        assert!(client
            .join()
            .unwrap()
            .unwrap_err()
            .contains("Agent stopped"));
        app.serve_control(false);
        assert_eq!(serde_json::to_value(app.store.session()).unwrap(), before);
        assert!(app.after_agent.is_none());
        drop(app);
        let _ = std::fs::remove_dir_all(directory);
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
        send::<LiveInput>();
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
        tx.send(Ok(RecordedAudio {
            buffer: audio::AudioBuffer::new(48000, vec![[0.1; 2]; 4800]).unwrap(),
            warning: None,
            recovery_path: None,
        }))
        .unwrap();
        app.poll();
        // Plugin reconciliation schedules an audio update; the quit waits for it too.
        let started = std::time::Instant::now();
        while app.job.is_some() && started.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(5));
            app.poll();
        }
        assert!(!app.closing);
        assert!(matches!(app.intent, Some(Intent::Quit)));
        assert!(app.store.dirty());
        let clip = &app.store.session().clips[0];
        assert_eq!(clip.start_bar, 2.0);
        assert!(clip.length_bars > 0.0);
    }
    #[test]
    fn midi_take_becomes_a_region_on_each_armed_instrument_track() {
        let mut app = Ondera::from_session(store::empty(), None);
        app.sync_needed = false;
        let track = app
            .store
            .session()
            .tracks
            .iter()
            .find(|t| t.kind == "midi")
            .unwrap()
            .id
            .clone();
        app.recording_midi_tracks = vec![track.clone()];
        app.midi_recording = true;
        app.record_start = 4.0;
        app.position = 4.0;
        app.record_note(true, 60, 100, 4.5);
        app.record_control(ondera_engine::plugin::Event::control(0, 1, 64), 4.75);
        app.record_note(true, 64, 90, 5.0);
        app.record_control(ondera_engine::plugin::Event::pitch_bend(0, -1.0), 5.5);
        app.record_note(false, 60, 0, 6.0);
        app.position = 7.0;
        app.finish_recording();
        let clips: Vec<_> = app
            .store
            .session()
            .clips
            .iter()
            .filter(|c| c.track_id == track)
            .collect();
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].start_bar, 1.0);
        assert_eq!(clips[0].length_bars, 1.0);
        let ClipData::Midi { notes, controllers } = &clips[0].data else {
            panic!()
        };
        assert_eq!(
            controllers
                .iter()
                .map(|c| (c.kind.as_str(), c.number, c.time, c.value))
                .collect::<Vec<_>>(),
            vec![("cc", Some(1), 0.75, 64), ("bend", None, 1.5, -8192)],
            "the take keeps controllers relative to the region"
        );
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].start, 0.5);
        assert_eq!(notes[0].length, 1.5);
        assert_eq!(notes[1].pitch, 64);
        assert_eq!(
            notes[1].length, 2.0,
            "open notes close when recording stops"
        );
        assert!(!app.midi_recording);
        app.dispatch(Command::Undo);
        assert!(app.store.session().clips.is_empty());
    }
    #[test]
    fn musical_typing_taps_in_one_frame_are_recorded_and_undoable() {
        let mut app = Ondera::from_session(store::empty(), None);
        let track = app
            .store
            .session()
            .tracks
            .iter()
            .find(|track| track.kind == "midi")
            .unwrap()
            .id
            .clone();
        app.recording_midi_tracks = vec![track];
        app.midi_recording = true;
        app.record_start = 4.0;
        app.position = 4.0;
        app.record_note(true, 60, 100, 4.0);
        app.record_note(false, 60, 0, 4.0);
        app.record_note(true, 64, 90, 4.0);
        app.commit_midi_take();
        let ClipData::Midi { notes, .. } = &app.store.session().clips[0].data else {
            panic!()
        };
        assert_eq!(notes.len(), 2);
        for note in notes {
            assert!(
                (note.length * 60.0 / app.store.session().transport.tempo - 0.001).abs() < 1e-9
            );
        }
        app.dispatch(Command::Undo);
        assert!(app.store.session().clips.is_empty());
    }

    #[test]
    fn live_sustain_records_pedal_length_and_stop_finishes_held_notes() {
        let mut app = Ondera::from_session(store::empty(), None);
        app.recording_midi_tracks = vec![app
            .store
            .session()
            .tracks
            .iter()
            .find(|track| track.kind == "midi")
            .unwrap()
            .id
            .clone()];
        app.midi_recording = true;
        let mut parser = midi::MidiNotes::default();
        for (beat, bytes) in [
            (0.0, [0x90, 60, 100]),
            (0.5, [0xb0, 64, 127]),
            (1.0, [0x80, 60, 0]),
            (2.0, [0x91, 60, 80]),
            (2.5, [0x81, 60, 0]),
            (3.0, [0xb0, 64, 0]),
            (4.0, [0x90, 67, 100]),
            (4.5, [0xb0, 64, 127]),
            (5.0, [0x80, 67, 0]),
        ] {
            parser.receive(&bytes, midi::route_id("track"), |event| {
                app.record_note_channel(event.on, event.pitch, event.velocity, beat, event.channel)
            });
        }
        app.position = 6.0;
        app.stop();
        let ClipData::Midi { notes, .. } = &app.store.session().clips[0].data else {
            panic!()
        };
        assert_eq!(
            notes
                .iter()
                .map(|note| (note.pitch, note.start, note.length))
                .collect::<Vec<_>>(),
            vec![(60, 0.0, 3.0), (60, 2.0, 0.5), (67, 4.0, 2.0)]
        );
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("sustain.ondera");
        document::save(app.store.session(), &app.library, &path).unwrap();
        let (loaded, _) = document::load(&path).unwrap();
        let ClipData::Midi { notes, .. } = &loaded.clips[0].data else {
            panic!()
        };
        assert_eq!(notes[0].length, 3.0);
        app.dispatch(Command::Undo);
        assert!(app.store.session().clips.is_empty());
    }
    #[test]
    fn effects_can_target_the_master_strip_and_reconcile_loads_stock_plugins() {
        let mut app = Ondera::from_session(store::empty(), None);
        app.dispatch(Command::Select {
            track: Some(MASTER.into()),
            clip: None,
            note: None,
        });
        app.add_effect("stock:Limiter", "Limiter");
        let master = &app.store.session().strips[MASTER];
        assert_eq!(master.inserts.len(), 1);
        assert_eq!(master.inserts[0].plugin_id(), "stock:Limiter");
        app.reconcile_plugins();
        let key = app.store.session().strips[MASTER].inserts[0].id.clone();
        assert!(app.plugins.loaded.contains_key(&key));
        assert!(app.plugins.windows.contains_key(&key));
        // Stock instruments of both bus effects and the instrument track are loaded too.
        assert!(app
            .plugins
            .loaded
            .values()
            .any(|l| l.plugin_id == "stock:Space"));
        assert!(app
            .plugins
            .loaded
            .values()
            .any(|l| l.plugin_id == "stock:Ondera Synth"));
        // Removing the insert retires its instance.
        let mut strip = app.store.session().strips[MASTER].clone();
        strip.inserts.clear();
        app.dispatch(Command::SetStrip {
            track: MASTER.into(),
            strip,
        });
        app.reconcile_plugins();
        assert!(!app.plugins.loaded.contains_key(&key));
    }
    #[test]
    fn installed_audio_unit_loads_through_the_plugin_bank() {
        // Apple's AUDelay ships with macOS; the test is a no-op elsewhere or before a scan.
        let id = "au:61756678:64656c79:6170706c";
        let Some(desc) = host::scan::lookup(id) else {
            return;
        };
        let mut app = Ondera::from_session(store::empty(), None);
        app.catalog = host::scan::installed();
        let track = app.store.session().tracks[0].id.clone();
        app.add_effect_to(&track, id, &desc.name);
        app.reconcile_plugins();
        let key = app.store.session().strips[&track].inserts[0].id.clone();
        let entry = app.plugins.loaded.get(&key).expect("AUDelay instantiates");
        assert!(entry.external);
        assert!(entry.editor.has_gui());
        assert_eq!(entry.editor.params().len(), 4);
        app.capture_plugin_states();
        assert!(!app.store.session().strips[&track].inserts[0]
            .blob
            .is_empty());
        app.unload_plugins();
        assert!(app.plugins.loaded.is_empty());
    }
    #[test]
    fn inspector_renders_bus_strips_without_a_track() {
        let (mut app, ctx) = setup();
        app.dispatch(Command::Select {
            track: Some(BUS_A.into()),
            clip: None,
            note: None,
        });
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1200.0, 800.0))),
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| app.inspector(ctx));
        assert!(app.error.is_none());
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

    #[test]
    fn interrupted_take_imports_recovered_audio_warns_and_cancels_deferred_quit() {
        let mut app = Ondera::from_session(store::empty(), None);
        app.sync_needed = false;
        let (tx, rx) = mpsc::sync_channel(1);
        app.record_finishing = Some(rx);
        app.request(Intent::Quit);
        tx.send(Ok(RecordedAudio {
            buffer: audio::AudioBuffer::new(48000, vec![[0.125; 2]; 4800]).unwrap(),
            warning: Some("Input disconnected; partial take was recovered".into()),
            recovery_path: None,
        }))
        .unwrap();
        app.poll();
        assert!(!app.closing);
        assert!(app.after_take.is_none());
        assert!(app.intent.is_none());
        assert!(app.store.dirty());
        let clip = app.store.session().clips.last().unwrap();
        let ClipData::Audio { source_id, .. } = &clip.data else {
            panic!("Recovered take must be audio")
        };
        assert_eq!(app.library[source_id].frames, vec![[0.125; 2]; 4800]);
        assert!(app
            .error
            .as_deref()
            .unwrap()
            .contains("partial take was recovered"));
        app.dispatch(Command::Undo);
        assert!(app.store.session().clips.is_empty());
    }

    #[test]
    fn unplaceable_take_keeps_memory_blocks_close_and_failed_backup_retry_keeps_ownership() {
        let mut session = store::empty();
        let track = session.tracks[0].clone();
        session.tracks = (0..128)
            .map(|i| Track {
                id: format!("track-{i}"),
                ..track.clone()
            })
            .collect();
        session.strips.clear();
        session.view.selected_track_id = None;
        let mut app = Ondera::from_session(session, None);
        app.sync_needed = false;
        let (tx, rx) = mpsc::sync_channel(1);
        app.record_finishing = Some(rx);
        tx.send(Ok(RecordedAudio {
            buffer: audio::AudioBuffer::new(48000, vec![[0.25; 2]; 100]).unwrap(),
            warning: Some("Disk full".into()),
            recovery_path: None,
        }))
        .unwrap();
        app.poll();
        assert_eq!(
            app.unplaced_recording.as_ref().unwrap().frames,
            vec![[0.25; 2]; 100]
        );
        app.request(Intent::Quit);
        assert!(!app.closing);
        assert!(app.intent.is_none());
        app.request(Intent::New);
        assert_eq!(app.store.session().tracks.len(), 128);
        let (tx, rx) = mpsc::sync_channel(1);
        app.recovered_recording_write = Some(rx);
        tx.send(Err("Disk remains full".into())).unwrap();
        app.poll();
        assert!(app.unplaced_recording.is_some());
        assert!(app.error.as_deref().unwrap().contains("remains in memory"));
    }

    #[test]
    fn typing_release_uses_original_owner_after_track_selection_changes() {
        let mut app = Ondera::from_session(store::empty(), None);
        app.midi_route
            .store(midi::route_id("first"), Ordering::Relaxed);
        app.live_note(true, 60, 100);
        app.midi_route
            .store(midi::route_id("second"), Ordering::Relaxed);
        assert_eq!(
            app.typing_owners.event(false, 60, 0, midi::UNROUTED),
            (None, Some(midi::route_id("first")))
        );
    }

    #[test]
    fn idle_control_keeps_a_drag_as_one_undo_step() {
        let (mut app, _ctx) = setup();
        let dir = std::env::temp_dir().join(format!("ondera-gesture-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        app.control = Some(
            ondera_engine::control::wire::Server::start_at(dir.join("control.json"), || {})
                .unwrap(),
        );
        let tempo = |app: &Ondera| app.store.session().transport.tempo;
        let start = tempo(&app);
        let set = |app: &mut Ondera, bpm: f64| {
            let mut t = app.store.session().transport.clone();
            t.tempo = bpm;
            app.dispatch(Command::SetTransport(t));
        };
        app.store.set_gesture(true);
        set(&mut app, start + 1.0);
        app.serve_control(true);
        set(&mut app, start + 2.0);
        app.store.set_gesture(false);
        app.dispatch(Command::Undo);
        assert_eq!(tempo(&app), start, "the whole drag is one undo step");
        app.control = None;
        let _ = std::fs::remove_dir_all(dir);
    }
}
