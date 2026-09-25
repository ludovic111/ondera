//! Where a track's MIDI events go: controllers, bend and pressure reach the instrument and
//! every insert that takes events (native ABI 2, CLAP note ports, VST3 event buses, AU music
//! effects), chase and rest included; stock and ABI 1 effects hear nothing.
use ondera_engine::{
    audio::Library,
    host::native,
    model::{Clip, ClipData, Controller, ControllerKind, Insert, Note, Session, Strip},
    plugin::{event, Descriptor, Event, Format, ParamChange, ProcessContext, Processor, Rack},
    render::Renderer,
    stock, store,
};
use ondera_plugin::{ffi, prelude::*};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

/// Every event a processor received, at its absolute sample.
type Log = Arc<Mutex<Vec<(i64, Event)>>>;

/// A rack processor that writes down what reaches it.
struct Probe {
    accepts: bool,
    log: Log,
}
impl Processor for Probe {
    fn process(
        &mut self,
        _: &mut [[f32; 2]],
        events: &[Event],
        _: &[ParamChange],
        ctx: &ProcessContext,
    ) {
        let mut log = self.log.lock().unwrap();
        for e in events {
            log.push((ctx.sample_time + e.frame as i64, *e));
        }
    }
    fn accepts_events(&self) -> bool {
        self.accepts
    }
}

fn point(kind: ControllerKind, number: Option<u8>, time: f64, value: i16) -> Controller {
    Controller {
        id: format!("{}{number:?}@{time}", kind.as_str()),
        kind,
        number,
        time,
        value,
        agent: false,
    }
}

/// One MIDI track with a clip of one note and `controllers`, and the named inserts.
fn session(controllers: Vec<Controller>, inserts: &[&str]) -> Session {
    let mut s = store::empty();
    s.tracks.retain(|t| t.kind == "midi");
    s.tracks.truncate(1);
    s.clips.clear();
    s.transport.tempo = 120.0;
    s.transport.metronome = false;
    s.transport.cycle = false;
    let track = s.tracks[0].id.clone();
    s.strips.insert(
        track.clone(),
        Strip {
            inserts: inserts
                .iter()
                .map(|id| Insert::new(id.to_string(), "native:tests.probe", id))
                .collect(),
            ..Strip::default()
        },
    );
    s.clips.push(Clip {
        id: "clip".into(),
        name: "Take".into(),
        agent: false,
        track_id: track,
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
            }],
            controllers,
        },
    });
    s
}

/// The instrument in slot 0, then one slot per insert, in order.
fn slots(s: &Session) -> HashMap<String, u32> {
    let track = &s.tracks[0].id;
    let strip = &s.strips[track];
    let mut slots = HashMap::from([(strip.synth_key(track), 0)]);
    for (i, insert) in strip.inserts.iter().enumerate() {
        slots.insert(insert.id.clone(), i as u32 + 1);
    }
    slots
}

fn probe(rack: &mut Rack, slot: u32, accepts: bool) -> Log {
    let log = Log::default();
    rack.mount(
        slot,
        Box::new(Probe {
            accepts,
            log: log.clone(),
        }),
    );
    log
}

/// Controllers only, as (sample, kind, key, value).
fn controls(log: &Log) -> Vec<(i64, u8, u8, i16)> {
    log.lock()
        .unwrap()
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
fn untimed(list: Vec<(i64, u8, u8, i16)>) -> Vec<(u8, u8, i16)> {
    list.into_iter().map(|(_, k, n, v)| (k, n, v)).collect()
}

#[test]
fn inserts_that_take_events_hear_the_track_s_controllers_chase_and_rest() {
    let s = session(
        vec![
            point(ControllerKind::Cc, Some(1), 0.01, 20),
            point(ControllerKind::Bend, None, 0.02, 4096),
        ],
        &["fx-events", "fx-plain"],
    );
    let mut rack = Rack::new(4);
    let instrument = probe(&mut rack, 0, false);
    let listening = probe(&mut rack, 1, true);
    let plain = probe(&mut rack, 2, false);
    let mut renderer = Renderer::new(s.clone(), &Library::new(), 48000, &slots(&s)).unwrap();
    renderer.playing = true;
    let mut block = [[0.0f32; 2]; 256];
    for _ in 0..3 {
        renderer.render(&mut rack, &mut block);
    }
    let played = vec![
        (240, event::CONTROL, 1, 20),
        (480, event::PITCH_BEND, 0, 4096),
    ];
    assert_eq!(controls(&instrument), played);
    assert_eq!(
        controls(&listening),
        played,
        "The insert hears each controller on the instrument's sample"
    );
    assert!(
        listening.lock().unwrap().iter().all(|(_, e)| !e.is_note()),
        "Notes stay with the instrument"
    );
    assert!(
        plain.lock().unwrap().is_empty(),
        "A plain effect hears nothing"
    );

    // Stop returns the bend to rest for the insert as well.
    listening.lock().unwrap().clear();
    renderer.stop();
    renderer.render(&mut rack, &mut block);
    assert_eq!(
        untimed(controls(&listening)),
        vec![(event::PITCH_BEND, 0, 0)]
    );

    // A locate chases the values in force for it too.
    listening.lock().unwrap().clear();
    renderer.locate(0.5);
    renderer.playing = true;
    renderer.render(&mut rack, &mut block);
    assert_eq!(
        untimed(controls(&listening)),
        vec![(event::PITCH_BEND, 0, 4096)]
    );

    // Live controllers reach it at once.
    listening.lock().unwrap().clear();
    renderer.control(0, Event::control(0, 11, 99));
    renderer.render(&mut rack, &mut block);
    assert_eq!(
        untimed(controls(&listening)),
        vec![(event::CONTROL, 11, 99)]
    );
    assert!(plain.lock().unwrap().is_empty());

    // An insert added while playing is told the values in force when the graph is rebuilt.
    let mut grown = s.clone();
    let track = grown.tracks[0].id.clone();
    grown
        .strips
        .get_mut(&track)
        .unwrap()
        .inserts
        .push(Insert::new("fx-new".into(), "native:tests.probe", "New"));
    let added = probe(&mut rack, 3, true);
    let mut next = Renderer::new(grown.clone(), &Library::new(), 48000, &slots(&grown)).unwrap();
    next.adopt(&renderer);
    next.render(&mut rack, &mut block);
    let heard = untimed(controls(&added));
    for expected in [
        (event::CONTROL, 1, 20),
        (event::CONTROL, 11, 99),
        (event::PITCH_BEND, 0, 4096),
    ] {
        assert!(heard.contains(&expected), "{heard:?}");
    }
}

/// An ABI 2 effect that writes down the events it gets.
static EFFECT_HEARD: Mutex<Vec<Event>> = Mutex::new(Vec::new());
struct EffectListener;
impl Plugin for EffectListener {
    const INFO: Info = Info::effect("org.ondera.tests.fx", "Fx Listener", "Tests", "Utility");
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
        _: &ProcessContext,
    ) {
        EFFECT_HEARD.lock().unwrap().extend_from_slice(events);
    }
}
static EFFECT_V2: ffi::PluginVTable2 = ffi::vtable2::<EffectListener>();
static EFFECT_V1: ffi::PluginVTable = ffi::vtable::<EffectListener>();

#[test]
fn native_abi_2_effects_take_events_and_abi_1_and_stock_effects_do_not() {
    let manifest = ffi::Manifest::of::<EffectListener>();
    let descriptor = Descriptor {
        id: "native:org.ondera.tests.fx".into(),
        format: Format::Native,
        name: "Fx Listener".into(),
        vendor: "Tests".into(),
        path: "in-process".into(),
        instrument: false,
        effect: true,
        category: "Utility".into(),
    };
    let mut v2 = native::instance_from(&EFFECT_V2, &manifest, descriptor.clone(), 48000).unwrap();
    let mut v1 = native::instance_from(&EFFECT_V1, &manifest, descriptor, 48000).unwrap();
    let mut rack = Rack::new(2);
    rack.mount(0, v2.processor.take().unwrap());
    rack.mount(1, v1.processor.take().unwrap());
    assert!(rack.accepts_events(0));
    assert!(!rack.accepts_events(1));
    assert!(!rack.accepts_events(7), "An empty slot takes nothing");
    let events = [Event::control(3, 1, 64), Event::channel_pressure(9, 12)];
    let mut audio = [[0.0f32; 2]; 64];
    rack.process(0, &mut audio, &events, &ProcessContext::default());
    assert_eq!(*EFFECT_HEARD.lock().unwrap(), events);
    for name in ondera_engine::dsp::EFFECTS {
        let mut instance = stock::create(name, 48000).unwrap();
        assert!(
            !instance.processor.take().unwrap().accepts_events(),
            "{name} is a stock effect"
        );
    }
    drop(rack);
    drop((v1, v2));
}
