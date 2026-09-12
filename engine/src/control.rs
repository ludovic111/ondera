//! Public command registry shared by every client of the store.
//!
//! The desktop window, `ondera-cli` and `ondera-mcp` are peers: each named command here becomes
//! a validated [`store::Command`] or a host action (open, save, bounce, transport). Nothing is
//! privileged and nothing bypasses the store, so undo, validation and dirty tracking behave the
//! same whether a person or an agent made the edit. The registry is introspectable: the CLI
//! help and the MCP tool list are generated from [`COMMANDS`], never hand-written.
//!
//! Bars and beats are zero-based floats, matching the `.ondera` file. Note times are beats
//! relative to their clip.

pub mod wire;

use crate::{
    audio::{self, AudioBuffer, Library},
    document,
    dsp::{EFFECTS, INSTRUMENTS},
    host as plugin_host,
    model::*,
    render,
    store::{self, Command, Store},
    Result,
};
use serde_json::{json, Map, Value};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

/// Default track colours, mirroring `TRACKS` in `desktop/src/theme.rs`. They are session data
/// (the `.ondera` file stores them), not paint tokens.
pub const TRACK_PALETTE: [&str; 8] = [
    "#ed835e", "#b191ea", "#6ab3fd", "#d991d2", "#e0af3b", "#95bd69", "#eb8182", "#ee9748",
];
const SEND_NAMES: [&str; 2] = ["A · Reverb", "B · Delay"];
const LOOPS: &str = include_str!("../tests/fixtures/loops.json");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    String,
    Number,
    Integer,
    Boolean,
    Array,
    Object,
}
impl Kind {
    pub fn schema_type(self) -> &'static str {
        match self {
            Kind::String => "string",
            Kind::Number => "number",
            Kind::Integer => "integer",
            Kind::Boolean => "boolean",
            Kind::Array => "array",
            Kind::Object => "object",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Param {
    pub name: &'static str,
    pub kind: Kind,
    pub required: bool,
    pub doc: &'static str,
}
pub(crate) const fn req(name: &'static str, kind: Kind, doc: &'static str) -> Param {
    Param {
        name,
        kind,
        required: true,
        doc,
    }
}
pub(crate) const fn opt(name: &'static str, kind: Kind, doc: &'static str) -> Param {
    Param {
        name,
        kind,
        required: false,
        doc,
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Spec {
    pub name: &'static str,
    pub doc: &'static str,
    pub params: &'static [Param],
    /// False for queries; true when the command can change the document, transport or files.
    pub mutates: bool,
}
pub(crate) const fn query(name: &'static str, doc: &'static str, params: &'static [Param]) -> Spec {
    Spec {
        name,
        doc,
        params,
        mutates: false,
    }
}
pub(crate) const fn edit(name: &'static str, doc: &'static str, params: &'static [Param]) -> Spec {
    Spec {
        name,
        doc,
        params,
        mutates: true,
    }
}

const TRACK_ID: Param = req(
    "trackId",
    Kind::String,
    "Track id, as listed by track.list.",
);
const CLIP_ID: Param = req("clipId", Kind::String, "Clip id, as listed by clip.list.");
const NOTES_DOC: &str = "Array of {start, length, pitch, velocity?} with start/length in beats relative to the clip, pitch 0-127 (60 = C4), velocity 1-127 (default 100).";

/// Every public command. Names use `family.action` and map one-to-one to CLI commands and MCP
/// tools (`family_action`).
pub const BASE_COMMANDS: &[Spec] = &[
    query("session.info", "Summarise the open session: name, file, transport, counts, selection and history state.", &[]),
    query("session.get", "Return the complete session document as JSON (tracks, clips with notes, sources, strips, transport, view).", &[]),
    query("session.inspect", "Inspect the arrangement, mixer and automation without opaque plugin state. Clips are summaries by default; use clip.get for individual notes.", &[opt("includeNotes",Kind::Boolean,"Include all MIDI notes instead of clip summaries (default false).")]),
    query("session.catalog", "List built-in instruments, effects and bundled MIDI loops.", &[]),
    query("plugin.list", "Search a page of installed plugins from the scanner cache. Use query/kind/format to avoid returning a large library; follow nextOffset for more.", &[
        opt("query",Kind::String,"Case-insensitive name, vendor or plugin ID search."),
        opt("format",Kind::String,"stock, clap, vst3 or au."),
        opt("kind",Kind::String,"instrument or effect."),
        opt("offset",Kind::Integer,"Zero-based result offset, default 0."),
        opt("limit",Kind::Integer,"Page size 1-200, default 50."),
    ]),
    edit("plugin.scan", "Scan installed plugin directories in isolated child processes and refresh the plugin cache. May take several minutes.", &[]),
    query("session.commands", "Describe every command with its parameters.", &[]),
    edit("session.new", "Replace the open session with an empty one (or the bundled Nightfall demo). Unsaved changes are discarded.", &[
        opt("demo", Kind::Boolean, "Load the Nightfall demo instead of an empty session."),
    ]),
    edit("session.open", "Open a .ondera session file, replacing the current session. Unsaved changes are discarded.", &[
        req("path", Kind::String, "Path to a .ondera file."),
    ]),
    edit("session.save", "Save the session as a .ondera file. Writes atomically; the old file survives a failed save.", &[
        opt("path", Kind::String, "Destination file. Defaults to the file the session was opened from."),
    ]),
    edit("session.rename", "Set the session name.", &[req("name", Kind::String, "New session name.")]),
    edit("session.bounce", "Render the whole arrangement offline to a stereo 48 kHz / 24-bit WAV file, including a 3-second effect tail.", &[
        req("path", Kind::String, "Destination .wav path."),
    ]),
    edit("session.importAudio", "Decode an audio file (WAV, AIFF, FLAC, MP3, Ogg, AAC) into the session and place it as a clip on an audio track.", &[
        req("path", Kind::String, "Audio file to import."),
        opt("trackId", Kind::String, "Audio track to place the clip on. Defaults to the selected audio track, or a new one."),
        opt("startBar", Kind::Number, "Bar to place the clip at. Defaults to the playhead."),
    ]),
    edit("transport.play", "Start playback from the playhead. Needs the Ondera app (live mode).", &[]),
    edit("transport.record", "Record armed audio and MIDI tracks in the running app. Disable cycle before recording.", &[]),
    edit("transport.stop", "Stop playback and recording.", &[]),
    edit("transport.locate", "Move the playhead. Give either bar or beats.", &[
        opt("bar", Kind::Number, "Zero-based bar position."),
        opt("beats", Kind::Number, "Zero-based beat position."),
    ]),
    edit("transport.returnToStart", "Move the playhead to the beginning.", &[]),
    edit("transport.setTempo", "Set the tempo.", &[req("bpm", Kind::Number, "Beats per minute, 20-400.")]),
    edit("transport.setTimeSignature", "Set the time signature.", &[
        req("numerator", Kind::Integer, "Beats per bar, 1-32."),
        req("denominator", Kind::Integer, "Beat unit: 1, 2, 4, 8, 16 or 32."),
    ]),
    edit("transport.setKey", "Set the displayed song key.", &[req("key", Kind::String, "Key label such as \"C minor\".")]),
    edit("transport.setCycle", "Enable or disable cycle (loop) playback and optionally set its range.", &[
        req("enabled", Kind::Boolean, "Cycle on or off."),
        opt("startBar", Kind::Number, "Cycle start bar."),
        opt("endBar", Kind::Number, "Cycle end bar; must be after startBar."),
    ]),
    edit("transport.setMetronome", "Enable or disable the click.", &[req("enabled", Kind::Boolean, "Metronome on or off.")]),
    edit("transport.setSnap", "Set the grid snap division.", &[req("division", Kind::Integer, "Notes per bar: 1, 2, 4, 8, 16, 32 or 64.")]),
    query("track.list", "List tracks in arrangement order with their instrument and clip count.", &[]),
    edit("track.add", "Add a track at the end of the arrangement and select it.", &[
        req("kind", Kind::String, "\"midi\" for an instrument track or \"audio\"."),
        opt("name", Kind::String, "Track name. Defaults to Instrument N / Audio N."),
        opt("color", Kind::String, "CSS colour: #rrggbb or oklch(l c h). Defaults to the palette."),
        opt("instrument", Kind::String, "Instrument for a MIDI track; see session.catalog."),
    ]),
    edit("track.remove", "Delete a track and every clip on it.", &[TRACK_ID]),
    edit("track.rename", "Rename a track.", &[TRACK_ID, req("name", Kind::String, "New name.")]),
    edit("track.setMute", "Mute or unmute a track.", &[TRACK_ID, req("muted", Kind::Boolean, "Muted or not.")]),
    edit("track.setSolo", "Solo or unsolo a track.", &[TRACK_ID, req("solo", Kind::Boolean, "Soloed or not.")]),
    edit("track.setArmed", "Arm or disarm an audio or MIDI track for recording.", &[TRACK_ID, req("armed", Kind::Boolean, "Armed or not.")]),
    edit("track.setVolume", "Set the fader.", &[TRACK_ID, req("volume", Kind::Number, "0.0 (silent) to 1.0 (+6 dB); 0.75 is unity.")]),
    edit("track.setPan", "Set stereo pan.", &[TRACK_ID, req("pan", Kind::Number, "-100 (left) to 100 (right).")]),
    edit("track.setColor", "Set the track colour.", &[TRACK_ID, req("color", Kind::String, "CSS colour: #rrggbb or oklch(l c h).")]),
    edit("track.move", "Move a track to another position.", &[TRACK_ID, req("index", Kind::Integer, "Zero-based target index.")]),
    edit("track.select", "Select a track in the interface.", &[TRACK_ID]),
    query("clip.list", "List clips (regions) without their notes.", &[
        opt("trackId", Kind::String, "Only clips on this track."),
    ]),
    query("clip.get", "Return one clip with its notes or audio reference.", &[CLIP_ID]),
    edit("clip.create", "Create a MIDI or audio clip. Audio clips need sourceId (see session.get sources).", &[
        TRACK_ID,
        req("startBar", Kind::Number, "Zero-based start bar."),
        req("lengthBars", Kind::Number, "Length in bars, greater than 0."),
        opt("name", Kind::String, "Clip name."),
        opt("notes", Kind::Array, NOTES_DOC),
        opt("sourceId", Kind::String, "Audio source id for a clip on an audio track."),
        opt("offsetSeconds", Kind::Number, "Seconds into the source where the audio clip starts (default 0)."),
    ]),
    edit("clip.move", "Move a clip to another bar and/or track of the same kind.", &[
        CLIP_ID,
        opt("startBar", Kind::Number, "New zero-based start bar."),
        opt("trackId", Kind::String, "Destination track."),
    ]),
    edit("clip.resize", "Change a clip's length.", &[CLIP_ID, req("lengthBars", Kind::Number, "New length in bars.")]),
    edit("clip.rename", "Rename a clip.", &[CLIP_ID, req("name", Kind::String, "New name.")]),
    edit("clip.split", "Split a clip at a bar, keeping notes and audio offsets aligned.", &[
        CLIP_ID,
        req("bar", Kind::Number, "Absolute bar inside the clip."),
    ]),
    edit("clip.duplicate", "Duplicate a clip immediately after itself.", &[CLIP_ID]),
    edit("clip.remove", "Delete a clip.", &[CLIP_ID]),
    edit("clip.setNotes", "Replace every note of a MIDI clip in one undo step.", &[
        CLIP_ID,
        req("notes", Kind::Array, NOTES_DOC),
    ]),
    edit("clip.addLoop", "Insert one of the bundled MIDI loops (see session.catalog) as a new clip, switching the track's instrument to the loop's.", &[
        req("name", Kind::String, "Loop name from session.catalog."),
        opt("trackId", Kind::String, "MIDI track. Defaults to the selected MIDI track, or a new one."),
        opt("startBar", Kind::Number, "Start bar. Defaults to the playhead's bar."),
    ]),
    edit("clip.select", "Select a clip and open it in the editor.", &[CLIP_ID]),
    query("note.list", "List the notes of a MIDI clip.", &[CLIP_ID]),
    edit("note.add", "Add a note to a MIDI clip.", &[
        CLIP_ID,
        req("start", Kind::Number, "Start in beats relative to the clip."),
        req("length", Kind::Number, "Length in beats, greater than 0."),
        req("pitch", Kind::Integer, "MIDI pitch 0-127 (60 = C4)."),
        opt("velocity", Kind::Integer, "1-127, default 100."),
    ]),
    edit("note.update", "Change a note's timing, pitch or velocity.", &[
        CLIP_ID,
        req("noteId", Kind::String, "Note id from note.list."),
        opt("start", Kind::Number, "Start in beats relative to the clip."),
        opt("length", Kind::Number, "Length in beats."),
        opt("pitch", Kind::Integer, "MIDI pitch 0-127."),
        opt("velocity", Kind::Integer, "1-127."),
    ]),
    edit("note.remove", "Delete a note.", &[CLIP_ID, req("noteId", Kind::String, "Note id from note.list.")]),
    query("strip.get", "Return a track or bus channel strip: instrument, eight inserts and two sends. Bus IDs: master, bus-a, bus-b.", &[TRACK_ID]),
    edit("strip.setInstrument", "Choose the instrument of a MIDI track.", &[
        TRACK_ID,
        req("instrument", Kind::String, "Instrument name from session.catalog."),
    ]),
    edit("strip.setInsert", "Load, bypass or clear an insert effect slot.", &[
        TRACK_ID,
        req("slot", Kind::Integer, "Insert slot 0-7."),
        opt("effect", Kind::String, "Effect name from session.catalog. Omit or null to empty the slot."),
        opt("bypassed", Kind::Boolean, "Bypass the effect instead of running it (default false)."),
    ]),
    edit("strip.setSendLevel", "Set a send level to the reverb (A) or delay (B) bus.", &[
        TRACK_ID,
        req("send", Kind::Integer, "0 for A · Reverb, 1 for B · Delay."),
        opt("levelDb", Kind::Number, "Level in dB, -100 to 0. Omit or null for off."),
    ]),
    edit("strip.setPlugin", "Load a stock or installed external plugin. Omit slot for a MIDI instrument; pass slot 0-7 for an insert on a track or bus.", &[
        TRACK_ID, opt("slot", Kind::Integer, "Insert slot 0-7. Omit for the instrument."),
        req("pluginId", Kind::String, "Stable descriptor ID from plugin.list, for example stock:Space."),
    ]),
    query("strip.parameters", "Read a plugin's parameter IDs, plain values, bounds and units. Omit slot for the MIDI instrument.", &[
        TRACK_ID, opt("slot", Kind::Integer, "Insert slot 0-7. Omit for the instrument."),
    ]),
    edit("strip.setParameter", "Set a plugin parameter using its plain value and ID from strip.parameters, in one undo step.", &[
        TRACK_ID, opt("slot", Kind::Integer, "Insert slot 0-7. Omit for the instrument."),
        req("parameterId", Kind::Integer, "Parameter ID from strip.parameters."),
        req("value", Kind::Number, "Plain parameter value within its min and max."),
    ]),
    edit("strip.setBypass", "Bypass or enable a plugin without replacing its settings.", &[
        TRACK_ID, opt("slot", Kind::Integer, "Insert slot 0-7. Omit for the instrument."),
        req("bypassed", Kind::Boolean, "Whether to bypass the processor."),
    ]),
    query("strip.getState", "Read the selected plugin's persisted parameters and base64 state. Saving first captures changes from its native editor.", &[
        TRACK_ID, opt("slot", Kind::Integer, "Insert slot 0-7. Omit for the instrument."),
    ]),
    edit("strip.setState", "Restore base64 state previously captured from this plugin, replacing its explicit parameter overrides.", &[
        TRACK_ID, opt("slot", Kind::Integer, "Insert slot 0-7. Omit for the instrument."),
        req("blob", Kind::String, "Base64 plugin state from strip.getState or session.get."),
    ]),
    edit("master.setVolume", "Set the stereo output fader, including offline exports.", &[
        req("volume", Kind::Number, "0.0 (silent) to 1.0 (+6 dB); 0.75 is unity."),
    ]),
    edit("history.undo", "Undo the last document edit.", &[]),
    edit("history.redo", "Redo the last undone edit.", &[]),
    query("history.info", "Report whether undo and redo are available and the current revision.", &[]),
];

/// One registry for native UI, CLI, MCP, MIDI/export and automation commands.
pub static COMMANDS: std::sync::LazyLock<Vec<Spec>> = std::sync::LazyLock::new(|| {
    BASE_COMMANDS
        .iter()
        .chain(crate::control_media::SPECS)
        .chain(crate::control_automation::SPECS)
        .copied()
        .collect()
});

pub fn spec(name: &str) -> Option<&'static Spec> {
    COMMANDS.iter().find(|s| s.name == name)
}

/// JSON Schema for a command's parameters, used verbatim as an MCP tool `inputSchema`.
pub fn schema(spec: &Spec) -> Value {
    let mut properties = Map::new();
    for p in spec.params {
        let mut prop = json!({ "type": p.kind.schema_type(), "description": p.doc });
        if p.name == "notes" {
            prop["items"] = json!({
                "type": "object",
                "properties": {
                    "start": { "type": "number" },
                    "length": { "type": "number" },
                    "pitch": { "type": "integer" },
                    "velocity": { "type": "integer" },
                    "id": { "type": "string" }
                },
                "required": ["start", "length", "pitch"]
            });
        }
        properties.insert(p.name.into(), prop);
    }
    let required: Vec<&str> = spec
        .params
        .iter()
        .filter(|p| p.required)
        .map(|p| p.name)
        .collect();
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}

/// Plain-data description of the registry.
pub fn describe() -> Value {
    Value::Array(
        COMMANDS
            .iter()
            .map(|s| {
                json!({
                    "name": s.name,
                    "description": s.doc,
                    "mutates": s.mutates,
                    "params": s.params.iter().map(|p| json!({
                        "name": p.name,
                        "type": p.kind.schema_type(),
                        "required": p.required,
                        "description": p.doc,
                    })).collect::<Vec<_>>(),
                })
            })
            .collect(),
    )
}

/// Unique ids share one scheme with the desktop: prefix, nanosecond clock, process counter.
pub fn new_id(prefix: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(1);
    format!(
        "{prefix}-{}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    )
}

/// What a command needs from the process that owns the store. The desktop window implements
/// this over its device engine; [`Headless`] implements it for files and tests.
pub trait Host {
    fn store(&self) -> &Store;
    fn store_mut(&mut self) -> &mut Store;
    fn library(&self) -> &Library;
    fn library_mut(&mut self) -> &mut Library;
    fn path(&self) -> Option<&Path>;
    /// "live" when a window and audio device back the host, "headless" otherwise.
    fn mode(&self) -> &'static str;
    fn playing(&self) -> bool;
    fn position(&self) -> f64;
    fn dispatch(&mut self, command: Command) -> Result<bool> {
        self.store_mut().dispatch(command)
    }
    fn play(&mut self) -> Result<()>;
    fn record(&mut self) -> Result<()> {
        Err("Recording needs the Ondera app in live mode.".into())
    }
    fn recording(&self) -> bool {
        false
    }
    fn plugin_parameters(&mut self, track: &str, slot: Option<usize>) -> Result<Value> {
        let insert = selected_plugin(self.store().session(), track, slot)?;
        let mut instance = plugin_host::instantiate(&insert.plugin_id(), &insert.name, 48000)?;
        if !insert.blob.is_empty() {
            instance
                .editor
                .load(&plugin_host::decode_blob(&insert.blob)?)?;
        }
        Ok(json!({
            "pluginId": insert.plugin_id(),
            "parameters": instance.editor.params().iter().map(|p| json!({
                "id": p.id, "name": p.name, "min": p.min, "max": p.max,
                "default": p.default, "unit": p.unit, "steps": p.steps,
                "logarithmic": p.log, "labels": p.labels,
                "value": insert.params.get(&p.id).copied().or_else(|| instance.editor.value(p.id)).unwrap_or(p.default),
            })).collect::<Vec<_>>()
        }))
    }
    fn stop(&mut self) -> Result<()>;
    fn locate(&mut self, beats: f64) -> Result<()>;
    fn new_session(&mut self, demo: bool) -> Result<()>;
    fn open(&mut self, path: &Path) -> Result<()>;
    fn save(&mut self, path: Option<&Path>) -> Result<PathBuf>;
    fn bounce(&mut self, path: &Path) -> Result<()>;
}

/// Decode an audio file with the same limits the desktop import applies.
pub fn decode_file(path: &Path) -> Result<AudioBuffer> {
    if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > audio::MAX_AUDIO_BYTES as u64 {
        return Err("Audio file exceeds 512 MiB".into());
    }
    audio::decode(
        std::fs::read(path).map_err(|e| e.to_string())?,
        path.extension().and_then(|s| s.to_str()),
    )
}

/// File-backed host without a window or audio device.
pub struct Headless {
    pub store: Store,
    pub library: Library,
    pub path: Option<PathBuf>,
    pub position: f64,
}
impl Default for Headless {
    fn default() -> Self {
        Self::new()
    }
}
impl Headless {
    pub fn new() -> Self {
        Self {
            store: Store::new(store::empty()).expect("Validated empty session"),
            library: Library::new(),
            path: None,
            position: 0.0,
        }
    }
    pub fn open(path: &Path) -> Result<Self> {
        let mut host = Self::new();
        Host::open(&mut host, path)?;
        Ok(host)
    }
}
impl Host for Headless {
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
        "headless"
    }
    fn playing(&self) -> bool {
        false
    }
    fn position(&self) -> f64 {
        self.position
    }
    fn play(&mut self) -> Result<()> {
        Err("Playback needs the Ondera app: start `ondera`, or render with session.bounce.".into())
    }
    fn stop(&mut self) -> Result<()> {
        Ok(())
    }
    fn locate(&mut self, beats: f64) -> Result<()> {
        self.position = beats.max(0.0);
        Ok(())
    }
    fn new_session(&mut self, demo: bool) -> Result<()> {
        self.store
            .load(if demo { store::demo() } else { store::empty() })?;
        self.library.clear();
        self.path = None;
        self.position = 0.0;
        Ok(())
    }
    fn open(&mut self, path: &Path) -> Result<()> {
        let (session, library) = document::load(path)?;
        self.position = session.transport.position_beats;
        self.store.load(session)?;
        self.library = library;
        self.path = Some(path.to_path_buf());
        Ok(())
    }
    fn save(&mut self, path: Option<&Path>) -> Result<PathBuf> {
        let path = path
            .map(Path::to_path_buf)
            .or_else(|| self.path.clone())
            .ok_or("The session has no file yet: pass `path`.")?;
        let mut session = (*self.store.snapshot()).clone();
        session.transport.position_beats = self.position;
        let revision = self.store.revision;
        document::save(&session, &self.library, &path)?;
        self.store.mark_saved(revision);
        self.path = Some(path.clone());
        Ok(path)
    }
    fn bounce(&mut self, path: &Path) -> Result<()> {
        let session = self.store.snapshot();
        let mut library = self.library.clone();
        audio::prepare_sources(&session, &mut library)?;
        render::bounce(&session, &library, path, 48000)
    }
}

struct Args<'a> {
    spec: &'static Spec,
    map: &'a Map<String, Value>,
}
impl Args<'_> {
    fn get(&self, key: &str) -> Option<&Value> {
        self.map.get(key).filter(|v| !v.is_null())
    }
    fn missing(&self, key: &str) -> String {
        format!("{} needs `{key}`", self.spec.name)
    }
    fn str(&self, key: &str) -> Result<&str> {
        self.get(key)
            .and_then(Value::as_str)
            .ok_or_else(|| self.missing(key))
    }
    fn opt_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }
    fn f64(&self, key: &str) -> Result<f64> {
        self.get(key)
            .and_then(Value::as_f64)
            .ok_or_else(|| self.missing(key))
    }
    fn opt_f64(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(Value::as_f64)
    }
    fn int(&self, key: &str) -> Result<i64> {
        self.get(key)
            .and_then(Value::as_i64)
            .ok_or_else(|| self.missing(key))
    }
    fn opt_int(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(Value::as_i64)
    }
    fn bool(&self, key: &str) -> Result<bool> {
        self.get(key)
            .and_then(Value::as_bool)
            .ok_or_else(|| self.missing(key))
    }
    fn opt_bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(Value::as_bool)
    }
}

/// Reject unknown, missing or mistyped parameters before anything touches the store.
fn validate<'a>(spec: &'static Spec, params: &'a Value) -> Result<Args<'a>> {
    let map = match params {
        Value::Null => {
            static EMPTY: std::sync::LazyLock<Map<String, Value>> =
                std::sync::LazyLock::new(Map::new);
            &EMPTY
        }
        Value::Object(m) => m,
        _ => return Err(format!("{} expects an object of parameters", spec.name)),
    };
    for (key, value) in map {
        let Some(p) = spec.params.iter().find(|p| p.name == key) else {
            let expected: Vec<&str> = spec.params.iter().map(|p| p.name).collect();
            return Err(format!(
                "Unknown parameter `{key}` for {}. Accepted: {}",
                spec.name,
                if expected.is_empty() {
                    "none".to_string()
                } else {
                    expected.join(", ")
                }
            ));
        };
        if value.is_null() {
            continue;
        }
        let ok = match p.kind {
            Kind::String => value.is_string(),
            Kind::Number => value.as_f64().is_some_and(f64::is_finite),
            Kind::Integer => value.as_i64().is_some(),
            Kind::Boolean => value.is_boolean(),
            Kind::Array => value.is_array(),
            Kind::Object => value.is_object(),
        };
        if !ok {
            return Err(format!(
                "Parameter `{key}` of {} must be a {}",
                spec.name,
                p.kind.schema_type()
            ));
        }
    }
    for p in spec.params.iter().filter(|p| p.required) {
        if map.get(p.name).is_none_or(Value::is_null) {
            return Err(format!("{} needs `{}`: {}", spec.name, p.name, p.doc));
        }
    }
    Ok(Args { spec, map })
}

/// Validate an invocation without touching the store, audio devices or files.
pub fn validate_request(name: &str, params: &Value) -> Result<()> {
    let command = spec(name)
        .ok_or_else(|| format!("Unknown command `{name}`. Use session.commands to list them."))?;
    validate(command, params).map(|_| ())
}

/// Run one named command. `agent` marks created clips and notes so the interface can show
/// what an assistant changed.
pub fn call(host: &mut dyn Host, name: &str, params: &Value, agent: bool) -> Result<Value> {
    let spec = spec(name).ok_or_else(|| {
        let mut close: Vec<&str> = COMMANDS
            .iter()
            .map(|s| s.name)
            .filter(|n| {
                n.split('.').next() == name.split('.').next()
                    || n.to_lowercase().contains(&name.to_lowercase())
            })
            .collect();
        close.truncate(8);
        if close.is_empty() {
            format!("Unknown command `{name}`. Use session.commands to list them.")
        } else {
            format!(
                "Unknown command `{name}`. Did you mean: {}",
                close.join(", ")
            )
        }
    })?;
    let a = validate(spec, params)?;
    if matches!(
        name,
        "session.bounce" | "session.exportMidi" | "session.exportAudio"
    ) {
        protect_session_file(host.path(), Path::new(a.str("path")?))?;
    }
    if name.starts_with("automation.") {
        return crate::control_automation::call(host, name, params, agent);
    }
    if crate::control_media::SPECS.iter().any(|s| s.name == name) {
        return crate::control_media::call(host, name, params, agent);
    }
    match name {
        "session.info" => Ok(info(host)),
        "session.get" => serde_json::to_value(host.store().session()).map_err(|e| e.to_string()),
        "session.inspect" => Ok(inspect(host, a.opt_bool("includeNotes").unwrap_or(false))),
        "session.catalog" => Ok(catalog()),
        "plugin.list" => plugin_page(&a, plugin_host::scan::installed()),
        "plugin.scan" => {
            let cache = plugin_host::scan::scan_all(|_| {});
            plugin_host::scan::store_cache(&cache)?;
            Ok(
                json!({ "pluginCount": plugin_host::scan::installed().len(), "scannedAt": cache.scanned_at,
                "errors": cache.entries.iter().filter(|e| e.error.is_some()).collect::<Vec<_>>() }),
            )
        }
        "session.commands" => Ok(describe()),
        "session.new" => {
            host.new_session(a.opt_bool("demo").unwrap_or(false))?;
            Ok(info(host))
        }
        "session.open" => {
            host.open(Path::new(a.str("path")?))?;
            Ok(info(host))
        }
        "session.save" => {
            let path = host.save(a.opt_str("path").map(Path::new))?;
            Ok(json!({ "path": path, "dirty": host.store().dirty() }))
        }
        "session.rename" => {
            host.dispatch(Command::Rename(a.str("name")?.into()))?;
            Ok(info(host))
        }
        "session.bounce" => {
            let path = PathBuf::from(a.str("path")?);
            host.bounce(&path)?;
            let s = host.store().session();
            Ok(json!({
                "path": path,
                "seconds": s.end_bar() * s.beats_per_bar() * 60.0 / s.transport.tempo + 3.0,
                "sampleRate": 48000,
                "bitDepth": 24,
            }))
        }
        "session.importAudio" => import_audio(host, &a, agent),
        "transport.play" => {
            host.play()?;
            Ok(transport(host))
        }
        "transport.record" => {
            host.record()?;
            Ok(transport(host))
        }
        "transport.stop" => {
            host.stop()?;
            Ok(transport(host))
        }
        "transport.locate" => {
            let bpb = host.store().session().beats_per_bar();
            let beats = match (a.opt_f64("bar"), a.opt_f64("beats")) {
                (Some(_), Some(_)) => return Err("Give either `bar` or `beats`, not both".into()),
                (Some(bar), None) => bar * bpb,
                (None, Some(beats)) => beats,
                (None, None) => return Err("transport.locate needs `bar` or `beats`".into()),
            };
            if !valid_time(beats) {
                return Err("Position must be between 0 and 1,000,000".into());
            }
            host.locate(beats)?;
            Ok(transport(host))
        }
        "transport.returnToStart" => {
            host.locate(0.0)?;
            Ok(transport(host))
        }
        "transport.setTempo"
        | "transport.setTimeSignature"
        | "transport.setKey"
        | "transport.setCycle"
        | "transport.setMetronome"
        | "transport.setSnap" => {
            let mut t = host.store().session().transport.clone();
            match name {
                "transport.setTempo" => t.tempo = a.f64("bpm")?,
                "transport.setTimeSignature" => {
                    t.time_signature = TimeSignature {
                        numerator: whole(a.int("numerator")?, "numerator")?,
                        denominator: whole(a.int("denominator")?, "denominator")?,
                    }
                }
                "transport.setKey" => t.key = a.str("key")?.into(),
                "transport.setCycle" => {
                    t.cycle = a.bool("enabled")?;
                    if let Some(v) = a.opt_f64("startBar") {
                        t.cycle_start_bar = v;
                    }
                    if let Some(v) = a.opt_f64("endBar") {
                        t.cycle_end_bar = v;
                    }
                }
                "transport.setMetronome" => t.metronome = a.bool("enabled")?,
                _ => t.snap_division = whole(a.int("division")?, "division")?,
            }
            host.dispatch(Command::SetTransport(t))?;
            Ok(transport(host))
        }
        "track.list" => {
            let s = host.store().session();
            Ok(Value::Array(
                s.tracks.iter().map(|t| track_json(s, t)).collect(),
            ))
        }
        "track.add" => {
            let kind = a.str("kind")?;
            if kind != "midi" && kind != "audio" {
                return Err("Track kind must be \"midi\" or \"audio\"".into());
            }
            let instrument = a.opt_str("instrument").map(str::to_string);
            if let Some(i) = &instrument {
                check_instrument(i)?;
                if kind != "midi" {
                    return Err("Only MIDI tracks have an instrument".into());
                }
            }
            let color = match a.opt_str("color") {
                Some(c) => check_color(c)?.to_string(),
                None => TRACK_PALETTE[host.store().session().tracks.len() % 8].into(),
            };
            let track = new_track(
                host.store().session(),
                kind,
                a.opt_str("name").map(str::to_string),
                color,
            );
            let id = track.id.clone();
            let mut commands = vec![Command::AddTrack(track)];
            if let Some(instrument) = instrument {
                commands.push(Command::SetStrip {
                    track: id.clone(),
                    strip: Strip {
                        instrument,
                        ..Default::default()
                    },
                });
            }
            host.dispatch(Command::Batch(commands))?;
            let s = host.store().session();
            let t = s
                .tracks
                .iter()
                .find(|t| t.id == id)
                .ok_or("Track not found")?;
            Ok(track_json(s, t))
        }
        "track.remove" => {
            let id = a.str("trackId")?;
            find_track(host.store().session(), id)?;
            host.dispatch(Command::RemoveTrack(id.into()))?;
            Ok(json!({ "removed": id }))
        }
        "track.rename" | "track.setMute" | "track.setSolo" | "track.setArmed"
        | "track.setVolume" | "track.setPan" | "track.setColor" => {
            let mut track = find_track(host.store().session(), a.str("trackId")?)?.clone();
            match name {
                "track.rename" => track.name = a.str("name")?.into(),
                "track.setMute" => track.mute = a.bool("muted")?,
                "track.setSolo" => track.solo = a.bool("solo")?,
                "track.setArmed" => track.armed = a.bool("armed")?,
                "track.setVolume" => {
                    let v = a.f64("volume")?;
                    if !(0.0..=1.0).contains(&v) {
                        return Err("Volume must be between 0.0 and 1.0".into());
                    }
                    track.volume = v as f32
                }
                "track.setPan" => {
                    let v = a.f64("pan")?;
                    if !(-100.0..=100.0).contains(&v) {
                        return Err("Pan must be between -100 and 100".into());
                    }
                    track.pan = v as f32
                }
                _ => track.color = check_color(a.str("color")?)?.into(),
            }
            let id = track.id.clone();
            host.dispatch(Command::UpdateTrack(track))?;
            let s = host.store().session();
            Ok(track_json(s, find_track(s, &id)?))
        }
        "track.move" => {
            let id = a.str("trackId")?;
            let index = a.int("index")?;
            let count = host.store().session().tracks.len() as i64;
            if index < 0 || index >= count {
                return Err(format!("Index must be between 0 and {}", count - 1));
            }
            find_track(host.store().session(), id)?;
            host.dispatch(Command::MoveTrack {
                id: id.into(),
                index: index as usize,
            })?;
            let s = host.store().session();
            Ok(Value::Array(
                s.tracks.iter().map(|t| track_json(s, t)).collect(),
            ))
        }
        "track.select" => {
            let id = a.str("trackId")?;
            find_track(host.store().session(), id)?;
            host.dispatch(Command::Select {
                track: Some(id.into()),
                clip: None,
                note: None,
            })?;
            Ok(selection(host))
        }
        "clip.list" => {
            let s = host.store().session();
            let track = a.opt_str("trackId");
            if let Some(t) = track {
                find_track(s, t)?;
            }
            Ok(Value::Array(
                s.clips
                    .iter()
                    .filter(|c| track.is_none_or(|t| c.track_id == t))
                    .map(clip_summary)
                    .collect(),
            ))
        }
        "clip.get" => serde_json::to_value(find_clip(host.store().session(), a.str("clipId")?)?)
            .map_err(|e| e.to_string()),
        "clip.create" => {
            let s = host.store().session();
            let track = find_track(s, a.str("trackId")?)?;
            let data = if track.kind == "audio" {
                let source_id = a.str("sourceId").map_err(|_| {
                    "Clips on audio tracks need `sourceId` (see the sources in session.get)"
                })?;
                if !s.sources.contains_key(source_id) {
                    return Err(format!("Unknown audio source `{source_id}`"));
                }
                if a.get("notes").is_some() {
                    return Err("Audio clips cannot hold notes".into());
                }
                ClipData::Audio {
                    source_id: source_id.into(),
                    offset_seconds: a.opt_f64("offsetSeconds").unwrap_or(0.0),
                }
            } else {
                if a.get("sourceId").is_some() || a.get("offsetSeconds").is_some() {
                    return Err("MIDI clips do not reference an audio source".into());
                }
                ClipData::Midi {
                    notes: match a.get("notes") {
                        Some(v) => parse_notes(v, agent)?,
                        None => vec![],
                    },
                }
            };
            let count = s.clips.iter().filter(|c| c.track_id == track.id).count();
            let clip = Clip {
                id: new_id("clip"),
                name: a
                    .opt_str("name")
                    .map(str::to_string)
                    .unwrap_or_else(|| match &data {
                        ClipData::Audio { source_id, .. } => s.sources[source_id].name.clone(),
                        ClipData::Midi { .. } => format!("{} {}", track.name, count + 1),
                    }),
                agent,
                track_id: track.id.clone(),
                start_bar: a.f64("startBar")?,
                length_bars: a.f64("lengthBars")?,
                data,
            };
            let id = clip.id.clone();
            host.dispatch(Command::PutClip(clip))?;
            Ok(clip_summary(find_clip(host.store().session(), &id)?))
        }
        "clip.move" | "clip.resize" | "clip.rename" => {
            let s = host.store().session();
            let mut clip = find_clip(s, a.str("clipId")?)?.clone();
            match name {
                "clip.move" => {
                    if let Some(t) = a.opt_str("trackId") {
                        let target = find_track(s, t)?;
                        let same =
                            matches!(clip.data, ClipData::Midi { .. }) == (target.kind == "midi");
                        if !same {
                            return Err("Clips move only between tracks of the same kind".into());
                        }
                        clip.track_id = t.into();
                    }
                    if let Some(b) = a.opt_f64("startBar") {
                        clip.start_bar = b;
                    }
                }
                "clip.resize" => clip.length_bars = a.f64("lengthBars")?,
                _ => clip.name = a.str("name")?.into(),
            }
            let id = clip.id.clone();
            host.dispatch(Command::PutClip(clip))?;
            Ok(clip_summary(find_clip(host.store().session(), &id)?))
        }
        "clip.split" => {
            let s = host.store().session();
            let clip = find_clip(s, a.str("clipId")?)?;
            let (left, right) = store::split(
                clip,
                a.f64("bar")?,
                new_id("clip"),
                s.beats_per_bar(),
                s.transport.tempo,
            )?;
            let ids = [left.id.clone(), right.id.clone()];
            host.dispatch(Command::Batch(vec![
                Command::PutClip(left),
                Command::PutClip(right),
            ]))?;
            let s = host.store().session();
            Ok(json!({
                "left": clip_summary(find_clip(s, &ids[0])?),
                "right": clip_summary(find_clip(s, &ids[1])?),
            }))
        }
        "clip.duplicate" => {
            let mut clip = find_clip(host.store().session(), a.str("clipId")?)?.clone();
            clip.id = new_id("clip");
            clip.start_bar += clip.length_bars;
            clip.agent = agent;
            if let ClipData::Midi { notes } = &mut clip.data {
                for n in notes {
                    n.id = new_id("note");
                }
            }
            let id = clip.id.clone();
            host.dispatch(Command::PutClip(clip))?;
            Ok(clip_summary(find_clip(host.store().session(), &id)?))
        }
        "clip.remove" => {
            let id = a.str("clipId")?;
            find_clip(host.store().session(), id)?;
            host.dispatch(Command::RemoveClip(id.into()))?;
            Ok(json!({ "removed": id }))
        }
        "clip.setNotes" => {
            let mut clip = find_clip(host.store().session(), a.str("clipId")?)?.clone();
            if !matches!(clip.data, ClipData::Midi { .. }) {
                return Err("Only MIDI clips hold notes".into());
            }
            clip.data = ClipData::Midi {
                notes: parse_notes(a.get("notes").ok_or_else(|| a.missing("notes"))?, agent)?,
            };
            let id = clip.id.clone();
            host.dispatch(Command::PutClip(clip))?;
            Ok(clip_summary(find_clip(host.store().session(), &id)?))
        }
        "clip.addLoop" => add_loop(host, &a, agent),
        "clip.select" => {
            let clip = find_clip(host.store().session(), a.str("clipId")?)?;
            let (track, id) = (clip.track_id.clone(), clip.id.clone());
            host.dispatch(Command::Select {
                track: Some(track),
                clip: Some(id),
                note: None,
            })?;
            Ok(selection(host))
        }
        "note.list" => {
            let clip = find_clip(host.store().session(), a.str("clipId")?)?;
            serde_json::to_value(midi_notes(clip)?).map_err(|e| e.to_string())
        }
        "note.add" | "note.update" | "note.remove" => {
            let mut clip = find_clip(host.store().session(), a.str("clipId")?)?.clone();
            let notes = match &mut clip.data {
                ClipData::Midi { notes } => notes,
                ClipData::Audio { .. } => return Err("Only MIDI clips hold notes".into()),
            };
            let touched = match name {
                "note.add" => {
                    let note = Note {
                        id: new_id("note"),
                        start: a.f64("start")?,
                        length: a.f64("length")?,
                        pitch: byte(a.int("pitch")?, "pitch")?,
                        velocity: byte(a.opt_int("velocity").unwrap_or(100), "velocity")?,
                        agent,
                    };
                    let id = note.id.clone();
                    notes.push(note);
                    Some(id)
                }
                "note.update" => {
                    let id = a.str("noteId")?;
                    let n = notes
                        .iter_mut()
                        .find(|n| n.id == id)
                        .ok_or_else(|| format!("Unknown note `{id}`"))?;
                    if let Some(v) = a.opt_f64("start") {
                        n.start = v;
                    }
                    if let Some(v) = a.opt_f64("length") {
                        n.length = v;
                    }
                    if let Some(v) = a.opt_int("pitch") {
                        n.pitch = byte(v, "pitch")?;
                    }
                    if let Some(v) = a.opt_int("velocity") {
                        n.velocity = byte(v, "velocity")?;
                    }
                    Some(id.to_string())
                }
                _ => {
                    let id = a.str("noteId")?;
                    let before = notes.len();
                    notes.retain(|n| n.id != id);
                    if notes.len() == before {
                        return Err(format!("Unknown note `{id}`"));
                    }
                    None
                }
            };
            let clip_id = clip.id.clone();
            host.dispatch(Command::PutClip(clip))?;
            let clip = find_clip(host.store().session(), &clip_id)?;
            match touched {
                Some(id) => serde_json::to_value(
                    midi_notes(clip)?
                        .iter()
                        .find(|n| n.id == id)
                        .ok_or("Note not found")?,
                )
                .map_err(|e| e.to_string()),
                None => {
                    Ok(json!({ "removed": a.str("noteId")?, "noteCount": midi_notes(clip)?.len() }))
                }
            }
        }
        "strip.get" => {
            let id = a.str("trackId")?;
            check_strip(host.store().session(), id)?;
            Ok(strip_json(host.store().session(), id))
        }
        "strip.parameters" => host.plugin_parameters(a.str("trackId")?, plugin_slot(&a)?),
        "strip.getState" => Ok(json!(selected_plugin(
            host.store().session(),
            a.str("trackId")?,
            plugin_slot(&a)?
        )?)),
        "strip.setPlugin" | "strip.setParameter" | "strip.setBypass" | "strip.setState" => {
            let id = a.str("trackId")?;
            let slot = plugin_slot(&a)?;
            check_strip(host.store().session(), id)?;
            if slot.is_none() && find_track(host.store().session(), id)?.kind != "midi" {
                return Err("Only MIDI tracks have an instrument".into());
            }
            let mut strip = full_strip(host.store().session(), id);
            let mut insert = if name == "strip.setPlugin" {
                let plugin_id = a.str("pluginId")?;
                let descriptor = plugin_host::scan::installed()
                    .into_iter()
                    .find(|d| d.id == plugin_id)
                    .ok_or_else(|| {
                        format!("Unknown plugin `{plugin_id}`. Run plugin.scan, then plugin.list.")
                    })?;
                if (slot.is_none() && !descriptor.instrument)
                    || (slot.is_some() && !descriptor.effect)
                {
                    return Err(
                        "The plugin is not compatible with this instrument or effect slot".into(),
                    );
                }
                // Verify loading before accepting an unusable plugin into the song.
                plugin_host::instantiate(&descriptor.id, &descriptor.name, 48000)?;
                Insert::new(new_id("plugin"), &descriptor.id, &descriptor.name)
            } else {
                selected_plugin(host.store().session(), id, slot)?
            };
            match name {
                "strip.setParameter" => {
                    let parameter = whole(a.int("parameterId")?, "parameterId")?;
                    let metadata = host.plugin_parameters(id, slot)?;
                    let param = metadata["parameters"]
                        .as_array()
                        .and_then(|p| p.iter().find(|p| p["id"] == parameter))
                        .ok_or_else(|| {
                            format!("Unknown parameter `{parameter}`. Use strip.parameters.")
                        })?;
                    let value = a.f64("value")?;
                    let min = param["min"]
                        .as_f64()
                        .ok_or("Plugin parameter has invalid bounds")?;
                    let max = param["max"]
                        .as_f64()
                        .ok_or("Plugin parameter has invalid bounds")?;
                    if !(min..=max).contains(&value) {
                        return Err(format!("Parameter value must be between {min} and {max}"));
                    }
                    insert.params.insert(parameter, value);
                }
                "strip.setBypass" => {
                    insert.state = if a.bool("bypassed")? {
                        "bypassed"
                    } else {
                        "active"
                    }
                    .into()
                }
                "strip.setState" => {
                    let blob = a.str("blob")?;
                    if blob.len() > 64 * 1024 * 1024 {
                        return Err("Plugin state exceeds 64 MiB".into());
                    }
                    let bytes = plugin_host::decode_blob(blob)?;
                    let mut instance =
                        plugin_host::instantiate(&insert.plugin_id(), &insert.name, 48000)?;
                    instance.editor.load(&bytes)?;
                    insert.blob = blob.into();
                    insert.params.clear();
                }
                _ => {}
            }
            if let Some(slot) = slot {
                strip.inserts[slot] = insert;
            } else {
                strip.synth = Some(insert);
            }
            host.dispatch(Command::SetStrip {
                track: id.into(),
                strip,
            })?;
            Ok(strip_json(host.store().session(), id))
        }
        "strip.setInstrument" | "strip.setInsert" | "strip.setSendLevel" => {
            let id = a.str("trackId")?;
            check_strip(host.store().session(), id)?;
            let mut strip = full_strip(host.store().session(), id);
            match name {
                "strip.setInstrument" => {
                    if find_track(host.store().session(), id)?.kind != "midi" {
                        return Err("Only MIDI tracks have an instrument".into());
                    }
                    let instrument = a.str("instrument")?;
                    check_instrument(instrument)?;
                    strip.instrument = instrument.into();
                    strip.synth = None;
                }
                "strip.setInsert" => {
                    let slot = plugin_slot(&a)?.ok_or("Insert slot required")?;
                    strip.inserts[slot] = match a.opt_str("effect") {
                        None => Insert::empty_slot(),
                        Some(effect) => {
                            if !EFFECTS.contains(&effect) {
                                return Err(format!(
                                    "Unknown effect `{effect}`. Available: {}",
                                    EFFECTS.join(", ")
                                ));
                            }
                            let mut insert =
                                Insert::new(new_id("plugin"), &format!("stock:{effect}"), effect);
                            if a.opt_bool("bypassed").unwrap_or(false) {
                                insert.state = "bypassed".into();
                            }
                            insert
                        }
                    };
                }
                _ => {
                    if is_bus(id) {
                        return Err(
                            "Sends are available on tracks; bus sends would create feedback".into(),
                        );
                    }
                    let send = a.int("send")?;
                    if !(0..2).contains(&send) {
                        return Err("Send must be 0 (A · Reverb) or 1 (B · Delay)".into());
                    }
                    let level = a.opt_f64("levelDb");
                    if level.is_some_and(|v| !(-100.0..=0.0).contains(&v)) {
                        return Err("Send level must be between -100 and 0 dB".into());
                    }
                    strip.sends[send as usize].level_db = level.map(|v| v as f32);
                }
            }
            host.dispatch(Command::SetStrip {
                track: id.into(),
                strip,
            })?;
            Ok(strip_json(host.store().session(), id))
        }
        "master.setVolume" => {
            let volume = a.f64("volume")?;
            if !(0.0..=1.0).contains(&volume) {
                return Err("Volume must be between 0.0 and 1.0".into());
            }
            host.dispatch(Command::SetMasterVolume(volume as f32))?;
            Ok(json!({ "volume": host.store().session().master_volume }))
        }
        "history.undo" | "history.redo" => {
            let done = host.dispatch(if name == "history.undo" {
                Command::Undo
            } else {
                Command::Redo
            })?;
            let mut v = history(host);
            v["applied"] = json!(done);
            Ok(v)
        }
        "history.info" => Ok(history(host)),
        _ => Err(format!(
            "Command `{name}` is registered but not implemented"
        )),
    }
}

/// Exports must not replace the document that the host is currently editing,
/// including through a relative path or a symlink alias.
fn protect_session_file(session_path: Option<&Path>, output: &Path) -> Result<()> {
    fn identity(path: &Path) -> Result<PathBuf> {
        if path.exists() {
            return std::fs::canonicalize(path).map_err(|e| e.to_string());
        }
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let name = path.file_name().ok_or("Export path needs a filename")?;
        Ok(std::fs::canonicalize(parent)
            .or_else(|_| std::path::absolute(parent))
            .map_err(|e| e.to_string())?
            .join(name))
    }
    if let Some(session) = session_path {
        if identity(session)? == identity(output)? {
            return Err(
                "Export cannot overwrite the open session file; choose a different output path"
                    .into(),
            );
        }
    }
    Ok(())
}

fn whole(v: i64, key: &str) -> Result<u32> {
    u32::try_from(v).map_err(|_| format!("`{key}` must be a positive integer"))
}
fn byte(v: i64, key: &str) -> Result<u8> {
    if (0..=127).contains(&v) {
        Ok(v as u8)
    } else {
        Err(format!("`{key}` must be between 0 and 127"))
    }
}
fn check_instrument(name: &str) -> Result<()> {
    if INSTRUMENTS.contains(&name) {
        Ok(())
    } else {
        Err(format!(
            "Unknown instrument `{name}`. Available: {}",
            INSTRUMENTS.join(", ")
        ))
    }
}
fn check_color(css: &str) -> Result<&str> {
    let hex = css
        .strip_prefix('#')
        .is_some_and(|h| h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit()));
    let oklch = css
        .strip_prefix("oklch(")
        .and_then(|s| s.strip_suffix(')'))
        .is_some_and(|raw| {
            raw.split_whitespace()
                .filter_map(|s| s.parse::<f64>().ok())
                .count()
                == 3
        });
    if hex || oklch {
        Ok(css)
    } else {
        Err("Colour must be #rrggbb or oklch(l c h)".into())
    }
}
fn find_track<'a>(s: &'a Session, id: &str) -> Result<&'a Track> {
    s.tracks.iter().find(|t| t.id == id).ok_or_else(|| {
        format!(
            "Unknown track `{id}`. Tracks: {}",
            s.tracks
                .iter()
                .map(|t| t.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    })
}
fn find_clip<'a>(s: &'a Session, id: &str) -> Result<&'a Clip> {
    s.clips
        .iter()
        .find(|c| c.id == id)
        .ok_or_else(|| format!("Unknown clip `{id}`. Use clip.list."))
}
fn midi_notes(clip: &Clip) -> Result<&Vec<Note>> {
    match &clip.data {
        ClipData::Midi { notes } => Ok(notes),
        ClipData::Audio { .. } => Err("Only MIDI clips hold notes".into()),
    }
}
fn parse_notes(value: &Value, agent: bool) -> Result<Vec<Note>> {
    let items = value.as_array().ok_or("`notes` must be an array")?;
    if items.len() > 200_000 {
        return Err("Too many notes".into());
    }
    items
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let obj = n
                .as_object()
                .ok_or_else(|| format!("Note {i} must be an object"))?;
            for key in obj.keys() {
                if !["id", "start", "length", "pitch", "velocity", "agent"].contains(&key.as_str())
                {
                    return Err(format!("Note {i} has an unknown field `{key}`"));
                }
            }
            let num = |k: &str| {
                obj.get(k)
                    .and_then(Value::as_f64)
                    .filter(|v| v.is_finite())
                    .ok_or_else(|| format!("Note {i} needs a numeric `{k}`"))
            };
            let pitch = obj
                .get("pitch")
                .and_then(Value::as_i64)
                .ok_or_else(|| format!("Note {i} needs an integer `pitch`"))?;
            let velocity = match obj.get("velocity") {
                None | Some(Value::Null) => 100,
                Some(v) => v
                    .as_i64()
                    .ok_or_else(|| format!("Note {i} has a non-integer `velocity`"))?,
            };
            let (start, length) = (num("start")?, num("length")?);
            if length <= 0.0 || !valid_time(start) || !valid_time(length) {
                return Err(format!("Note {i} needs start >= 0 and length > 0 beats"));
            }
            if !(1..=127).contains(&velocity) {
                return Err(format!("Note {i} velocity must be 1-127"));
            }
            Ok(Note {
                id: obj
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| new_id("note")),
                start,
                length,
                pitch: byte(pitch, "pitch")?,
                velocity: velocity as u8,
                agent: obj.get("agent").and_then(Value::as_bool).unwrap_or(agent),
            })
        })
        .collect()
}
fn new_track(s: &Session, kind: &str, name: Option<String>, color: String) -> Track {
    let index = s.tracks.len();
    Track {
        id: new_id("track"),
        name: name.unwrap_or_else(|| {
            format!(
                "{} {}",
                if kind == "audio" {
                    "Audio"
                } else {
                    "Instrument"
                },
                index + 1
            )
        }),
        color,
        armed: false,
        extra: Default::default(),
        kind: kind.into(),
        volume: 0.75,
        pan: 0.0,
        mute: false,
        solo: false,
    }
}
/// A strip padded to its eight inserts and two sends, as the inspector shows it.
fn full_strip(s: &Session, track: &str) -> Strip {
    let mut strip = s.strips.get(track).cloned().unwrap_or_default();
    while strip.inserts.len() < MAX_INSERTS {
        strip.inserts.push(Insert {
            name: "Empty slot".into(),
            state: "empty".into(),
            ..Default::default()
        });
    }
    while strip.sends.len() < 2 {
        strip.sends.push(Send {
            level_db: None,
            name: SEND_NAMES[strip.sends.len()].into(),
        });
    }
    strip
}
fn import_audio(host: &mut dyn Host, a: &Args, agent: bool) -> Result<Value> {
    let path = Path::new(a.str("path")?);
    let buffer = Arc::new(decode_file(path)?);
    if audio::library_bytes(host.library()).saturating_add(buffer.frames.len() * 8)
        > audio::MAX_LIBRARY_BYTES
    {
        return Err("Decoded audio library exceeds 1 GiB".into());
    }
    let s = host.store().session();
    let track = match a.opt_str("trackId") {
        Some(t) => {
            let t = find_track(s, t)?;
            if t.kind != "audio" {
                return Err("Audio can only be placed on an audio track".into());
            }
            Some(t.id.clone())
        }
        None => s
            .tracks
            .iter()
            .find(|t| Some(&t.id) == s.view.selected_track_id.as_ref() && t.kind == "audio")
            .map(|t| t.id.clone()),
    };
    let mut commands = vec![];
    let track = track.unwrap_or_else(|| {
        let t = new_track(s, "audio", None, TRACK_PALETTE[s.tracks.len() % 8].into());
        let id = t.id.clone();
        commands.push(Command::AddTrack(t));
        id
    });
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Audio")
        .to_string();
    let bpb = s.beats_per_bar();
    let start_bar = a
        .opt_f64("startBar")
        .unwrap_or_else(|| host.position() / bpb);
    let source = Source {
        id: new_id("source"),
        name: name.clone(),
        sample_rate: buffer.sample_rate,
        channels: 2,
        file_name: Some(format!("{name}.wav")),
        duration_seconds: buffer.duration(),
        origin: "file".into(),
        seed: None,
        wave_kind: None,
    };
    let clip = Clip {
        id: new_id("clip"),
        name,
        agent,
        track_id: track,
        start_bar,
        length_bars: buffer.duration() * s.transport.tempo / 60.0 / bpb,
        data: ClipData::Audio {
            source_id: source.id.clone(),
            offset_seconds: 0.0,
        },
    };
    let (source_id, clip_id) = (source.id.clone(), clip.id.clone());
    commands.push(Command::PutSource(source));
    commands.push(Command::PutClip(clip));
    host.library_mut().insert(source_id.clone(), buffer);
    if let Err(e) = host.dispatch(Command::Batch(commands)) {
        host.library_mut().remove(&source_id);
        return Err(e);
    }
    let s = host.store().session();
    Ok(json!({
        "source": s.sources.get(&source_id),
        "clip": clip_summary(find_clip(s, &clip_id)?),
    }))
}
fn add_loop(host: &mut dyn Host, a: &Args, agent: bool) -> Result<Value> {
    let patterns: Value = serde_json::from_str(LOOPS).map_err(|e| e.to_string())?;
    let name = a.str("name")?;
    let pattern = patterns.get(name).ok_or_else(|| {
        format!(
            "Unknown loop `{name}`. Available: {}",
            loop_names(&patterns).join(", ")
        )
    })?;
    let instrument = pattern["instrument"].as_str().unwrap_or("Ondera Synth");
    let s = host.store().session();
    let track = match a.opt_str("trackId") {
        Some(t) => {
            let t = find_track(s, t)?;
            if t.kind != "midi" {
                return Err("Loops need a MIDI track".into());
            }
            Some(t.id.clone())
        }
        None => s
            .tracks
            .iter()
            .find(|t| Some(&t.id) == s.view.selected_track_id.as_ref() && t.kind == "midi")
            .map(|t| t.id.clone()),
    };
    let mut commands = vec![];
    let track = track.unwrap_or_else(|| {
        let t = new_track(s, "midi", None, TRACK_PALETTE[s.tracks.len() % 8].into());
        let id = t.id.clone();
        commands.push(Command::AddTrack(t));
        id
    });
    let mut strip = full_strip(s, &track);
    strip.instrument = instrument.into();
    strip.synth = None;
    commands.push(Command::SetStrip {
        track: track.clone(),
        strip,
    });
    let notes = pattern["notes"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|n| Note {
            id: new_id("note"),
            start: n["start"].as_f64().unwrap_or(0.0),
            length: n["length"].as_f64().unwrap_or(0.25),
            pitch: n["pitch"].as_u64().unwrap_or(60) as u8,
            velocity: n["velocity"].as_u64().unwrap_or(100) as u8,
            agent,
        })
        .collect();
    // Patterns are authored in 4/4; length is translated to current bars.
    let bpb = s.beats_per_bar();
    let clip = Clip {
        id: new_id("clip"),
        name: name.into(),
        agent,
        track_id: track,
        start_bar: a
            .opt_f64("startBar")
            .unwrap_or_else(|| (host.position() / bpb).floor()),
        length_bars: pattern["bars"].as_f64().unwrap_or(1.0) * 4.0 / bpb,
        data: ClipData::Midi { notes },
    };
    let id = clip.id.clone();
    commands.push(Command::PutClip(clip));
    host.dispatch(Command::Batch(commands))?;
    Ok(clip_summary(find_clip(host.store().session(), &id)?))
}
fn loop_names(patterns: &Value) -> Vec<String> {
    let mut names: Vec<String> = patterns
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    names.sort();
    names
}

fn catalog() -> Value {
    let patterns: Value = serde_json::from_str(LOOPS).unwrap_or(Value::Null);
    json!({
        "instruments": INSTRUMENTS,
        "effects": EFFECTS,
        "loops": loop_names(&patterns).iter().map(|n| json!({
            "name": n,
            "instrument": patterns[n]["instrument"],
            "bars": patterns[n]["bars"],
        })).collect::<Vec<_>>(),
        "sends": SEND_NAMES,
        "plugins": crate::stock::descriptors(),
        "buses": [MASTER, BUS_A, BUS_B],
        "insertSlots": MAX_INSERTS,
    })
}
fn plugin_page(args: &Args, plugins: Vec<crate::plugin::Descriptor>) -> Result<Value> {
    let format = args.opt_str("format");
    if format.is_some_and(|format| !["stock", "clap", "vst3", "au"].contains(&format)) {
        return Err("Plugin format must be stock, clap, vst3 or au".into());
    }
    let kind = args.opt_str("kind");
    if kind.is_some_and(|kind| !["instrument", "effect"].contains(&kind)) {
        return Err("Plugin kind must be instrument or effect".into());
    }
    let limit = args.opt_int("limit").unwrap_or(50);
    let offset = args.opt_int("offset").unwrap_or(0);
    if !(1..=200).contains(&limit) || offset < 0 {
        return Err("Plugin limit must be 1-200 and offset must be non-negative".into());
    }
    let query = args.opt_str("query").unwrap_or("").to_lowercase();
    let filtered: Vec<_> = plugins
        .into_iter()
        .filter(|plugin| {
            let format_name = plugin.format.prefix();
            format.is_none_or(|format| format == format_name)
                && kind.is_none_or(|kind| {
                    if kind == "instrument" {
                        plugin.instrument
                    } else {
                        plugin.effect
                    }
                })
                && (query.is_empty()
                    || plugin.name.to_lowercase().contains(&query)
                    || plugin.vendor.to_lowercase().contains(&query)
                    || plugin.id.to_lowercase().contains(&query))
        })
        .collect();
    let total = filtered.len();
    let offset = usize::try_from(offset).map_err(|_| "Plugin offset is too large")?;
    let end = offset.saturating_add(limit as usize).min(total);
    let page = &filtered[offset.min(total)..end];
    Ok(
        json!({"plugins":page,"total":total,"offset":offset,"limit":limit,"nextOffset":if end<total {Some(end)} else {None},"cachePath":plugin_host::scan::cache_path()}),
    )
}
fn inspect(host: &dyn Host, include_notes: bool) -> Value {
    let session = host.store().session();
    let insert = |insert: &Insert| {
        json!({
            "id":insert.id,"name":insert.name,"pluginId":insert.plugin_id(),
            "state":insert.state,"params":insert.params,"hasSavedState":!insert.blob.is_empty()
        })
    };
    let strips: Map<String, Value> = session.strips.iter().map(|(id,strip)| {
        (id.clone(),json!({
            "instrument":strip.instrument_name(),"synth":strip.synth.as_ref().map(insert),
            "inserts":strip.inserts.iter().map(&insert).collect::<Vec<_>>(),"sends":strip.sends
        }))
    }).collect();
    let clips = if include_notes {
        json!(session.clips)
    } else {
        json!(session.clips.iter().map(clip_summary).collect::<Vec<_>>())
    };
    json!({
        "info":info(host),"tracks":session.tracks,"clips":clips,"sources":session.sources,
        "strips":strips,"automation":session.automation,"masterVolume":session.master_volume,
        "includesNotes":include_notes,"includesPluginState":false
    })
}
fn transport(host: &dyn Host) -> Value {
    let s = host.store().session();
    let t = &s.transport;
    let beats = host.position();
    json!({
        "playing": host.playing(),
        "recording": host.recording(),
        "positionBeats": beats,
        "positionBar": beats / s.beats_per_bar(),
        "tempo": t.tempo,
        "timeSignature": { "numerator": t.time_signature.numerator, "denominator": t.time_signature.denominator },
        "key": t.key,
        "snapDivision": t.snap_division,
        "cycle": { "enabled": t.cycle, "startBar": t.cycle_start_bar, "endBar": t.cycle_end_bar },
        "metronome": t.metronome,
    })
}
fn selection(host: &dyn Host) -> Value {
    let v = &host.store().session().view;
    json!({
        "trackId": v.selected_track_id,
        "clipId": v.selected_clip_id,
        "noteId": v.selected_note_id,
    })
}
fn history(host: &dyn Host) -> Value {
    let store = host.store();
    json!({
        "canUndo": store.can_undo(),
        "canRedo": store.can_redo(),
        "revision": store.revision,
        "dirty": store.dirty(),
    })
}
fn info(host: &dyn Host) -> Value {
    let s = host.store().session();
    json!({
        "name": s.name,
        "path": host.path(),
        "mode": host.mode(),
        "dirty": host.store().dirty(),
        "transport": transport(host),
        "trackCount": s.tracks.len(),
        "clipCount": s.clips.len(),
        "sourceCount": s.sources.len(),
        "endBar": s.end_bar(),
        "selection": selection(host),
        "history": history(host),
    })
}
fn track_json(s: &Session, t: &Track) -> Value {
    json!({
        "id": t.id,
        "name": t.name,
        "kind": t.kind,
        "color": t.color,
        "volume": t.volume,
        "pan": t.pan,
        "mute": t.mute,
        "solo": t.solo,
        "armed": t.armed,
        "instrument": if t.kind == "midi" { Some(full_strip(s, &t.id).instrument_name()) } else { None },
        "clipCount": s.clips.iter().filter(|c| c.track_id == t.id).count(),
        "index": s.tracks.iter().position(|x| x.id == t.id),
    })
}
fn clip_summary(c: &Clip) -> Value {
    let mut v = json!({
        "id": c.id,
        "name": c.name,
        "trackId": c.track_id,
        "startBar": c.start_bar,
        "lengthBars": c.length_bars,
        "endBar": c.start_bar + c.length_bars,
        "agent": c.agent,
    });
    match &c.data {
        ClipData::Midi { notes } => {
            v["kind"] = json!("midi");
            v["noteCount"] = json!(notes.len());
        }
        ClipData::Audio {
            source_id,
            offset_seconds,
        } => {
            v["kind"] = json!("audio");
            v["sourceId"] = json!(source_id);
            v["offsetSeconds"] = json!(offset_seconds);
        }
    }
    v
}
fn strip_json(s: &Session, id: &str) -> Value {
    let strip = full_strip(s, id);
    let midi = s.tracks.iter().any(|t| t.id == id && t.kind == "midi");
    json!({
        "trackId": id,
        "instrument": if midi { Some(strip.instrument_name()) } else { None },
        "synth": strip.synth,
        "inserts": strip.inserts.iter().enumerate().map(|(i, ins)| json!({
            "slot": i,
            "effect": if ins.state == "empty" { None } else { Some(&ins.name) },
            "state": ins.state,
            "pluginId": if ins.is_empty() { None } else { Some(ins.plugin_id()) },
            "id": ins.id,
            "params": ins.params,
        })).collect::<Vec<_>>(),
        "sends": strip.sends.iter().enumerate().map(|(i, send)| json!({
            "send": i,
            "name": send.name,
            "levelDb": send.level_db,
        })).collect::<Vec<_>>(),
    })
}

fn check_strip(s: &Session, id: &str) -> Result<()> {
    if is_bus(id) {
        Ok(())
    } else {
        find_track(s, id).map(|_| ())
    }
}
fn plugin_slot(args: &Args) -> Result<Option<usize>> {
    args.opt_int("slot")
        .map(|slot| {
            if (0..MAX_INSERTS as i64).contains(&slot) {
                Ok(slot as usize)
            } else {
                Err(format!("Insert slot must be 0-{}", MAX_INSERTS - 1))
            }
        })
        .transpose()
}
/// Resolve a slot, including the implicit stock instrument, using the same stable
/// instance key as the renderer and native plugin rack.
pub fn selected_plugin(s: &Session, track: &str, slot: Option<usize>) -> Result<Insert> {
    check_strip(s, track)?;
    let strip = full_strip(s, track);
    let insert = if let Some(slot) = slot {
        strip
            .inserts
            .get(slot)
            .cloned()
            .ok_or("Invalid insert slot")?
    } else {
        if find_track(s, track)?.kind != "midi" {
            return Err("Only MIDI tracks have an instrument".into());
        }
        strip.synth.clone().unwrap_or_else(|| {
            Insert::new(
                strip.synth_key(track),
                &format!("stock:{}", strip.instrument),
                &strip.instrument,
            )
        })
    };
    if insert.is_empty() {
        return Err("This insert slot is empty".into());
    }
    Ok(insert)
}
