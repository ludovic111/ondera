//! Sounds made from a description, and any sound turned into an instrument.
//!
//! `generate.audio` asks the service chosen in Settings > Generation (ElevenLabs, Stable
//! Audio, fal.ai or a custom endpoint) for a song, a loop, a one-shot or an instrument note.
//! The network call belongs to the window (`Host::live`, desktop `generate.rs`); this module
//! shapes the request from the song (a loop follows its tempo and key), keeps every result
//! in `<data dir>/generated` with a JSON note beside it, and places a result in the song as
//! an audio clip or as a Sample Keys instrument, in one undo step. Everything but the
//! network call also works headless, so the CLI can list, audition and place what the window
//! generated.
use crate::{
    audio::AudioBuffer,
    control::{
        decode_file, edit, find_track, full_strip, new_id, new_track, opt, place_audio, query, req,
        strip_json, Args, Host, Kind, Spec, TRACK_PALETTE,
    },
    host::scan::data_dir,
    model::{Insert, Session},
    sample_keys,
    settings::{Service, Settings},
    store::Command,
    Result,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

pub const SPECS: &[Spec] = &[
    query("generate.services", "The sound generation services ryolune can call (ElevenLabs, Stable Audio, fal.ai and a custom endpoint): the one chosen in Settings > Generation, which have a key, and what each makes best.", &[]),
    edit("generate.audio", "Make a sound from a description with a generation service and put it in the song: a song or a loop as an audio clip (a loop follows the song's tempo and key), a sound effect or one-shot as an audio clip, or an instrument note as a Sample Keys track you play from the keyboard. Runs as a job over the network, on the service's credits, and keeps the result in generate.list. One undo step.", &[
        req("prompt", Kind::String, "What it should sound like, in plain words: genre, instruments, mood, character."),
        opt("kind", Kind::String, "song, loop (default), sound or instrument."),
        opt("seconds", Kind::Number, "Length in seconds. Defaults: song 60, sound 3, instrument 3; a loop defaults to its bars."),
        opt("bars", Kind::Integer, "For a loop: its length in bars at the song's tempo, 1-32 (default 4)."),
        opt("service", Kind::String, "elevenlabs, stability, fal or custom (default: the one in Settings > Generation)."),
        opt("instrumental", Kind::Boolean, "No vocals (default true; songs only)."),
        opt("followSong", Kind::Boolean, "Tell the service the song's tempo and key (default true for loops and songs)."),
        opt("seed", Kind::Integer, "Seed for services that take one, to repeat a result."),
        opt("name", Kind::String, "Name of the clip or instrument track (default: from the prompt)."),
        opt("place", Kind::Boolean, "Put it in the song (default true); false only keeps it in generate.list."),
        opt("trackId", Kind::String, "Track to place it on: an audio track for audio, a MIDI track for an instrument (default: a new track)."),
        opt("startBar", Kind::Number, "Zero-based bar where an audio clip starts (default: the playhead)."),
        opt("rootNote", Kind::Integer, "For an instrument: the MIDI note the sound plays at its own pitch, 0-127 (default 60, middle C)."),
    ]),
    query("generate.list", "The sounds generated on this computer, newest first, with the description, service, kind and length of each. They stay in ryolune's data folder until generate.delete.", &[
        opt("limit", Kind::Integer, "At most this many, 1-200 (default 50)."),
    ]),
    query("generate.preview", "One generated sound as base64 audio with its MIME type, to audition it before placing it.", &[
        req("id", Kind::String, "Generated sound id, from generate.list."),
    ]),
    edit("generate.place", "Put a sound from generate.list in the song: as an audio clip, or as a Sample Keys instrument played from the keyboard. One undo step.", &[
        req("id", Kind::String, "Generated sound id, from generate.list."),
        opt("as", Kind::String, "audio or instrument (default: how it was made)."),
        opt("trackId", Kind::String, "Track to place it on (default: a new track)."),
        opt("startBar", Kind::Number, "Zero-based bar where an audio clip starts (default: the playhead)."),
        opt("rootNote", Kind::Integer, "For an instrument: the MIDI note the sound plays at its own pitch (default 60)."),
    ]),
    edit("generate.delete", "Delete a generated sound from this computer. Clips already in a song keep their audio.", &[
        req("id", Kind::String, "Generated sound id, from generate.list."),
    ]),
    edit("strip.loadSample", "Turn a sound into an instrument: load an audio file, or audio already in the song, into Sample Keys, which plays it across the keyboard from its root note. On the given MIDI track, else on a new one named after the sound. One undo step.", &[
        opt("trackId", Kind::String, "MIDI track to play it on (default: a new track)."),
        opt("path", Kind::String, "Audio file to load (WAV, AIFF, FLAC, MP3, Ogg, AAC…)."),
        opt("sourceId", Kind::String, "Audio already in the song, by source id from session.overview, instead of a file."),
        opt("rootNote", Kind::Integer, "The MIDI note that plays the sound at its own pitch, 0-127 (default 60)."),
        opt("name", Kind::String, "Name of a new track (default: the sound's name)."),
    ]),
];

/// What a generation is for: it decides the service endpoint, the default length and where
/// the result goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GenKind {
    Song,
    Loop,
    Sound,
    Instrument,
}
impl GenKind {
    fn parse(text: &str) -> Result<Self> {
        Ok(match text {
            "song" => Self::Song,
            "loop" => Self::Loop,
            "sound" => Self::Sound,
            "instrument" => Self::Instrument,
            other => {
                return Err(format!(
                    "Unknown kind `{other}`: song, loop, sound or instrument"
                ))
            }
        })
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Song => "song",
            Self::Loop => "loop",
            Self::Sound => "sound",
            Self::Instrument => "instrument",
        }
    }
    /// Shortest and longest length a request may ask for, in seconds.
    fn range(self) -> (f64, f64) {
        match self {
            Self::Song => (5.0, 300.0),
            Self::Loop => (1.0, 60.0),
            Self::Sound => (0.5, 30.0),
            Self::Instrument => (0.5, 10.0),
        }
    }
}

/// A generation ready to send: everything the window's network call needs, decided here so
/// the CLI help, the agent and the window agree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub service: Service,
    pub kind: GenKind,
    /// The person's description.
    pub description: String,
    /// What the service is sent: the description shaped for the kind and the song.
    pub prompt: String,
    pub seconds: f64,
    pub instrumental: bool,
    pub seed: Option<u64>,
    pub name: String,
    pub place: bool,
    pub track_id: Option<String>,
    pub start_bar: Option<f64>,
    pub root_note: u8,
    /// A loop must be exactly this long to sit on the bars: the result is trimmed or padded.
    pub fit_seconds: Option<f64>,
}

/// A generated sound kept on this computer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Generated {
    pub id: String,
    pub name: String,
    pub description: String,
    pub prompt: String,
    pub kind: GenKind,
    pub service: Service,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub model: String,
    pub seconds: f64,
    /// Unix seconds.
    pub created: u64,
    /// File name inside the generated folder.
    pub file: String,
    #[serde(default = "middle_c")]
    pub root_note: u8,
    /// A loop's exact length on the song's bars when it was made; placing trims or pads to it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit_seconds: Option<f64>,
}
fn middle_c() -> u8 {
    60
}

/// Where generated sounds are kept.
pub fn folder() -> PathBuf {
    data_dir().join("generated")
}

/// Shape a `generate.audio` request from its JSON parameters, checked against the registry.
pub fn request_for(settings: &Settings, session: &Session, params: &Value) -> Result<Request> {
    let a = crate::control::args_for("generate.audio", params)?;
    request(settings, session, &a)
}

/// Shape a `generate.audio` request from its arguments, the settings and the song.
pub(crate) fn request(settings: &Settings, session: &Session, a: &Args) -> Result<Request> {
    let description = a.str("prompt")?.trim().to_string();
    if description.is_empty() || description.chars().count() > 2000 {
        return Err("Describe the sound in 1 to 2000 characters".into());
    }
    let kind = GenKind::parse(a.opt_str("kind").unwrap_or("loop"))?;
    let service = match a.opt_str("service") {
        Some(key) => Service::parse(key).ok_or_else(|| {
            format!("Unknown service `{key}`: elevenlabs, stability, fal or custom")
        })?,
        None => settings.generation.service,
    };
    if !settings.generation_ready(service) {
        return Err(format!(
            "{} is not connected. Add its key in Settings > Generation{}.",
            service.label(),
            if service == Service::Custom {
                " (the endpoint address)"
            } else {
                ""
            }
        ));
    }
    let tempo = session.tempo_map().bpm(0.0);
    let bpb = session.beats_per_bar();
    let bars = a.opt_int("bars");
    if bars.is_some() && kind != GenKind::Loop {
        return Err("bars is for loops; give seconds for other kinds".into());
    }
    let fit_seconds = (kind == GenKind::Loop && a.opt_f64("seconds").is_none()).then(|| {
        let bars = bars.unwrap_or(4).clamp(1, 32) as f64;
        bars * bpb * 60.0 / tempo
    });
    let seconds = a.opt_f64("seconds").or(fit_seconds).unwrap_or(match kind {
        GenKind::Song => 60.0,
        _ => 3.0,
    });
    let (min, max) = kind.range();
    if !(min..=max).contains(&seconds) {
        return Err(format!(
            "A {} is {min}-{max} seconds long; this one would be {seconds:.1}",
            kind.key()
        ));
    }
    let follow = a
        .opt_bool("followSong")
        .unwrap_or(matches!(kind, GenKind::Song | GenKind::Loop));
    let mut prompt = description.clone();
    match kind {
        GenKind::Loop => prompt.push_str(". A seamless, perfectly looping musical loop"),
        GenKind::Instrument => prompt.push_str(
            ". One single sustained note at middle C (C4), isolated, dry, no reverb, no other sounds, starting immediately",
        ),
        GenKind::Sound => prompt.push_str(". One isolated sound, starting immediately"),
        GenKind::Song => {}
    }
    if follow && matches!(kind, GenKind::Song | GenKind::Loop) {
        prompt.push_str(&format!(", {} BPM", tempo.round()));
        let key = session.transport.key.trim();
        if !key.is_empty() {
            prompt.push_str(&format!(", in {key}"));
        }
    }
    let root_note = match a.opt_int("rootNote") {
        Some(n) if (0..=127).contains(&n) => n as u8,
        Some(_) => return Err("rootNote must be between 0 and 127".into()),
        None => 60,
    };
    let name = a
        .opt_str("name")
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| name_from(&description));
    if name.chars().count() > 80 {
        return Err("Names are at most 80 characters".into());
    }
    if let Some(track) = a.opt_str("trackId") {
        let t = find_track(session, track)?;
        let wanted = if kind == GenKind::Instrument {
            "midi"
        } else {
            "audio"
        };
        if t.kind != wanted {
            return Err(format!(
                "A generated {} goes on {} track",
                kind.key(),
                if wanted == "midi" {
                    "a MIDI"
                } else {
                    "an audio"
                }
            ));
        }
    }
    Ok(Request {
        service,
        kind,
        description,
        prompt,
        seconds,
        instrumental: a.opt_bool("instrumental").unwrap_or(true),
        seed: a.opt_int("seed").map(|s| s.unsigned_abs()),
        name,
        place: a.opt_bool("place").unwrap_or(true),
        track_id: a.opt_str("trackId").map(str::to_string),
        start_bar: a.opt_f64("startBar"),
        root_note,
        fit_seconds,
    })
}

/// A short title from a description: its first words, capitalised.
fn name_from(description: &str) -> String {
    let words: Vec<&str> = description
        .split(|c: char| c.is_whitespace() || c == ',' || c == '.')
        .filter(|w| !w.is_empty())
        .take(4)
        .collect();
    let mut name = words.join(" ");
    if let Some(first) = name.get(..1) {
        name = first.to_uppercase() + &name[1..];
    }
    if name.is_empty() {
        "Generated".into()
    } else {
        name.chars().take(40).collect()
    }
}

/// Trim or pad a loop to exactly its bars, with 5 ms fades where it was cut.
pub fn fit(buffer: AudioBuffer, seconds: f64) -> Result<AudioBuffer> {
    let want = (seconds * buffer.sample_rate as f64).round() as usize;
    if want == 0 || buffer.frames.len() == want {
        return Ok(buffer);
    }
    let mut frames = buffer.frames;
    let cut = frames.len() > want;
    frames.resize(want, [0.0; 2]);
    if cut {
        let fade = (buffer.sample_rate as usize / 200).min(want);
        for (i, frame) in frames[want - fade..].iter_mut().enumerate() {
            let g = 1.0 - (i + 1) as f32 / fade as f32;
            frame[0] *= g;
            frame[1] *= g;
        }
    }
    AudioBuffer::new(buffer.sample_rate, frames)
}

/// Keep a result: the audio file as the service sent it and a JSON note beside it.
pub fn keep(bytes: &[u8], extension: &str, request: &Request, model: &str) -> Result<Generated> {
    let dir = folder();
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let slug: String = request
        .name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .take(5)
        .collect::<Vec<_>>()
        .join("-");
    let id = format!(
        "{}-{}",
        now.as_millis(),
        if slug.is_empty() { "sound" } else { &slug }
    );
    let extension = match extension {
        "mp3" | "wav" | "flac" | "ogg" | "opus" | "m4a" | "aac" | "aiff" | "webm" => extension,
        _ => "bin",
    };
    let file = format!("{id}.{extension}");
    crate::document::atomic_write(&dir.join(&file), |f| {
        use std::io::Write;
        f.write_all(bytes).map_err(|e| e.to_string())
    })?;
    let generated = Generated {
        id,
        name: request.name.clone(),
        description: request.description.clone(),
        prompt: request.prompt.clone(),
        kind: request.kind,
        service: request.service,
        model: model.into(),
        seconds: request.seconds,
        created: now.as_secs(),
        file,
        root_note: request.root_note,
        fit_seconds: request.fit_seconds,
    };
    let note = serde_json::to_vec_pretty(&generated).map_err(|e| e.to_string())?;
    crate::document::atomic_write(&dir.join(format!("{}.json", generated.id)), |f| {
        use std::io::Write;
        f.write_all(&note).map_err(|e| e.to_string())
    })?;
    Ok(generated)
}

/// Every kept result, newest first.
pub fn list() -> Vec<Generated> {
    let Ok(entries) = std::fs::read_dir(folder()) else {
        return vec![];
    };
    let mut all: Vec<Generated> = entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| std::fs::read(e.path()).ok())
        .filter_map(|bytes| serde_json::from_slice::<Generated>(&bytes).ok())
        .filter(|g| valid_id(&g.id) && folder().join(&g.file).is_file())
        .collect();
    all.sort_by(|a, b| b.created.cmp(&a.created).then_with(|| b.id.cmp(&a.id)));
    all
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 120 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// One kept result by id.
pub fn find(id: &str) -> Result<Generated> {
    if !valid_id(id) {
        return Err(format!("Unknown generated sound `{id}`"));
    }
    list()
        .into_iter()
        .find(|g| g.id == id)
        .ok_or_else(|| format!("Unknown generated sound `{id}`. Use generate.list."))
}

/// A kept result decoded, a loop fitted to its bars unless it becomes an instrument.
pub fn decoded(generated: &Generated, as_instrument: bool) -> Result<AudioBuffer> {
    let buffer = decode_file(&path_of(generated))?;
    match generated.fit_seconds.filter(|_| !as_instrument) {
        Some(seconds) => fit(buffer, seconds),
        None => Ok(buffer),
    }
}

/// The audio file of a kept result.
pub fn path_of(generated: &Generated) -> PathBuf {
    folder().join(&generated.file)
}

/// Put decoded audio in the song the way `kind` asks: an audio clip, or Sample Keys on a
/// MIDI track. One undo step.
#[allow(clippy::too_many_arguments)]
pub fn place(
    host: &mut dyn Host,
    buffer: Arc<AudioBuffer>,
    name: &str,
    as_instrument: bool,
    track_id: Option<&str>,
    start_bar: Option<f64>,
    root_note: u8,
    agent: bool,
) -> Result<Value> {
    if as_instrument {
        load_sample(host, &buffer, name, track_id, root_note)
    } else {
        place_audio(host, buffer, name, track_id, start_bar, agent)
    }
}

/// Sample Keys with `buffer` loaded, on the given MIDI track or a new one. One undo step.
pub fn load_sample(
    host: &mut dyn Host,
    buffer: &AudioBuffer,
    name: &str,
    track_id: Option<&str>,
    root_note: u8,
) -> Result<Value> {
    let state = sample_keys::encode(buffer)?;
    let s = host.store().session();
    let mut commands = vec![];
    let track = match track_id {
        Some(id) => {
            let t = find_track(s, id)?;
            if t.kind != "midi" {
                return Err("An instrument goes on a MIDI track".into());
            }
            t.id.clone()
        }
        None => {
            let t = new_track(
                s,
                "midi",
                Some(name.chars().take(80).collect()),
                TRACK_PALETTE[s.tracks.len() % 8].into(),
            );
            let id = t.id.clone();
            commands.push(Command::AddTrack(t));
            id
        }
    };
    let mut strip = full_strip(s, &track);
    let mut insert = Insert::new(new_id("plugin"), "stock:Sample Keys", "Sample Keys");
    insert.blob = sample_keys::insert_blob(&state, root_note);
    insert
        .params
        .insert(sample_keys::ROOT as u32, f64::from(root_note));
    strip.synth = Some(insert);
    commands.push(Command::SetStrip {
        track: track.clone(),
        strip,
    });
    host.dispatch(Command::Batch(commands))?;
    let mut out = strip_json(host.store().session(), &track);
    out["seconds"] = json!(buffer.duration().min(sample_keys::MAX_SECONDS));
    out["rootNote"] = json!(root_note);
    Ok(out)
}

fn root(a: &Args, default: u8) -> Result<u8> {
    match a.opt_int("rootNote") {
        Some(n) if (0..=127).contains(&n) => Ok(n as u8),
        Some(_) => Err("rootNote must be between 0 and 127".into()),
        None => Ok(default),
    }
}

/// The services, as `generate.services` reports them.
pub fn services(settings: &Settings) -> Value {
    json!({
        "chosen": settings.generation.service.key(),
        "services": Service::ALL.iter().map(|s| json!({
            "id": s.key(),
            "label": s.label(),
            "ready": settings.generation_ready(*s),
            "makes": match s {
                Service::ElevenLabs => "Songs with or without vocals and loops (Eleven Music), sound effects and one-shots up to 30 s".to_string(),
                Service::Stability => "Loops, songs up to 3 minutes and sound design, instrumental".to_string(),
                Service::Fal => format!("Whatever its audio model makes ({})", settings.generation.fal_model),
                Service::Custom => "Whatever your endpoint makes".to_string(),
            },
            "help": s.help_url(),
        })).collect::<Vec<_>>(),
    })
}

pub(crate) fn call(host: &mut dyn Host, name: &str, a: &Args, agent: bool) -> Result<Value> {
    match name {
        "generate.services" => Ok(services(&Settings::load())),
        "generate.audio" => host.live(name, &args_value(a)),
        "generate.list" => {
            let limit = a.opt_int("limit").unwrap_or(50);
            if !(1..=200).contains(&limit) {
                return Err("limit is 1-200".into());
            }
            let all = list();
            Ok(json!({
                "folder": folder(),
                "total": all.len(),
                "sounds": all.into_iter().take(limit as usize).collect::<Vec<_>>(),
            }))
        }
        "generate.preview" => {
            let generated = find(a.str("id")?)?;
            let path = path_of(&generated);
            let bytes =
                std::fs::read(&path).map_err(|e| format!("Could not read the sound: {e}"))?;
            if bytes.len() > 64 * 1024 * 1024 {
                return Err("The sound is too large to preview here".into());
            }
            let mime = match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
                "mp3" => "audio/mpeg",
                "wav" => "audio/wav",
                "flac" => "audio/flac",
                "ogg" | "opus" => "audio/ogg",
                "m4a" | "aac" => "audio/mp4",
                "webm" => "audio/webm",
                _ => "application/octet-stream",
            };
            Ok(json!({
                "id": generated.id,
                "mime": mime,
                "base64": crate::host::encode_blob(&bytes),
            }))
        }
        "generate.place" => {
            let generated = find(a.str("id")?)?;
            let as_instrument = match a.opt_str("as") {
                Some("audio") => false,
                Some("instrument") => true,
                Some(other) => return Err(format!("as is audio or instrument, not `{other}`")),
                None => generated.kind == GenKind::Instrument,
            };
            let buffer = Arc::new(decoded(&generated, as_instrument)?);
            place(
                host,
                buffer,
                &generated.name,
                as_instrument,
                a.opt_str("trackId"),
                a.opt_f64("startBar"),
                root(a, generated.root_note)?,
                agent,
            )
        }
        "generate.delete" => {
            let generated = find(a.str("id")?)?;
            std::fs::remove_file(path_of(&generated))
                .map_err(|e| format!("Could not delete the sound: {e}"))?;
            let _ = std::fs::remove_file(folder().join(format!("{}.json", generated.id)));
            Ok(json!({ "deleted": generated.id }))
        }
        "strip.loadSample" => {
            let (buffer, sound_name) = match (a.opt_str("path"), a.opt_str("sourceId")) {
                (Some(path), None) => {
                    let path = Path::new(path);
                    (
                        Arc::new(decode_file(path)?),
                        path.file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("Sample")
                            .to_string(),
                    )
                }
                (None, Some(source)) => {
                    let s = host.store().session();
                    let found = s
                        .sources
                        .values()
                        .find(|x| x.id == source || x.name == source)
                        .ok_or_else(|| format!("Unknown source `{source}`"))?;
                    let buffer = host
                        .library()
                        .get(&found.id)
                        .cloned()
                        .ok_or("That audio is not loaded")?;
                    (buffer, found.name.clone())
                }
                _ => return Err("Give either path or sourceId".into()),
            };
            let name = a.opt_str("name").unwrap_or(&sound_name).to_string();
            load_sample(host, &buffer, &name, a.opt_str("trackId"), root(a, 60)?)
        }
        _ => Err(format!("Unknown command `{name}`")),
    }
}

fn args_value(a: &Args) -> Value {
    let mut map = serde_json::Map::new();
    for p in a.spec().params {
        if let Some(v) = a.get(p.name) {
            map.insert(p.name.into(), v.clone());
        }
    }
    Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_loop_fits_its_bars_exactly() {
        let buffer = AudioBuffer::new(8000, vec![[0.5; 2]; 20_000]).unwrap();
        assert_eq!(fit(buffer, 2.0).unwrap().frames.len(), 16_000);
        let short = AudioBuffer::new(8000, vec![[0.5; 2]; 12_000]).unwrap();
        let padded = fit(short, 2.0).unwrap();
        assert_eq!(padded.frames.len(), 16_000);
        assert_eq!(padded.frames[15_999], [0.0; 2]);
    }

    #[test]
    fn names_come_from_the_first_words() {
        assert_eq!(
            name_from("warm rhodes chords, lazy and dusty"),
            "Warm rhodes chords lazy"
        );
        assert_eq!(name_from("   "), "Generated");
    }
}
