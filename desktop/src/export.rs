//! Native audio export and MIDI file settings. File choosers run on workers;
//! file operations use the same asynchronous registry path as CLI and MCP.

use crate::{app::Ondera, theme::*};
use eframe::egui::{self, vec2};
use ondera_engine::{model::Session, Result};
use serde_json::{json, Value};
use std::{collections::BTreeSet, path::PathBuf, sync::mpsc};

const SAMPLE_RATES: [u32; 3] = [44100, 48000, 96000];
const FORMATS: [(&str, &str); 3] = [
    ("pcm16", "16-bit PCM"),
    ("pcm24", "24-bit PCM"),
    ("float32", "32-bit float"),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Audio,
    MidiImport,
    MidiExport,
}

pub(crate) struct ExportDialog {
    open: bool,
    mode: Mode,
    sample_rate: u32,
    format: usize,
    dither: bool,
    range: bool,
    start_bar: f64,
    end_bar: f64,
    tail_seconds: f64,
    stems: bool,
    include_effects: bool,
    include_master: bool,
    tracks: BTreeSet<String>,
    folder_name: String,
    import_tempo: bool,
    chooser: Option<Chooser>,
    awaiting: Option<String>,
    report: Option<String>,
    error: Option<String>,
}

impl Default for ExportDialog {
    fn default() -> Self {
        Self {
            open: false,
            mode: Mode::Audio,
            sample_rate: 48000,
            format: 1,
            dither: true,
            range: false,
            start_bar: 1.0,
            end_bar: 5.0,
            tail_seconds: 3.0,
            stems: false,
            include_effects: true,
            include_master: false,
            tracks: BTreeSet::new(),
            folder_name: "Song stems".into(),
            import_tempo: false,
            chooser: None,
            awaiting: None,
            report: None,
            error: None,
        }
    }
}

struct Chooser {
    receiver: mpsc::Receiver<Option<PathBuf>>,
    request: PreparedCommand,
    path_key: &'static str,
}

struct PreparedCommand {
    method: &'static str,
    params: Value,
}

impl ExportDialog {
    pub(crate) fn is_open(&self) -> bool {
        self.open
    }

    fn show(&mut self, mode: Mode, session: &Session, position_beats: f64) {
        self.open = true;
        if self.busy() {
            return;
        }
        self.mode = mode;
        self.report = None;
        self.error = None;
        self.start_bar = if mode == Mode::MidiImport {
            position_beats / session.beats_per_bar() + 1.0
        } else {
            1.0
        };
        self.end_bar = session.end_bar().max(1.0) + 1.0;
        self.folder_name = format!("{} stems", session.name.trim_end_matches(".ondera"));
        self.tracks = session
            .tracks
            .iter()
            .filter(|track| mode != Mode::MidiExport || track.kind == "midi")
            .map(|track| track.id.clone())
            .collect();
    }

    pub(crate) fn busy(&self) -> bool {
        self.chooser.is_some() || self.awaiting.is_some()
    }
    pub(crate) fn close(&mut self) {
        self.open = false;
    }

    fn request(&self, session: &Session) -> Result<PreparedCommand> {
        let selected: Vec<&str> = session
            .tracks
            .iter()
            .filter(|track| self.tracks.contains(&track.id))
            .filter(|track| self.mode != Mode::MidiExport || track.kind == "midi")
            .map(|track| track.id.as_str())
            .collect();
        let request = match self.mode {
            Mode::MidiImport => {
                if !self.start_bar.is_finite() || self.start_bar < 1.0 {
                    return Err("The start bar must be at least 1.".into());
                }
                PreparedCommand {
                    method: "session.importMidi",
                    params: json!({"startBar": self.start_bar - 1.0, "importTempo": self.import_tempo}),
                }
            }
            Mode::MidiExport => {
                if selected.is_empty() {
                    return Err("Select at least one instrument track.".into());
                }
                PreparedCommand {
                    method: "session.exportMidi",
                    params: json!({"trackIds":selected}),
                }
            }
            Mode::Audio => {
                if !SAMPLE_RATES.contains(&self.sample_rate) || self.format >= FORMATS.len() {
                    return Err("Choose a supported sample rate and WAV encoding.".into());
                }
                if !self.tail_seconds.is_finite() || !(0.0..=120.0).contains(&self.tail_seconds) {
                    return Err("The effect tail must be between 0 and 120 seconds.".into());
                }
                if self.range
                    && (!self.start_bar.is_finite()
                        || !self.end_bar.is_finite()
                        || self.start_bar < 1.0
                        || self.end_bar <= self.start_bar)
                {
                    return Err("The end bar must follow the start bar.".into());
                }
                let mut params = json!({
                    "sampleRate":self.sample_rate,
                    "format":FORMATS[self.format].0,
                    "dither":self.dither,
                    "tailSeconds":self.tail_seconds,
                });
                if self.range {
                    params["startBar"] = json!(self.start_bar - 1.0);
                    params["endBar"] = json!(self.end_bar - 1.0);
                }
                if self.stems {
                    if selected.is_empty() {
                        return Err("Select at least one track for the stems.".into());
                    }
                    if !valid_folder_name(&self.folder_name) {
                        return Err(
                            "Use a new folder name without slashes or a drive prefix.".into()
                        );
                    }
                    params["trackIds"] = json!(selected);
                    params["includeEffects"] = json!(self.include_effects);
                    params["includeMaster"] = json!(self.include_master);
                }
                PreparedCommand {
                    method: if self.stems {
                        "session.exportStems"
                    } else {
                        "session.exportAudio"
                    },
                    params,
                }
            }
        };
        Ok(request)
    }

    fn choose(&mut self, session: &Session) {
        let request = match self.request(session) {
            Ok(request) => request,
            Err(error) => {
                self.error = Some(error);
                return;
            }
        };
        let mode = self.mode;
        let stems = mode == Mode::Audio && self.stems;
        let folder = self.folder_name.trim().to_string();
        let name = session.name.trim_end_matches(".ondera").to_string();
        let (tx, receiver) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let path = match mode {
                Mode::MidiImport => rfd::FileDialog::new()
                    .add_filter("MIDI file", &["mid", "midi"])
                    .pick_file(),
                Mode::MidiExport => rfd::FileDialog::new()
                    .add_filter("MIDI file", &["mid"])
                    .set_file_name(format!("{name}.mid"))
                    .save_file()
                    .map(|mut path| {
                        if path.extension().is_none() {
                            path.set_extension("mid");
                        }
                        path
                    }),
                Mode::Audio if stems => rfd::FileDialog::new()
                    .set_title("Choose parent folder for stems")
                    .pick_folder()
                    .map(|parent| parent.join(folder)),
                Mode::Audio => rfd::FileDialog::new()
                    .add_filter("WAV audio", &["wav"])
                    .add_filter("AIFF audio", &["aiff", "aif"])
                    .set_file_name(format!("{name}.wav"))
                    .save_file()
                    .map(|mut path| {
                        if path.extension().is_none() {
                            path.set_extension("wav");
                        }
                        path
                    }),
            };
            let _ = tx.send(path);
        });
        self.chooser = Some(Chooser {
            receiver,
            request,
            path_key: if stems { "directory" } else { "path" },
        });
        self.error = None;
        self.report = None;
    }

    fn poll_chooser(&mut self) -> Option<PreparedCommand> {
        let result = self
            .chooser
            .as_ref()
            .map(|chooser| chooser.receiver.try_recv())?;
        match result {
            Ok(path) => {
                let mut chooser = self.chooser.take().unwrap();
                let path = path?;
                chooser.request.params[chooser.path_key] = json!(path);
                Some(chooser.request)
            }
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.chooser = None;
                self.error = Some("The file chooser closed unexpectedly. Try again.".into());
                None
            }
        }
    }

    pub(crate) fn completed(&mut self, method: &str, result: &Result<Value>) {
        if self.awaiting.as_deref() != Some(method) {
            return;
        }
        self.awaiting = None;
        match result {
            Ok(value) => self.report = Some(report(method, value)),
            Err(error) => self.error = Some(error.clone()),
        }
    }

    fn track_choices(&mut self, ui: &mut egui::Ui, session: &Session) {
        ui.horizontal(|ui| {
            ui.label(caps("Tracks"));
            if text_button(ui, "All", Face::Raised).clicked() {
                self.tracks = session
                    .tracks
                    .iter()
                    .filter(|track| self.mode != Mode::MidiExport || track.kind == "midi")
                    .map(|track| track.id.clone())
                    .collect();
            }
            if text_button(ui, "None", Face::Raised).clicked() {
                self.tracks.clear();
            }
        });
        egui::ScrollArea::vertical()
            .id_salt("export-tracks")
            .max_height(FADER_H)
            .show(ui, |ui| {
                for track in session
                    .tracks
                    .iter()
                    .filter(|track| self.mode != Mode::MidiExport || track.kind == "midi")
                {
                    let mut selected = self.tracks.contains(&track.id);
                    if ui.checkbox(&mut selected, &track.name).changed() {
                        if selected {
                            self.tracks.insert(track.id.clone());
                        } else {
                            self.tracks.remove(&track.id);
                        }
                    }
                }
            });
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &Session, blocked: bool) -> bool {
        ui.spacing_mut().item_spacing = vec2(GAP, GAP);
        let mut choose = false;
        ui.add_enabled_ui(!self.busy() && !blocked, |ui| {
            match self.mode {
                Mode::Audio => {
                    ui.horizontal(|ui| {
                        if let Some(selected) = segmented(ui, &["Stereo mix", "Track stems"], usize::from(self.stems), BROWSER / 2.0) {
                            self.stems = selected == 1;
                        }
                    });
                    egui::Grid::new("export-format").spacing(vec2(GAP * 2.0, GAP)).show(ui, |ui| {
                        ui.label("Sample rate");
                        egui::ComboBox::from_id_salt("export-rate").selected_text(format!("{} kHz", self.sample_rate as f64 / 1000.0)).show_ui(ui, |ui| {
                            for rate in SAMPLE_RATES { ui.selectable_value(&mut self.sample_rate, rate, format!("{} kHz", rate as f64 / 1000.0)); }
                        });
                        ui.end_row();
                        ui.label("WAV encoding");
                        egui::ComboBox::from_id_salt("export-format").selected_text(FORMATS[self.format].1).show_ui(ui, |ui| {
                            for (index, (_, label)) in FORMATS.iter().enumerate() { ui.selectable_value(&mut self.format, index, *label); }
                        });
                        ui.end_row();
                        ui.label("Effect tail");
                        ui.add(egui::DragValue::new(&mut self.tail_seconds).range(0.0..=120.0).speed(0.1).suffix(" s"));
                        ui.end_row();
                    });
                    ui.add_enabled_ui(self.format != 2, |ui| {
                        ui.checkbox(&mut self.dither, "Dither integer PCM output");
                    });
                    ui.checkbox(&mut self.range, "Export a range");
                    if self.range {
                        ui.horizontal(|ui| {
                            ui.label("From bar");
                            ui.add(egui::DragValue::new(&mut self.start_bar).range(1.0..=100000.0).speed(0.25));
                            ui.label("Until bar");
                            ui.add(egui::DragValue::new(&mut self.end_bar).range(1.0..=100000.0).speed(0.25));
                        });
                        ui.label(text("The end marker is excluded. Bars 1 to 9 export eight bars, plus the effect tail.", FS_SECONDARY, Weight::Medium, DIM));
                        if text_button(ui, "Use cycle markers", Face::Raised).clicked() {
                            self.start_bar = session.transport.cycle_start_bar + 1.0;
                            self.end_bar = session.transport.cycle_end_bar + 1.0;
                        }
                    } else {
                        ui.label(text("Exports the full arrangement, followed by the effect tail.", FS_SECONDARY, Weight::Medium, DIM));
                    }
                    if self.stems {
                        ui.separator();
                        self.track_choices(ui, session);
                        ui.label(text("Selected tracks render individually, including muted tracks.", FS_SECONDARY, Weight::Medium, DIM));
                        ui.checkbox(&mut self.include_effects, "Include track effects and sends");
                        ui.checkbox(&mut self.include_master, "Apply master processing to each stem");
                        ui.label(text("Shared buses and nonlinear effects can make the stem sum differ from the full mix.", FS_SECONDARY, Weight::Medium, DIM));
                        ui.horizontal(|ui| { ui.label("New folder"); ui.text_edit_singleline(&mut self.folder_name); });
                        ui.label(text("Choose its parent folder next. Existing folders are preserved.", FS_SECONDARY, Weight::Medium, DIM));
                    }
                }
                Mode::MidiImport => {
                    ui.label(text("Import MIDI notes into new instrument tracks.", FS_BODY, Weight::Medium, INK));
                    ui.horizontal(|ui| {
                        ui.label("Start at bar");
                        ui.add(egui::DragValue::new(&mut self.start_bar).range(1.0..=100000.0).speed(0.25));
                    });
                    ui.checkbox(&mut self.import_tempo, "Use the file's initial tempo and time signature");
                    ui.label(text("Notes retain their musical timing. Later tempo/signature changes, controller data and program changes may require manual editing; the result lists ignored data.", FS_SECONDARY, Weight::Medium, DIM));
                }
                Mode::MidiExport => {
                    ui.label(text("Export instrument-track notes as a Standard MIDI File. Audio tracks and plugin sounds are not included.", FS_BODY, Weight::Medium, INK));
                    self.track_choices(ui, session);
                }
            }
            ui.separator();
            match self.request(session) {
                Ok(_) => {
                    choose = text_button(ui, match self.mode {
                        Mode::Audio if self.stems => "Choose folder and export…",
                        Mode::Audio => "Export WAV…",
                        Mode::MidiImport => "Choose MIDI file…",
                        Mode::MidiExport => "Export MIDI…",
                    }, Face::Raised).clicked();
                }
                Err(error) => { ui.label(text(error, FS_SECONDARY, Weight::Medium, INK_DIM)); }
            }
        });
        if self.chooser.is_some() {
            ui.label("Choose a location in the file dialog…");
        } else if self.awaiting.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Working…");
            });
        } else if blocked {
            ui.label("Wait for the current file operation or recording to finish.");
        }
        if let Some(error) = &self.error {
            ui.label(text(error, FS_BODY, Weight::Medium, INK_BRIGHT));
        }
        if let Some(report) = &self.report {
            ui.label(text(report, FS_BODY, Weight::Medium, INK));
        }
        choose
    }
}

impl Ondera {
    pub(crate) fn open_export_dialog(&mut self) {
        self.export
            .show(Mode::Audio, self.store.session(), self.position);
    }

    pub(crate) fn import_midi_dialog(&mut self) {
        self.export
            .show(Mode::MidiImport, self.store.session(), self.position);
    }

    pub(crate) fn export_midi_dialog(&mut self) {
        self.export
            .show(Mode::MidiExport, self.store.session(), self.position);
    }

    pub(crate) fn export_dialog(&mut self, ctx: &egui::Context) {
        if let Some(command) = self.export.poll_chooser() {
            self.export.awaiting = Some(command.method.into());
            let result =
                self.run_control_command(command.method, &command.params, false, "File menu");
            if !result
                .as_ref()
                .is_ok_and(|value| value["status"] == "running")
            {
                self.export.completed(command.method, &result);
            }
        }
        if !self.export.open {
            return;
        }
        let session = self.store.snapshot();
        let blocked = (self.job.is_some() && !self.preparing)
            || self.control_job.is_some()
            || self.midi_recording
            || self.recorder.is_some()
            || self.record_pending.is_some()
            || self.record_finishing.is_some();
        let mut open = true;
        let mut choose = false;
        egui::Window::new(match self.export.mode {
            Mode::Audio => "Export audio",
            Mode::MidiImport => "Import MIDI",
            Mode::MidiExport => "Export MIDI",
        })
        .id(egui::Id::new("media-export-dialog"))
        .open(&mut open)
        .default_width(BROWSER * 2.0)
        .resizable(true)
        .frame(window_frame())
        .show(ctx, |ui| {
            plate(ui, "export-plate", |ui| {
                choose = self.export.ui(ui, &session, blocked);
            });
        });
        self.export.open = open;
        if choose {
            self.export.choose(&session);
        }
        if self.export.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}

fn valid_folder_name(name: &str) -> bool {
    let name = name.trim();
    !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\', ':'])
}

fn report(method: &str, value: &Value) -> String {
    let mut lines = vec![if method == "session.importMidi" {
        "MIDI import complete".into()
    } else {
        "Export complete".into()
    }];
    for key in ["path", "directory"] {
        if let Some(path) = value[key].as_str() {
            lines.push(path.into());
        }
    }
    if let Some(duration) = value["seconds"].as_f64() {
        lines.push(format!("Duration: {duration:.2} seconds"));
    }
    if let Some(files) = value["files"].as_array() {
        lines.push(format!("{} stem files", files.len()));
        for file in files {
            if let Some(path) = file["path"].as_str() {
                lines.push(path.into());
            }
            if let Some(warnings) = file["warnings"].as_array() {
                lines.extend(warnings.iter().filter_map(Value::as_str).map(String::from));
            }
        }
    }
    if let Some(notes) = value["noteCount"]
        .as_u64()
        .or_else(|| value["notes"].as_u64())
    {
        lines.push(format!("{notes} MIDI notes"));
    }
    if let Some(count) = value["clippedSamples"].as_u64().filter(|count| *count > 0) {
        lines.push(format!("{count} samples exceeded the output range. Lower the mix level if this clipping is unwanted."));
    }
    if let Some(warnings) = value["warnings"].as_array() {
        lines.extend(warnings.iter().filter_map(Value::as_str).map(String::from));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ondera_engine::store;

    #[test]
    fn musical_bar_range_maps_to_shared_export_parameters() {
        let dialog = ExportDialog {
            range: true,
            start_bar: 1.0,
            end_bar: 9.0,
            sample_rate: 96000,
            format: 2,
            ..Default::default()
        };
        let request = dialog.request(&store::demo()).unwrap();
        assert_eq!(request.method, "session.exportAudio");
        assert_eq!(request.params["startBar"], 0.0);
        assert_eq!(request.params["endBar"], 8.0);
        assert_eq!(request.params["sampleRate"], 96000);
        assert_eq!(request.params["format"], "float32");
    }

    #[test]
    fn stems_require_selected_tracks_and_a_new_folder_name() {
        let session = store::demo();
        let mut dialog = ExportDialog {
            stems: true,
            ..Default::default()
        };
        assert!(dialog.request(&session).is_err());
        dialog.tracks.insert(session.tracks[0].id.clone());
        let request = dialog.request(&session).unwrap();
        assert_eq!(request.method, "session.exportStems");
        assert_eq!(request.params["trackIds"], json!([session.tracks[0].id]));
        assert_eq!(request.params["includeMaster"], false);
        dialog.folder_name = "../existing".into();
        assert!(dialog.request(&session).is_err());
    }

    #[test]
    fn midi_export_only_selects_midi_tracks_and_import_defaults_to_current_bar() {
        let session = store::demo();
        let mut dialog = ExportDialog::default();
        dialog.show(Mode::MidiExport, &session, 0.0);
        let request = dialog.request(&session).unwrap();
        assert_eq!(
            request.params["trackIds"].as_array().unwrap().len(),
            session
                .tracks
                .iter()
                .filter(|track| track.kind == "midi")
                .count()
        );
        dialog.show(Mode::MidiImport, &session, session.beats_per_bar() * 4.0);
        let request = dialog.request(&session).unwrap();
        assert_eq!(request.params["startBar"], 4.0);
        assert_eq!(request.params["importTempo"], false);
    }

    #[test]
    fn invalid_ranges_and_tails_are_rejected_before_choosing_a_file() {
        let session = store::demo();
        let mut dialog = ExportDialog {
            range: true,
            start_bar: 9.0,
            end_bar: 9.0,
            ..Default::default()
        };
        assert!(dialog.request(&session).is_err());
        dialog.range = false;
        dialog.tail_seconds = f64::NAN;
        assert!(dialog.request(&session).is_err());
    }

    #[test]
    fn completion_displays_the_matching_operation_and_warnings() {
        let mut dialog = ExportDialog {
            awaiting: Some("session.exportAudio".into()),
            ..Default::default()
        };
        dialog.completed("session.info", &Ok(json!({})));
        assert!(dialog.awaiting.is_some());
        dialog.completed(
            "session.exportAudio",
            &Ok(json!({"path":"mix.wav","clippedSamples":8,"warnings":["Tail was truncated"]})),
        );
        assert!(dialog.awaiting.is_none());
        let result = dialog.report.unwrap();
        assert!(
            result.contains("mix.wav")
                && result.contains("8 samples")
                && result.contains("Tail was truncated")
        );
    }
}
