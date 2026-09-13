//! Tauri owns the window; the existing Rust host remains the single document/audio owner.
//! Plugin editors must stay on the OS main thread. Requests are marshalled there, never
//! through the audio callback. Only document revisions and small telemetry packets reach JS.

use crate::{
    app::{Intent, Ondera},
    control::{LiveWait, Reply},
};
use eframe::egui;
use ondera_engine::{control, model::ClipData, store::Command, Result};
use serde_json::{json, Value};
use std::{
    cell::RefCell,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager};

thread_local! { static HOST: RefCell<Option<WebHost>> = const { RefCell::new(None) }; }

#[tauri::command]
async fn daw_pick(kind: String, name: Option<String>) -> Result<Option<String>> {
    tauri::async_runtime::spawn_blocking(move || {
        let dialog = rfd::FileDialog::new().set_file_name(name.unwrap_or_default());
        let path = match kind.as_str() {
            "audio" => dialog
                .add_filter("Audio", &["wav", "aiff", "flac", "mp3", "m4a", "ogg"])
                .pick_file(),
            "midi" => dialog.add_filter("MIDI", &["mid", "midi"]).pick_file(),
            "wav" => dialog.add_filter("WAV", &["wav"]).save_file(),
            "saveMidi" => dialog.add_filter("MIDI", &["mid"]).save_file(),
            "folder" => dialog.pick_folder(),
            _ => return Err("Unknown file chooser".into()),
        };
        Ok(path.map(|path| path.to_string_lossy().into_owned()))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// WebKit captures composited canvases and CSS, which SVG foreignObject snapshots omit.
#[tauri::command]
async fn daw_snapshot(window: tauri::WebviewWindow) -> Result<String> {
    #[cfg(target_os = "macos")]
    {
        use base64::Engine;
        use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSImage};
        use objc2_foundation::{NSDictionary, NSError};
        let (tx, rx) = tokio::sync::oneshot::channel();
        window
            .with_webview(move |webview| {
                let tx = RefCell::new(Some(tx));
                let completion =
                    block2::RcBlock::new(move |image: *mut NSImage, error: *mut NSError| {
                        let result = (|| {
                            // WebKit provides these borrowed objects for the lifetime of this callback.
                            let image = unsafe { image.as_ref() }.ok_or_else(|| {
                                unsafe { error.as_ref() }.map_or_else(
                                    || "Window capture failed".to_string(),
                                    |e| e.localizedDescription().to_string(),
                                )
                            })?;
                            let tiff = image.TIFFRepresentation().ok_or("No snapshot pixels")?;
                            let bitmap = NSBitmapImageRep::imageRepWithData(&tiff)
                                .ok_or("Invalid snapshot pixels")?;
                            // Empty properties need no dynamically typed values.
                            let png = unsafe {
                                bitmap.representationUsingType_properties(
                                    NSBitmapImageFileType::PNG,
                                    &NSDictionary::new(),
                                )
                            }
                            .ok_or("Cannot encode snapshot")?;
                            Ok(base64::engine::general_purpose::STANDARD.encode(png.to_vec()))
                        })();
                        if let Some(tx) = tx.borrow_mut().take() {
                            let _ = tx.send(result);
                        }
                    });
                // Tauri exposes the live WKWebView and executes this closure on its main thread.
                unsafe {
                    (&*webview.inner().cast::<objc2_web_kit::WKWebView>())
                        .takeSnapshotWithConfiguration_completionHandler(None, &completion);
                }
            })
            .map_err(|e| e.to_string())?;
        rx.await.map_err(|e| e.to_string())?
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
        Err("Use the frontend capture on this platform".into())
    }
}

#[tauri::command]
async fn daw_signin(provider: String, status: bool) -> Result<String> {
    tauri::async_runtime::spawn_blocking(move || {
        let settings = ondera_engine::settings::Settings::load();
        let (executable, args) = match provider.as_str() {
            "codex" => (
                crate::agent::discover_codex(&settings.agent.codex_executable),
                if status {
                    vec!["login", "status"]
                } else {
                    vec!["login"]
                },
            ),
            "claude" => (
                crate::agent::discover_claude(&settings.agent.claude_executable),
                vec!["auth", if status { "status" } else { "login" }],
            ),
            _ => return Err("Sign-in is available for Codex and Claude Code".into()),
        };
        crate::settings::run_cli(
            &executable,
            &args.into_iter().map(String::from).collect::<Vec<_>>(),
            Duration::from_secs(240),
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

struct WebHost {
    app: Ondera,
    context: egui::Context,
    last_document: Option<(u64, Value)>,
    snapshot_sequence: std::cell::Cell<u64>,
    last_ui: Value,
    last_agent: Value,
    last_telemetry: Value,
    last_metadata: Instant,
    ready: bool,
    startup_capture: bool,
    last_library: std::collections::BTreeMap<String, usize>,
}

#[tauri::command]
async fn daw_command(handle: tauri::AppHandle, method: String, params: Value) -> Result<Value> {
    let (tx, rx) = mpsc::sync_channel(1);
    handle
        .run_on_main_thread(move || {
            HOST.with_borrow_mut(|slot| {
                let Some(host) = slot else {
                    let _ = tx.send(Err("Ondera is starting".into()));
                    return;
                };
                let result = host.command(&method, &params);
                if result.as_ref().is_ok_and(|v| v["status"] == "running") {
                    if let Err(reply) = host.app.attach_reply(Reply::Channel(tx)) {
                        reply.respond(result);
                    }
                } else {
                    let _ = tx.send(result);
                }
            });
        })
        .map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        rx.recv_timeout(Duration::from_secs(610))
            .map_err(|_| "The operation did not finish; check Ondera before retrying".to_string())?
    })
    .await
    .map_err(|e| e.to_string())?
}

impl WebHost {
    fn request_exit(&mut self) -> bool {
        if self.app.closing {
            return true;
        }
        self.app.request(Intent::Quit);
        false
    }

    fn document(&self) -> Result<Value> {
        // Plugin blobs and legacy presentation extras are not needed by the web renderer.
        self.snapshot_sequence
            .set(self.snapshot_sequence.get().saturating_add(1));
        let session = self.app.store.session();
        let insert = |i: &ondera_engine::model::Insert| {
            json!({
                "id":i.id,"name":i.name,"state":i.state,"plugin":i.plugin,"params":i.params,
                "meta":if i.state == "empty" { "" } else { "Ondera" }
            })
        };
        let strips: serde_json::Map<String,Value> = session.strips.iter().map(|(id,s)| {
            (id.clone(),json!({"instrument":s.instrument,"sends":s.sends,
                "synth":s.synth.as_ref().map(insert),"inserts":s.inserts.iter().map(insert).collect::<Vec<_>>()}))
        }).collect();
        Ok(
            json!({"snapshotSequence":self.snapshot_sequence.get(),"name":session.name,"tracks":session.tracks,"clips":session.clips,
            "sources":session.sources,"strips":strips,"transport":session.transport,
            "view":session.view,"masterVolume":session.master_volume,"automation":session.automation}),
        )
    }
    fn ui(&mut self) -> Result<Value> {
        let mut value = control::call(&mut self.app, "ui.status", &json!({}), false)?;
        value["dirty"] = json!(self.app.store.dirty());
        value["path"] = json!(self.app.path);
        value["history"] = control::call(&mut self.app, "history.info", &json!({}), false)?;
        value["prompt"] = json!(self.app.intent.map(|intent| match intent {
            Intent::New => "new",
            Intent::Open => "open",
            Intent::Recover => "recover",
            Intent::Demo => "demo",
            Intent::Quit => "quit",
            Intent::Relaunch => "relaunch",
        }));
        value["export"] = json!(self.app.export.is_open());
        value["recovery"] = json!(self.app.recovery.is_open());
        value["recoveryStatus"] = json!(self.app.recovery.status());
        value["scale"] = json!(self.app.settings.interface.scale);
        value["update"] = json!({"available":self.app.updates.available.as_ref().map(|r| &r.version),
            "installed":self.app.updates.installed.is_some(),"busy":self.app.updates.busy()});
        Ok(value)
    }
    fn command(&mut self, method: &str, params: &Value) -> Result<Value> {
        match method {
            "web.ready" => {
                self.ready = true;
                Ok(json!({"session":self.document()?,"ui":self.ui()?,
                    "catalog":control::call(&mut self.app,"session.catalog",&json!({}),false)?,
                    "platform":std::env::consts::OS,"version":env!("CARGO_PKG_VERSION")}))
            }
            "web.rendered" => {
                self.app.frontend_ready = true;
                Ok(Value::Null)
            }
            "web.document" => self.document(),
            "web.dismissError" => {
                self.app.error = None;
                Ok(Value::Null)
            }
            "web.file" => {
                match params["action"].as_str().unwrap_or("") {
                    "new" => self.app.request(Intent::New),
                    "demo" => self.app.request(Intent::Demo),
                    "open" => self.app.request(Intent::Open),
                    "save" => self.app.save(params["saveAs"].as_bool().unwrap_or(false)),
                    "import" => self.app.import(None),
                    "export" => self.app.open_export_dialog(),
                    "relaunch" => self.app.request(Intent::Relaunch),
                    "recoverTake" => self.app.save_recovered_take(),
                    "importMidi" => self.app.import_midi_dialog(),
                    "exportMidi" => self.app.export_midi_dialog(),
                    "quit" => self.app.request(Intent::Quit),
                    _ => return Err("Unknown file action".into()),
                }
                Ok(Value::Null)
            }
            "web.confirm" => {
                match params["choice"].as_str().unwrap_or("") {
                    "cancel" => self.app.intent = None,
                    "discard" => {
                        if let Some(intent) = self.app.intent.take() {
                            self.app.execute(intent);
                        }
                    }
                    "save" => {
                        if let Some(intent) = self.app.intent.take() {
                            self.app.after_save = Some(intent);
                            self.app.save(false);
                        }
                    }
                    _ => return Err("Unknown confirmation choice".into()),
                }
                Ok(Value::Null)
            }
            "web.peaks" => {
                let id = params["sourceId"].as_str().ok_or("Missing sourceId")?;
                let buffer = self.app.library.get(id).ok_or("Audio is still loading")?;
                Ok(
                    json!({"peaks":buffer.peaks,"rate":buffer.sample_rate as f64 / (buffer.sample_rate / 400).max(1) as f64}),
                )
            }
            "web.liveNote" => {
                let pitch = params["pitch"]
                    .as_u64()
                    .filter(|v| *v <= 127)
                    .ok_or("Pitch must be 0-127")?;
                let on = params["on"].as_bool().ok_or("Missing on/off")?;
                let pitch = pitch as u8;
                if on && self.app.musical_typing && !self.app.typing_down.contains(&pitch) {
                    self.app.typing_down.push(pitch);
                    self.app.live_note(true, pitch, 100);
                } else if !on {
                    self.app.typing_down.retain(|p| *p != pitch);
                    self.app.live_note(false, pitch, 0);
                }
                Ok(Value::Null)
            }
            "web.releaseKeys" => {
                self.app.release_typing();
                Ok(Value::Null)
            }
            "web.gesture" => {
                self.app
                    .store
                    .set_gesture(params["active"].as_bool().unwrap_or(false));
                Ok(Value::Null)
            }
            "web.recordArm" => {
                let enabled = params["enabled"].as_bool().ok_or("Missing enabled")?;
                if self.app.record_enabled != enabled {
                    self.app.record_enabled = enabled;
                    if self.app.playing {
                        if enabled {
                            self.app.start_recording();
                        } else {
                            self.app.finish_recording();
                        }
                    }
                }
                Ok(Value::Null)
            }
            "web.quantize" => {
                self.app.quantize_selected();
                Ok(Value::Null)
            }
            "web.transpose" => {
                let n = params["semitones"]
                    .as_i64()
                    .filter(|n| (-127..=127).contains(n))
                    .ok_or("Invalid transposition")?;
                self.app.transpose_selected(n as i32);
                Ok(Value::Null)
            }
            "web.typing" => {
                self.app.toggle_musical_typing();
                Ok(Value::Null)
            }
            "web.reconnect" => {
                self.app.connect();
                Ok(Value::Null)
            }
            "web.sdk" => {
                crate::settings::reveal(std::path::Path::new(
                    "https://github.com/ludovic111/ondera/blob/main/docs/NATIVE_PLUGINS.md",
                ));
                Ok(Value::Null)
            }
            "web.clearSelection" => {
                let track = self.app.store.session().view.selected_track_id.clone();
                self.app.try_dispatch(Command::Select {
                    track,
                    clip: None,
                    note: None,
                })?;
                Ok(Value::Null)
            }
            "web.trimClip" => self.trim_clip(params),
            "web.agentChanges" => Ok(self.app.agents.changes_json(self.app.store.undo_depth())),
            "web.agentRevert" => {
                let sequence = params["sequence"]
                    .as_u64()
                    .ok_or("Missing change sequence")?;
                if params["redo"].as_bool().unwrap_or(false) {
                    self.app.redo_activity(sequence);
                } else {
                    self.app.revert_activity(sequence);
                }
                Ok(Value::Null)
            }
            "web.closePlugin" => {
                self.app
                    .close_plugin_window(params["id"].as_str().ok_or("Missing plugin id")?);
                Ok(Value::Null)
            }
            "web.capture" => {
                use base64::Engine;
                let encoded = params["png"].as_str().ok_or("Missing PNG")?;
                if encoded.len() > 64 * 1024 * 1024 {
                    return Err("Capture is too large".into());
                }
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(encoded)
                    .map_err(|e| e.to_string())?;
                let decoded = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
                    .map_err(|e| e.to_string())?
                    .to_rgba8();
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [decoded.width() as usize, decoded.height() as usize],
                    &decoded,
                );
                if !self.app.deliver_screenshot(&image) {
                    if let Some(path) = self.app.screenshot.take() {
                        decoded.save(path).map_err(|e| e.to_string())?;
                        self.app.closing = true;
                    } else {
                        return Err("No screenshot was requested".into());
                    }
                }
                Ok(Value::Null)
            }
            "web.captureError" => {
                self.app.finish_live(
                    |wait| matches!(wait, LiveWait::Screenshot { .. }),
                    Err("Window capture failed".into()),
                );
                Ok(Value::Null)
            }
            _ => self
                .app
                .run_control_command(method, params, false, "Interface"),
        }
    }
    fn trim_clip(&mut self, params: &Value) -> Result<Value> {
        let id = params["clipId"].as_str().ok_or("Missing clipId")?;
        let mut clip = self
            .app
            .store
            .session()
            .clips
            .iter()
            .find(|c| c.id == id)
            .cloned()
            .ok_or("Unknown clip")?;
        let start = params["startBar"]
            .as_f64()
            .filter(|n| n.is_finite() && *n >= 0.)
            .ok_or("Invalid start")?;
        let length = params["lengthBars"]
            .as_f64()
            .filter(|n| n.is_finite() && *n > 0.)
            .ok_or("Invalid length")?;
        let delta = start - clip.start_bar;
        let bpb = self.app.store.session().beats_per_bar();
        match &mut clip.data {
            ClipData::Midi { notes } => {
                for note in notes.iter_mut() {
                    let end = note.start + note.length - delta * bpb;
                    note.start = (note.start - delta * bpb).max(0.);
                    note.length = (end.min(length * bpb) - note.start).max(0.);
                }
                notes.retain(|n| n.length > 0.);
            }
            ClipData::Audio { offset_seconds, .. } => {
                let offset =
                    *offset_seconds + delta * bpb * 60. / self.app.store.session().transport.tempo;
                if offset < 0. {
                    return Err("Cannot trim before the audio source".into());
                }
                *offset_seconds = offset;
            }
        }
        clip.start_bar = start;
        clip.length_bars = length;
        self.app.try_dispatch(Command::PutClip(clip))?;
        Ok(Value::Null)
    }
    fn tick(&mut self, handle: &tauri::AppHandle) {
        self.app.frames += 1;
        // No egui windows or painting are run. Context only supplies the existing worker wakeups.
        let _ = self.context.run(egui::RawInput::default(), |ctx| {
            self.app.poll();
            self.app.poll_control_job();
            self.app.poll_recovery(ctx);
            self.app.poll_agent(ctx);
            self.app.serve_control(false);
            self.app.poll_updates();
        });
        if self.app.closing {
            self.app.shutdown_audio();
            handle.exit(0);
            return;
        }
        if !self.ready {
            return;
        }
        let library = self
            .app
            .library
            .iter()
            .map(|(id, buffer)| (id.clone(), Arc::as_ptr(buffer) as usize))
            .collect();
        if self.last_library != library {
            self.last_library = library;
            let _ = handle.emit(
                "daw:audioSources",
                self.last_library.keys().collect::<Vec<_>>(),
            );
        }
        let peaks = self
            .app
            .device
            .as_ref()
            .map_or([0.; 4], |d| d.telemetry.peaks());
        let telemetry = json!({"position":self.app.position,"playing":self.app.playing,
            "recording":self.app.record_enabled,"peaks":peaks,
            "cpu":(self.app.device.as_ref().map_or(0.,|d|d.telemetry.load())*1000.).round()/1000.});
        if telemetry != self.last_telemetry {
            let _ = handle.emit("daw:telemetry", &telemetry);
            self.last_telemetry = telemetry;
        }
        let view = json!([
            self.app.store.session().view,
            self.app.zoom,
            self.app.scroll
        ]);
        let key = (self.app.store.revision, view);
        if self.last_document.as_ref() != Some(&key) {
            if let Ok(document) = self.document() {
                let _ = handle.emit("daw:document", document);
            }
            self.last_document = Some(key);
        }
        if self.last_metadata.elapsed() >= Duration::from_millis(100) {
            self.last_metadata = Instant::now();
            if let Ok(ui) = self.ui() {
                if ui != self.last_ui {
                    let _ = handle.emit("daw:ui", &ui);
                    if let Some(window) = handle.get_webview_window("main") {
                        let _ = window.set_title(&format!(
                            "{}{} — Ondera",
                            self.app.store.session().name,
                            if self.app.store.dirty() { " *" } else { "" }
                        ));
                    }
                    self.last_ui = ui;
                }
            }
            let agent = json!({"status":self.app.agents.status_json(&self.app.settings),
                "transcript":self.app.agents.transcript_json(100),"changes":self.app.agents.changes_json(self.app.store.undo_depth())});
            if agent != self.last_agent {
                let _ = handle.emit("daw:agent", &agent);
                self.last_agent = agent;
            }
        }
        let mut capture = false;
        for job in &mut self.app.live_jobs {
            if let LiveWait::Screenshot { requested, .. } = &mut job.wait {
                if !*requested {
                    *requested = true;
                    capture = true;
                }
            }
        }
        self.app.poll_live_jobs(&self.context);
        if capture
            || (self.app.screenshot.is_some() && !self.startup_capture && self.app.frames >= 90)
        {
            self.startup_capture = true;
            let _ = handle.emit("daw:capture", ());
        }
    }
}

pub fn run(
    path: Option<PathBuf>,
    screenshot: Option<PathBuf>,
    control: bool,
    updates: bool,
    agents: bool,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let running = Arc::new(AtomicBool::new(true));
    let running_setup = running.clone();
    let app = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            daw_command,
            daw_pick,
            daw_snapshot,
            daw_signin
        ])
        .setup(move |application| {
            let context = egui::Context::default();
            let mut app = Ondera::new(&context, path, screenshot, control, updates);
            app.agents.open |= agents;
            HOST.with_borrow_mut(|slot| {
                *slot = Some(WebHost {
                    app,
                    context,
                    last_document: None,
                    snapshot_sequence: Default::default(),
                    last_ui: Value::Null,
                    last_agent: Value::Null,
                    last_telemetry: Value::Null,
                    last_metadata: Instant::now(),
                    ready: false,
                    startup_capture: false,
                    last_library: Default::default(),
                })
            });
            let handle = application.handle().clone();
            // At most one pending tick. A stalled web renderer cannot build an unbounded queue.
            std::thread::spawn(move || {
                let pending = Arc::new(AtomicBool::new(false));
                while running_setup.load(Ordering::Relaxed) {
                    if !pending.swap(true, Ordering::AcqRel) {
                        let pending = pending.clone();
                        let app_handle = handle.clone();
                        if handle
                            .run_on_main_thread(move || {
                                HOST.with_borrow_mut(|slot| {
                                    if let Some(host) = slot {
                                        host.tick(&app_handle)
                                    }
                                });
                                pending.store(false, Ordering::Release);
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                    std::thread::sleep(Duration::from_millis(33));
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                HOST.with_borrow_mut(|slot| {
                    if let Some(host) = slot {
                        host.app.request(Intent::Quit)
                    }
                });
            }
            tauri::WindowEvent::Focused(false) => HOST.with_borrow_mut(|slot| {
                if let Some(host) = slot {
                    host.app.release_typing()
                }
            }),
            tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) => {
                HOST.with_borrow_mut(|slot| {
                    if let Some(host) = slot {
                        host.app.import(Some(paths.clone()))
                    }
                });
            }
            _ => {
                let _ = window;
            }
        })
        .build(tauri::generate_context!())?;
    app.run(move |_, event| match event {
        tauri::RunEvent::ExitRequested { api, .. } => {
            HOST.with_borrow_mut(|slot| {
                if let Some(host) = slot {
                    if !host.request_exit() {
                        api.prevent_exit();
                    }
                }
            });
        }
        tauri::RunEvent::Exit => {
            running.store(false, Ordering::Relaxed);
            HOST.with_borrow_mut(|slot| {
                if let Some(mut host) = slot.take() {
                    host.app.shutdown_audio();
                }
            });
        }
        _ => {}
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ondera_engine::{
        model::{Clip, Note},
        store,
    };

    fn host() -> WebHost {
        WebHost {
            app: Ondera::from_session(store::empty(), None),
            context: egui::Context::default(),
            last_document: None,
            snapshot_sequence: Default::default(),
            last_ui: Value::Null,
            last_agent: Value::Null,
            last_telemetry: Value::Null,
            last_metadata: Instant::now(),
            ready: false,
            startup_capture: false,
            last_library: Default::default(),
        }
    }

    #[test]
    fn native_quit_waits_for_the_same_unsaved_changes_confirmation() {
        let mut host = host();
        let track = host.app.store.session().tracks[0].id.clone();
        host.command("track.setVolume", &json!({"trackId":track,"volume":0.2}))
            .unwrap();
        assert!(!host.request_exit());
        assert!(host.app.intent.is_some());
        assert!(!host.app.closing);
        host.command("web.confirm", &json!({"choice":"cancel"}))
            .unwrap();
        assert!(host.app.intent.is_none());
        assert!(!host.request_exit());
        host.command("web.confirm", &json!({"choice":"discard"}))
            .unwrap();
        assert!(host.request_exit());
    }

    #[test]
    fn web_trim_rebases_overlapping_notes_and_undo_restores_the_region() {
        let mut host = host();
        let track = host.command("track.add", &json!({"kind":"midi"})).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();
        let clip = Clip {
            id: "region".into(),
            track_id: track,
            name: "Test".into(),
            start_bar: 0.,
            length_bars: 4.,
            agent: false,
            data: ClipData::Midi {
                notes: vec![
                    Note {
                        id: "overlap".into(),
                        start: 3.,
                        length: 3.,
                        pitch: 60,
                        velocity: 100,
                        agent: false,
                    },
                    Note {
                        id: "outside".into(),
                        start: 12.,
                        length: 1.,
                        pitch: 64,
                        velocity: 100,
                        agent: false,
                    },
                ],
            },
        };
        host.app
            .try_dispatch(Command::PutClip(clip.clone()))
            .unwrap();
        host.command(
            "web.trimClip",
            &json!({"clipId":"region","startBar":1.,"lengthBars":1.}),
        )
        .unwrap();
        let trimmed = &host.app.store.session().clips[0];
        let ClipData::Midi { notes } = &trimmed.data else {
            panic!()
        };
        assert_eq!(notes.len(), 1);
        assert_eq!((notes[0].start, notes[0].length), (0., 2.));
        host.command("history.undo", &json!({})).unwrap();
        assert_eq!(
            serde_json::to_value(&host.app.store.session().clips[0]).unwrap(),
            serde_json::to_value(clip).unwrap()
        );
    }

    #[test]
    fn web_document_never_serializes_plugin_blobs_or_session_extras() {
        let mut host = host();
        let track = host.app.store.session().tracks[0].id.clone();
        let mut strip = host
            .app
            .store
            .session()
            .strips
            .get(&track)
            .cloned()
            .unwrap_or_default();
        let mut insert = ondera_engine::model::Insert::new("effect".into(), "stock:Space", "Space");
        insert.blob = "opaque-plugin-state".repeat(1024);
        strip.inserts.push(insert.clone());
        strip.synth = Some(insert);
        host.app
            .try_dispatch(Command::SetStrip {
                track: track.clone(),
                strip,
            })
            .unwrap();
        let snapshot = host.document().unwrap();
        assert!(snapshot["strips"][&track]["inserts"][0]
            .get("blob")
            .is_none());
        assert!(snapshot["strips"][&track]["synth"].get("blob").is_none());
        assert!(!snapshot.to_string().contains("opaque-plugin-state"));
    }

    #[test]
    fn typing_release_drains_every_held_note_and_rejects_invalid_pitches() {
        let mut host = host();
        host.command("web.typing", &json!({})).unwrap();
        host.command("web.liveNote", &json!({"pitch":60,"on":true}))
            .unwrap();
        host.command("web.liveNote", &json!({"pitch":64,"on":true}))
            .unwrap();
        assert_eq!(host.app.typing_down, vec![60, 64]);
        host.command("web.releaseKeys", &json!({})).unwrap();
        assert!(host.app.typing_down.is_empty());
        assert!(host
            .command("web.liveNote", &json!({"pitch":128,"on":true}))
            .is_err());
    }

    #[test]
    fn a_fader_gesture_is_one_undo_step() {
        let mut host = host();
        let track = host.app.store.session().tracks[0].id.clone();
        let original = host.app.store.session().tracks[0].volume;
        let depth = host.app.store.undo_depth();
        host.command("web.gesture", &json!({"active":true}))
            .unwrap();
        for volume in [0.2, 0.4, 0.6] {
            host.command("track.setVolume", &json!({"trackId":track,"volume":volume}))
                .unwrap();
        }
        host.command("web.gesture", &json!({"active":false}))
            .unwrap();
        assert_eq!(host.app.store.undo_depth(), depth + 1);
        host.command("history.undo", &json!({})).unwrap();
        assert_eq!(host.app.store.session().tracks[0].volume, original);
    }
}
