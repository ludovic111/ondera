use ondera_engine::{
    audio::Library,
    model::{Clip, ClipData, Insert, Note, Send, Strip, BUS_A, BUS_B, MASTER},
    plugin::{NoteEvent, ParamChange, ProcessContext, Processor, Rack},
    render::Renderer,
    store,
};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

struct DelayProcessor {
    samples: Vec<[f32; 2]>,
    position: usize,
    synth: bool,
}
impl DelayProcessor {
    fn new(latency: usize, synth: bool) -> Self {
        Self {
            samples: vec![[0.0; 2]; latency],
            position: 0,
            synth,
        }
    }
}
impl Processor for DelayProcessor {
    fn process(
        &mut self,
        audio: &mut [[f32; 2]],
        notes: &[NoteEvent],
        _: &[ParamChange],
        _: &ProcessContext,
    ) {
        if self.synth {
            for note in notes.iter().filter(|note| note.on) {
                audio[note.frame as usize] = [0.1; 2];
            }
        }
        if !self.samples.is_empty() {
            for frame in audio {
                std::mem::swap(frame, &mut self.samples[self.position]);
                self.position = (self.position + 1) % self.samples.len();
            }
        }
    }
}
fn fixture() -> ondera_engine::model::Session {
    let mut session = store::empty();
    session.tracks.truncate(1);
    session.tracks[0].kind = "midi".into();
    session.tracks[0].volume = 0.75;
    session.tracks[0].pan = 0.0;
    session.strips.clear();
    session.master_volume = 0.75;
    session.transport.tempo = 120.0;
    session.transport.metronome = false;
    let track_id = session.tracks[0].id.clone();
    session.clips = vec![Clip {
        id: "first".into(),
        name: "Impulse".into(),
        agent: false,
        track_id,
        start_bar: 0.0,
        length_bars: 1.0,
        data: ClipData::Midi {
            notes: vec![Note {
                id: "note".into(),
                start: 0.0,
                length: 0.2,
                pitch: 60,
                velocity: 100,
                agent: false,
            }],
        },
    }];
    session
}
#[test]
fn delay_compensation_aligns_tracks_sends_and_master_to_one_sample() {
    let mut session = fixture();
    let mut other = session.tracks[0].clone();
    other.id = "other".into();
    session.tracks.push(other);
    let mut second = session.clips[0].clone();
    second.id = "second".into();
    second.track_id = "other".into();
    session.clips.push(second);
    let mut slots = HashMap::new();
    for (index, track) in session.tracks.iter().enumerate() {
        let strip = Strip {
            sends: vec![
                Send {
                    name: BUS_A.into(),
                    level_db: Some(0.0),
                },
                Send {
                    name: BUS_B.into(),
                    level_db: Some(0.0),
                },
            ],
            ..Default::default()
        };
        slots.insert(strip.synth_key(&track.id), index as u32);
        session.strips.insert(track.id.clone(), strip);
    }
    for (id, slot) in [(BUS_A, 2), (BUS_B, 3), (MASTER, 4)] {
        session.strips.insert(
            id.into(),
            Strip {
                inserts: vec![Insert::new(id.into(), "stock:Echo", "Echo")],
                ..Default::default()
            },
        );
        slots.insert(id.into(), slot);
    }
    let mut renderer = Renderer::new(session, &Library::new(), 48000, &slots).unwrap();
    renderer
        .set_latencies(&HashMap::from([(0, 0), (1, 19), (2, 13), (3, 7), (4, 5)]))
        .unwrap();
    assert_eq!(renderer.latency_samples(), 37);
    let mut rack = Rack::new(5);
    for (slot, latency) in [0, 19, 13, 7, 5].into_iter().enumerate() {
        rack.mount(
            slot as u32,
            Box::new(DelayProcessor::new(latency, slot < 2)),
        );
    }
    renderer.playing = true;
    let mut block = [[0.0; 2]; 256];
    renderer.render(&mut rack, &mut block);
    for (index, frame) in block.iter().enumerate() {
        let expected = if index == 37 { 0.6 } else { 0.0 };
        assert!(
            (frame[0] - expected).abs() < 1e-6,
            "sample {index}: {frame:?}"
        );
        assert_eq!(frame[0], frame[1]);
    }
}
#[test]
fn unreasonable_delay_compensation_is_rejected_without_allocating() {
    let session = fixture();
    let key = Strip::default().synth_key(&session.tracks[0].id);
    let mut renderer =
        Renderer::new(session, &Library::new(), 48000, &HashMap::from([(key, 0)])).unwrap();
    assert!(renderer
        .set_latencies(&HashMap::from([(0, u32::MAX)]))
        .is_err());
    assert_eq!(renderer.latency_samples(), 0);
}

struct NoteObserver(Arc<Mutex<Vec<NoteEvent>>>);
impl Processor for NoteObserver {
    fn process(
        &mut self,
        _: &mut [[f32; 2]],
        notes: &[NoteEvent],
        _: &[ParamChange],
        _: &ProcessContext,
    ) {
        self.0.lock().unwrap().extend_from_slice(notes);
    }
}
#[test]
fn preview_releases_and_sequence_events_are_chronological() {
    let mut session = fixture();
    let ClipData::Midi { notes } = &mut session.clips[0].data else {
        unreachable!()
    };
    notes[0].start = 14090.0 / 24000.0;
    let key = Strip::default().synth_key(&session.tracks[0].id);
    let mut renderer =
        Renderer::new(session, &Library::new(), 48000, &HashMap::from([(key, 0)])).unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut rack = Rack::new(1);
    rack.mount(0, Box::new(NoteObserver(events.clone())));
    renderer.preview(0, 72, 96);
    renderer.playing = true;
    let mut block = [[0.0; 2]; 256];
    for _ in 0..56 {
        renderer.render(&mut rack, &mut block);
    }
    // Position 14336; preview ends at frame 64. Place another sequenced start
    // at frame 10 of that same block, before the queued preview note-off.
    let mut updated = renderer.session().clone();
    let ClipData::Midi { notes } = &mut updated.clips[0].data else {
        unreachable!()
    };
    notes.push(Note {
        id: "next".into(),
        start: 14346.0 / 24000.0,
        length: 0.1,
        pitch: 64,
        velocity: 96,
        agent: false,
    });
    let key = Strip::default().synth_key(&updated.tracks[0].id);
    let mut updated =
        Renderer::new(updated, &Library::new(), 48000, &HashMap::from([(key, 0)])).unwrap();
    updated.adopt(&renderer);
    events.lock().unwrap().clear();
    updated.render(&mut rack, &mut block);
    let events = events.lock().unwrap();
    assert!(events
        .iter()
        .any(|note| note.on && note.pitch == 64 && note.frame == 10));
    assert!(events
        .iter()
        .any(|note| !note.on && note.pitch == 72 && note.frame == 64));
    assert!(events.windows(2).all(|pair| pair[0].frame <= pair[1].frame));
}

struct Lifecycle(Arc<AtomicUsize>, Arc<AtomicUsize>);
impl Processor for Lifecycle {
    fn stop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
    fn process(
        &mut self,
        _: &mut [[f32; 2]],
        _: &[NoteEvent],
        _: &[ParamChange],
        _: &ProcessContext,
    ) {
    }
}
impl Drop for Lifecycle {
    fn drop(&mut self) {
        assert_eq!(self.0.load(Ordering::SeqCst), 1);
        self.1.fetch_add(1, Ordering::SeqCst);
    }
}
#[test]
fn rack_stops_processors_before_offline_destruction_and_drain() {
    for drain in [false, true] {
        let stopped = Arc::new(AtomicUsize::new(0));
        let dropped = Arc::new(AtomicUsize::new(0));
        let mut rack = Rack::new(1);
        rack.mount(0, Box::new(Lifecycle(stopped.clone(), dropped.clone())));
        if drain {
            drop(rack.drain());
        }
        drop(rack);
        assert_eq!(stopped.load(Ordering::SeqCst), 1);
        assert_eq!(dropped.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn replacing_an_instrument_chases_a_sustained_sequence_note() {
    let session = fixture();
    let key = Strip::default().synth_key(&session.tracks[0].id);
    let mut old = Renderer::new(
        session.clone(),
        &Library::new(),
        48000,
        &HashMap::from([(key.clone(), 0)]),
    )
    .unwrap();
    let old_events = Arc::new(Mutex::new(Vec::new()));
    let new_events = Arc::new(Mutex::new(Vec::new()));
    let mut rack = Rack::new(2);
    rack.mount(0, Box::new(NoteObserver(old_events)));
    rack.mount(1, Box::new(NoteObserver(new_events.clone())));
    old.playing = true;
    let mut block = [[0.0; 2]; 256];
    old.render(&mut rack, &mut block);
    let mut new =
        Renderer::new(session, &Library::new(), 48000, &HashMap::from([(key, 1)])).unwrap();
    new.adopt(&old);
    new.render(&mut rack, &mut block);
    assert!(new_events
        .lock()
        .unwrap()
        .iter()
        .any(|note| note.on && note.pitch == 60 && note.frame == 0));
}

#[test]
fn pending_live_note_survives_renderer_replacement_before_audio_block() {
    let session = fixture();
    let key = Strip::default().synth_key(&session.tracks[0].id);
    let slots = HashMap::from([(key, 0)]);
    let mut old = Renderer::new(session.clone(), &Library::new(), 48000, &slots).unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut rack = Rack::new(1);
    rack.mount(0, Box::new(NoteObserver(events.clone())));
    old.note(0, true, 72, 101);
    let mut new = Renderer::new(session, &Library::new(), 48000, &slots).unwrap();
    new.adopt(&old);
    new.render(&mut rack, &mut [[0.0; 2]; 256]);
    assert_eq!(
        events
            .lock()
            .unwrap()
            .iter()
            .filter(|note| note.on && note.pitch == 72 && note.velocity == 101)
            .count(),
        1
    );
}

struct ParameterObserver(Arc<Mutex<Vec<ParamChange>>>);
#[test]
fn live_pedal_keeps_stock_audio_sounding_after_key_up_and_releases_on_pedal_up() {
    let session = fixture();
    let route = ondera_engine::midi::route_id(&session.tracks[0].id);
    let (mut renderer, mut rack) =
        ondera_engine::render::offline(&session, &Library::new(), 48000).unwrap();
    let mut midi = ondera_engine::midi::MidiNotes::default();
    for bytes in [[0x90, 60, 100], [0xb0, 64, 127], [0x80, 60, 0]] {
        midi.receive(&bytes, route, |event| {
            renderer.routed_note(event.route, event.on, event.pitch, event.velocity)
        });
    }
    let mut audio = vec![[0.0; 2]; 48000];
    renderer.render(&mut rack, &mut audio);
    let held = audio[44000..]
        .iter()
        .flatten()
        .map(|sample| sample.abs())
        .fold(0.0f32, f32::max);
    assert!(
        held > 0.01,
        "Sustain must keep the stock instrument audible: {held}"
    );
    midi.receive(&[0xb0, 64, 0], route, |event| {
        renderer.routed_note(event.route, event.on, event.pitch, event.velocity)
    });
    renderer.render(&mut rack, &mut audio);
    let released = audio[44000..]
        .iter()
        .flatten()
        .map(|sample| sample.abs())
        .fold(0.0f32, f32::max);
    assert!(
        released < held * 0.01,
        "Pedal release must decay: held={held}, released={released}"
    );
}
#[test]
fn seeking_a_dense_chord_limits_chased_voices_and_releases_every_started_note() {
    let mut session = fixture();
    let ClipData::Midi { notes } = &mut session.clips[0].data else {
        unreachable!()
    };
    notes.clear();
    for i in 0..257 {
        notes.push(Note {
            id: format!("voice-{i}"),
            start: 0.0,
            length: 1.0,
            pitch: 60,
            velocity: 100,
            agent: false,
        });
    }
    let key = Strip::default().synth_key(&session.tracks[0].id);
    let mut renderer =
        Renderer::new(session, &Library::new(), 48000, &HashMap::from([(key, 0)])).unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut rack = Rack::new(1);
    rack.mount(0, Box::new(NoteObserver(events.clone())));
    renderer.locate(0.5);
    renderer.render(&mut rack, &mut [[0.0; 2]; 256]);
    renderer.stop();
    renderer.render(&mut rack, &mut [[0.0; 2]; 256]);
    let events = events.lock().unwrap();
    assert_eq!(events.iter().filter(|note| note.on).count(), 256);
    assert_eq!(events.iter().filter(|note| !note.on).count(), 256);
    assert_eq!(renderer.voice_overflows, 1);
    assert_eq!(renderer.note_overflows, 0);
}
#[test]
fn routed_note_release_finds_original_track_after_reordering_the_session() {
    let mut session = fixture();
    let mut second = session.tracks[0].clone();
    second.id = "second-track".into();
    session.tracks.push(second);
    let first = session.tracks[0].id.clone();
    let slots = HashMap::from([
        (Strip::default().synth_key(&first), 0),
        (Strip::default().synth_key("second-track"), 1),
    ]);
    let mut renderer = Renderer::new(session.clone(), &Library::new(), 48000, &slots).unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut rack = Rack::new(2);
    rack.mount(0, Box::new(NoteObserver(events.clone())));
    renderer.routed_note(ondera_engine::midi::route_id(&first), true, 72, 100);
    renderer.render(&mut rack, &mut [[0.0; 2]; 256]);
    session.tracks.swap(0, 1);
    let mut updated = Renderer::new(session, &Library::new(), 48000, &slots).unwrap();
    updated.adopt(&renderer);
    updated.routed_note(ondera_engine::midi::route_id(&first), false, 72, 0);
    updated.render(&mut rack, &mut [[0.0; 2]; 256]);
    assert_eq!(
        events
            .lock()
            .unwrap()
            .iter()
            .map(|note| note.on)
            .collect::<Vec<_>>(),
        vec![true, false]
    );
}
#[test]
fn same_callback_live_taps_release_after_attack_even_with_one_frame_blocks() {
    let session = fixture();
    let key = Strip::default().synth_key(&session.tracks[0].id);
    for block_size in [1, 256] {
        let mut renderer = Renderer::new(
            session.clone(),
            &Library::new(),
            48000,
            &HashMap::from([(key.clone(), 0)]),
        )
        .unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut rack = Rack::new(1);
        rack.mount(0, Box::new(NoteObserver(events.clone())));
        renderer.note(0, true, 72, 100);
        renderer.note(0, false, 72, 0);
        renderer.note(0, true, 72, 100);
        renderer.note(0, false, 72, 0);
        for _ in 0..3 {
            renderer.render(&mut rack, &mut vec![[0.0; 2]; block_size]);
        }
        let events = events.lock().unwrap();
        assert_eq!(
            events.iter().map(|note| note.on).collect::<Vec<_>>(),
            vec![true, false, true, false]
        );
        assert_eq!(
            events.iter().map(|note| note.frame).collect::<Vec<_>>(),
            if block_size == 1 {
                vec![0, 0, 0, 0]
            } else {
                vec![0, 1, 1, 2]
            }
        );
        assert_eq!(renderer.note_overflows, 0);
    }
}
impl Processor for ParameterObserver {
    fn process(
        &mut self,
        _: &mut [[f32; 2]],
        _: &[NoteEvent],
        params: &[ParamChange],
        _: &ProcessContext,
    ) {
        self.0.lock().unwrap().extend_from_slice(params);
    }
}
#[test]
fn offline_parameter_reservation_keeps_every_saved_knob() {
    let values = Arc::new(Mutex::new(Vec::new()));
    let mut rack = Rack::with_parameter_capacity(1, 300);
    rack.mount(0, Box::new(ParameterObserver(values.clone())));
    for id in 0..300 {
        rack.set_param(0, id, id as f64 / 300.0);
    }
    rack.process(0, &mut [[0.0; 2]; 256], &[], &ProcessContext::default());
    let values = values.lock().unwrap();
    assert_eq!(values.len(), 300);
    assert_eq!(values.last().unwrap().id, 299);
}

#[test]
fn stock_state_load_changes_audio_and_parameter_overrides_still_win() {
    let mut instance = ondera_engine::stock::create("Utility", 48000).unwrap();
    let mut rack = Rack::new(1);
    rack.mount(0, instance.processor.take().unwrap());
    instance.editor.set_value(0, -12.0);
    let blob = instance.editor.save().unwrap();
    instance.editor.set_value(0, 0.0);
    instance.editor.load(&blob).unwrap();
    let mut audio = [[0.2; 2]; 256];
    rack.process(0, &mut audio, &[], &ProcessContext::default());
    assert!((audio[255][0] - 0.2 * 10f32.powf(-12.0 / 20.0)).abs() < 1e-6);
    instance.editor.load(&blob).unwrap();
    rack.set_param(0, 0, 0.0);
    audio.fill([0.2; 2]);
    rack.process(0, &mut audio, &[], &ProcessContext::default());
    assert!((audio[255][0] - 0.2).abs() < 1e-6);
}
#[test]
fn stock_limiter_exposes_its_lookahead_to_the_graph() {
    for rate in [44100, 48000, 96000] {
        let instance = ondera_engine::stock::create("Limiter", rate).unwrap();
        assert_eq!(instance.editor.latency(), rate / 1000);
        assert_eq!(
            instance.editor.latency(),
            instance.processor.as_ref().unwrap().latency()
        );
    }
}
