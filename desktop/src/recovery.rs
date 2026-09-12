//! Periodic recovery copies, separate from the project chosen by the user.
//! One worker owns file I/O. Generation/revision checks prevent an old job
//! from replacing a newer document or masking edits made during recovery.

use crate::{
    app::{Intent, Ondera},
    theme::*,
};
use eframe::egui;
use ondera_engine::{audio::Library, document, host, model::Session, Result};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::mpsc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const INTERVAL: Duration = Duration::from_secs(30);

pub(crate) struct Recovery {
    generation: u64,
    run: u128,
    last_attempt: Instant,
    last_revision: Option<u64>,
    path: Option<PathBuf>,
    latest: Option<(PathBuf, SystemTime)>,
    worker: Option<Worker>,
    open: bool,
    refresh: bool,
    candidates: Vec<Candidate>,
    selected: Option<PathBuf>,
    error: Option<String>,
}

impl Default for Recovery {
    fn default() -> Self {
        Self {
            generation: 0,
            run: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            last_attempt: Instant::now(),
            last_revision: None,
            path: None,
            latest: None,
            worker: None,
            open: false,
            refresh: false,
            candidates: vec![],
            selected: None,
            error: None,
        }
    }
}

struct Worker {
    generation: u64,
    receiver: mpsc::Receiver<Result<Outcome>>,
}

enum Outcome {
    Saved {
        path: PathBuf,
        revision: u64,
    },
    Listed(Vec<Candidate>),
    Loaded {
        path: PathBuf,
        session: Box<Session>,
        library: Library,
        revision: u64,
    },
}

struct Candidate {
    path: PathBuf,
    title: String,
    modified: SystemTime,
    bytes: u64,
}

impl Recovery {
    pub(crate) fn new_document(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.last_attempt = Instant::now();
        self.last_revision = None;
        self.path = None;
        self.latest = None;
        self.open = false;
        self.refresh = false;
        self.selected = None;
        self.error = None;
        // Keep the old worker until it finishes: document changes cannot create
        // another concurrent writer. Its result belongs to the old generation.
    }

    pub(crate) fn status(&self) -> String {
        if let Some(error) = &self.error {
            return format!("Recovery: {error}");
        }
        if self.worker.is_some() {
            return "Recovery: working…".into();
        }
        match &self.latest {
            Some((_, time)) => format!("Recovery snapshot · {}", age(*time)),
            None => "Recovery snapshots · every 30s when edited and idle".into(),
        }
    }

    fn due(&self, now: Instant, dirty: bool, revision: u64, native_plugins: bool) -> bool {
        self.worker.is_none()
            && (native_plugins || (dirty && self.last_revision != Some(revision)))
            && now.saturating_duration_since(self.last_attempt) >= INTERVAL
    }

    fn spawn(&mut self, task: impl FnOnce() -> Result<Outcome> + Send + 'static) {
        debug_assert!(self.worker.is_none());
        let (tx, receiver) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(task))
                .unwrap_or_else(|_| Err("Recovery worker stopped unexpectedly".into()));
            let _ = tx.send(outcome);
        });
        self.worker = Some(Worker {
            generation: self.generation,
            receiver,
        });
    }

    fn poll(&mut self) -> Option<(u64, Result<Outcome>)> {
        let worker = self.worker.as_ref()?;
        let result = match worker.receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => Err("Recovery worker disconnected".into()),
        };
        let generation = self.worker.take().unwrap().generation;
        Some((generation, result))
    }
}

impl Ondera {
    pub(crate) fn open_recovery(&mut self) {
        self.recovery.open = true;
        self.recovery.refresh = true;
    }

    pub(crate) fn restore_recovery(&mut self) {
        let Some(path) = self.recovery.selected.take() else {
            return;
        };
        if self.recovery.worker.is_some() {
            self.error =
                Some("Wait for the recovery worker, then choose the snapshot again.".into());
            return;
        }
        let revision = self.store.revision;
        self.recovery.spawn(move || {
            ensure_generated(&directory(), &path)?;
            let (session, library) = document::load(&path)?;
            Ok(Outcome::Loaded {
                path,
                session: Box::new(session),
                library,
                revision,
            })
        });
    }

    pub(crate) fn poll_recovery(&mut self, ctx: &egui::Context) {
        if let Some((generation, result)) = self.recovery.poll() {
            if generation == self.recovery.generation {
                match result {
                    Ok(Outcome::Saved { path, revision }) => {
                        self.recovery.last_revision = Some(revision);
                        self.recovery.latest = Some((path, SystemTime::now()));
                        self.recovery.error = None;
                    }
                    Ok(Outcome::Listed(candidates)) => {
                        self.recovery.candidates = candidates;
                        self.recovery.error = None;
                    }
                    Ok(Outcome::Loaded {
                        path,
                        session,
                        library,
                        revision,
                    }) => {
                        if self.store.revision == revision {
                            self.loaded(*session, library, path, None);
                            if self.store.revision != revision {
                                self.path = None;
                                self.store.mark_unsaved();
                                self.status =
                                    "Recovered a copy — Save chooses its project file".into();
                            }
                        } else {
                            self.recovery.error = Some("The session changed while recovery was opening. Choose the snapshot again.".into());
                            self.recovery.open = true;
                        }
                    }
                    Err(error) => {
                        self.recovery.error = Some(error);
                    }
                }
            }
        }
        if self.recovery.refresh && self.recovery.worker.is_none() {
            self.recovery.refresh = false;
            self.recovery
                .spawn(|| Ok(Outcome::Listed(list(&directory())?)));
        }
        let idle = self.job.is_none()
            && self.control_job.is_none()
            && self.scan_job.is_none()
            && !self.export.busy()
            && !self.recovery.open
            && self.recorder.is_none()
            && self.record_pending.is_none()
            && self.record_finishing.is_none()
            && !self.midi_recording
            && self.after_take.is_none()
            && self.intent.is_none()
            && !ctx.input(|input| input.pointer.any_down())
            && !ctx.wants_keyboard_input();
        let now = Instant::now();
        if idle
            && self.recovery.due(
                now,
                self.store.dirty(),
                self.store.revision,
                self.plugins.loaded.values().any(|entry| entry.external),
            )
        {
            self.recovery.last_attempt = now;
            let previous_error = self.error.take();
            self.capture_plugin_states();
            if let Some(error) = self.error.take() {
                self.recovery.error = Some(error);
            } else {
                if !self.store.dirty() || self.recovery.last_revision == Some(self.store.revision) {
                    self.error = previous_error;
                    self.recovery_dialog(ctx);
                    return;
                }
                let mut session = self.store.session().clone();
                session.transport.position_beats = self.position;
                session.view.pixels_per_bar = self.zoom;
                session.view.scroll_bars = self.scroll;
                let library = self.library.clone();
                let revision = self.store.revision;
                let path = self
                    .recovery
                    .path
                    .get_or_insert_with(|| {
                        generated_path(
                            &directory(),
                            self.recovery.run,
                            self.recovery.generation,
                            &session.name,
                        )
                    })
                    .clone();
                self.recovery.spawn(move || {
                    write_snapshot(&path, &session, &library)?;
                    Ok(Outcome::Saved { path, revision })
                });
            }
            self.error = previous_error;
        }
        self.recovery_dialog(ctx);
        if self.recovery.worker.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    fn recovery_dialog(&mut self, ctx: &egui::Context) {
        if !self.recovery.open {
            return;
        }
        let mut open = true;
        let mut selected = None;
        egui::Window::new("Recover a session").open(&mut open).default_width(BROWSER * 2.0)
            .resizable(true).show(ctx, |ui| {
                ui.label(text("Ondera saves a separate recovery copy every 30 seconds while an edited session is idle. Your project file is preserved.", FS_BODY, Weight::Medium, INK));
                ui.label(text("Choose a snapshot to open a copy. Current unsaved edits will be offered for saving first; Save then chooses the recovered project's destination.", FS_SECONDARY, Weight::Medium, DIM));
                ui.label(mono(self.recovery.status(), FS_SMALL, FAINT));
                ui.label(mono(directory().display().to_string(), FS_SMALL, FAINT));
                if text_button(ui, "Refresh", Face::Raised).clicked() { self.recovery.refresh = true; }
                ui.separator();
                if self.recovery.worker.is_some() { ui.spinner(); }
                if self.recovery.candidates.is_empty() && self.recovery.worker.is_none() {
                    ui.label("No recovery snapshots are available yet.");
                }
                egui::ScrollArea::vertical().max_height(FADER_H * 2.0).show(ui, |ui| {
                    for candidate in &self.recovery.candidates {
                        ui.horizontal(|ui| {
                            ui.add_enabled_ui(self.recovery.worker.is_none(), |ui| {
                                if text_button(ui, &candidate.title, Face::Raised).clicked() {
                                    selected = Some(candidate.path.clone());
                                }
                            });
                            ui.label(mono(format!("{} · {:.1} MB", age(candidate.modified), candidate.bytes as f64 / 1_000_000.0), FS_SMALL, DIM));
                        });
                    }
                });
            });
        self.recovery.open = open;
        if let Some(path) = selected {
            self.recovery.selected = Some(path);
            self.recovery.open = false;
            self.request(Intent::Recover);
        }
    }
}

fn directory() -> PathBuf {
    host::scan::data_dir().join("recovery")
}

fn generated_path(directory: &Path, run: u128, generation: u64, name: &str) -> PathBuf {
    let title: String = name
        .trim_end_matches(".ondera")
        .chars()
        .take(80)
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    directory.join(format!(
        "recovery-{run}-{}-{generation}-{}.ondera",
        std::process::id(),
        if title.is_empty() { "Untitled" } else { &title }
    ))
}

fn generated_title(path: &Path) -> Option<String> {
    if path.extension()?.to_str()? != "ondera" {
        return None;
    }
    let name = path.file_stem()?.to_str()?.strip_prefix("recovery-")?;
    let mut parts = name.splitn(4, '-');
    parts.next()?.parse::<u128>().ok()?;
    parts.next()?.parse::<u32>().ok()?;
    parts.next()?.parse::<u64>().ok()?;
    let title = parts.next()?;
    if title.is_empty()
        || !title
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    {
        return None;
    }
    Some(title.replace('_', " "))
}

fn ensure_generated(directory: &Path, path: &Path) -> Result<()> {
    if generated_title(path).is_none() {
        return Err("Choose an Ondera-generated recovery snapshot.".into());
    }
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !metadata.file_type().is_file() {
        return Err("Recovery requires a regular generated snapshot file.".into());
    }
    let expected = directory
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let actual = path
        .parent()
        .ok_or("Recovery path has no parent")?
        .canonicalize()
        .map_err(|error| error.to_string())?;
    if actual != expected {
        return Err("Recovery files must be in Ondera's recovery directory.".into());
    }
    Ok(())
}

fn write_snapshot(path: &Path, session: &Session, library: &Library) -> Result<()> {
    if generated_title(path).is_none() {
        return Err("Invalid generated recovery filename".into());
    }
    fs::create_dir_all(path.parent().ok_or("Recovery path has no parent")?)
        .map_err(|error| error.to_string())?;
    document::save(session, library, path)
}

fn list(directory: &Path) -> Result<Vec<Candidate>> {
    if !directory.exists() {
        return Ok(vec![]);
    }
    let mut candidates = vec![];
    for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        if !entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_file()
        {
            continue;
        }
        let path = entry.path();
        let Some(title) = generated_title(&path) else {
            continue;
        };
        let metadata = entry.metadata().map_err(|error| error.to_string())?;
        candidates.push(Candidate {
            path,
            title,
            modified: metadata.modified().unwrap_or(UNIX_EPOCH),
            bytes: metadata.len(),
        });
    }
    candidates.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(candidates)
}

fn age(time: SystemTime) -> String {
    let seconds = time.elapsed().unwrap_or_default().as_secs();
    if seconds < 60 {
        format!("{seconds}s ago")
    } else if seconds < 3600 {
        format!("{}m ago", seconds / 60)
    } else if seconds < 86400 {
        format!("{}h ago", seconds / 3600)
    } else {
        format!("{}d ago", seconds / 86400)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ondera_engine::store;

    fn temp() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "ondera-recovery-test-{}",
            ondera_engine::control::new_id("case")
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn generated_recovery_writes_preserve_originals_and_latest_copy_loads() {
        let root = temp();
        let original = root.join("song.ondera");
        let mut session = store::empty();
        document::save(&session, &Library::new(), &original).unwrap();
        let original_bytes = fs::read(&original).unwrap();
        let path = generated_path(&root.join("recovery"), 1234, 1, "My Song / Copy");
        session.name = "Unsaved arrangement".into();
        write_snapshot(&path, &session, &Library::new()).unwrap();
        assert_eq!(fs::read(&original).unwrap(), original_bytes);
        assert_eq!(document::load(&path).unwrap().0.name, "Unsaved arrangement");
        session.name = "Latest arrangement".into();
        write_snapshot(&path, &session, &Library::new()).unwrap();
        assert_eq!(list(&root.join("recovery")).unwrap().len(), 1);
        assert_eq!(document::load(&path).unwrap().0.name, "Latest arrangement");
        let previous_snapshot = fs::read(&path).unwrap();
        session.transport.tempo = f64::NAN;
        assert!(write_snapshot(&path, &session, &Library::new()).is_err());
        assert_eq!(fs::read(&path).unwrap(), previous_snapshot);
        assert_eq!(fs::read(&original).unwrap(), original_bytes);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn recovery_eligibility_is_dirty_interval_and_revision_sensitive() {
        let mut recovery = Recovery::default();
        let later = recovery.last_attempt + INTERVAL;
        assert!(!recovery.due(later, false, 1, false));
        assert!(!recovery.due(recovery.last_attempt, true, 1, false));
        assert!(recovery.due(later, true, 1, false));
        recovery.last_revision = Some(1);
        assert!(!recovery.due(later, true, 1, false));
        assert!(recovery.due(later, true, 2, false));
        assert!(
            recovery.due(later, false, 1, true),
            "Native-only edits need a capture even without a new store revision"
        );
    }

    #[test]
    fn new_document_keeps_single_worker_and_rejects_old_generation() {
        let mut recovery = Recovery::default();
        let (tx, receiver) = mpsc::sync_channel(1);
        recovery.worker = Some(Worker {
            generation: recovery.generation,
            receiver,
        });
        recovery.new_document();
        assert!(!recovery.due(recovery.last_attempt + INTERVAL, true, 2, true));
        tx.send(Ok(Outcome::Listed(vec![]))).unwrap();
        let (generation, _) = recovery.poll().unwrap();
        assert_ne!(generation, recovery.generation);
    }

    #[test]
    fn completed_restore_cannot_replace_a_new_document_or_intervening_edit() {
        for new_document in [false, true] {
            let mut app = Ondera::from_session(store::empty(), None);
            let revision = app.store.revision;
            let mut recovered = store::empty();
            recovered.name = "Obsolete recovery".into();
            let (tx, receiver) = mpsc::sync_channel(1);
            app.recovery.worker = Some(Worker {
                generation: app.recovery.generation,
                receiver,
            });
            tx.send(Ok(Outcome::Loaded {
                path: "/tmp/recovery-snapshot.ondera".into(),
                session: Box::new(recovered),
                library: Library::new(),
                revision,
            }))
            .unwrap();
            if new_document {
                app.recovery.new_document();
            }
            app.dispatch(ondera_engine::store::Command::Rename("Current work".into()));
            let ctx = egui::Context::default();
            install(&ctx);
            let _ = ctx.run(egui::RawInput::default(), |ctx| app.poll_recovery(ctx));
            assert_eq!(app.store.session().name, "Current work");
            assert!(app.path.is_none());
            if !new_document {
                assert!(app.recovery.error.is_some());
            }
        }
    }

    #[test]
    fn restored_copy_is_unsaved_without_a_fake_edit_and_requires_save_on_quit() {
        let mut app = Ondera::from_session(store::empty(), None);
        let mut recovered = store::empty();
        recovered.name = "Recovered song".into();
        let (tx, receiver) = mpsc::sync_channel(1);
        app.recovery.worker = Some(Worker {
            generation: app.recovery.generation,
            receiver,
        });
        tx.send(Ok(Outcome::Loaded {
            path: "/tmp/recovery-snapshot.ondera".into(),
            session: Box::new(recovered),
            library: Library::new(),
            revision: app.store.revision,
        }))
        .unwrap();
        let ctx = egui::Context::default();
        install(&ctx);
        let _ = ctx.run(egui::RawInput::default(), |ctx| app.poll_recovery(ctx));
        assert_eq!(app.store.session().name, "Recovered song");
        assert!(app.path.is_none());
        assert!(app.store.dirty());
        assert!(!app.store.can_undo());
        app.dispatch(ondera_engine::store::Command::Rename("Edit".into()));
        app.dispatch(ondera_engine::store::Command::Undo);
        assert!(
            app.store.dirty(),
            "Undo cannot claim a recovered copy was saved"
        );
        app.request(Intent::Quit);
        assert!(matches!(app.intent, Some(Intent::Quit)));
        assert!(!app.closing);
        app.store.mark_saved(app.store.revision);
        assert!(!app.store.dirty());
    }

    #[test]
    fn recovery_lists_only_generated_regular_files_in_its_directory() {
        let root = temp();
        let generated = generated_path(&root, 1234, 1, "Song");
        fs::write(&generated, b"not loaded by directory listing").unwrap();
        fs::write(root.join("my-original.ondera"), b"original").unwrap();
        assert_eq!(list(&root).unwrap().len(), 1);
        assert!(ensure_generated(&root, &generated).is_ok());
        assert!(ensure_generated(&root, &root.join("my-original.ondera")).is_err());
        let other = root.join("other");
        fs::create_dir(&other).unwrap();
        assert!(ensure_generated(&other, &generated).is_err());
        #[cfg(unix)]
        {
            let link = generated_path(&root, 1234, 2, "Linked");
            std::os::unix::fs::symlink(&generated, &link).unwrap();
            assert!(ensure_generated(&root, &link).is_err());
            assert_eq!(list(&root).unwrap().len(), 1);
        }
        fs::remove_dir_all(root).unwrap();
    }
}
