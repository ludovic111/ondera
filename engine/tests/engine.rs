use ondera_engine::{
    audio::{self, AudioBuffer, Library},
    document,
    dsp::{Effect, EFFECTS, INSTRUMENTS},
    model::*,
    render::{self, Renderer},
    store::{self, Command, Store},
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    sync::Arc,
};

thread_local! { static COUNTING: Cell<bool> = const { Cell::new(false) }; static ALLOCATIONS: Cell<usize> = const { Cell::new(0) }; static DEALLOCATIONS: Cell<usize> = const { Cell::new(0) }; }
struct TrackingAllocator;
unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        COUNTING.with(|v| {
            if v.get() {
                ALLOCATIONS.with(|n| n.set(n.get() + 1));
            }
        });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        COUNTING.with(|v| {
            if v.get() {
                DEALLOCATIONS.with(|n| n.set(n.get() + 1));
            }
        });
        unsafe { System.dealloc(ptr, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: TrackingAllocator = TrackingAllocator;

fn midi_session() -> Session {
    let mut s = store::empty();
    s.tracks.retain(|t| t.kind == "midi");
    s.view.selected_track_id = Some(s.tracks[0].id.clone());
    s.transport.metronome = false;
    s.clips.push(Clip {
        id: "clip".into(),
        name: "Test".into(),
        agent: false,
        track_id: s.tracks[0].id.clone(),
        start_bar: 0.0,
        length_bars: 1.0,
        data: ClipData::Midi {
            notes: vec![Note {
                id: "note".into(),
                start: 0.0,
                length: 2.0,
                pitch: 60,
                velocity: 100,
                agent: false,
            }],
        },
    });
    s
}
fn audio_session() -> (Session, Library) {
    let mut s = store::empty();
    s.tracks.retain(|t| t.kind == "audio");
    s.view.selected_track_id = Some(s.tracks[0].id.clone());
    s.transport.metronome = false;
    let frames = (0..4800)
        .map(|i| {
            let x = (i as f32 * 0.05).sin() * 0.3;
            [x, x * 0.5]
        })
        .collect();
    let buffer = Arc::new(AudioBuffer::new(48000, frames).unwrap());
    s.sources.insert(
        "src".into(),
        Source {
            id: "src".into(),
            name: "Sine".into(),
            sample_rate: 48000,
            channels: 2,
            file_name: Some("sine.wav".into()),
            duration_seconds: buffer.duration(),
            origin: "file".into(),
            seed: None,
            wave_kind: None,
        },
    );
    s.clips.push(Clip {
        id: "audio".into(),
        name: "Sine".into(),
        agent: false,
        track_id: s.tracks[0].id.clone(),
        start_bar: 0.0,
        length_bars: 0.25,
        data: ClipData::Audio {
            source_id: "src".into(),
            offset_seconds: 0.0,
        },
    });
    (s, Library::from([("src".into(), buffer)]))
}
fn energy(mut r: Renderer, frames: usize) -> f64 {
    r.playing = true;
    (0..frames)
        .map(|_| {
            let f = r.next_frame();
            (f[0] * f[0] + f[1] * f[1]) as f64
        })
        .sum()
}

#[test]
fn reads_original_typescript_demo() {
    let s: Session = serde_json::from_str(include_str!("fixtures/nightfall.json")).unwrap();
    s.validate().unwrap();
    assert_eq!(s.tracks.len(), 8);
    assert_eq!(s.clips.len(), 13);
    assert_eq!(s.strips["bass"].sends[1].level_db, None);
    assert_eq!(s.strips["bass"].extra["output"], "Stereo Out");
}
#[test]
fn history_is_atomic_and_transient_selection_stays_clean() {
    let mut st = Store::new(midi_session()).unwrap();
    st.dispatch(Command::Select {
        track: Some("bass".into()),
        clip: Some("clip".into()),
        note: None,
    })
    .unwrap();
    assert!(!st.dirty());
    st.dispatch(Command::Rename("Edited".into())).unwrap();
    assert!(st.dirty());
    st.dispatch(Command::Undo).unwrap();
    assert!(!st.dirty());
    st.dispatch(Command::Redo).unwrap();
    assert_eq!(st.session().name, "Edited");
}
#[test]
fn invalid_batch_preserves_document_and_history() {
    let mut st = Store::new(midi_session()).unwrap();
    let original = serde_json::to_value(st.session()).unwrap();
    let mut t = st.session().tracks[0].clone();
    t.volume = f32::NAN;
    assert!(st
        .dispatch(Command::Batch(vec![
            Command::Rename("Bad".into()),
            Command::UpdateTrack(t)
        ]))
        .is_err());
    assert_eq!(serde_json::to_value(st.session()).unwrap(), original);
    assert!(!st.can_undo());
}
#[test]
fn slider_gesture_is_one_undo_step() {
    let mut st = Store::new(midi_session()).unwrap();
    let original = st.session().tracks[0].volume;
    st.set_gesture(true);
    for i in 1..20 {
        let mut t = st.session().tracks[0].clone();
        t.volume = i as f32 / 20.0;
        st.dispatch(Command::UpdateTrack(t)).unwrap();
    }
    st.set_gesture(false);
    st.dispatch(Command::Undo).unwrap();
    assert_eq!(st.session().tracks[0].volume, original);
    assert!(!st.can_undo());
    assert!(!st.dirty());
}
#[test]
fn saving_old_revision_cannot_mark_new_edits_clean() {
    let mut st = Store::new(midi_session()).unwrap();
    st.dispatch(Command::Rename("First".into())).unwrap();
    let rev = st.revision;
    st.dispatch(Command::Rename("Second".into())).unwrap();
    st.mark_saved(rev);
    assert!(st.dirty());
}
#[test]
fn split_retains_crossing_notes() {
    let s = midi_session();
    let (l, r) = store::split(&s.clips[0], 0.25, "right".into(), 4.0, 120.0).unwrap();
    let ClipData::Midi { notes: a } = l.data else {
        panic!()
    };
    let ClipData::Midi { notes: b } = r.data else {
        panic!()
    };
    assert_eq!(a[0].length, 1.0);
    assert_eq!(b[0].start, 0.0);
    assert_eq!(b[0].length, 1.0);
}
#[test]
fn split_audio_advances_source_offset() {
    let (s, _) = audio_session();
    let (_, r) = store::split(&s.clips[0], 0.125, "right".into(), 4.0, 120.0).unwrap();
    let ClipData::Audio { offset_seconds, .. } = r.data else {
        panic!()
    };
    assert_eq!(offset_seconds, 0.25);
}
#[test]
fn transport_uses_output_samples_at_multiple_rates() {
    for rate in [44100, 48000, 96000] {
        let mut r = Renderer::new(midi_session(), &Library::new(), rate).unwrap();
        r.playing = true;
        for _ in 0..rate {
            r.next_frame();
        }
        assert!((r.position() - 2.0).abs() < 1e-8);
    }
}
#[test]
fn cycle_wraps_without_drift() {
    let mut s = midi_session();
    s.transport.cycle = true;
    s.transport.cycle_start_bar = 0.0;
    s.transport.cycle_end_bar = 0.25;
    let mut r = Renderer::new(s, &Library::new(), 48000).unwrap();
    r.playing = true;
    for _ in 0..120_000 {
        r.next_frame();
    }
    assert!(r.position() <= 1.000001);
    assert!((r.position() - 1.0).abs() < 1e-7 || r.position().abs() < 1e-7);
}
#[test]
fn locate_into_sustained_note_remains_audible() {
    let mut r = Renderer::new(midi_session(), &Library::new(), 48000).unwrap();
    r.locate(0.5);
    assert!(energy(r, 4800) > 1.0);
}
#[test]
fn mute_also_mutes_effect_sends() {
    let mut s = midi_session();
    s.tracks[0].mute = true;
    let strip = Strip {
        sends: vec![
            Send {
                level_db: Some(0.0),
                name: "Reverb".into(),
            },
            Send {
                level_db: Some(0.0),
                name: "Delay".into(),
            },
        ],
        ..Default::default()
    };
    s.strips.insert(s.tracks[0].id.clone(), strip);
    assert_eq!(
        energy(Renderer::new(s, &Library::new(), 48000).unwrap(), 48000),
        0.0
    );
}
#[test]
fn solo_excludes_unsoloed_tracks() {
    let mut s = midi_session();
    let mut other = s.tracks[0].clone();
    other.id = "solo".into();
    other.solo = true;
    s.tracks.push(other);
    assert_eq!(
        energy(Renderer::new(s, &Library::new(), 48000).unwrap(), 4800),
        0.0
    );
}
#[test]
fn every_instrument_is_finite_and_audible() {
    for name in INSTRUMENTS {
        let mut s = midi_session();
        s.strips.insert(
            s.tracks[0].id.clone(),
            Strip {
                instrument: name.into(),
                ..Default::default()
            },
        );
        let mut r = Renderer::new(s, &Library::new(), 48000).unwrap();
        r.playing = true;
        let mut energy = 0.0;
        for _ in 0..48000 {
            let v = r.next_frame();
            assert!(v.iter().all(|s| s.is_finite() && s.abs() <= 0.98));
            energy += v[0] * v[0];
        }
        assert!(energy > 0.01, "Silent instrument: {name}");
    }
}
#[test]
fn all_effects_decay_and_are_finite() {
    for name in EFFECTS {
        let mut effect = Effect::new(name, 48000).unwrap();
        let mut last = [0.0; 2];
        for i in 0..240_000 {
            last = effect.process(if i == 0 { [0.5, -0.5] } else { [0.0; 2] }, 48000);
            assert!(last.iter().all(|v| v.is_finite()), "{name}");
        }
        assert!(
            last.iter().all(|v| v.abs() < 0.001),
            "Effect tail does not decay: {name}"
        );
    }
}
#[test]
fn audio_round_trip_preserves_float_samples() {
    let (_, lib) = audio_session();
    let original = &lib["src"];
    let bytes = audio::encode_wav(original).unwrap();
    let decoded = audio::decode(bytes, Some("wav")).unwrap();
    assert_eq!(decoded.sample_rate, original.sample_rate);
    assert_eq!(decoded.frames, original.frames);
}
#[test]
fn complete_session_round_trip_preserves_audio_and_metadata() {
    let (s, lib) = audio_session();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.ondera");
    document::save(&s, &lib, &path).unwrap();
    let (loaded, samples) = document::load(&path).unwrap();
    assert_eq!(loaded.clips.len(), s.clips.len());
    assert_eq!(samples["src"].frames, lib["src"].frames);
    assert!(!loaded.transport.playing);
}
#[test]
fn save_with_missing_audio_does_not_replace_existing_file() {
    let (s, _) = audio_session();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("keep.ondera");
    std::fs::write(&path, "original").unwrap();
    assert!(document::save(&s, &Library::new(), &path).is_err());
    assert_eq!(std::fs::read_to_string(path).unwrap(), "original");
}
#[test]
fn interrupted_atomic_write_preserves_original() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("keep.ondera");
    std::fs::write(&path, "original").unwrap();
    assert!(document::atomic_write(&path, |f| {
        use std::io::Write;
        f.write_all(b"partial").unwrap();
        Err("simulated failure".into())
    })
    .is_err());
    assert_eq!(std::fs::read_to_string(path).unwrap(), "original");
}
#[test]
fn newer_session_version_is_rejected() {
    let json=serde_json::json!({"format":"ondera-session","version":99,"session":midi_session(),"audio":{}}).to_string();
    assert!(document::decode_session(&json)
        .unwrap_err()
        .contains("version"));
}
#[test]
fn realtime_render_seek_and_preview_allocate_nothing() {
    let mut r = Renderer::new(midi_session(), &Library::new(), 48000).unwrap();
    r.playing = true;
    ALLOCATIONS.with(|n| n.set(0));
    DEALLOCATIONS.with(|n| n.set(0));
    COUNTING.with(|v| v.set(true));
    for i in 0..4096 {
        if i == 2000 {
            r.locate(0.5);
            r.preview(0, 64, 100);
        }
        std::hint::black_box(r.next_frame());
    }
    COUNTING.with(|v| v.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    assert_eq!(DEALLOCATIONS.with(Cell::get), 0);
}
#[test]
fn offline_wav_uses_same_samples_as_playback() {
    let s = midi_session();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("render.wav");
    render::bounce(&s, &Library::new(), &path, 48000).unwrap();
    let mut wav = hound::WavReader::open(path).unwrap();
    assert_eq!(wav.spec().bits_per_sample, 24);
    assert_eq!(wav.duration(), 240000);
    let mut r = Renderer::new(s, &Library::new(), 48000).unwrap();
    r.playing = true;
    let mut samples = wav.samples::<i32>();
    for _ in 0..4800 {
        for v in r.next_frame() {
            assert_eq!(
                samples.next().unwrap().unwrap(),
                (v * 8_388_607.0).round() as i32
            );
        }
    }
}
#[test]
fn malicious_project_allocation_is_rejected() {
    let mut s = store::demo();
    for src in s.sources.values_mut() {
        src.duration_seconds = 14400.0;
    }
    assert!(s.validate().unwrap_err().contains("1 GiB"));
}
#[test]
fn invalid_audio_bytes_report_error() {
    assert!(audio::decode(vec![1, 2, 3, 4], None).is_err());
}
