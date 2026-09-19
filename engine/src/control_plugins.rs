//! The plugin library as a musician browses it: every plugin filed in a sound folder
//! (Synths, Drums, Dynamics, Space & Time…), favourites, and what was used last.
use crate::{
    control::{edit, opt, query, req, Args, Host, Kind, Spec},
    host::scan,
    plugin::Descriptor,
    settings::Plugins,
    Result,
};
use serde_json::{json, Value};

/// Folders in the order the browser shows them.
pub const INSTRUMENT_FOLDERS: &[&str] = &[
    "Synths",
    "Keys",
    "Bass",
    "Drums",
    "Pads",
    "Samplers",
    "Textures",
    "Other Instruments",
];
pub const EFFECT_FOLDERS: &[&str] = &[
    "Dynamics",
    "EQ & Filter",
    "Distortion",
    "Modulation",
    "Space & Time",
    "Pitch",
    "Channel Strips",
    "Mastering",
    "Restoration",
    "Utility",
    "Other Effects",
];

pub const SPECS: &[Spec] = &[
    query("plugin.folders", "The sound folders of the plugin library with how many instruments and effects each holds, plus favourites and recents.", &[]),
    edit("plugin.setFavorite", "Star or unstar a plugin so it shows under Favourites in the browser.", &[
        req("pluginId", Kind::String, "Plugin id from plugin.list."),
        req("favorite", Kind::Boolean, "Star (true) or unstar (false)."),
    ]),
    edit("plugin.setFolder", "File a plugin under another sound folder, or a new one of your own. Omit folder to return it to the automatic one.", &[
        req("pluginId", Kind::String, "Plugin id from plugin.list."),
        opt("folder", Kind::String, "Folder name, 1-40 characters."),
    ]),
    edit("plugin.scaffold", "Start a new Ondera native plugin in Rust: writes a crate with a working effect or instrument, a test that runs it through the real plugin ABI, and build notes. Build it with cargo, then plugin.install.", &[
        req("path", Kind::String, "Directory to create. It must not exist yet."),
        req("name", Kind::String, "Plugin display name, for example Warm Drive."),
        opt("kind", Kind::String, "effect (default) or instrument."),
        opt("vendor", Kind::String, "Your name or label, default My Studio."),
    ]),
    edit("plugin.install", "Copy a built native plugin library (.dylib, .so, .dll or .onplug) into Ondera's plugin folder. Run plugin.scan afterwards to load it.", &[
        req("path", Kind::String, "The built library, for example target/release/libwarm_drive.dylib."),
    ]),
];

const SDK_GIT: &str = "https://github.com/ludovic111/ondera";

fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.trim().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

fn scaffold(path: &std::path::Path, name: &str, instrument: bool, vendor: &str) -> Result<Value> {
    let crate_name = slug(name);
    if crate_name.is_empty() || name.len() > 60 || name.chars().any(|c| c.is_control() || c == '"')
    {
        return Err(
            "The plugin name needs letters or digits, at most 60 characters, no quotes".into(),
        );
    }
    if vendor.len() > 60 || vendor.chars().any(|c| c.is_control() || c == '"') {
        return Err("The vendor is at most 60 characters, no quotes".into());
    }
    if path.exists() {
        return Err(format!(
            "{} already exists; choose a new directory",
            path.display()
        ));
    }
    let ty: String = crate_name
        .split('-')
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_ascii_uppercase().to_string() + c.as_str())
                .unwrap_or_default()
        })
        .collect();
    let ty = if ty.starts_with(|c: char| c.is_ascii_digit()) {
        format!("P{ty}")
    } else {
        ty
    };
    let id = format!(
        "com.{}.{}",
        slug(vendor).replace('-', ""),
        crate_name.replace('-', "")
    );
    let cargo = format!(
        "[package]\nname = \"{crate_name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\ncrate-type = [\"cdylib\", \"rlib\"]\n\n[dependencies]\nondera-plugin = {{ git = \"{SDK_GIT}\", package = \"ondera-plugin\" }}\n"
    );
    let body = if instrument {
        INSTRUMENT_TEMPLATE
    } else {
        EFFECT_TEMPLATE
    }
    .replace("__TYPE__", &ty)
    .replace("__ID__", &id)
    .replace("__NAME__", name)
    .replace("__VENDOR__", vendor);
    let lib_name = crate_name.replace('-', "_");
    let readme = format!(
        "# {name}\n\nAn Ondera native plugin.\n\n    cargo test              # runs the plugin through the real plugin ABI\n    cargo build --release\n    ondera-cli plugin.install path=target/release/lib{lib_name}.dylib   # .so on Linux, {lib_name}.dll on Windows\n    ondera-cli plugin.scan\n\n`process` runs on the audio thread: no allocation, locks, files or logging there.\nParameters are stored in the session by Ondera, so they undo, save and automate for free.\nGuide: {SDK_GIT}/blob/main/docs/NATIVE_PLUGINS.md\n"
    );
    std::fs::create_dir_all(path.join("src")).map_err(|e| e.to_string())?;
    for (file, text) in [
        ("Cargo.toml", cargo),
        ("src/lib.rs", body),
        ("README.md", readme),
    ] {
        std::fs::write(path.join(file), text).map_err(|e| e.to_string())?;
    }
    Ok(
        json!({ "path": path, "crate": crate_name, "pluginId": format!("native:{id}"),
        "files": ["Cargo.toml", "src/lib.rs", "README.md"],
        "next": ["cargo test", "cargo build --release", "plugin.install", "plugin.scan"] }),
    )
}

const EFFECT_TEMPLATE: &str = r#"use ondera_plugin::{export_plugins, prelude::*};

/// A drive stage with a wet/dry blend. Replace the maths in `process` with your own.
pub struct __TYPE__ {
    drive: Smoother,
    mix: f32,
}

impl Plugin for __TYPE__ {
    const INFO: Info = Info::effect("__ID__", "__NAME__", "__VENDOR__", "Distortion")
        .describe("Soft saturation with a blend control.");

    fn params() -> Vec<ParamSpec> {
        vec![param("Drive", 0.0, 36.0, 12.0, "dB"), param("Mix", 0.0, 100.0, 100.0, "%")]
    }
    fn new(rate: f64) -> Self {
        Self { drive: Smoother::new(rate, 0.02, db_to_gain(12.0)), mix: 1.0 }
    }
    fn set_param(&mut self, index: usize, value: f64) {
        match index {
            0 => self.drive.set(db_to_gain(value)),
            1 => self.mix = (value / 100.0) as f32,
            _ => {}
        }
    }
    // Audio thread: no allocation, locks, files or logging in here.
    fn process(&mut self, audio: &mut [[f32; 2]], _notes: &[NoteEvent], _ctx: &ProcessContext) {
        for frame in audio {
            let drive = self.drive.step();
            for sample in frame.iter_mut() {
                let wet = (*sample * drive).tanh() / drive.sqrt();
                *sample += (wet - *sample) * self.mix;
            }
        }
    }
}

export_plugins!(__TYPE__);

#[cfg(test)]
mod tests {
    use super::*;
    use ondera_plugin::testing::Bench;

    #[test]
    fn drives_without_blowing_up_and_bypasses_at_zero_mix() {
        let mut bench = Bench::<__TYPE__>::new(48_000.0);
        bench.set("Drive", 36.0);
        let loud = bench.sine(220.0, 1.0, 0.25);
        Bench::<__TYPE__>::assert_sane(&loud);
        bench.set("Mix", 0.0);
        let dry = bench.sine(220.0, 0.5, 0.25);
        assert!((Bench::<__TYPE__>::peak(&dry) - 0.5).abs() < 0.01);
    }
}
"#;

const INSTRUMENT_TEMPLATE: &str = r#"use ondera_plugin::{export_plugins, prelude::*};
use std::f64::consts::TAU;

const VOICES: usize = 16;

#[derive(Clone, Copy, Default)]
struct Voice {
    pitch: u8,
    phase: f64,
    level: f32,
    held: bool,
    active: bool,
}

/// A sixteen-voice sine synth with a release. Replace the oscillator with your own sound.
pub struct __TYPE__ {
    rate: f64,
    voices: [Voice; VOICES],
    release: f32,
    gain: f32,
}

impl Plugin for __TYPE__ {
    const INFO: Info = Info::instrument("__ID__", "__NAME__", "__VENDOR__")
        .describe("A small polyphonic synth.");

    fn params() -> Vec<ParamSpec> {
        vec![param("Release", 10.0, 4000.0, 300.0, "ms"), param("Level", -24.0, 6.0, -6.0, "dB")]
    }
    fn new(rate: f64) -> Self {
        let mut plugin = Self { rate, voices: [Voice::default(); VOICES], release: 0.0, gain: 0.5 };
        plugin.set_param(0, 300.0);
        plugin
    }
    fn set_param(&mut self, index: usize, value: f64) {
        match index {
            0 => self.release = (-1.0 / (self.rate * value / 1000.0)).exp() as f32,
            1 => self.gain = db_to_gain(value),
            _ => {}
        }
    }
    fn reset(&mut self) {
        self.voices = [Voice::default(); VOICES];
    }
    // Audio thread: no allocation, locks, files or logging in here.
    fn process(&mut self, audio: &mut [[f32; 2]], notes: &[NoteEvent], _ctx: &ProcessContext) {
        let mut next = 0;
        for (i, frame) in audio.iter_mut().enumerate() {
            while next < notes.len() && notes[next].frame as usize <= i {
                let n = notes[next];
                next += 1;
                if n.on {
                    let slot = self.voices.iter().position(|v| !v.active).unwrap_or(0);
                    self.voices[slot] = Voice {
                        pitch: n.pitch,
                        phase: 0.0,
                        level: n.velocity as f32 / 127.0,
                        held: true,
                        active: true,
                    };
                } else {
                    for v in self.voices.iter_mut().filter(|v| v.held && v.pitch == n.pitch) {
                        v.held = false;
                    }
                }
            }
            let mut sum = 0.0;
            for v in self.voices.iter_mut().filter(|v| v.active) {
                let hz = 440.0 * 2f64.powf((v.pitch as f64 - 69.0) / 12.0);
                v.phase = (v.phase + hz / self.rate).fract();
                sum += (TAU * v.phase).sin() as f32 * v.level;
                if !v.held {
                    v.level *= self.release;
                    v.active = v.level > 1e-4;
                }
            }
            // Instruments add to the buffer; the host hands them silence.
            frame[0] += sum * self.gain * 0.3;
            frame[1] += sum * self.gain * 0.3;
        }
    }
}

export_plugins!(__TYPE__);

#[cfg(test)]
mod tests {
    use super::*;
    use ondera_plugin::testing::Bench;

    #[test]
    fn a_note_sounds_and_then_fades_to_silence() {
        let mut bench = Bench::<__TYPE__>::new(48_000.0);
        bench.set("Release", 50.0);
        let out = bench.note(60, 100, 0.2, 2.0);
        Bench::<__TYPE__>::assert_sane(&out);
        assert!(Bench::<__TYPE__>::peak(&out[..9_600]) > 0.05, "the note is audible");
        assert!(Bench::<__TYPE__>::peak(&out[86_400..]) < 1e-3, "and it ends");
    }
}
"#;

fn has(text: &str, words: &[&str]) -> bool {
    words.iter().any(|w| text.contains(w))
}

/// The folder a plugin belongs in when the user has not filed it themselves. Formats describe
/// themselves differently (CLAP features, VST3 sub-categories, a bare AU type), so the name
/// is read as well as the category.
pub fn automatic_folder(d: &Descriptor) -> &'static str {
    // Ondera's own plugins, and native ones written for it, name their folder outright.
    let own: &[&str] = if d.instrument && !d.effect {
        INSTRUMENT_FOLDERS
    } else {
        EFFECT_FOLDERS
    };
    if let Some(named) = own
        .iter()
        .find(|f| f.eq_ignore_ascii_case(d.category.trim()) && !f.starts_with("Other"))
    {
        return named;
    }
    let text = format!("{} {}", d.category, d.name).to_lowercase();
    if d.instrument && (!d.effect || has(&text, &["instrument", "synth"])) {
        return if has(&text, &["bass", "808"]) {
            "Bass"
        } else if has(&text, &["drum", "percuss", "kick", "snare", "beat"]) {
            "Drums"
        } else if has(
            &text,
            &[
                "piano", "keys", "organ", "rhodes", "clav", "wurli", "mallet",
            ],
        ) {
            "Keys"
        } else if has(&text, &["pad", "choir", "string", "ambient", "atmos"]) {
            "Pads"
        } else if has(&text, &["sampl", "rompler", "player"]) {
            "Samplers"
        } else if has(&text, &["riser", "texture", "noise", "drone", "fx"]) {
            "Textures"
        } else if has(
            &text,
            &["synth", "instrument", "lead", "pluck", "wavetable", "fm"],
        ) {
            "Synths"
        } else {
            "Other Instruments"
        };
    }
    EFFECT_RULES
        .iter()
        .find(|(_, words)| has(&text, words))
        .map_or("Other Effects", |(folder, _)| folder)
}

/// Effect folders by the words that give them away, most specific first. Audio Units carry
/// no category at all, so well-known hardware model names are part of the vocabulary.
const EFFECT_RULES: &[(&str, &[&str])] = &[
    (
        "Channel Strips",
        &[
            "channel strip",
            "cla ",
            "jjp",
            "maserati",
            "channel",
            "console",
            "strip",
            "vocal suite",
            "vocals",
            "microphone",
            "mic collection",
        ],
    ),
    (
        "Mastering",
        &["mastering", "master ", "ozone", "loudness", "maximiz"],
    ),
    (
        "Restoration",
        &[
            "restoration",
            "denois",
            "de-nois",
            "declick",
            "de-click",
            "dehum",
            "x-noise",
            "x-hum",
            "x-click",
            "x-crackle",
            "z-noise",
            "clarity",
            "fdbk",
            "feedback supp",
            "soothe",
        ],
    ),
    (
        "Pitch",
        &[
            "pitch", "tune", "auto-key", "vocoder", "harmon", "shift", "formant",
        ],
    ),
    (
        "Dynamics",
        &[
            "compress",
            "limit",
            "gate",
            "dynamic",
            "transient",
            "transx",
            "expander",
            "de-ess",
            "deess",
            "comp",
            "rider",
            "1176",
            "la-2",
            "la2a",
            "la-3",
            "2500",
            "660",
            "670",
            "fairchild",
            "dbx",
            "vca",
            "opto",
            "leveler",
            "levell",
            "multiband",
            "c1 ",
            "c4 ",
            "c6 ",
            "l1 ",
            "l2 ",
            "l3",
            "maxxvolume",
        ],
    ),
    (
        "Space & Time",
        &[
            "reverb", "verb", "delay", "echo", "space", "room", "hall", "plate", "shimmer",
            "spring", "emt", "supertap", "ir-l", "ir1", "ir360", "tap",
        ],
    ),
    (
        "Modulation",
        &[
            "chorus",
            "phaser",
            "flang",
            "tremolo",
            "modulat",
            "vibrato",
            "rotary",
            "doubler",
            "adt",
            "ce-1",
            "ensemble",
            "autopan",
            "auto pan",
            "mondomod",
            "metaflanger",
        ],
    ),
    (
        "Distortion",
        &[
            "distort", "satur", "drive", "fuzz", "crush", "amp", "tape", "clip", "lo-fi", "lofi",
            "guitar", "gtr", "stomp", "vinyl", "retro", "j37", "kramer", "exciter", "enhancer",
            "diezel", "marshall", "fender", "cabinet",
        ],
    ),
    (
        "EQ & Filter",
        &[
            "eq", "filter", "tilt", "shelf", "pultec", "puigtec", "1073", "1081", "88rs", "helios",
            "curves", "bass", "loair", "brighter", "q4", "q6", "q10", "f6",
        ],
    ),
    (
        "Utility",
        &[
            "util",
            "gain",
            "meter",
            "analy",
            "stereo",
            "width",
            "tool",
            "imager",
            "trim",
            "surround",
            "360",
            "5.1",
            "7.1",
            "5.0",
            "immersive",
            "wrapper",
            "phase",
            "send",
            "receive",
            "relay",
            "recall",
            "dorrough",
            "monitor",
            "nx ",
        ],
    ),
];

/// The folder shown for a plugin: the user's filing when there is one.
pub fn folder(d: &Descriptor, library: &Plugins) -> String {
    library
        .folders
        .get(&d.id)
        .cloned()
        .unwrap_or_else(|| automatic_folder(d).to_string())
}

fn entry(d: &Descriptor, library: &Plugins) -> Value {
    let mut value = serde_json::to_value(d).unwrap_or_else(|_| json!({}));
    value["folder"] = json!(folder(d, library));
    value["favorite"] = json!(library.favorites.contains(&d.id));
    value
}

pub(crate) fn page(args: &Args, library: &Plugins) -> Result<Value> {
    let format = args.opt_str("format");
    if format.is_some_and(|format| !["stock", "native", "clap", "vst3", "au"].contains(&format)) {
        return Err("Plugin format must be stock, native, clap, vst3 or au".into());
    }
    let kind = args.opt_str("kind");
    if kind.is_some_and(|kind| !["instrument", "effect"].contains(&kind)) {
        return Err("Plugin kind must be instrument or effect".into());
    }
    let sort = args.opt_str("sort").unwrap_or("name");
    if !["name", "recent"].contains(&sort) {
        return Err("Plugin sort must be name or recent".into());
    }
    let limit = args.opt_int("limit").unwrap_or(50);
    let offset = args.opt_int("offset").unwrap_or(0);
    if !(1..=200).contains(&limit) || offset < 0 {
        return Err("Plugin limit must be 1-200 and offset must be non-negative".into());
    }
    let query = args.opt_str("query").unwrap_or("").to_lowercase();
    let wanted_folder = args.opt_str("folder").map(str::to_lowercase);
    let favorite = args.opt_bool("favorite").unwrap_or(false);
    let mut filtered: Vec<Descriptor> = scan::installed()
        .into_iter()
        .filter(|plugin| {
            format.is_none_or(|format| format == plugin.format.prefix())
                && kind.is_none_or(|kind| {
                    if kind == "instrument" {
                        plugin.instrument
                    } else {
                        plugin.effect
                    }
                })
                && (!favorite || library.favorites.contains(&plugin.id))
                && wanted_folder
                    .as_ref()
                    .is_none_or(|f| folder(plugin, library).to_lowercase() == *f)
                && (query.is_empty()
                    || plugin.name.to_lowercase().contains(&query)
                    || plugin.vendor.to_lowercase().contains(&query)
                    || plugin.id.to_lowercase().contains(&query)
                    || folder(plugin, library).to_lowercase().contains(&query))
        })
        .collect();
    if sort == "recent" {
        let rank = |d: &Descriptor| {
            library
                .recent
                .iter()
                .position(|id| *id == d.id)
                .unwrap_or(usize::MAX)
        };
        filtered.retain(|d| rank(d) != usize::MAX);
        filtered.sort_by_key(rank);
    }
    let total = filtered.len();
    let offset = usize::try_from(offset).map_err(|_| "Plugin offset is too large")?;
    let end = offset.saturating_add(limit as usize).min(total);
    let page: Vec<Value> = filtered[offset.min(total)..end]
        .iter()
        .map(|d| entry(d, library))
        .collect();
    Ok(
        json!({"plugins":page,"total":total,"offset":offset,"limit":limit,
        "nextOffset":if end<total {Some(end)} else {None},"cachePath":scan::cache_path()}),
    )
}

/// Remember that a plugin was just loaded, for the browser's Recent folder. Only the window
/// keeps this list: a headless host (the CLI on a file, a bounce, a test) must never rewrite
/// the person's settings as a side effect of an edit, and concurrent headless hosts doing a
/// read-modify-write of one file would overwrite each other. Best effort even when live: a
/// read-only settings file must not fail the edit that loaded the plugin.
pub(crate) fn note_recent(host: &mut dyn Host, plugin_id: &str) {
    if host.mode() != "live" {
        return;
    }
    let mut settings = host.settings();
    if settings
        .plugins
        .recent
        .first()
        .is_some_and(|id| id == plugin_id)
    {
        return;
    }
    settings.plugins.recent.retain(|id| id != plugin_id);
    settings.plugins.recent.insert(0, plugin_id.to_string());
    settings.plugins.recent.truncate(12);
    let _ = host.update_settings(settings);
}

pub(crate) fn call(host: &mut dyn Host, name: &str, a: &Args) -> Result<Value> {
    let installed = scan::installed();
    let known = |id: &str| -> Result<Descriptor> {
        installed
            .iter()
            .find(|d| d.id == id)
            .cloned()
            .ok_or_else(|| format!("Unknown plugin `{id}`. Run plugin.scan, then plugin.list."))
    };
    match name {
        "plugin.folders" => {
            let library = host.settings().plugins;
            let mut names: Vec<String> = INSTRUMENT_FOLDERS
                .iter()
                .chain(EFFECT_FOLDERS)
                .map(|s| s.to_string())
                .collect();
            for custom in library.folders.values() {
                if !names.contains(custom) {
                    names.push(custom.clone());
                }
            }
            let folders: Vec<Value> = names
                .iter()
                .map(|name| {
                    let inside: Vec<&Descriptor> = installed.iter().filter(|d| folder(d, &library) == *name).collect();
                    json!({ "name": name,
                        "custom": !INSTRUMENT_FOLDERS.contains(&name.as_str()) && !EFFECT_FOLDERS.contains(&name.as_str()),
                        "instruments": inside.iter().filter(|d| d.instrument).count(),
                        "effects": inside.iter().filter(|d| d.effect).count() })
                })
                .filter(|f| f["instruments"] != 0 || f["effects"] != 0 || f["custom"] == true)
                .collect();
            Ok(
                json!({ "folders": folders, "favorites": library.favorites, "recent": library.recent }),
            )
        }
        "plugin.setFavorite" => {
            let plugin = known(a.str("pluginId")?)?;
            let favorite = a.bool("favorite")?;
            let mut settings = host.settings();
            settings.plugins.favorites.retain(|id| *id != plugin.id);
            if favorite {
                settings.plugins.favorites.push(plugin.id.clone());
            }
            let reply = entry(&plugin, &settings.plugins);
            host.update_settings(settings)?;
            Ok(reply)
        }
        "plugin.setFolder" => {
            let plugin = known(a.str("pluginId")?)?;
            let mut settings = host.settings();
            match a.opt_str("folder").map(str::trim).filter(|f| !f.is_empty()) {
                Some(name) if name == automatic_folder(&plugin) => {
                    settings.plugins.folders.remove(&plugin.id);
                }
                Some(name) => {
                    settings
                        .plugins
                        .folders
                        .insert(plugin.id.clone(), name.to_string());
                }
                None => {
                    settings.plugins.folders.remove(&plugin.id);
                }
            }
            settings.validate()?;
            let reply = entry(&plugin, &settings.plugins);
            host.update_settings(settings)?;
            Ok(reply)
        }
        "plugin.scaffold" => {
            let kind = a.opt_str("kind").unwrap_or("effect");
            if !["effect", "instrument"].contains(&kind) {
                return Err("kind must be effect or instrument".into());
            }
            scaffold(
                std::path::Path::new(a.str("path")?),
                a.str("name")?,
                kind == "instrument",
                a.opt_str("vendor").unwrap_or("My Studio"),
            )
        }
        "plugin.install" => {
            let source = std::path::Path::new(a.str("path")?);
            let extension = source
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if !["dylib", "so", "dll", "onplug"].contains(&extension.as_str()) {
                return Err("Install a built library: .dylib, .so, .dll or .onplug".into());
            }
            if !source.is_file() {
                return Err(format!(
                    "{} is not a file; build it with cargo build --release",
                    source.display()
                ));
            }
            let directory = scan::data_dir().join("plugins");
            std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
            let target = directory.join(source.file_name().ok_or("The path has no file name")?);
            // Copy beside the destination first so a failed copy never truncates a working plugin.
            let staged = target.with_extension(format!("{extension}.part"));
            std::fs::copy(source, &staged).map_err(|e| e.to_string())?;
            std::fs::rename(&staged, &target).map_err(|e| {
                let _ = std::fs::remove_file(&staged);
                e.to_string()
            })?;
            Ok(json!({ "installed": target, "next": "plugin.scan" }))
        }
        _ => Err(format!("Unknown command `{name}`")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaffold_writes_a_crate_and_refuses_to_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("warm-drive");
        let reply = scaffold(&path, "Warm Drive 2", false, "Night Owl").unwrap();
        assert_eq!(reply["crate"], "warm-drive-2");
        assert_eq!(reply["pluginId"], "native:com.nightowl.warmdrive2");
        let lib = std::fs::read_to_string(path.join("src/lib.rs")).unwrap();
        assert!(
            lib.contains("pub struct WarmDrive2") && lib.contains("export_plugins!(WarmDrive2);")
        );
        assert!(!lib.contains("__"));
        assert!(scaffold(&path, "Warm Drive 2", false, "Night Owl").is_err());
        let synth = dir.path().join("synth");
        scaffold(&synth, "9 Lives", true, "x").unwrap();
        assert!(std::fs::read_to_string(synth.join("src/lib.rs"))
            .unwrap()
            .contains("pub struct P9Lives"));
    }

    #[test]
    fn well_known_third_party_names_find_their_folder() {
        let effect = |name: &str, category: &str| Descriptor {
            id: format!("au:{name}"),
            format: crate::plugin::Format::AudioUnit,
            name: name.into(),
            vendor: String::new(),
            path: String::new(),
            instrument: false,
            effect: true,
            category: category.into(),
        };
        for (name, category, folder) in [
            ("UAD Teletronix LA-2A Silver", "", "Dynamics"),
            ("PuigChild 660 (m)", "", "Dynamics"),
            ("UAD Neve 1073", "", "EQ & Filter"),
            ("SSLGChannel (s)", "", "Channel Strips"),
            ("Ozone 11 Stabilizer", "", "Mastering"),
            ("UAD EMT 250", "", "Space & Time"),
            ("Doubler2 (m)", "", "Modulation"),
            ("GTR Stomp 2 (m->s)", "", "Distortion"),
            ("Auto-Key", "", "Pitch"),
            ("soothe2", "", "Restoration"),
            ("Immersive Wrapper 7.0.6", "Surround", "Utility"),
            ("Pro-Q 3", "Fx|EQ", "EQ & Filter"),
            ("Channel EQ", "EQ & Filter", "EQ & Filter"),
            ("MaxxBass (m)", "Bass", "EQ & Filter"),
            ("CLA Drums (m->s)", "Drums", "Channel Strips"),
        ] {
            assert_eq!(automatic_folder(&effect(name, category)), folder, "{name}");
        }
    }

    #[test]
    fn every_stock_plugin_lands_in_a_named_folder() {
        for d in crate::stock::descriptors() {
            let folder = automatic_folder(&d);
            assert!(
                !folder.starts_with("Other"),
                "{} fell into {folder}",
                d.name
            );
            let list = if d.instrument {
                INSTRUMENT_FOLDERS
            } else {
                EFFECT_FOLDERS
            };
            assert!(list.contains(&folder), "{} -> {folder}", d.name);
        }
    }
}
