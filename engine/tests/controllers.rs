//! MIDI controllers end to end: clips that carry them, the renderer that plays them at their
//! sample through the ABI 2 path, chasing on locate, rest at stop, the stock instruments
//! that answer them, and Standard MIDI Files that carry them both ways.
use ondera_engine::{
    audio::Library,
    host::native,
    midi_file,
    model::{Clip, ClipData, Controller, ControllerKind, Note, Session},
    plugin::{event, Descriptor, Event, Format, ProcessContext, Rack},
    render::Renderer,
    stock, store,
};
use ondera_plugin::{ffi, prelude::*};
use std::{collections::HashMap, sync::Mutex};

/// Every event an instance received, at its absolute sample.
static HEARD: Mutex<Vec<(i64, Event)>> = Mutex::new(Vec::new());

/// An ABI 2 instrument that writes down what reaches it.
struct Listener;
impl Plugin for Listener {
    const INFO: Info = Info::instrument("org.ondera.tests.listener", "Listener", "Tests");
    fn params() -> Vec<ParamSpec> {
        vec![]
    }
    fn new(_: f64) -> Self {
        Self
    }
    fn set_param(&mut self, _: usize, _: f64) {}
    fn process(&mut self, _: &mut [[f32; 2]], _: &[NoteEvent], _: &ProcessContext) {}
    fn process_events(
        &mut self,
        _: &mut [[f32; 2]],
        events: &[Event],
        _: &[TimedParam],
        ctx: &ProcessContext,
    ) {
        let mut heard = HEARD.lock().unwrap();
        for e in events {
            heard.push((ctx.sample_time + e.frame as i64, *e));
        }
    }
}
static LISTENER: ffi::PluginVTable2 = ffi::vtable2::<Listener>();

fn point(kind: ControllerKind, number: Option<u8>, time: f64, value: i16) -> Controller {
    Controller {
        id: format!("{}{number:?}@{time}", kind.as_str()),
        kind,
        number,
        time,
        value,
        agent: false,
        channel: 0,
    }
}
fn session(controllers: Vec<Controller>) -> Session {
    let mut s = store::empty();
    s.tracks.retain(|t| t.kind == "midi");
    s.tracks.truncate(1);
    s.transport.tempo = 120.0;
    s.transport.metronome = false;
    s.transport.cycle = false;
    s.clips.push(Clip {
        id: "clip".into(),
        name: "Bends".into(),
        agent: false,
        track_id: s.tracks[0].id.clone(),
        start_bar: 0.0,
        length_bars: 1.0,
        data: ClipData::Midi {
            notes: vec![Note {
                id: "note".into(),
                start: 0.0,
                length: 4.0,
                pitch: 60,
                velocity: 100,
                agent: false,
                channel: 0,
            }],
            controllers,
        },
    });
    s
}
/// A renderer whose only instrument is the listener, through the real native ABI 2 adapter.
fn listening(s: Session) -> (Renderer, Rack) {
    let manifest = ffi::Manifest::of::<Listener>();
    let descriptor = Descriptor {
        id: "native:org.ondera.tests.listener".into(),
        format: Format::Native,
        name: "Listener".into(),
        vendor: "Tests".into(),
        path: "in-process".into(),
        instrument: true,
        effect: false,
        category: "Instrument".into(),
    };
    let mut instance = native::instance_from(&LISTENER, &manifest, descriptor, 48000).unwrap();
    let mut rack = Rack::new(1);
    rack.mount(0, instance.processor.take().unwrap());
    std::mem::forget(instance.editor);
    let id = &s.tracks[0].id;
    let key = s.strips.get(id).cloned().unwrap_or_default().synth_key(id);
    let renderer = Renderer::new(s, &Library::new(), 48000, &HashMap::from([(key, 0)])).unwrap();
    (renderer, rack)
}
fn controls(heard: &[(i64, Event)]) -> Vec<(i64, u8, u8, i16)> {
    heard
        .iter()
        .filter(|(_, e)| !e.is_note())
        .map(|(at, e)| {
            let value = if e.kind == event::PITCH_BEND {
                e.bend
            } else {
                e.value as i16
            };
            (*at, e.kind, e.key, value)
        })
        .collect()
}

/// One test drives the shared listener so the recording is never interleaved.
#[test]
fn the_renderer_plays_chases_and_rests_controllers_at_their_samples() {
    // At 120 BPM and 48 kHz a beat is 24 000 samples.
    let s = session(vec![
        point(ControllerKind::Cc, Some(1), 0.01, 20),
        point(ControllerKind::Bend, None, 0.02, 4096),
        point(ControllerKind::Cc, Some(64), 0.03, 127),
        point(ControllerKind::Pressure, None, 3.0, 70),
    ]);
    let (mut renderer, mut rack) = listening(s);
    renderer.playing = true;
    let mut block = [[0.0f32; 2]; 256];
    HEARD.lock().unwrap().clear();
    for _ in 0..4 {
        renderer.render(&mut rack, &mut block);
    }
    let heard = HEARD.lock().unwrap().clone();
    assert_eq!(
        controls(&heard),
        vec![
            (240, event::CONTROL, 1, 20),
            (480, event::PITCH_BEND, 0, 4096),
            (720, event::CONTROL, 64, 127),
        ],
        "Each controller lands on its own sample, across block boundaries"
    );
    let note_on = heard
        .iter()
        .find(|(_, e)| e.kind == event::NOTE_ON)
        .unwrap();
    assert_eq!(note_on.0, 0);

    // Locate into the middle of the clip: the values in force there arrive first, once.
    HEARD.lock().unwrap().clear();
    renderer.stop();
    let stopped = controls(&HEARD.lock().unwrap().clone());
    renderer.render(&mut rack, &mut block);
    let stopped: Vec<_> = stopped
        .into_iter()
        .chain(controls(&HEARD.lock().unwrap().clone()))
        .map(|(_, kind, key, value)| (kind, key, value))
        .collect();
    assert!(stopped.contains(&(event::CONTROL, 64, 0)), "{stopped:?}");
    assert!(stopped.contains(&(event::PITCH_BEND, 0, 0)), "{stopped:?}");
    assert!(
        !stopped
            .iter()
            .any(|(kind, key, _)| *kind == event::CONTROL && *key == 1),
        "The mod wheel has no rest value and is left alone: {stopped:?}"
    );

    HEARD.lock().unwrap().clear();
    renderer.locate(2.0);
    renderer.playing = true;
    renderer.render(&mut rack, &mut block);
    let heard = HEARD.lock().unwrap().clone();
    let chased: Vec<_> = controls(&heard)
        .into_iter()
        .map(|(_, kind, key, value)| (kind, key, value))
        .collect();
    // Stop returned the bend and the pedal to rest; the mod wheel is still where the clip
    // left it, so it is not sent again.
    assert_eq!(
        chased,
        vec![(event::CONTROL, 64, 127), (event::PITCH_BEND, 0, 4096)],
        "Only what the instrument lacks at beat 2"
    );
    let first_note = heard.iter().position(|(_, e)| e.is_note()).unwrap();
    let last_control = heard.iter().rposition(|(_, e)| !e.is_note()).unwrap();
    assert!(
        last_control < first_note,
        "Controllers are in place before the chased note starts"
    );

    // A second locate to the same place sends nothing new: the instrument already has it.
    HEARD.lock().unwrap().clear();
    renderer.locate(2.5);
    renderer.render(&mut rack, &mut block);
    assert!(controls(&HEARD.lock().unwrap()).is_empty());

    // The clip leaves the bend and the pedal at rest when it ends (beat 4), and a pressure
    // point at beat 3 plays on the way.
    HEARD.lock().unwrap().clear();
    renderer.locate(2.99);
    for _ in 0..(24000 * 11 / 10 / 256 + 2) {
        renderer.render(&mut rack, &mut block);
    }
    let ended: Vec<_> = controls(&HEARD.lock().unwrap())
        .into_iter()
        .map(|(at, kind, key, value)| (at, (kind, key, value)))
        .collect();
    let at = |target: (u8, u8, i16)| ended.iter().find(|(_, e)| *e == target).map(|(a, _)| *a);
    let pressure = at((event::CHANNEL_PRESSURE, 0, 70)).expect("pressure at beat 3");
    let bend_rest = at((event::PITCH_BEND, 0, 0)).expect("bend returns at the clip end");
    let pedal_rest = at((event::CONTROL, 64, 0)).expect("pedal lifts at the clip end");
    assert_eq!(bend_rest - pressure, 24000);
    assert_eq!(pedal_rest, bend_rest);

    // Live controllers reach the instrument at once, and only the controller slots it knows.
    HEARD.lock().unwrap().clear();
    renderer.control(0, Event::control(0, 11, 99));
    renderer.control(0, Event::note_on(0, 1, 1));
    renderer.render(&mut rack, &mut block);
    assert_eq!(
        controls(&HEARD.lock().unwrap())
            .into_iter()
            .map(|(_, kind, key, value)| (kind, key, value))
            .collect::<Vec<_>>(),
        vec![(event::CONTROL, 11, 99)]
    );
}

#[test]
fn old_files_load_and_files_without_controllers_stay_byte_identical() {
    let text = include_str!("fixtures/nightfall.json");
    let demo: Session = serde_json::from_str(text).unwrap();
    let written = serde_json::to_value(&demo).unwrap();
    for clip in written["clips"].as_array().unwrap() {
        if clip["data"]["kind"] == "midi" {
            let mut keys: Vec<&String> = clip["data"].as_object().unwrap().keys().collect();
            keys.sort();
            assert_eq!(
                keys,
                ["kind", "notes"],
                "No controllers key in a clip without them"
            );
        }
    }
    // Clips are a list, so their bytes are stable (strips and sources are maps).
    let bytes = serde_json::to_string(&demo.clips).unwrap();
    let again: Session = serde_json::from_str(&serde_json::to_string(&demo).unwrap()).unwrap();
    assert_eq!(serde_json::to_string(&again.clips).unwrap(), bytes);

    let with = session_json_round_trip(session(vec![
        point(ControllerKind::Cc, Some(11), 0.5, 64),
        point(ControllerKind::Bend, None, 1.0, -8192),
    ]));
    let ClipData::Midi { controllers, .. } = &with.clips[0].data else {
        panic!()
    };
    assert_eq!(controllers.len(), 2);
    assert_eq!(controllers[1].value, -8192);
    let json = serde_json::to_value(&with).unwrap();
    let first = &json["clips"][0]["data"]["controllers"][0];
    assert_eq!(first["kind"], "cc");
    assert_eq!(first["number"], 11);
    assert!(first.get("agent").is_none());
    assert!(json["clips"][0]["data"]["controllers"][1]
        .get("number")
        .is_none());

    let mut broken = with.clone();
    if let ClipData::Midi { controllers, .. } = &mut broken.clips[0].data {
        controllers[0].value = 200;
    }
    assert!(broken.validate().is_err());
}
fn session_json_round_trip(s: Session) -> Session {
    serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap()
}

#[test]
fn standard_midi_files_carry_controllers_both_ways() {
    let s = session(vec![
        point(ControllerKind::Cc, Some(1), 0.5, 100),
        point(ControllerKind::Cc, Some(64), 1.0, 127),
        point(ControllerKind::Bend, None, 1.5, -4096),
        point(ControllerKind::Pressure, None, 2.0, 30),
    ]);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("controllers.mid");
    let report = midi_file::export(&s, &path, None).unwrap();
    // Four points and the rest the clip returns the pedal, bend and pressure to at its end.
    assert_eq!(report.controller_count, 7);
    let (command, imported) = midi_file::import(
        &path,
        &store::empty(),
        &midi_file::ImportOptions::default(),
        false,
    )
    .unwrap();
    assert_eq!(imported.controllers, 7);
    let mut target = store::Store::new(store::empty()).unwrap();
    target.dispatch(command).unwrap();
    let clip = target.session().clips.last().unwrap();
    let ClipData::Midi { controllers, .. } = &clip.data else {
        panic!()
    };
    let summary: Vec<_> = controllers
        .iter()
        .map(|c| (c.kind, c.number, c.time, c.value))
        .collect();
    for expected in [
        (ControllerKind::Cc, Some(1), 0.5, 100),
        (ControllerKind::Cc, Some(64), 1.0, 127),
        (ControllerKind::Bend, None, 1.5, -4096),
        (ControllerKind::Pressure, None, 2.0, 30),
        (ControllerKind::Cc, Some(64), 4.0, 0),
        (ControllerKind::Bend, None, 4.0, 0),
    ] {
        assert!(summary.contains(&expected), "{summary:?}");
    }
}

/// Zero crossings per second of the left channel.
fn pitch_of(audio: &[[f32; 2]]) -> f64 {
    let crossings = audio
        .windows(2)
        .filter(|w| w[0][0] <= 0.0 && w[1][0] > 0.0)
        .count();
    crossings as f64 * 48000.0 / audio.len() as f64
}
fn play(
    processor: &mut Box<dyn ondera_engine::plugin::Processor>,
    first: &[Event],
    blocks: usize,
) -> Vec<[f32; 2]> {
    let ctx = ProcessContext::default();
    let mut out = vec![];
    for i in 0..blocks {
        let mut block = [[0.0f32; 2]; 256];
        processor.process(&mut block, if i == 0 { first } else { &[] }, &[], &ctx);
        out.extend_from_slice(&block);
    }
    out
}
fn energy(audio: &[[f32; 2]]) -> f64 {
    audio.iter().map(|f| (f[0] * f[0]) as f64).sum::<f64>() / audio.len() as f64
}

#[test]
fn stock_instruments_bend_two_semitones_and_hold_notes_on_the_pedal() {
    let mut sub = stock::create("Sub Bass 808", 48000).unwrap();
    let mut processor = sub.processor.take().unwrap();
    let straight = play(&mut processor, &[Event::note_on(0, 69, 100)], 200);
    assert!((pitch_of(&straight[9600..]) - 440.0).abs() < 3.0);
    processor.process(
        &mut [[0.0; 2]; 16],
        &[Event::note_off(0, 69)],
        &[],
        &ProcessContext::default(),
    );
    processor.reset();
    let bent = play(
        &mut processor,
        &[Event::pitch_bend(0, 1.0), Event::note_on(0, 69, 100)],
        200,
    );
    let up = 440.0 * 2f64.powf(8191.0 / 8192.0 * 2.0 / 12.0);
    assert!(
        (pitch_of(&bent[9600..]) - up).abs() < 3.0,
        "{} vs {up}",
        pitch_of(&bent[9600..])
    );
    let down = play(&mut processor, &[Event::pitch_bend(0, -1.0)], 200);
    let expected = 440.0 * 2f64.powf(-2.0 / 12.0);
    assert!((pitch_of(&down[9600..]) - expected).abs() < 3.0);
    processor.reset();

    // The pedal keeps a released key sounding until it lifts.
    let mut synth = stock::create("Ondera Synth", 48000).unwrap();
    let mut processor = synth.processor.take().unwrap();
    play(
        &mut processor,
        &[Event::control(0, 64, 127), Event::note_on(0, 60, 100)],
        20,
    );
    let held = play(&mut processor, &[Event::note_off(0, 60)], 400);
    assert!(
        energy(&held[held.len() - 4800..]) > 1e-4,
        "The pedal holds the note"
    );
    let lifted = play(&mut processor, &[Event::control(0, 64, 0)], 400);
    assert!(
        energy(&lifted[lifted.len() - 4800..]) < 1e-8,
        "Lifting the pedal releases it"
    );

    // The mod wheel adds vibrato: the pitch wobbles around where it was.
    let mut sub = stock::create("Sub Bass 808", 48000).unwrap();
    let mut processor = sub.processor.take().unwrap();
    let wobble = play(
        &mut processor,
        &[Event::control(0, 1, 127), Event::note_on(0, 69, 100)],
        400,
    );
    let windows: Vec<f64> = wobble[9600..].chunks(2400).map(pitch_of).collect();
    let spread = windows.iter().cloned().fold(0.0, f64::max)
        - windows.iter().cloned().fold(f64::MAX, f64::min);
    assert!(spread > 1.0, "{windows:?}");
    assert!((pitch_of(&wobble[9600..]) - 440.0).abs() < 3.0);
}

#[test]
fn stock_instruments_hear_controllers_and_effects_keep_their_single_instance() {
    for name in ondera_engine::dsp::INSTRUMENTS {
        let mut instance = stock::create(name, 48000).unwrap();
        let mut processor = instance.processor.take().unwrap();
        // Nothing breaks when every kind arrives, known or not.
        play(
            &mut processor,
            &[
                Event::control(0, 1, 90),
                Event::pitch_bend(0, 0.25),
                Event::channel_pressure(0, 40),
                Event::poly_pressure(0, 60, 10),
                Event::new(0, 42, 0, 0, 0, 0),
                Event::note_on(0, 60, 100),
            ],
            4,
        );
        assert_eq!(instance.editor.save().unwrap(), {
            let values: Vec<f64> = instance.editor.params().iter().map(|p| p.default).collect();
            serde_json::to_vec(&values).unwrap()
        });
    }
}
