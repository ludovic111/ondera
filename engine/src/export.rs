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
        }
    }
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportReport {
    pub path: PathBuf,
    pub sample_rate: u32,
    pub format: SampleFormat,
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
    let seconds_per_beat = 60.0 / song.transport.tempo;
    let frames = (((end - start) * seconds_per_beat + options.tail_seconds)
        * options.sample_rate as f64)
        .ceil() as u64;
    let mut report = ExportReport {
        path: path.into(),
        sample_rate: options.sample_rate,
        format: options.format,
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
        let mut writer = hound::WavWriter::new(std::io::BufWriter::new(file), spec)
            .map_err(|e| e.to_string())?;
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
                match options.format {
                    SampleFormat::Float32 => writer.write_sample(*sample),
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
                        writer.write_sample(pcm)
                    }
                }
                .map_err(|e| e.to_string())?;
            }
            written += n as u64;
        }
        if renderer.voice_overflows > 0 || renderer.note_overflows > 0 {
            return Err(format!("Export exceeded renderer capacity ({} voice and {} note events); no output was replaced",renderer.voice_overflows,renderer.note_overflows));
        }
        writer.finalize().map_err(|e| e.to_string())
    })?;
    if report.clipped_samples > 0 {
        report.warnings.push(format!("{} samples exceeded integer PCM headroom and were clipped; lower the master or export float32.",report.clipped_samples));
    }
    if options.format == SampleFormat::Float32 && report.peak > 1.0 {
        report.warnings.push("Floating-point audio retains levels above 0 dBFS; downstream integer conversion needs attenuation.".into());
    }
    Ok(report)
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
            "{:02}-{}.wav",
            index + 1,
            if clean.is_empty() { "Track" } else { &clean }
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
        std::fs::rename(source, destination).map_err(|e| format!("Could not publish stems: {e}"))
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
