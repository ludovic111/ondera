//! Opt-in integration check against installed, licensed plugins. Does not scan
//! or change the user's cache. Each invocation should run in a child process.
//! Usage: plugin_roundtrip <instrument-id> <effect-id> <output-directory>
use ondera_engine::{
    audio::{AudioBuffer, Library},
    document, host,
    model::{Clip, ClipData, Insert, Note, Source, Strip},
    plugin::{Event, NoteEvent, ProcessContext, Rack, MAX_BLOCK},
    render, store,
};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

fn capture(id: &str, key: &str) -> Insert {
    let mut instance = host::instantiate(id, id, 48000).expect("instantiate");
    let descriptor = instance.editor.descriptor().clone();
    let parameter = instance
        .editor
        .params()
        .iter()
        .find(|p| p.max > p.min)
        .cloned();
    let mut rack = Rack::new(1);
    rack.mount(0, instance.processor.take().expect("processor"));
    let mut params = BTreeMap::new();
    if let Some(parameter) = &parameter {
        let original = instance
            .editor
            .value(parameter.id)
            .unwrap_or(parameter.default);
        let value = parameter.denormalize(if parameter.normalize(original) < 0.5 {
            0.75
        } else {
            0.25
        });
        instance.editor.set_value(parameter.id, value);
        rack.set_param(0, parameter.id, value);
        params.insert(parameter.id, value);
    }
    let context = ProcessContext {
        playing: true,
        tempo: 120.0,
        numerator: 4,
        denominator: 4,
        ..Default::default()
    };
    let note = [Event::from(NoteEvent {
        frame: 0,
        on: true,
        pitch: 60,
        velocity: 96,
        channel: 0,
    })];
    let mut energy = 0.0f64;
    for index in 0..188 {
        let mut block = [[0.0; 2]; MAX_BLOCK];
        if !descriptor.instrument {
            for (frame, sample) in block.iter_mut().enumerate() {
                let value = (((index * MAX_BLOCK + frame) as f32 * 440.0 * std::f32::consts::TAU
                    / 48000.0)
                    .sin())
                    * 0.1;
                *sample = [value; 2];
            }
        }
        rack.process(
            0,
            &mut block,
            if index == 0 { &note } else { &[] },
            &context,
        );
        assert!(block.iter().flatten().all(|sample| sample.is_finite()));
        energy += block
            .iter()
            .flatten()
            .map(|sample| (*sample as f64).powi(2))
            .sum::<f64>();
        instance.editor.idle();
    }
    assert!(energy > 1e-8, "plugin produced no audible output: {id}");
    drop(rack.unmount(0));
    let state = instance.editor.save().expect("plugin state save");
    println!(
        "{id}: audio energy {energy:.6}, state {} bytes",
        state.len()
    );
    drop(instance);
    let mut restored = host::instantiate(id, id, 48000).expect("fresh instance");
    restored
        .editor
        .load(&state)
        .expect("load state into fresh instance");
    for (id, expected) in &params {
        let actual = restored.editor.value(*id).expect("restored parameter");
        assert!(
            (actual - expected).abs() <= 1e-5 * expected.abs().max(1.0),
            "state parameter {id}: {actual} != {expected}"
        );
        println!("  restored parameter {id} = {actual}");
    }
    let mut insert = Insert::new(key.into(), id, &descriptor.name);
    insert.params = params;
    insert.blob = host::encode_blob(&state);
    insert
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert!(
        args.len() == 4,
        "usage: plugin_roundtrip <instrument-id> <effect-id> <output-directory>"
    );
    let output = PathBuf::from(&args[3]);
    std::fs::create_dir_all(&output).unwrap();
    let synth = capture(&args[1], "external-synth");
    let effect = capture(&args[2], "external-effect");
    let mut session = store::empty();
    session.name = "External plugin round trip".into();
    session.transport.tempo = 120.0;
    session.transport.metronome = false;
    session.master_volume = 0.75;
    session.tracks.truncate(2);
    session.tracks[0].kind = "midi".into();
    session.tracks[0].volume = 0.6;
    session.tracks[1].kind = "audio".into();
    session.tracks[1].volume = 0.45;
    session.strips.clear();
    let first_id = session.tracks[0].id.clone();
    let second_id = session.tracks[1].id.clone();
    session.strips.insert(
        first_id.clone(),
        Strip {
            synth: Some(synth),
            inserts: vec![effect],
            ..Default::default()
        },
    );
    session.strips.insert(second_id.clone(), Strip::default());
    session.clips.push(Clip {
        id: "melody".into(),
        name: "External synth melody".into(),
        agent: false,
        track_id: first_id,
        start_bar: 0.0,
        length_bars: 2.0,
        data: ClipData::Midi {
            notes: [60, 64, 67, 72, 67, 64, 62, 60]
                .into_iter()
                .enumerate()
                .map(|(i, pitch)| Note {
                    id: format!("note-{i}"),
                    start: i as f64,
                    length: 0.8,
                    pitch,
                    velocity: 92,
                    agent: false,
                })
                .collect(),
            controllers: vec![],
        },
    });
    let buffer = Arc::new(
        AudioBuffer::new(
            48000,
            (0..192000)
                .map(|sample| {
                    let t = sample as f32 / 48000.0;
                    let value = (t * 110.0 * std::f32::consts::TAU).sin() * 0.06;
                    [value; 2]
                })
                .collect(),
        )
        .unwrap(),
    );
    session.sources.insert(
        "imported".into(),
        Source {
            id: "imported".into(),
            name: "Imported audio bed".into(),
            sample_rate: 48000,
            channels: 2,
            file_name: Some("audio-bed.wav".into()),
            duration_seconds: 4.0,
            origin: "file".into(),
            seed: None,
            wave_kind: None,
        },
    );
    session.clips.push(Clip {
        id: "audio".into(),
        name: "Imported audio bed".into(),
        agent: false,
        track_id: second_id,
        start_bar: 0.0,
        length_bars: 2.0,
        data: ClipData::Audio {
            source_id: "imported".into(),
            offset_seconds: 0.0,
        },
    });
    session.normalize();
    let library = Library::from([("imported".into(), buffer)]);
    let project = output.join("song.ondera");
    document::save(&session, &library, &project).expect("save project");
    let (loaded, library) = document::load(&project).expect("reopen project");
    assert_eq!(loaded.clips.len(), 2);
    let wav_path = output.join("song.wav");
    render::bounce(&loaded, &library, &wav_path, 48000).expect("bounce reopened project");
    let mut wav = hound::WavReader::open(&wav_path).expect("decode bounce");
    assert_eq!(wav.spec().bits_per_sample, 24);
    assert_eq!(wav.duration(), 336000);
    let mut peak = 0i32;
    let mut energy = 0f64;
    for sample in wav.samples::<i32>() {
        let sample = sample.expect("valid PCM");
        peak = peak.max(sample.abs());
        energy += (sample as f64 / 8_388_607.0).powi(2);
    }
    assert!(energy > 1.0, "silent bounce");
    println!("PASS: fresh instance state restored; project saved/reopened; 7s PCM24 stereo 48kHz bounce decoded, peak {peak}, energy {energy:.3}; {}", output.display());
}
