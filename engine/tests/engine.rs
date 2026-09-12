use ondera_engine::{
    audio::{self, AudioBuffer, Library},
    document,
    dsp::{EFFECTS, INSTRUMENTS},
    host::scan,
    model::*,
    plugin::{NoteEvent, ProcessContext, Rack},
    render::{self, Renderer},
    stock,
    store::{self, Command, Store},
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    collections::HashMap,
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
fn offline(s: Session, library: &Library, rate: u32) -> (Renderer, render::OfflineRack) {
    render::offline(&s, library, rate).unwrap()
}
fn energy(r: &mut Renderer, rack: &mut Rack, frames: usize) -> f64 {
    r.playing = true;
    let mut total = 0.0;
    let mut block = [[0.0f32; 2]; 64];
    let mut done = 0;
    while done < frames {
        let n = (frames - done).min(64);
        r.render(rack, &mut block[..n]);
        total += block[..n]
            .iter()
            .map(|f| (f[0] * f[0] + f[1] * f[1]) as f64)
            .sum::<f64>();
        done += n;
    }
    total
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
fn v1_session_gains_insert_ids_and_buses_when_normalized() {
    let mut s: Session = serde_json::from_str(include_str!("fixtures/nightfall.json")).unwrap();
    assert!(s.strips["keys"].inserts.iter().all(|i| i.id.is_empty()));
    s.normalize();
    let ids: Vec<&str> = s
        .strips
        .values()
        .flat_map(|st| st.inserts.iter().map(|i| i.id.as_str()))
        .collect();
    assert!(ids.iter().all(|id| !id.is_empty()));
    let mut unique = ids.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), ids.len());
    assert_eq!(s.strips[BUS_A].inserts[0].name, "Space");
    assert_eq!(s.strips[BUS_B].inserts[0].plugin_id(), "stock:Echo");
    assert!(s.strips.contains_key(MASTER));
    // Normalizing again changes nothing.
    let before = serde_json::to_value(&s).unwrap();
    s.normalize();
    assert_eq!(serde_json::to_value(&s).unwrap(), before);
    // Every MIDI track needs an instrument; buses need their effects.
    let needs = s.needs();
    assert!(needs.iter().filter(|n| n.instrument).count() >= 1);
    assert!(needs.iter().any(|n| n.plugin == "stock:Space"));
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
fn amend_keeps_history_and_dirty_state() {
    let mut st = Store::new(midi_session()).unwrap();
    let revision = st.revision;
    st.amend(|s| {
        s.strips.get_mut(BUS_A).unwrap().inserts[0].blob = "AAAA".into();
    })
    .unwrap();
    assert!(!st.dirty());
    assert!(!st.can_undo());
    assert!(st.revision > revision);
    assert_eq!(st.session().strips[BUS_A].inserts[0].blob, "AAAA");
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
fn master_and_bus_strips_are_editable_and_validated() {
    let mut st = Store::new(midi_session()).unwrap();
    let mut strip = st.session().strips[MASTER].clone();
    strip
        .inserts
        .push(Insert::new("limiter-1".into(), "stock:Limiter", "Limiter"));
    st.dispatch(Command::SetStrip {
        track: MASTER.into(),
        strip,
    })
    .unwrap();
    st.dispatch(Command::SetMasterVolume(0.5)).unwrap();
    assert!(st.dispatch(Command::SetMasterVolume(2.0)).is_err());
    let needs = st.session().needs();
    assert!(needs.iter().any(|n| n.key == "limiter-1"));
    let mut too_many = st.session().strips[MASTER].clone();
    for i in 0..MAX_INSERTS {
        too_many
            .inserts
            .push(Insert::new(format!("x{i}"), "stock:Utility", "Utility"));
    }
    assert!(st
        .dispatch(Command::SetStrip {
            track: MASTER.into(),
            strip: too_many
        })
        .is_err());
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
        let (mut r, mut rack) = offline(midi_session(), &Library::new(), rate);
        r.playing = true;
        let mut block = [[0.0f32; 2]; 100];
        for _ in 0..rate / 100 {
            r.render(&mut rack, &mut block);
        }
        assert!((r.position() - 2.0).abs() < 1e-8);
    }
}
#[test]
fn cycle_wraps_without_drift_across_block_boundaries() {
    let mut s = midi_session();
    s.transport.cycle = true;
    s.transport.cycle_start_bar = 0.0;
    s.transport.cycle_end_bar = 0.25;
    let (mut r, mut rack) = offline(s, &Library::new(), 48000);
    r.playing = true;
    let mut block = [[0.0f32; 2]; 125];
    for _ in 0..960 {
        r.render(&mut rack, &mut block);
    }
    assert!(r.position() <= 1.000001);
    assert!((r.position() - 1.0).abs() < 1e-7 || r.position().abs() < 1e-7);
}
#[test]
fn locate_into_sustained_note_remains_audible() {
    let (mut r, mut rack) = offline(midi_session(), &Library::new(), 48000);
    r.locate(0.5);
    assert!(energy(&mut r, &mut rack, 4800) > 1.0);
}
#[test]
fn deleting_a_sounding_note_releases_it_on_graph_swap() {
    let (mut old, mut rack) = offline(midi_session(), &Library::new(), 48000);
    old.locate(0.0);
    assert!(energy(&mut old, &mut rack, 4800) > 1.0);
    // The same rack, a session without the note: the swap must send a note-off.
    let mut without = midi_session();
    without.clips.clear();
    let needs = without.needs();
    let slots: HashMap<String, u32> = needs
        .iter()
        .enumerate()
        .map(|(i, n)| (n.key.clone(), i as u32))
        .collect();
    let mut new = Renderer::new(without, &Library::new(), 48000, &slots).unwrap();
    new.adopt(&old);
    // Let the release run out, then measure a later window.
    energy(&mut new, &mut rack, 48000);
    assert!(energy(&mut new, &mut rack, 4800) < 1e-6);
}
#[test]
fn live_notes_survive_graph_swaps_and_stop_releases_them() {
    let (mut old, mut rack) = offline(midi_session(), &Library::new(), 48000);
    old.note(0, true, 64, 100);
    assert!(energy(&mut old, &mut rack, 4800) > 1.0);
    let mut renamed = midi_session();
    renamed.name = "Renamed".into();
    let slots: HashMap<String, u32> = renamed
        .needs()
        .iter()
        .enumerate()
        .map(|(i, n)| (n.key.clone(), i as u32))
        .collect();
    let mut new = Renderer::new(renamed, &Library::new(), 48000, &slots).unwrap();
    new.adopt(&old);
    new.playing = false;
    // Still sounding after the swap, without a retrigger.
    let later = {
        let mut block = [[0.0f32; 2]; 64];
        let mut total = 0.0;
        for _ in 0..75 {
            new.render(&mut rack, &mut block);
            total += block.iter().map(|f| (f[0] * f[0]) as f64).sum::<f64>();
        }
        total
    };
    assert!(later > 0.5);
    new.stop();
    energy(&mut new, &mut rack, 48000);
    new.playing = false;
    let mut block = [[0.0f32; 2]; 64];
    let mut silent = 0.0;
    for _ in 0..75 {
        new.render(&mut rack, &mut block);
        silent += block.iter().map(|f| (f[0] * f[0]) as f64).sum::<f64>();
    }
    assert!(silent < 1e-6);
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
    let (mut r, mut rack) = offline(s, &Library::new(), 48000);
    assert_eq!(energy(&mut r, &mut rack, 48000), 0.0);
}
#[test]
fn sends_feed_the_reverb_bus() {
    let mut s = midi_session();
    let strip = Strip {
        sends: vec![Send {
            level_db: Some(0.0),
            name: "Reverb".into(),
        }],
        ..Default::default()
    };
    s.strips.insert(s.tracks[0].id.clone(), strip);
    let dry = {
        let mut plain = s.clone();
        plain
            .strips
            .get_mut(&plain.tracks[0].id)
            .unwrap()
            .sends
            .clear();
        let (mut r, mut rack) = offline(plain, &Library::new(), 48000);
        energy(&mut r, &mut rack, 48000)
    };
    let (mut r, mut rack) = offline(s, &Library::new(), 48000);
    let wet = energy(&mut r, &mut rack, 48000);
    assert!(
        wet > dry * 1.05,
        "reverb return adds energy: {wet} vs {dry}"
    );
}
#[test]
fn solo_excludes_unsoloed_tracks() {
    let mut s = midi_session();
    let mut other = s.tracks[0].clone();
    other.id = "solo".into();
    other.solo = true;
    s.tracks.push(other);
    let (mut r, mut rack) = offline(s, &Library::new(), 48000);
    assert_eq!(energy(&mut r, &mut rack, 4800), 0.0);
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
        let (mut r, mut rack) = offline(s, &Library::new(), 48000);
        r.playing = true;
        let mut energy = 0.0;
        let mut block = [[0.0f32; 2]; 128];
        for _ in 0..375 {
            r.render(&mut rack, &mut block);
            for v in &block {
                assert!(v.iter().all(|s| s.is_finite() && s.abs() <= 0.98));
                energy += v[0] * v[0];
            }
        }
        assert!(energy > 0.01, "Silent instrument: {name}");
    }
}
#[test]
fn all_stock_effects_decay_and_are_finite() {
    for name in EFFECTS {
        let mut instance = stock::create(name, 48000).unwrap();
        let mut processor = instance.processor.take().unwrap();
        let ctx = ProcessContext {
            tempo: 120.0,
            numerator: 4,
            denominator: 4,
            ..Default::default()
        };
        let mut block = [[0.0f32; 2]; 64];
        let mut last = [0.0f32; 2];
        for i in 0..3750 {
            block.fill([0.0; 2]);
            if i == 0 {
                block[0] = [0.5, -0.5];
            }
            processor.process(&mut block, &[], &[], &ctx);
            assert!(
                block.iter().all(|f| f.iter().all(|v| v.is_finite())),
                "{name}"
            );
            last = block[63];
        }
        assert!(
            last.iter().all(|v| v.abs() < 0.001),
            "Effect tail does not decay: {name}"
        );
    }
}
#[test]
fn stock_parameters_change_the_sound_and_persist() {
    let mut instance = stock::create("Filter", 48000).unwrap();
    let mut processor = instance.processor.take().unwrap();
    let ctx = ProcessContext::default();
    let noise = |seed: &mut u32| {
        *seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        (*seed as f32 / u32::MAX as f32) * 0.5 - 0.25
    };
    let run = |processor: &mut Box<dyn ondera_engine::plugin::Processor>, cutoff: f64| {
        let mut seed = 7;
        let mut total = 0.0f64;
        let mut block = [[0.0f32; 2]; 64];
        for i in 0..200 {
            for f in &mut block {
                let v = noise(&mut seed);
                *f = [v, v];
            }
            let change = [ondera_engine::plugin::ParamChange {
                id: 1,
                value: cutoff,
            }];
            processor.process(&mut block, &[], if i == 0 { &change } else { &[] }, &ctx);
            if i > 20 {
                total += block.iter().map(|f| (f[0] * f[0]) as f64).sum::<f64>();
            }
        }
        total
    };
    let open = run(&mut processor, 18000.0);
    let closed = run(&mut processor, 60.0);
    assert!(
        closed < open * 0.2,
        "low cutoff removes energy: {closed} vs {open}"
    );
    // The editor half stores values as JSON state.
    instance.editor.set_value(1, 123.0);
    let saved = instance.editor.save().unwrap();
    let mut fresh = stock::create("Filter", 48000).unwrap();
    fresh.editor.load(&saved).unwrap();
    assert_eq!(fresh.editor.value(1), Some(123.0));
}
#[test]
fn instrument_processor_handles_note_on_and_off() {
    let mut instance = stock::create("Ondera Synth", 48000).unwrap();
    let mut processor = instance.processor.take().unwrap();
    let ctx = ProcessContext::default();
    let mut block = [[0.0f32; 2]; 64];
    let on = [NoteEvent {
        frame: 0,
        on: true,
        pitch: 60,
        velocity: 100,
        channel: 0,
    }];
    processor.process(&mut block, &on, &[], &ctx);
    let mut held = 0.0;
    for _ in 0..50 {
        block.fill([0.0; 2]);
        processor.process(&mut block, &[], &[], &ctx);
        held += block.iter().map(|f| (f[0] * f[0]) as f64).sum::<f64>();
    }
    assert!(held > 0.1);
    let off = [NoteEvent {
        frame: 0,
        on: false,
        pitch: 60,
        velocity: 0,
        channel: 0,
    }];
    block.fill([0.0; 2]);
    processor.process(&mut block, &off, &[], &ctx);
    for _ in 0..1500 {
        block.fill([0.0; 2]);
        processor.process(&mut block, &[], &[], &ctx);
    }
    block.fill([0.0; 2]);
    processor.process(&mut block, &[], &[], &ctx);
    assert!(block.iter().all(|f| f[0].abs() < 1e-5));
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
    let (mut s, lib) = audio_session();
    let mut strip = Strip::default();
    let mut insert = Insert::new("comp-1".into(), "stock:Ondera Comp", "Ondera Comp");
    insert.params.insert(0, -30.0);
    strip.inserts.push(insert);
    s.strips.insert(s.tracks[0].id.clone(), strip);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.ondera");
    document::save(&s, &lib, &path).unwrap();
    let (loaded, samples) = document::load(&path).unwrap();
    assert_eq!(loaded.clips.len(), s.clips.len());
    assert_eq!(samples["src"].frames, lib["src"].frames);
    assert!(!loaded.transport.playing);
    let insert = &loaded.strips[&s.tracks[0].id].inserts[0];
    assert_eq!(insert.params[&0], -30.0);
    assert_eq!(insert.id, "comp-1");
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
    let mut s = midi_session();
    let mut strip = Strip::default();
    for (i, name) in ["Channel EQ", "Space", "Echo", "Limiter"]
        .iter()
        .enumerate()
    {
        strip.inserts.push(Insert::new(
            format!("fx{i}"),
            &format!("stock:{name}"),
            name,
        ));
    }
    s.strips.insert(s.tracks[0].id.clone(), strip);
    use ondera_engine::automation::{
        AutomationLane, AutomationPoint, AutomationTarget, Interpolation,
    };
    for (id, target, min, max, start, end) in [
        (
            "volume",
            AutomationTarget::TrackVolume {
                track_id: s.tracks[0].id.clone(),
            },
            0.0,
            1.0,
            0.25,
            0.75,
        ),
        (
            "pan",
            AutomationTarget::TrackPan {
                track_id: s.tracks[0].id.clone(),
            },
            -100.0,
            100.0,
            -50.0,
            50.0,
        ),
        (
            "master",
            AutomationTarget::MasterVolume,
            0.0,
            1.0,
            0.5,
            0.75,
        ),
        (
            "plugin",
            AutomationTarget::PluginParameter {
                track_id: s.tracks[0].id.clone(),
                insert_id: "fx3".into(),
                plugin_id: "stock:Limiter".into(),
                parameter_id: 0,
            },
            -24.0,
            24.0,
            0.0,
            3.0,
        ),
    ] {
        s.automation.push(AutomationLane {
            id: id.into(),
            name: id.into(),
            target,
            min,
            max,
            manual_value: start,
            interpolation: Interpolation::Linear,
            enabled: true,
            points: vec![
                AutomationPoint {
                    id: "a".into(),
                    beat: 0.0,
                    value: start,
                },
                AutomationPoint {
                    id: "b".into(),
                    beat: 4.0,
                    value: end,
                },
            ],
        });
    }
    let (mut r, mut rack) = offline(s, &Library::new(), 48000);
    r.playing = true;
    let mut block = [[0.0f32; 2]; 256];
    let mut midi = ondera_engine::midi::MidiNotes::default();
    let mut midi_events = 0;
    ALLOCATIONS.with(|n| n.set(0));
    DEALLOCATIONS.with(|n| n.set(0));
    COUNTING.with(|v| v.set(true));
    midi.receive(&[0xb0, 64, 127], 0, |event| {
        std::hint::black_box(event);
        midi_events += 1;
    });
    for pitch in 0..128 {
        midi.receive(&[0x90, pitch, 100], 0, |event| {
            std::hint::black_box(event);
            midi_events += 1;
        });
        midi.receive(&[0x80, pitch, 0], 0, |event| {
            std::hint::black_box(event);
            midi_events += 1;
        });
    }
    midi.receive(&[0xb0, 64, 0], 1, |event| {
        std::hint::black_box(event);
        midi_events += 1;
    });
    midi.reset(|event| {
        std::hint::black_box(event);
        midi_events += 1;
    });
    for i in 0..64 {
        if i == 20 {
            r.locate(0.5);
            r.preview(0, 64, 100);
            r.note(0, true, 67, 90);
            rack.set_param(0, 0, 0.5);
        }
        if i == 40 {
            r.note(0, false, 67, 0);
            r.stop();
            r.playing = true;
        }
        r.render(&mut rack, &mut block);
        std::hint::black_box(&block);
    }
    COUNTING.with(|v| v.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    assert_eq!(DEALLOCATIONS.with(Cell::get), 0);
    assert_eq!(midi_events, 256);
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
    let (mut r, mut rack) = offline(s, &Library::new(), 48000);
    r.playing = true;
    r.locate(0.0);
    let mut samples = wav.samples::<i32>();
    let mut block = [[0.0f32; 2]; 256];
    for _ in 0..20 {
        r.render(&mut rack, &mut block);
        for frame in &block {
            for v in frame {
                assert_eq!(
                    samples.next().unwrap().unwrap(),
                    (v * 8_388_607.0).round() as i32
                );
            }
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
#[test]
fn plugin_cache_round_trips_and_lists_stock_first() {
    let dir = tempfile::tempdir().unwrap();
    std::env::set_var("ONDERA_DATA_DIR", dir.path());
    let cache = scan::Cache {
        version: 1,
        entries: vec![scan::CacheEntry {
            path: "/tmp/Fake.clap".into(),
            modified: 1,
            format: Some(ondera_engine::plugin::Format::Clap),
            descriptors: vec![ondera_engine::plugin::Descriptor {
                id: "clap:org.example.fake".into(),
                format: ondera_engine::plugin::Format::Clap,
                name: "Fake".into(),
                vendor: "Example".into(),
                path: "/tmp/Fake.clap".into(),
                instrument: false,
                effect: true,
                category: "reverb".into(),
            }],
            error: None,
        }],
        scanned_at: 0,
    };
    scan::store_cache(&cache).unwrap();
    assert!(scan::cache_path().starts_with(dir.path()));
    let loaded = scan::load_cache();
    assert_eq!(loaded.descriptors().len(), 1);
    assert_eq!(scan::lookup("clap:org.example.fake").unwrap().name, "Fake");
    let installed = scan::installed();
    assert_eq!(installed[0].format, ondera_engine::plugin::Format::Stock);
    assert!(installed.iter().any(|d| d.id == "clap:org.example.fake"));
    assert!(ondera_engine::host::instantiate("clap:org.example.fake", "Fake", 48000).is_err());
    std::env::remove_var("ONDERA_DATA_DIR");
}
