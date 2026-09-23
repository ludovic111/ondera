//! Streaming mix/stem exports using the same plugin graph as live playback.
//! Range exports pre-roll from the start so sustained notes and effect tails are
//! correct at the requested boundary. Destinations are published only on success.
use crate::{
    audio::{self, Library},
    document,
    model::*,
    plugin::MAX_BLOCK,
    render, Result,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SampleFormat {
    Pcm16,
    #[default]
    Pcm24,
    Float32,
}
impl SampleFormat {
    fn bits(self) -> u16 {
        match self {
            Self::Pcm16 => 16,
            Self::Pcm24 => 24,
            Self::Float32 => 32,
        }
    }
}
/// The file written. A mix takes it from the destination's extension; stems, which are
/// given a folder, from `ExportOptions::container`.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Container {
    #[default]
    Wav,
    Aiff,
    Flac,
    /// Ogg Vorbis, lossy. Encodes the float mix at [`ExportOptions::quality`].
    Ogg,
}
impl Container {
    pub fn of(path: &Path) -> Self {
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase);
        match extension.as_deref() {
            Some("aif" | "aiff") => Self::Aiff,
            Some("flac") => Self::Flac,
            Some("ogg" | "oga") => Self::Ogg,
            _ => Self::Wav,
        }
    }
    /// The container for a mix written to `path`, refusing an extension Ondera does not write
    /// (a `.mp3` must not quietly hold WAV). No extension means WAV.
    pub fn for_path(path: &Path) -> Result<Self> {
        match path.extension().and_then(|e| e.to_str()) {
            None => Ok(Self::Wav),
            Some(ext) if ext.eq_ignore_ascii_case("wav") || ext.eq_ignore_ascii_case("wave") => {
                Ok(Self::Wav)
            }
            Some(ext) => match Self::of(path) {
                Self::Wav => Err(format!(
                    "Ondera exports .wav, .aiff, .flac or .ogg files, not .{ext}"
                )),
                container => Ok(container),
            },
        }
    }
    pub fn parse(text: &str) -> Result<Self> {
        match text.to_ascii_lowercase().as_str() {
            "wav" => Ok(Self::Wav),
            "aiff" | "aif" => Ok(Self::Aiff),
            "flac" => Ok(Self::Flac),
            "ogg" | "vorbis" => Ok(Self::Ogg),
            other => Err(format!(
                "Container must be wav, aiff, flac or ogg, not {other}"
            )),
        }
    }
    pub fn extension(self) -> &'static str {
        match self {
            Self::Wav => "wav",
            Self::Aiff => "aiff",
            Self::Flac => "flac",
            Self::Ogg => "ogg",
        }
    }
}
/// Vorbis quality when none is given: about 192 kbit/s for a stereo mix.
pub const DEFAULT_QUALITY: f64 = 0.6;
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportOptions {
    pub sample_rate: u32,
    pub format: SampleFormat,
    pub start_bar: Option<f64>,
    pub end_bar: Option<f64>,
    pub start_beat: Option<f64>,
    pub end_beat: Option<f64>,
    pub tail_seconds: f64,
    /// Triangular dither at one LSB for integer PCM. Floating-point ignores it.
    pub dither: bool,
    /// File type of each stem. A mix follows its destination's extension instead.
    pub container: Container,
    /// Ogg Vorbis quality, 0 (about 64 kbit/s) to 1 (about 500 kbit/s). Ignored by the
    /// lossless containers; `format` and `dither` are ignored by Ogg.
    pub quality: f64,
}
impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            format: SampleFormat::Pcm24,
            start_bar: None,
            end_bar: None,
            start_beat: None,
            end_beat: None,
            tail_seconds: 3.0,
            dither: true,
            container: Container::Wav,
            quality: DEFAULT_QUALITY,
        }
    }
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportReport {
    pub path: PathBuf,
    pub container: Container,
    pub sample_rate: u32,
    pub format: SampleFormat,
    /// Ogg only: the Vorbis quality and the average bitrate it came to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kbps: Option<f64>,
    pub frames: u64,
    pub seconds: f64,
    pub start_beat: f64,
    pub end_beat: f64,
    pub tail_seconds: f64,
    pub peak: f64,
    pub clipped_samples: u64,
    pub warnings: Vec<String>,
}
impl ExportOptions {
    pub fn range(&self, session: &Session) -> Result<(f64, f64)> {
        if ![44100, 48000, 96000].contains(&self.sample_rate) {
            return Err("Export sampleRate must be 44100, 48000 or 96000".into());
        }
        if !self.tail_seconds.is_finite() || !(0.0..=120.0).contains(&self.tail_seconds) {
            return Err("Export tailSeconds must be between 0 and 120".into());
        }
        if !self.quality.is_finite() || !(0.0..=1.0).contains(&self.quality) {
            return Err("Export quality must be between 0 and 1".into());
        }
        if (self.start_bar.is_some() || self.end_bar.is_some())
            && (self.start_beat.is_some() || self.end_beat.is_some())
        {
            return Err("Use either bar range or beat range, not both".into());
        }
        let start = self
            .start_beat
            .or_else(|| self.start_bar.map(|v| v * session.beats_per_bar()))
            .unwrap_or(0.0);
        let end = self
            .end_beat
            .or_else(|| self.end_bar.map(|v| v * session.beats_per_bar()))
            .unwrap_or_else(|| session.end_bar() * session.beats_per_bar());
        if !valid_time(start) || !valid_time(end) || end <= start {
            return Err("Export range needs finite end > start >= 0".into());
        }
        if end * 60.0 / session.transport.tempo + self.tail_seconds > 14400.0 {
            return Err("Export, including pre-roll and tail, is limited to four hours".into());
        }
        let frames = (((end - start) * 60.0 / session.transport.tempo + self.tail_seconds)
            * self.sample_rate as f64)
            .ceil() as u64;
        if frames.saturating_mul(2 * self.format.bits() as u64 / 8) > u32::MAX as u64 - 128 {
            return Err(
                "Export exceeds the WAV 4 GiB limit; select a shorter range or lower sample format"
                    .into(),
            );
        }
        Ok((start, end))
    }
}

/// Atomic WAV export. PCM is dithered and clipped only at integer conversion;
/// float preserves headroom above 0 dBFS. No implicit normalization is applied.
pub fn mix(
    session: &Session,
    library: &Library,
    path: &Path,
    options: &ExportOptions,
) -> Result<ExportReport> {
    session.validate()?;
    Container::for_path(path)?;
    let (start, end) = options.range(session)?;
    let mut song = session.clone();
    song.transport.cycle = false;
    song.transport.metronome = false;
    let bpb = song.beats_per_bar();
    song.clips.retain(|clip| clip.start_bar * bpb < end);
    for clip in &mut song.clips {
        clip.length_bars = clip.length_bars.min(end / bpb - clip.start_bar);
    }
    crate::automation::hold_after(&mut song, end);
    let mut library = library.clone();
    audio::prepare_sources(&song, &mut library)?;
    let (mut renderer, mut rack) = render::offline(&song, &library, options.sample_rate)?;
    renderer.playing = true;
    renderer.locate(0.0);
    let plugin_tail = rack
        .longest_tail()
        .map(|(tail, name)| (tail, name.to_string()));
    let seconds_per_beat = 60.0 / song.transport.tempo;
    let frames = (((end - start) * seconds_per_beat + options.tail_seconds)
        * options.sample_rate as f64)
        .ceil() as u64;
    let container = Container::of(path);
    let lossy = container == Container::Ogg;
    let mut report = ExportReport {
        path: path.into(),
        container,
        sample_rate: options.sample_rate,
        format: options.format,
        quality: lossy.then_some(options.quality),
        kbps: None,
        frames,
        seconds: frames as f64 / options.sample_rate as f64,
        start_beat: start,
        end_beat: end,
        tail_seconds: options.tail_seconds,
        peak: 0.0,
        clipped_samples: 0,
        warnings: vec![],
    };
    document::atomic_write(path, |file| {
        let mut writer = Sink::open(file, path, options, frames, &song.name)?;
        let mut block = [[0.0f32; 2]; MAX_BLOCK];
        let mut skip = (start * seconds_per_beat * options.sample_rate as f64).round() as u64
            + renderer.latency_samples() as u64;
        while skip > 0 {
            let n = skip.min(MAX_BLOCK as u64) as usize;
            renderer.render(&mut rack, &mut block[..n]);
            rack.idle();
            skip -= n as u64;
        }
        let mut written = 0;
        let mut rng = 0xa734_fd21u32;
        while written < frames {
            let n = (frames - written).min(MAX_BLOCK as u64) as usize;
            renderer.render(&mut rack, &mut block[..n]);
            rack.idle();
            for sample in block[..n].iter().flatten() {
                if !sample.is_finite() {
                    return Err(
                        "A plugin produced non-finite audio; export was not published".into(),
                    );
                }
                let value = *sample as f64;
                report.peak = report.peak.max(value.abs());
                if lossy {
                    // Vorbis keeps the float mix; players clip what is above full scale.
                    if value.abs() > 1.0 {
                        report.clipped_samples += 1;
                    }
                    continue;
                }
                match options.format {
                    SampleFormat::Float32 => writer.float(*sample),
                    format => {
                        if value.abs() > 1.0 {
                            report.clipped_samples += 1;
                        }
                        let scale = if format == SampleFormat::Pcm16 {
                            32768.0
                        } else {
                            8_388_608.0
                        };
                        let noise = if options.dither {
                            uniform(&mut rng) - uniform(&mut rng)
                        } else {
                            0.0
                        };
                        let pcm = (value.clamp(-1.0, 1.0) * scale + noise)
                            .round()
                            .clamp(-scale, scale - 1.0) as i32;
                        writer.int(pcm)
                    }
                }?;
            }
            if lossy {
                writer.block(&block[..n])?;
            }
            written += n as u64;
        }
        if renderer.voice_overflows > 0 || renderer.note_overflows > 0 {
            return Err(format!("Export exceeded renderer capacity ({} voice and {} note events); no output was replaced",renderer.voice_overflows,renderer.note_overflows));
        }
        writer.finish()
    })?;
    if let Some((tail, name)) = plugin_tail.filter(|(tail, _)| *tail > options.tail_seconds) {
        report.warnings.push(if tail.is_finite() {
            format!("{name} rings for {tail:.2} s but the export's tail is {:.1} s; raise tailSeconds to keep its release.", options.tail_seconds)
        } else {
            format!("{name} never falls silent on its own; the export stops it after {:.1} s.", options.tail_seconds)
        });
    }
    if lossy {
        let bytes = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
        report.kbps = Some((bytes as f64 * 8.0 / report.seconds.max(1e-9) / 100.0).round() / 10.0);
        if report.clipped_samples > 0 {
            report.warnings.push(format!(
                "{} samples are above full scale and will clip when played; lower the master.",
                report.clipped_samples
            ));
        }
    } else if report.clipped_samples > 0 {
        report.warnings.push(format!("{} samples exceeded integer PCM headroom and were clipped; lower the master or export float32.",report.clipped_samples));
    }
    if !lossy && options.format == SampleFormat::Float32 && report.peak > 1.0 {
        report.warnings.push("Floating-point audio retains levels above 0 dBFS; downstream integer conversion needs attenuation.".into());
    }
    Ok(report)
}
/// Where rendered samples go. The container follows the file extension: `.aif`/`.aiff`
/// write AIFF, `.flac` FLAC, `.ogg` Ogg Vorbis, anything else WAV. All stream, so an export
/// never holds the song in memory.
enum Sink<'a> {
    Flac(Box<crate::flac::Encoder<std::io::BufWriter<&'a mut std::fs::File>>>),
    Ogg(Box<Vorbis<'a>>),
    Wav(hound::WavWriter<std::io::BufWriter<&'a mut std::fs::File>>),
    Aiff {
        out: std::io::BufWriter<&'a mut std::fs::File>,
        bytes: usize,
    },
}
pub fn is_aiff(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| ["aif", "aiff"].contains(&e.to_ascii_lowercase().as_str()))
}
/// A sample rate as the 80-bit extended float an AIFF COMM chunk wants.
fn extended(rate: u32) -> [u8; 10] {
    let mut out = [0u8; 10];
    if rate == 0 {
        return out;
    }
    let shift = rate.leading_zeros();
    let exponent = 16383 + 31 - shift as u16;
    out[..2].copy_from_slice(&exponent.to_be_bytes());
    out[2..6].copy_from_slice(&(rate << shift).to_be_bytes());
    out
}
/// A streaming Ogg Vorbis encoder fed one render block at a time.
struct Vorbis<'a> {
    encoder: vorbis_rs::VorbisEncoder<std::io::BufWriter<&'a mut std::fs::File>>,
    left: Vec<f32>,
    right: Vec<f32>,
}
impl<'a> Vorbis<'a> {
    fn new(file: &'a mut std::fs::File, options: &ExportOptions, title: &str) -> Result<Self> {
        use std::num::{NonZeroU32, NonZeroU8};
        let rate = NonZeroU32::new(options.sample_rate).ok_or("Export needs a sample rate")?;
        // The Ogg stream serial only has to differ between streams chained in one file.
        let serial = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0x0dea, |time| time.subsec_nanos() as i32);
        let mut builder = vorbis_rs::VorbisEncoderBuilder::new_with_serial(
            rate,
            NonZeroU8::new(2).expect("two channels"),
            std::io::BufWriter::new(file),
            serial,
        );
        builder.bitrate_management_strategy(
            vorbis_rs::VorbisBitrateManagementStrategy::QualityVbr {
                target_quality: options.quality as f32,
            },
        );
        if !title.trim().is_empty() {
            builder
                .comment_tag("TITLE", title.trim())
                .map_err(|e| format!("Ogg Vorbis title: {e}"))?;
        }
        let encoder = builder.build().map_err(|e| format!("Ogg Vorbis: {e}"))?;
        Ok(Self {
            encoder,
            left: Vec::with_capacity(MAX_BLOCK),
            right: Vec::with_capacity(MAX_BLOCK),
        })
    }
    fn block(&mut self, frames: &[[f32; 2]]) -> Result<()> {
        self.left.clear();
        self.right.clear();
        for frame in frames {
            self.left.push(frame[0]);
            self.right.push(frame[1]);
        }
        self.encoder
            .encode_audio_block([&self.left[..], &self.right[..]])
            .map_err(|e| format!("Ogg Vorbis: {e}"))
    }
    fn finish(self) -> Result<()> {
        use std::io::Write;
        self.encoder
            .finish()
            .map_err(|e| format!("Ogg Vorbis: {e}"))?
            .flush()
            .map_err(|e| e.to_string())
    }
}
impl<'a> Sink<'a> {
    fn open(
        file: &'a mut std::fs::File,
        path: &Path,
        options: &ExportOptions,
        frames: u64,
        title: &str,
    ) -> Result<Self> {
        use std::io::Write;
        if Container::of(path) == Container::Ogg {
            return Vorbis::new(file, options, title).map(|ogg| Sink::Ogg(Box::new(ogg)));
        }
        if Container::of(path) == Container::Flac {
            if options.format == SampleFormat::Float32 {
                return Err("FLAC holds 16 or 24-bit audio here; choose pcm16 or pcm24, or export float32 as WAV".into());
            }
            return crate::flac::Encoder::new(
                std::io::BufWriter::new(file),
                options.sample_rate,
                options.format.bits() as u32,
            )
            .map(|encoder| Sink::Flac(Box::new(encoder)));
        }
        if !is_aiff(path) {
            let spec = hound::WavSpec {
                channels: 2,
                sample_rate: options.sample_rate,
                bits_per_sample: options.format.bits(),
                sample_format: if options.format == SampleFormat::Float32 {
                    hound::SampleFormat::Float
                } else {
                    hound::SampleFormat::Int
                },
            };
            return hound::WavWriter::new(std::io::BufWriter::new(file), spec)
                .map(Sink::Wav)
                .map_err(|e| e.to_string());
        }
        if options.format == SampleFormat::Float32 {
            return Err("AIFF holds 16 or 24-bit audio here; choose pcm16 or pcm24, or export float32 as WAV".into());
        }
        let bytes = options.format.bits() as usize / 8;
        let data = frames
            .checked_mul(2 * bytes as u64)
            .filter(|n| *n < u32::MAX as u64 - 64)
            .ok_or("This export is too long for one AIFF file; export WAV or a shorter range")?;
        let frames = u32::try_from(frames).map_err(|_| "This export is too long for AIFF")?;
        let mut out = std::io::BufWriter::new(file);
        let mut header = Vec::with_capacity(54);
        header.extend_from_slice(b"FORM");
        header.extend_from_slice(&(4 + 26 + 16 + data as u32).to_be_bytes());
        header.extend_from_slice(b"AIFFCOMM");
        header.extend_from_slice(&18u32.to_be_bytes());
        header.extend_from_slice(&2u16.to_be_bytes());
        header.extend_from_slice(&frames.to_be_bytes());
        header.extend_from_slice(&options.format.bits().to_be_bytes());
        header.extend_from_slice(&extended(options.sample_rate));
        header.extend_from_slice(b"SSND");
        header.extend_from_slice(&(8 + data as u32).to_be_bytes());
        header.extend_from_slice(&[0; 8]);
        out.write_all(&header).map_err(|e| e.to_string())?;
        Ok(Sink::Aiff { out, bytes })
    }
    fn int(&mut self, pcm: i32) -> Result<()> {
        use std::io::Write;
        match self {
            Sink::Wav(writer) => writer.write_sample(pcm).map_err(|e| e.to_string()),
            Sink::Flac(encoder) => encoder.write(pcm),
            Sink::Ogg(_) => Err("Ogg export takes whole blocks".into()),
            Sink::Aiff { out, bytes } => {
                let be = pcm.to_be_bytes();
                out.write_all(&be[4 - *bytes..]).map_err(|e| e.to_string())
            }
        }
    }
    fn float(&mut self, sample: f32) -> Result<()> {
        match self {
            Sink::Wav(writer) => writer.write_sample(sample).map_err(|e| e.to_string()),
            Sink::Aiff { .. } => Err("AIFF export is integer PCM".into()),
            Sink::Flac(_) => Err("FLAC export is integer PCM".into()),
            Sink::Ogg(_) => Err("Ogg export takes whole blocks".into()),
        }
    }
    fn block(&mut self, frames: &[[f32; 2]]) -> Result<()> {
        match self {
            Sink::Ogg(ogg) => ogg.block(frames),
            _ => Err("Only Ogg export takes whole blocks".into()),
        }
    }
    fn finish(self) -> Result<()> {
        use std::io::Write;
        match self {
            Sink::Wav(writer) => writer.finalize().map_err(|e| e.to_string()),
            Sink::Flac(encoder) => encoder.finish(),
            Sink::Ogg(ogg) => ogg.finish(),
            Sink::Aiff { mut out, .. } => out.flush().map_err(|e| e.to_string()),
        }
    }
}
fn uniform(state: &mut u32) -> f64 {
    *state = state.wrapping_mul(1664525).wrapping_add(1013904223);
    (*state >> 8) as f64 / 16777216.0
}

pub fn select_tracks<'a>(session: &'a Session, ids: Option<&[String]>) -> Result<Vec<&'a Track>> {
    match ids {
        None => Ok(session.tracks.iter().collect()),
        Some(ids) => {
            if ids.is_empty() {
                return Err("trackIds must contain at least one track".into());
            }
            let mut seen = HashSet::new();
            for id in ids {
                if !seen.insert(id) {
                    return Err(format!("Duplicate track ID `{id}`"));
                }
                if !session.tracks.iter().any(|t| &t.id == id) {
                    return Err(format!("Unknown track `{id}`"));
                }
            }
            Ok(session
                .tracks
                .iter()
                .filter(|t| seen.contains(&t.id))
                .collect())
        }
    }
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StemReport {
    pub directory: PathBuf,
    pub include_effects: bool,
    pub include_master: bool,
    pub files: Vec<ExportReport>,
    pub warnings: Vec<String>,
}

/// Render every stem into a private sibling directory, then publish the complete
/// folder in one rename. Existing destinations are never replaced.
pub fn stems(
    session: &Session,
    library: &Library,
    directory: &Path,
    options: &ExportOptions,
    track_ids: Option<&[String]>,
    include_effects: bool,
    include_master: bool,
) -> Result<StemReport> {
    session.validate()?;
    options.range(session)?;
    if directory.exists() {
        return Err("Stem destination already exists; choose a new folder".into());
    }
    let parent = directory
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let staging = tempfile::Builder::new()
        .prefix(".ondera-stems-")
        .tempdir_in(parent)
        .map_err(|e| e.to_string())?;
    let tracks = select_tracks(session, track_ids)?;
    if tracks.is_empty() {
        return Err("There are no tracks to export".into());
    }
    let mut files = vec![];
    let (start, end) = options.range(session)?;
    let mut resolved = options.clone();
    resolved.start_bar = None;
    resolved.end_bar = None;
    resolved.start_beat = Some(start);
    resolved.end_beat = Some(end);
    for (index, track) in tracks.iter().enumerate() {
        let mut song = session.clone();
        // The title an Ogg stem carries.
        song.name = format!("{} - {}", session.name, track.name);
        song.tracks.retain(|t| t.id == track.id);
        song.tracks[0].mute = false;
        song.tracks[0].solo = false;
        song.clips.retain(|c| c.track_id == track.id);
        song.strips.retain(|id, _| id == &track.id || is_bus(id));
        if !include_effects {
            for (id, strip) in &mut song.strips {
                if id != MASTER {
                    strip.inserts.clear();
                    strip.sends.clear();
                }
            }
        }
        if !include_master {
            song.strips.remove(MASTER);
            song.master_volume = 0.75;
            song.automation
                .retain(|lane| lane.target != crate::automation::AutomationTarget::MasterVolume);
        }
        song.view.selected_track_id = Some(track.id.clone());
        song.view.selected_clip_id = None;
        song.view.editor_clip_id = None;
        song.view.selected_note_id = None;
        crate::automation::retain_targets(&mut song);
        let clean: String = track
            .name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .take(80)
            .collect();
        let filename = format!(
            "{:02}-{}.{}",
            index + 1,
            if clean.is_empty() { "Track" } else { &clean },
            options.container.extension()
        );
        let mut report = mix(&song, library, &staging.path().join(&filename), &resolved)?;
        report.path = directory.join(&filename);
        files.push(report);
    }
    let mut warnings=vec!["Stems ignore track mute/solo and share the same range. Faders, pans and instruments are retained.".into()];
    if include_effects || include_master {
        warnings.push("Solo-rendered effects, shared buses and nonlinear or time-varying processors can prevent stems from summing exactly to the full mix. Export the mix separately as a reference.".into());
    }
    let report = StemReport {
        directory: directory.into(),
        include_effects,
        include_master,
        files,
        warnings,
    };
    let manifest = serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?;
    document::atomic_write(&staging.path().join("manifest.json"), |file| {
        use std::io::Write;
        file.write_all(&manifest).map_err(|e| e.to_string())
    })?;
    if directory.exists() {
        return Err("Stem destination appeared during export; nothing was replaced".into());
    }
    publish_directory(staging.path(), directory)?;
    #[cfg(unix)]
    std::fs::File::open(parent)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())?;
    Ok(report)
}

/// Filesystem-level exclusive rename closes the race between checking a target
/// folder and publishing it. A concurrently created folder remains untouched.
fn publish_directory(source: &Path, destination: &Path) -> Result<()> {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        use std::{
            ffi::{c_char, c_int, c_uint, CString},
            os::unix::ffi::OsStrExt,
        };
        let source = CString::new(source.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
        let destination =
            CString::new(destination.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
        #[cfg(target_os = "macos")]
        unsafe extern "C" {
            fn renamex_np(from: *const c_char, to: *const c_char, flags: c_uint) -> c_int;
        }
        #[cfg(target_os = "linux")]
        unsafe extern "C" {
            fn renameat2(
                from_dir: c_int,
                from: *const c_char,
                to_dir: c_int,
                to: *const c_char,
                flags: c_uint,
            ) -> c_int;
        }
        // Both C strings remain live through the call. RENAME_EXCL (Darwin)
        // and RENAME_NOREPLACE (Linux) require a nonexistent destination.
        #[cfg(target_os = "macos")]
        let code = unsafe { renamex_np(source.as_ptr(), destination.as_ptr(), 4) };
        #[cfg(target_os = "linux")]
        let code = unsafe { renameat2(-100, source.as_ptr(), -100, destination.as_ptr(), 1) };
        if code == 0 {
            Ok(())
        } else {
            Err(format!(
                "Could not publish stems without replacing another folder: {}",
                std::io::Error::last_os_error()
            ))
        }
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "kernel32")]
        unsafe extern "system" {
            #[link_name = "MoveFileExW"]
            fn move_file_ex_w(source: *const u16, destination: *const u16, flags: u32) -> i32;
        }
        fn wide(path: &Path) -> Result<Vec<u16>> {
            let mut value: Vec<u16> = path.as_os_str().encode_wide().collect();
            if value.contains(&0) {
                return Err("Stem destination contains a null character".into());
            }
            value.push(0);
            Ok(value)
        }
        // Canonicalizing the existing source/parent also supplies Windows'
        // extended-length prefix without requiring the destination to exist.
        let source = std::fs::canonicalize(source).map_err(|e| e.to_string())?;
        let parent = destination
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let name = destination
            .file_name()
            .ok_or("Stem destination needs a folder name")?;
        let destination = std::fs::canonicalize(parent)
            .map_err(|e| e.to_string())?
            .join(name);
        let (source, destination) = (wide(&source)?, wide(&destination)?);
        // Both null-terminated UTF-16 buffers stay live. Flags are zero:
        // MOVEFILE_REPLACE_EXISTING and cross-volume copy are intentionally absent.
        // Unlike std::fs::rename, this fails if even an empty target folder exists.
        let code = unsafe { move_file_ex_w(source.as_ptr(), destination.as_ptr(), 0) };
        if code != 0 {
            Ok(())
        } else {
            Err(format!(
                "Could not publish stems without replacing another folder: {}",
                std::io::Error::last_os_error()
            ))
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = (source, destination);
        Err("Atomic stem folder export is unsupported on this platform".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_mix_path_names_a_container_ondera_writes() {
        let of = |p: &str| Container::for_path(Path::new(p));
        assert_eq!(of("song").unwrap(), Container::Wav);
        assert_eq!(of("song.WAV").unwrap(), Container::Wav);
        assert_eq!(of("song.flac").unwrap(), Container::Flac);
        assert_eq!(of("song.ogg").unwrap(), Container::Ogg);
        assert_eq!(of("song.aif").unwrap(), Container::Aiff);
        let refused = of("song.mp3").unwrap_err().to_string();
        assert!(refused.contains(".mp3"), "{refused}");
        let session = crate::store::empty();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("song.mp3");
        assert!(mix(
            &session,
            &Library::default(),
            &path,
            &ExportOptions::default()
        )
        .is_err());
        assert!(!path.exists());
    }
    #[test]
    fn exclusive_stem_publish_preserves_a_concurrent_destination() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let target = dir.path().join("target");
        std::fs::create_dir(&source).unwrap();
        std::fs::write(source.join("render.wav"), b"complete export").unwrap();
        std::fs::create_dir(&target).unwrap();
        assert!(publish_directory(&source, &target).is_err());
        assert!(source.join("render.wav").exists());
        assert_eq!(std::fs::read_dir(&target).unwrap().count(), 0);
        std::fs::remove_dir(&target).unwrap();
        publish_directory(&source, &target).unwrap();
        assert_eq!(
            std::fs::read(target.join("render.wav")).unwrap(),
            b"complete export"
        );
    }
}
