use crate::{model::Source, Result};
use std::{collections::HashMap, io::Cursor, sync::Arc};
use symphonia::core::{
    audio::SampleBuffer, codecs::DecoderOptions, formats::FormatOptions, io::MediaSourceStream,
    meta::MetadataOptions, probe::Hint,
};

pub const MAX_AUDIO_BYTES: usize = 512 * 1024 * 1024;
pub const MAX_LIBRARY_BYTES: usize = 1024 * 1024 * 1024;
#[derive(Debug)]
pub struct AudioBuffer {
    pub sample_rate: u32,
    pub frames: Vec<[f32; 2]>,
    pub peaks: Vec<f32>,
}
pub type Library = HashMap<String, Arc<AudioBuffer>>;
pub fn library_bytes(library: &Library) -> usize {
    library
        .values()
        .map(|b| b.frames.len() * 8 + b.peaks.len() * 4)
        .sum()
}

impl AudioBuffer {
    pub fn new(sample_rate: u32, mut frames: Vec<[f32; 2]>) -> Result<Self> {
        if !(8_000..=384_000).contains(&sample_rate)
            || frames.is_empty()
            || frames.len() > MAX_AUDIO_BYTES / 8
        {
            return Err("Audio size or sample rate is unsupported".into());
        }
        for frame in &mut frames {
            for s in frame {
                if !s.is_finite() {
                    *s = 0.0;
                }
            }
        }
        let chunk = (sample_rate / 400).max(1) as usize;
        let peaks = frames
            .chunks(chunk)
            .map(|fs| {
                fs.iter()
                    .flat_map(|f| f.iter())
                    .fold(0.0_f32, |a, x| a.max(x.abs()))
            })
            .collect();
        Ok(Self {
            sample_rate,
            frames,
            peaks,
        })
    }
    pub fn duration(&self) -> f64 {
        self.frames.len() as f64 / self.sample_rate as f64
    }
    pub fn sample(&self, seconds: f64) -> [f32; 2] {
        if seconds < 0.0 {
            return [0.0; 2];
        }
        let pos = seconds * self.sample_rate as f64;
        let i = pos as usize;
        let Some(a) = self.frames.get(i) else {
            return [0.0; 2];
        };
        let b = self.frames.get(i + 1).unwrap_or(a);
        let t = (pos - i as f64) as f32;
        [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
    }
}

/// Every extension the importer accepts, for file pickers and drag-and-drop. The decoder
/// probes content, so a correct file with an unusual extension still opens from the CLI.
pub const IMPORT_EXTENSIONS: &[&str] = &[
    "wav", "wave", "aif", "aiff", "aifc", "flac", "mp3", "mp2", "ogg", "oga", "m4a", "m4b", "mp4",
    "aac", "caf", "mka", "mkv", "webm",
];
pub fn is_importable(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| IMPORT_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Fold one interleaved frame of any channel count to stereo. Mono goes to both sides;
/// 5.1 and 7.1 follow the ITU-R BS.775 downmix (centre and surrounds at -3 dB, LFE dropped);
/// any other layout alternates channels left and right.
fn to_stereo(frame: &[f32]) -> [f32; 2] {
    const H: f32 = std::f32::consts::FRAC_1_SQRT_2;
    match frame.len() {
        1 => [frame[0], frame[0]],
        2 => [frame[0], frame[1]],
        // L R C LFE Ls Rs (and Lb Rb for 7.1), scaled so full-scale input cannot clip.
        6 => {
            let n = 1.0 / (1.0 + 2.0 * H);
            [
                (frame[0] + H * frame[2] + H * frame[4]) * n,
                (frame[1] + H * frame[2] + H * frame[5]) * n,
            ]
        }
        8 => {
            let n = 1.0 / (1.0 + 3.0 * H);
            [
                (frame[0] + H * (frame[2] + frame[4] + frame[6])) * n,
                (frame[1] + H * (frame[2] + frame[5] + frame[7])) * n,
            ]
        }
        count => {
            let (mut l, mut r) = (0.0, 0.0);
            for (i, v) in frame.iter().enumerate() {
                if i % 2 == 0 {
                    l += v;
                } else {
                    r += v;
                }
            }
            [l / count.div_ceil(2) as f32, r / (count / 2).max(1) as f32]
        }
    }
}

pub fn decode(data: Vec<u8>, extension: Option<&str>) -> Result<AudioBuffer> {
    if data.len() > MAX_AUDIO_BYTES {
        return Err("Audio file exceeds 512 MiB".into());
    }
    let mut hint = Hint::new();
    if let Some(ext) = extension {
        hint.with_extension(ext);
    }
    let stream = MediaSourceStream::new(Box::new(Cursor::new(data)), Default::default());
    use symphonia::core::{codecs::CODEC_TYPE_NULL, errors::Error};
    let mut format = symphonia::default::get_probe()
        .format(
            &hint,
            stream,
            // Without gapless, MP3 and AAC keep the encoder's priming samples and a loop
            // starts some 25-50 ms late.
            &FormatOptions {
                enable_gapless: true,
                ..Default::default()
            },
            &MetadataOptions::default(),
        )
        .map_err(|e| match e {
            Error::Unsupported(_) => {
                "Not an audio file Ondera can read (WAV, AIFF, FLAC, MP3, Ogg, AAC/M4A, CAF)"
                    .to_string()
            }
            other => other.to_string(),
        })?
        .format;
    // A video file lists its picture track too, often first: take the first one with sound.
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .or_else(|| format.default_track())
        .ok_or("No audio track in this file")?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| match e {
            Error::Unsupported(_) => "This file's audio codec is not supported".to_string(),
            other => other.to_string(),
        })?;
    let mut rate = track.codec_params.sample_rate.unwrap_or(48000);
    let mut frames = Vec::new();
    // A damaged frame in a downloaded MP3 is skipped, as players do; a file that is mostly
    // damage is refused.
    let mut damaged = 0usize;
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.to_string()),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(Error::DecodeError(_)) | Err(Error::IoError(_)) if damaged < 100 => {
                damaged += 1;
                continue;
            }
            Err(e) => return Err(e.to_string()),
        };
        if !frames.is_empty() && rate != decoded.spec().rate {
            return Err("Changing sample rate within a file is unsupported".into());
        }
        rate = decoded.spec().rate;
        let channels = decoded.spec().channels.count();
        if channels == 0 || channels > 32 {
            return Err("This file reports an unusable channel count".into());
        }
        let mut samples = SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
        samples.copy_interleaved_ref(decoded);
        if frames.len() + samples.len() / channels > MAX_AUDIO_BYTES / 8 {
            return Err("Decoded audio exceeds 512 MiB".into());
        }
        frames.extend(samples.samples().chunks_exact(channels).map(to_stereo));
    }
    AudioBuffer::new(rate, frames)
}

pub fn encode_wav(buffer: &AudioBuffer) -> Result<Vec<u8>> {
    let mut cursor = Cursor::new(Vec::new());
    {
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: buffer.sample_rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::new(&mut cursor, spec).map_err(|e| e.to_string())?;
        for f in &buffer.frames {
            for &s in f {
                writer.write_sample(s).map_err(|e| e.to_string())?;
            }
        }
        writer.finalize().map_err(|e| e.to_string())?;
    }
    Ok(cursor.into_inner())
}

/// Deterministic demo audio; generation runs on a worker, never in the audio callback.
pub fn generate(src: &Source) -> Result<AudioBuffer> {
    let rate = 48_000;
    let len = (src.duration_seconds * rate as f64).round() as usize;
    if len == 0 || len > MAX_AUDIO_BYTES / 8 {
        return Err("Generated source exceeds capacity".into());
    }
    let mut seed = src.seed.unwrap_or(1).wrapping_mul(7919).wrapping_add(13);
    let mut random = || {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        seed as f64 / 4294967296.0
    };
    let mut mono = vec![0.0_f32; len];
    if src.wave_kind.as_deref() == Some("drums") {
        let variation = random();
        for beat in 0..(src.duration_seconds / 0.5).ceil() as usize {
            let start = beat as f64 * 0.5;
            let mut hit = |at: f64, kind: u8, gain: f64| {
                let a = (at * rate as f64) as usize;
                let mut phase = 0.0_f64;
                for (i, sample) in mono
                    .iter_mut()
                    .skip(a)
                    .take((rate as f64 * 0.4) as usize)
                    .enumerate()
                {
                    let t = i as f64 / rate as f64;
                    let value = match kind {
                        0 => {
                            phase += std::f64::consts::TAU * (150.0 * (-t * 18.0).exp() + 45.0)
                                / rate as f64;
                            phase.sin() * (-t * 9.0).exp()
                        }
                        1 => {
                            (std::f64::consts::TAU * 190.0 * t).sin() * (-t * 30.0).exp() * 0.5
                                + (random() * 2.0 - 1.0) * (-t * 16.0).exp() * 0.6
                        }
                        _ => (random() * 2.0 - 1.0) * (-t * 60.0).exp(),
                    };
                    *sample += (value * gain) as f32;
                }
            };
            if beat % 2 == 0 {
                hit(start, 0, 0.9);
            } else {
                hit(start, 1, 0.8);
            }
            if beat % 4 == 3 && variation > 0.5 {
                hit(start + 0.25, 0, 0.45);
            }
            hit(start, 2, 0.35);
            hit(start + 0.25, 2, 0.22);
        }
    } else {
        let chords = [
            [48, 55, 60, 63],
            [44, 51, 56, 60],
            [41, 48, 53, 56],
            [43, 50, 55, 58],
        ];
        let detune = 0.3 + random() * 0.4;
        let mut phases = [0.0_f64; 8];
        for (i, sample) in mono.iter_mut().enumerate() {
            let t = i as f64 / rate as f64;
            let pos = (t % 4.0) / 4.0;
            let env = (pos * 6.0).min(1.0) * ((1.0 - pos) * 8.0).min(1.0);
            let mut value = 0.0;
            for (v, pitch) in chords[(t / 4.0) as usize % 4].iter().enumerate() {
                let hz = 440.0 * 2.0_f64.powf((*pitch as f64 - 69.0) / 12.0);
                phases[v * 2] = (phases[v * 2] + hz / rate as f64).fract();
                phases[v * 2 + 1] =
                    (phases[v * 2 + 1] + hz * (1.0 + detune / 100.0) / rate as f64).fract();
                for k in 1..=6 {
                    value += ((std::f64::consts::TAU * k as f64 * phases[v * 2]).sin()
                        + (std::f64::consts::TAU * k as f64 * phases[v * 2 + 1]).sin())
                        * 0.3
                        / k as f64;
                }
            }
            *sample =
                (value * 0.0875 * env * (0.85 + 0.15 * (std::f64::consts::TAU * 0.4 * t).sin()))
                    as f32;
        }
    }
    let frames = mono
        .iter()
        .enumerate()
        .map(|(i, &v)| [v.tanh(), (mono[i.saturating_sub(17)] * 0.98).tanh()])
        .collect();
    AudioBuffer::new(rate, frames)
}

pub fn prepare_sources(session: &crate::model::Session, library: &mut Library) -> Result<()> {
    session.validate()?;
    if library_bytes(library) > MAX_LIBRARY_BYTES {
        return Err("Decoded audio library exceeds 1 GiB".into());
    }
    for src in session.sources.values() {
        if !library.contains_key(&src.id) {
            if src.origin != "generated" {
                return Err(format!("Missing audio: {}", src.name));
            }
            let expected = (src.duration_seconds * 48000.0 * 8.0) as usize;
            if library_bytes(library).saturating_add(expected) > MAX_LIBRARY_BYTES {
                return Err("Decoded audio library exceeds 1 GiB".into());
            }
            library.insert(src.id.clone(), Arc::new(generate(src)?));
        }
    }
    Ok(())
}

#[cfg(test)]
mod decode_tests {
    use super::decode;

    // One second of a 440 Hz tone at 44.1 kHz, made with `ffmpeg -f lavfi -i sine=… -c:a
    // libmp3lame`; the MP4 holds a video stream before its AAC audio.
    const TONE_MP3: &[u8] = include_bytes!("../tests/fixtures/tone.mp3");
    const VIDEO_FIRST_MP4: &[u8] = include_bytes!("../tests/fixtures/video-first.mp4");

    #[test]
    fn mp3_priming_and_padding_are_trimmed() {
        let decoded = decode(TONE_MP3.to_vec(), Some("mp3")).unwrap();
        assert_eq!(decoded.sample_rate, 44100);
        assert!(
            decoded.frames.len().abs_diff(44100) < 64,
            "a one-second file decodes to {} frames",
            decoded.frames.len()
        );
    }

    #[test]
    fn one_damaged_frame_does_not_refuse_the_whole_file() {
        let mut bytes = TONE_MP3.to_vec();
        // Four bytes of one frame header: the decoder rejects that packet.
        for b in &mut bytes[1925..1929] {
            *b ^= 0x5a;
        }
        let decoded = decode(bytes, Some("mp3")).expect("the rest of the file decodes");
        assert!(decoded.frames.len() > 30000, "{}", decoded.frames.len());
    }

    #[test]
    fn a_video_file_imports_its_sound() {
        let decoded = decode(VIDEO_FIRST_MP4.to_vec(), Some("mp4")).unwrap();
        assert!(decoded.frames.len() > 40000, "{}", decoded.frames.len());
    }

    #[test]
    fn a_file_that_is_not_audio_says_so_plainly() {
        let error = decode(b"just some text, not audio".to_vec(), Some("wav")).unwrap_err();
        assert!(
            error.starts_with("Not an audio file Ondera can read"),
            "{error}"
        );
    }
}

#[cfg(test)]
mod downmix_tests {
    use super::to_stereo;

    #[test]
    fn downmix_keeps_sides_apart_and_never_clips() {
        assert_eq!(to_stereo(&[0.5]), [0.5, 0.5]);
        assert_eq!(to_stereo(&[0.25, -0.5]), [0.25, -0.5]);
        let full = to_stereo(&[1.0, 1.0, 1.0, 1.0, 1.0, 1.0]);
        assert!(full[0] <= 1.0 && (full[0] - 1.0).abs() < 1e-6 && full[0] == full[1]);
        assert_eq!(to_stereo(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0]), [0.0, 0.0]);
        let ls = to_stereo(&[0.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
        assert!(ls[0] > 0.2 && ls[1] == 0.0);
        assert_eq!(to_stereo(&[0.6, 0.2, 0.2]), [0.4, 0.2]);
    }
}
