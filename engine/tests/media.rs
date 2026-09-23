use ondera_engine::{
    audio::AudioBuffer,
    control::{self, Headless},
    export::{self, ExportOptions, SampleFormat},
    midi_file::{self, ImportOptions},
    model::*,
    store::{Command, Store},
};
use serde_json::{json, Value};
use std::{path::Path, sync::Arc};

fn command(host: &mut Headless, name: &str, args: Value) -> Value {
    control::call(host, name, &args, false).unwrap_or_else(|e| panic!("{name}: {e}"))
}
fn song() -> Headless {
    let mut host = Headless::new();
    let id = command(
        &mut host,
        "track.add",
        json!({"kind":"midi","instrument":"Glass Keys"}),
    )["id"]
        .clone();
    command(
        &mut host,
        "clip.create",
        json!({"trackId":id,"startBar":0,"lengthBars":2,"notes":[{"start":0,"length":8,"pitch":60,"velocity":90}]}),
    );
    command(&mut host, "transport.setTempo", json!({"bpm":120}));
    host
}
fn read_float(path: &Path) -> Vec<f32> {
    hound::WavReader::open(path)
        .unwrap()
        .samples::<f32>()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn exports_cannot_replace_the_open_session_document() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ondera");
    let mut host = song();
    command(&mut host, "session.save", json!({"path":path}));
    let before = std::fs::read(&path).unwrap();
    let outputs = vec![path.clone(), dir.path().join(".").join("song.ondera")];
    #[cfg(unix)]
    let outputs = {
        let mut outputs = outputs;
        let alias = dir.path().join("alias.wav");
        std::os::unix::fs::symlink(&path, &alias).unwrap();
        outputs.push(alias);
        outputs
    };
    for output in outputs {
        for name in [
            "session.bounce",
            "session.exportMidi",
            "session.exportAudio",
        ] {
            let error = control::call(&mut host, name, &json!({"path":output}), false).unwrap_err();
            assert!(
                error.contains("cannot overwrite the open session"),
                "{name}: {error}"
            );
            assert_eq!(std::fs::read(&path).unwrap(), before);
        }
    }
}

#[test]
fn midi_roundtrip_preserves_arrangement_timing_meter_channel_and_note_clipping() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("music.mid");
    let mut host = Headless::new();
    let id = command(
        &mut host,
        "track.add",
        json!({"kind":"midi","name":"Piano"}),
    )["id"]
        .as_str()
        .unwrap()
        .to_string();
    command(
        &mut host,
        "transport.setTimeSignature",
        json!({"numerator":3,"denominator":8}),
    );
    command(&mut host, "transport.setTempo", json!({"bpm":100}));
    command(
        &mut host,
        "clip.create",
        json!({"trackId":id,"startBar":2,"lengthBars":2,"notes":[{"start":0.5,"length":9,"pitch":64,"velocity":83}]}),
    );
    let report =
        midi_file::export(host.store.session(), &path, Some(std::slice::from_ref(&id))).unwrap();
    assert_eq!(report.track_count, 1);
    assert_eq!(report.note_count, 1);
    let smf_bytes = std::fs::read(&path).unwrap();
    assert!(smf_bytes.starts_with(b"MThd"));
    let parsed = midly::Smf::parse(&smf_bytes).unwrap();
    assert_eq!(parsed.header.format, midly::Format::Parallel);
    assert_eq!(parsed.header.timing, midly::Timing::Metrical(960.into()));
    let mut destination = Headless::new();
    let (batch, report) = midi_file::import(
        &path,
        destination.store.session(),
        &ImportOptions {
            start_bar: 1.0,
            import_tempo: true,
        },
        true,
    )
    .unwrap();
    destination.store.dispatch(batch).unwrap();
    assert_eq!(report.notes, 1);
    assert_eq!(report.file_tempo, 100.0);
    assert_eq!(
        destination
            .store
            .session()
            .transport
            .time_signature
            .denominator,
        8
    );
    let clip = destination
        .store
        .session()
        .clips
        .iter()
        .find(|c| c.id == report.clip_ids[0])
        .unwrap();
    assert_eq!(clip.start_bar, 1.0);
    assert!(clip.agent);
    let ClipData::Midi { notes, .. } = &clip.data else {
        panic!()
    };
    assert_eq!(notes[0].start, 3.5);
    assert_eq!(notes[0].length, 2.5);
    assert_eq!(notes[0].velocity, 83);
    destination.store.dispatch(Command::Undo).unwrap();
    assert!(destination.store.session().clips.is_empty());
}

#[test]
fn midi_type_zero_running_status_and_channel_split_are_supported() {
    // Type 0, PPQ 96: simultaneous C4 and E4 on separate channels, quarter-note duration.
    let track = [
        0x00, 0x90, 60, 100, 0x00, 0x91, 64, 90, 0x60, 0x81, 64, 0, 0x00, 0x80, 60, 0, 0x00, 0xff,
        0x2f, 0x00,
    ];
    let mut bytes = b"MThd\0\0\0\x06\0\0\0\x01\0\x60MTrk".to_vec();
    bytes.extend_from_slice(&(track.len() as u32).to_be_bytes());
    bytes.extend_from_slice(&track);
    let empty = Headless::new();
    let (batch, report) = midi_file::import_bytes(
        &bytes,
        empty.store.session(),
        &ImportOptions::default(),
        false,
    )
    .unwrap();
    assert_eq!(report.track_ids.len(), 2);
    assert_eq!(report.notes, 2);
    let mut store = Store::new(empty.store.session().clone()).unwrap();
    store.dispatch(batch).unwrap();
    for clip in &store.session().clips {
        let ClipData::Midi { notes, .. } = &clip.data else {
            panic!()
        };
        assert_eq!(notes[0].start, 0.0);
        assert_eq!(notes[0].length, 1.0);
    }
    // A note-off encoded as running-status NoteOn velocity zero.
    let track = [0, 0x90, 60, 100, 0x60, 60, 0, 0, 0xff, 0x2f, 0];
    bytes.truncate(18);
    bytes.extend_from_slice(&(track.len() as u32).to_be_bytes());
    bytes.extend_from_slice(&track);
    let (_, report) = midi_file::import_bytes(
        &bytes,
        empty.store.session(),
        &ImportOptions::default(),
        false,
    )
    .unwrap();
    assert_eq!(report.notes, 1);
}

#[test]
fn malformed_midi_and_unsupported_timecode_never_change_the_session() {
    let mut host = song();
    let revision = host.store.revision;
    assert!(midi_file::import_bytes(
        b"corrupt",
        host.store.session(),
        &ImportOptions::default(),
        false
    )
    .is_err());
    let bytes = b"MThd\0\0\0\x06\0\0\0\x01\xe7\x28MTrk\0\0\0\x04\0\xff\x2f\0";
    assert!(midi_file::import_bytes(
        bytes,
        host.store.session(),
        &ImportOptions::default(),
        false
    )
    .unwrap_err()
    .contains("SMPTE"));
    assert_eq!(host.store.revision, revision);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("old.mid");
    std::fs::write(&path, b"keep me").unwrap();
    let audio = command(&mut host, "track.add", json!({"kind":"audio"}))["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(midi_file::export(host.store.session(), &path, Some(&[audio])).is_err());
    assert_eq!(std::fs::read(path).unwrap(), b"keep me");
}

#[test]
fn export_wav_formats_rates_ranges_and_sustained_preroll() {
    let dir = tempfile::tempdir().unwrap();
    let host = song();
    for (rate, format, bits, sample_format) in [
        (44100, SampleFormat::Pcm16, 16, hound::SampleFormat::Int),
        (48000, SampleFormat::Pcm24, 24, hound::SampleFormat::Int),
        (96000, SampleFormat::Float32, 32, hound::SampleFormat::Float),
    ] {
        let path = dir.path().join(format!("{rate}.wav"));
        let options = ExportOptions {
            sample_rate: rate,
            format,
            end_beat: Some(1.0),
            tail_seconds: 0.25,
            ..Default::default()
        };
        let report = export::mix(host.store.session(), &host.library, &path, &options).unwrap();
        let spec = hound::WavReader::open(path).unwrap().spec();
        assert_eq!(spec.sample_rate, rate);
        assert_eq!(spec.bits_per_sample, bits);
        assert_eq!(spec.sample_format, sample_format);
        assert_eq!(report.frames, (0.75 * rate as f64).ceil() as u64);
        assert!(report.peak > 0.01);
    }
    let full = dir.path().join("full.wav");
    let range = dir.path().join("range.wav");
    let options = ExportOptions {
        format: SampleFormat::Float32,
        tail_seconds: 0.0,
        end_beat: Some(8.0),
        ..Default::default()
    };
    export::mix(host.store.session(), &host.library, &full, &options).unwrap();
    export::mix(
        host.store.session(),
        &host.library,
        &range,
        &ExportOptions {
            start_beat: Some(4.0),
            ..options
        },
    )
    .unwrap();
    let all = read_float(&full);
    let part = read_float(&range);
    assert_eq!(part.len(), 192000);
    assert_eq!(
        &all[192000..],
        part.as_slice(),
        "range pre-roll retains exact ongoing note/effect history"
    );
}

#[test]
fn float_keeps_headroom_pcm_reports_clipping_and_failure_preserves_output() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = Headless::new();
    let id = command(&mut host, "track.add", json!({"kind":"audio"}))["id"]
        .as_str()
        .unwrap()
        .to_string();
    host.store
        .dispatch(Command::PutSource(Source {
            id: "audio".into(),
            name: "Constant".into(),
            sample_rate: 48000,
            channels: 2,
            file_name: None,
            duration_seconds: 0.1,
            origin: "file".into(),
            seed: None,
            wave_kind: None,
        }))
        .unwrap();
    host.library.insert(
        "audio".into(),
        Arc::new(AudioBuffer::new(48000, vec![[0.8; 2]; 4800]).unwrap()),
    );
    command(
        &mut host,
        "clip.create",
        json!({"trackId":id,"startBar":0,"lengthBars":0.05,"sourceId":"audio"}),
    );
    command(
        &mut host,
        "track.setVolume",
        json!({"trackId":id,"volume":1.0}),
    );
    command(&mut host, "master.setVolume", json!({"volume":1.0}));
    let path = dir.path().join("headroom.wav");
    let options = ExportOptions {
        format: SampleFormat::Float32,
        tail_seconds: 0.0,
        ..Default::default()
    };
    let report = export::mix(host.store.session(), &host.library, &path, &options).unwrap();
    assert!(report.peak > 1.0);
    assert_eq!(report.clipped_samples, 0);
    assert!(read_float(&path).iter().any(|v| *v > 1.0));
    let report = export::mix(
        host.store.session(),
        &host.library,
        &path,
        &ExportOptions {
            format: SampleFormat::Pcm16,
            ..options.clone()
        },
    )
    .unwrap();
    assert!(report.clipped_samples > 0);
    let before = std::fs::read(&path).unwrap();
    assert!(export::mix(
        host.store.session(),
        &host.library,
        &path,
        &ExportOptions {
            sample_rate: 12345,
            ..options
        }
    )
    .is_err());
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn stems_are_aligned_and_publish_all_or_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = song();
    let second = command(
        &mut host,
        "track.add",
        json!({"kind":"midi","name":"../unsafe/name","instrument":"Sub Bass 808"}),
    )["id"]
        .clone();
    command(
        &mut host,
        "clip.create",
        json!({"trackId":second,"startBar":1,"lengthBars":1,"notes":[{"start":0,"length":1,"pitch":36}]}),
    );
    let folder = dir.path().join("stems");
    let options = ExportOptions {
        format: SampleFormat::Float32,
        tail_seconds: 0.1,
        ..Default::default()
    };
    let report = export::stems(
        host.store.session(),
        &host.library,
        &folder,
        &options,
        None,
        false,
        false,
    )
    .unwrap();
    assert_eq!(report.files.len(), host.store.session().tracks.len());
    assert!(report
        .files
        .iter()
        .all(|f| f.frames == report.files[0].frames));
    assert!(report
        .files
        .iter()
        .all(|f| f.path.parent() == Some(folder.as_path()) && f.path.exists()));
    assert!(folder.join("manifest.json").exists());
    assert!(export::stems(
        host.store.session(),
        &host.library,
        &folder,
        &options,
        None,
        false,
        false
    )
    .is_err());
    let fail_folder = dir.path().join("failed-stems");
    let mut strip = host.store.session().strips[second.as_str().unwrap()].clone();
    strip.synth = Some(Insert::new(
        "missing".into(),
        "stock:No Such Plugin",
        "Missing",
    ));
    host.store
        .dispatch(Command::SetStrip {
            track: second.as_str().unwrap().into(),
            strip,
        })
        .unwrap();
    assert!(export::stems(
        host.store.session(),
        &host.library,
        &fail_folder,
        &options,
        None,
        true,
        false
    )
    .is_err());
    assert!(!fail_folder.exists());
    assert!(!std::fs::read_dir(dir.path()).unwrap().any(|e| e
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".ondera-stems-")));
}

#[test]
fn stems_only_apply_master_automation_when_requested() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = song();
    command(
        &mut host,
        "automation.create",
        json!({
            "target":"masterVolume", "points":[{"beat":0,"value":0}]
        }),
    );
    let options = ExportOptions {
        format: SampleFormat::Float32,
        end_beat: Some(1.0),
        tail_seconds: 0.0,
        ..Default::default()
    };
    for include_master in [false, true] {
        let report = export::stems(
            host.store.session(),
            &host.library,
            &dir.path()
                .join(if include_master { "master" } else { "dry" }),
            &options,
            None,
            false,
            include_master,
        )
        .unwrap();
        if include_master {
            assert!(report.files.iter().all(|file| file.peak == 0.0));
        } else {
            assert!(
                report.files.iter().any(|file| file.peak > 0.01),
                "excluding the master must also exclude its automation"
            );
        }
    }
}

#[test]
fn media_registry_is_available_to_cli_and_mcp_validation() {
    let mut host = song();
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("registry.wav");
    let report = command(
        &mut host,
        "session.exportAudio",
        json!({"path":output,"sampleRate":44100,"format":"float32","endBeat":1,"tailSeconds":0}),
    );
    assert_eq!(report["frames"], 22050);
    assert_eq!(report["sampleRate"], 44100);
    assert!(control::call(
        &mut host,
        "session.exportAudio",
        &json!({"path":output,"startBar":0,"endBeat":1}),
        true
    )
    .unwrap_err()
    .contains("either"));
    assert!(control::spec("session.importMidi").is_some());
    assert!(control::spec("session.exportStems").is_some());
}

#[test]
fn aiff_export_holds_the_same_audio_as_wav() {
    use ondera_engine::{audio, export, store};
    let session = store::demo();
    let dir = tempfile::tempdir().unwrap();
    let options = export::ExportOptions {
        end_bar: Some(1.0),
        tail_seconds: 0.0,
        dither: false,
        ..Default::default()
    };
    let library = audio::Library::new();
    let mut prepared = library.clone();
    audio::prepare_sources(&session, &mut prepared).unwrap();
    let wav = dir.path().join("mix.wav");
    let aiff = dir.path().join("mix.aiff");
    export::mix(&session, &prepared, &wav, &options).unwrap();
    export::mix(&session, &prepared, &aiff, &options).unwrap();
    let a = audio::decode(std::fs::read(&wav).unwrap(), Some("wav")).unwrap();
    let b = audio::decode(std::fs::read(&aiff).unwrap(), Some("aiff")).unwrap();
    assert_eq!(a.sample_rate, b.sample_rate);
    assert_eq!(a.frames.len(), b.frames.len());
    assert!(
        a.frames.iter().flatten().any(|v| v.abs() > 0.01),
        "the demo is audible"
    );
    assert_eq!(a.frames, b.frames);
    let float = export::ExportOptions {
        format: export::SampleFormat::Float32,
        ..options
    };
    assert!(export::mix(&session, &prepared, &aiff, &float).is_err());
    assert!(
        aiff.exists(),
        "a refused export leaves the previous file alone"
    );
}

#[test]
fn flac_export_holds_the_same_audio_as_wav() {
    use ondera_engine::{audio, export, store};
    let session = store::demo();
    let dir = tempfile::tempdir().unwrap();
    let library = audio::Library::new();
    let mut prepared = library.clone();
    audio::prepare_sources(&session, &mut prepared).unwrap();
    for format in [export::SampleFormat::Pcm16, export::SampleFormat::Pcm24] {
        let options = export::ExportOptions {
            // Not a whole number of FLAC blocks, so the short last frame is covered.
            end_beat: Some(5.3),
            tail_seconds: 0.0,
            dither: false,
            format,
            ..Default::default()
        };
        let wav = dir.path().join("mix.wav");
        let flac = dir.path().join("mix.flac");
        export::mix(&session, &prepared, &wav, &options).unwrap();
        let report = export::mix(&session, &prepared, &flac, &options).unwrap();
        let a = audio::decode(std::fs::read(&wav).unwrap(), Some("wav")).unwrap();
        let b = audio::decode(std::fs::read(&flac).unwrap(), Some("flac")).unwrap();
        assert_eq!(a.sample_rate, b.sample_rate);
        assert_eq!(report.frames as usize, b.frames.len());
        assert!(a.frames.iter().flatten().any(|v| v.abs() > 0.01));
        assert_eq!(a.frames, b.frames, "{format:?} is lossless");
        let (wav_size, flac_size) = (
            std::fs::metadata(&wav).unwrap().len(),
            std::fs::metadata(&flac).unwrap().len(),
        );
        assert!(
            flac_size * 10 < wav_size * 8,
            "FLAC compresses: {flac_size} vs {wav_size}"
        );
    }
    let float = export::ExportOptions {
        format: export::SampleFormat::Float32,
        end_bar: Some(1.0),
        ..Default::default()
    };
    let flac = dir.path().join("mix.flac");
    assert!(export::mix(&session, &prepared, &flac, &float)
        .unwrap_err()
        .contains("FLAC"));
    assert!(
        flac.exists(),
        "a refused export leaves the previous file alone"
    );
}

#[test]
fn stems_take_the_container_that_was_asked_for() {
    use ondera_engine::{audio, export, store};
    let session = store::demo();
    let mut prepared = audio::Library::new();
    audio::prepare_sources(&session, &mut prepared).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let options = export::ExportOptions {
        end_bar: Some(1.0),
        tail_seconds: 0.0,
        container: export::Container::Flac,
        ..Default::default()
    };
    let ids = [session.tracks[0].id.clone()];
    let report = export::stems(
        &session,
        &prepared,
        &dir.path().join("stems"),
        &options,
        Some(&ids),
        true,
        true,
    )
    .unwrap();
    let path = &report.files[0].path;
    assert_eq!(path.extension().unwrap(), "flac");
    audio::decode(std::fs::read(path).unwrap(), Some("flac")).unwrap();
    assert_eq!(
        export::Container::of(std::path::Path::new("a.AIF")),
        export::Container::Aiff
    );
    assert_eq!(
        export::Container::of(std::path::Path::new("a.flac")),
        export::Container::Flac
    );
    assert_eq!(
        export::Container::of(std::path::Path::new("a.bin")),
        export::Container::Wav
    );
}

#[test]
fn ogg_export_decodes_back_to_the_mix_at_a_fraction_of_the_size() {
    use ondera_engine::{audio, export, store};
    let mut session = store::demo();
    session.name = "Night Drive".into();
    let dir = tempfile::tempdir().unwrap();
    let mut prepared = audio::Library::new();
    audio::prepare_sources(&session, &mut prepared).unwrap();
    let options = export::ExportOptions {
        end_beat: Some(9.3),
        tail_seconds: 0.0,
        format: export::SampleFormat::Float32,
        ..Default::default()
    };
    let wav = dir.path().join("mix.wav");
    let ogg = dir.path().join("mix.ogg");
    export::mix(&session, &prepared, &wav, &options).unwrap();
    let report = export::mix(&session, &prepared, &ogg, &options).unwrap();
    assert_eq!(report.container, export::Container::Ogg);
    assert_eq!(report.quality, Some(export::DEFAULT_QUALITY));
    let kbps = report.kbps.unwrap();
    assert!((80.0..400.0).contains(&kbps), "{kbps} kbit/s");
    let bytes = std::fs::read(&ogg).unwrap();
    assert_eq!(&bytes[..4], b"OggS");
    assert!(
        bytes.windows(17).any(|w| w == b"TITLE=Night Drive"),
        "the title is tagged"
    );
    // Ondera imports what it exports.
    assert!(audio::is_importable(&ogg));
    let original = audio::decode(std::fs::read(&wav).unwrap(), Some("wav")).unwrap();
    let decoded = audio::decode(bytes.clone(), Some("ogg")).unwrap();
    assert_eq!(decoded.sample_rate, 48000);
    let length = decoded.frames.len() as i64 - report.frames as i64;
    assert!(length.abs() < 2048, "{length} frames off");
    let (mut signal, mut error) = (0f64, 0f64);
    for (a, b) in original.frames.iter().zip(&decoded.frames) {
        for c in 0..2 {
            signal += (a[c] as f64).powi(2);
            error += (a[c] as f64 - b[c] as f64).powi(2);
        }
    }
    let snr = 10.0 * (signal / error.max(1e-12)).log10();
    assert!(signal > 1.0, "the demo is audible");
    eprintln!("Ogg Vorbis at quality 0.6: {kbps} kbit/s, {snr:.1} dB SNR, {length} frames off");
    assert!(snr > 15.0, "decoded audio follows the mix: {snr:.1} dB");
    let wav_size = std::fs::metadata(&wav).unwrap().len();
    assert!(
        (bytes.len() as u64) * 5 < wav_size,
        "Vorbis is small: {} vs {wav_size}",
        bytes.len()
    );
    // A lower quality is smaller; a quality outside 0-1 is refused and replaces nothing.
    let low = export::ExportOptions {
        quality: 0.1,
        ..options.clone()
    };
    let small = export::mix(&session, &prepared, &dir.path().join("low.ogg"), &low).unwrap();
    assert!(small.kbps.unwrap() < kbps);
    let wrong = export::ExportOptions {
        quality: 1.5,
        ..options
    };
    assert!(export::mix(&session, &prepared, &ogg, &wrong)
        .unwrap_err()
        .contains("quality"));
    assert_eq!(std::fs::read(&ogg).unwrap(), bytes);
}

#[test]
fn ogg_stems_carry_the_track_name() {
    use ondera_engine::{audio, export, store};
    let session = store::demo();
    let mut prepared = audio::Library::new();
    audio::prepare_sources(&session, &mut prepared).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let options = export::ExportOptions {
        end_bar: Some(1.0),
        tail_seconds: 0.0,
        container: export::Container::parse("ogg").unwrap(),
        quality: 0.3,
        ..Default::default()
    };
    let ids = [session.tracks[0].id.clone()];
    let report = export::stems(
        &session,
        &prepared,
        &dir.path().join("stems"),
        &options,
        Some(&ids),
        true,
        false,
    )
    .unwrap();
    let path = &report.files[0].path;
    assert_eq!(path.extension().unwrap(), "ogg");
    let bytes = std::fs::read(path).unwrap();
    let title = format!("TITLE={} - {}", session.name, session.tracks[0].name);
    assert!(bytes.windows(title.len()).any(|w| w == title.as_bytes()));
    let decoded = audio::decode(bytes, Some("ogg")).unwrap();
    assert!(!decoded.frames.is_empty());
    assert_eq!(
        export::Container::of(std::path::Path::new("a.OGG")),
        export::Container::Ogg
    );
}
