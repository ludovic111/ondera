//! Bus tracks: groups (tracks routed to a bus) and aux returns (tracks sending to one).
use ondera_engine::{
    audio::AudioBuffer,
    control::{self, Headless},
    model::*,
    render,
    store::Command,
};
use serde_json::{json, Value};
use std::sync::Arc;

fn run(host: &mut Headless, name: &str, params: Value) -> Value {
    control::call(host, name, &params, false).unwrap_or_else(|e| panic!("{name}: {e}"))
}
fn fails(host: &mut Headless, name: &str, params: Value) -> String {
    control::call(host, name, &params, false).expect_err(name)
}

/// A song of `n` audio tracks, each playing one second of `frames` from bar 0, nothing else.
fn song(n: usize, frames: Vec<[f32; 2]>) -> (Headless, Vec<String>) {
    let mut host = Headless::new();
    for t in host.store.session().tracks.clone() {
        host.store.dispatch(Command::RemoveTrack(t.id)).unwrap();
    }
    let buffer = Arc::new(AudioBuffer::new(48000, frames).unwrap());
    host.store
        .dispatch(Command::PutSource(Source {
            id: "src".into(),
            name: "Test".into(),
            sample_rate: 48000,
            channels: 2,
            file_name: Some("test.wav".into()),
            duration_seconds: buffer.duration(),
            origin: "file".into(),
            seed: None,
            wave_kind: None,
        }))
        .unwrap();
    host.library.insert("src".into(), buffer);
    run(&mut host, "transport.setTempo", json!({"bpm": 120}));
    run(&mut host, "master.setVolume", json!({"volume": 0.75}));
    let mut ids = vec![];
    for i in 0..n {
        let id = run(
            &mut host,
            "track.add",
            json!({"kind": "audio", "name": format!("T{i}")}),
        )["id"]
            .as_str()
            .unwrap()
            .to_string();
        run(
            &mut host,
            "clip.create",
            json!({"trackId": id, "startBar": 0, "lengthBars": 0.5, "sourceId": "src"}),
        );
        ids.push(id);
    }
    (host, ids)
}

fn render(host: &Headless, frames: usize) -> Vec<[f32; 2]> {
    let (mut r, mut rack) =
        render::offline(host.store.session(), &host.library, 48000).expect("renders");
    r.playing = true;
    r.locate(0.0);
    let mut out = vec![[0.0f32; 2]; frames];
    for chunk in out.chunks_mut(256) {
        r.render(&mut rack, chunk);
    }
    out
}

/// The output at 0.5 s, away from the clip's edge ramps.
fn level(host: &Headless) -> [f32; 2] {
    render(host, 24_000 + 1)[24_000]
}

const DC: [f32; 2] = [0.25, 0.25];

#[test]
fn a_group_sums_its_tracks_through_its_own_fader_pan_and_mute() {
    let (mut host, ids) = song(2, vec![DC; 48000]);
    assert!((level(&host)[0] - 0.5).abs() < 1e-4);
    let bus = run(
        &mut host,
        "track.group",
        json!({"trackIds": ["T0", ids[1]], "name": "Stack"}),
    );
    assert_eq!(bus["kind"], "bus");
    assert_eq!(bus["name"], "Stack");
    assert_eq!(bus["inputs"], json!(["T0", "T1"]));
    let bus_id = bus["id"].as_str().unwrap().to_string();
    // At unity the group changes nothing.
    assert!((level(&host)[0] - 0.5).abs() < 1e-4);
    // Its fader, pan and mute act on both tracks at once.
    run(
        &mut host,
        "track.setPan",
        json!({"trackId": "Stack", "pan": 100}),
    );
    let v = level(&host);
    assert!(v[0].abs() < 1e-6 && v[1] > 0.4, "{v:?}");
    run(
        &mut host,
        "track.setPan",
        json!({"trackId": "Stack", "pan": 0}),
    );
    run(
        &mut host,
        "track.setMute",
        json!({"trackId": "Stack", "muted": true}),
    );
    assert_eq!(level(&host), [0.0, 0.0]);
    let overview = run(&mut host, "session.overview", json!({}));
    let t0 = &overview["tracks"][0];
    assert_eq!(t0["output"], "Stack");
    assert!(t0["problems"]
        .to_string()
        .contains("its bus Stack is muted"));
    run(
        &mut host,
        "track.setMute",
        json!({"trackId": bus_id, "muted": false}),
    );
    // One track back to the Stereo Out: the other still goes through the bus.
    run(
        &mut host,
        "track.setVolume",
        json!({"trackId": "Stack", "volume": 0}),
    );
    run(
        &mut host,
        "track.setOutput",
        json!({"trackId": "T0", "output": "Stereo Out"}),
    );
    assert!((level(&host)[0] - 0.25).abs() < 1e-4);
}

#[test]
fn sends_reach_a_bus_track_as_an_aux_return() {
    let (mut host, ids) = song(1, vec![DC; 48000]);
    run(
        &mut host,
        "track.add",
        json!({"kind": "bus", "name": "Room"}),
    );
    let strip = run(
        &mut host,
        "strip.setSend",
        json!({"trackId": ids[0], "send": 2, "bus": "room", "levelDb": 0}),
    );
    assert_eq!(strip["sends"][2]["name"], "Room");
    assert_eq!(strip["sends"][0]["name"], "A · Reverb");
    // Dry plus the return at 0 dB.
    assert!((level(&host)[0] - 0.5).abs() < 1e-4);
    run(
        &mut host,
        "strip.setSend",
        json!({"trackId": ids[0], "send": 2, "levelDb": -6.0206}),
    );
    assert!((level(&host)[0] - 0.375).abs() < 1e-3);
    // A send can leave A for a bus and come back.
    run(
        &mut host,
        "strip.setSend",
        json!({"trackId": ids[0], "send": 0, "bus": "Room", "levelDb": 0}),
    );
    let strip = run(&mut host, "strip.get", json!({"trackId": ids[0]}));
    assert_eq!(strip["sends"][0]["bus"], host.store.session().tracks[1].id);
    run(
        &mut host,
        "strip.setSend",
        json!({"trackId": ids[0], "send": 0, "bus": "A"}),
    );
    let s = host.store.session();
    assert_eq!(
        s.strips[&ids[0]].sends[0].bus, None,
        "the default stays out of the file"
    );
    run(
        &mut host,
        "strip.setSend",
        json!({"trackId": ids[0], "send": 2, "bus": "none"}),
    );
    assert_eq!(host.store.session().strips[&ids[0]].sends.len(), 2);
    assert!(fails(
        &mut host,
        "strip.setSend",
        json!({"trackId": ids[0], "send": 3, "bus": "Room"})
    )
    .contains("Point send 2"));
}

#[test]
fn solo_keeps_what_a_group_or_return_needs() {
    let (mut host, ids) = song(3, vec![DC; 48000]);
    run(
        &mut host,
        "track.group",
        json!({"trackIds": [ids[0], ids[1]], "name": "G"}),
    );
    // Soloing the group keeps both of its tracks, not the third.
    run(
        &mut host,
        "track.setSolo",
        json!({"trackId": "G", "solo": true}),
    );
    assert!((level(&host)[0] - 0.5).abs() < 1e-4);
    run(
        &mut host,
        "track.setSolo",
        json!({"trackId": "G", "solo": false}),
    );
    // Soloing a grouped track keeps its group.
    run(
        &mut host,
        "track.setSolo",
        json!({"trackId": ids[0], "solo": true}),
    );
    assert!((level(&host)[0] - 0.25).abs() < 1e-4);
}

#[test]
fn routing_never_loops_and_forgets_removed_buses() {
    let (mut host, ids) = song(1, vec![DC; 48000]);
    let a = run(&mut host, "track.add", json!({"kind": "bus", "name": "A1"}))["id"].clone();
    run(&mut host, "track.add", json!({"kind": "bus", "name": "B1"}));
    assert!(fails(
        &mut host,
        "track.setOutput",
        json!({"trackId": "A1", "output": "B1"})
    )
    .contains("buses feed the Stereo Out"));
    assert!(fails(
        &mut host,
        "strip.setSend",
        json!({"trackId": "A1", "send": 0, "bus": "B1"})
    )
    .contains("A or B"));
    assert!(fails(
        &mut host,
        "track.setOutput",
        json!({"trackId": ids[0], "output": "Nope"})
    )
    .contains("Buses: A1, B1"));
    assert!(fails(
        &mut host,
        "track.setArmed",
        json!({"trackId": "A1", "armed": true})
    )
    .contains("no input"));
    assert!(fails(
        &mut host,
        "clip.create",
        json!({"trackId": "A1", "startBar": 0, "lengthBars": 1})
    )
    .contains("hold no clips"));
    run(
        &mut host,
        "track.setOutput",
        json!({"trackId": ids[0], "output": "A1"}),
    );
    run(
        &mut host,
        "strip.setSend",
        json!({"trackId": ids[0], "send": 2, "bus": "B1", "levelDb": 0}),
    );
    run(&mut host, "track.remove", json!({"trackId": a}));
    run(&mut host, "track.remove", json!({"trackId": "B1"}));
    let s = host.store.session();
    assert_eq!(s.tracks[0].output, None);
    assert_eq!(s.strips[&ids[0]].sends.len(), 2);
    assert!((level(&host)[0] - 0.25).abs() < 1e-4);
    // Undo brings the bus and the routing back together.
    run(&mut host, "history.undo", json!({"steps": 2}));
    assert_eq!(host.store.session().tracks[0].output.as_deref(), a.as_str());
}

#[test]
fn a_latent_insert_on_a_bus_keeps_every_path_on_the_same_sample() {
    // A click at 0.1 s on two tracks; one goes through a bus with a look-ahead limiter.
    let mut frames = vec![[0.0f32; 2]; 48000];
    frames[4800] = [0.25, 0.25];
    let (mut host, ids) = song(2, frames);
    run(
        &mut host,
        "track.group",
        json!({"trackIds": [ids[0]], "name": "Limited"}),
    );
    run(
        &mut host,
        "strip.setInsert",
        json!({"trackId": "Limited", "slot": 0, "effect": "Limiter"}),
    );
    let (r, _) = render::offline(host.store.session(), &host.library, 48000).unwrap();
    let latency = r.latency_samples() as usize;
    assert!(latency > 0, "the limiter looks ahead");
    let out = render(&host, 4800 + latency + 100);
    let peak = out
        .iter()
        .enumerate()
        .max_by(|a, b| a.1[0].abs().total_cmp(&b.1[0].abs()))
        .unwrap();
    // One click of both tracks together, not two of one each.
    assert_eq!(peak.0, 4800 + latency);
    assert!((peak.1[0] - 0.5).abs() < 1e-3, "{:?}", peak.1);
}

#[test]
fn stems_follow_the_routing() {
    let (mut host, ids) = song(2, vec![DC; 48000]);
    run(
        &mut host,
        "track.group",
        json!({"trackIds": [ids[0], ids[1]], "name": "Pair"}),
    );
    run(
        &mut host,
        "track.setVolume",
        json!({"trackId": "Pair", "volume": 0.5}),
    );
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("stems");
    let options = ondera_engine::export::ExportOptions {
        tail_seconds: 0.0,
        format: ondera_engine::export::SampleFormat::Float32,
        dither: false,
        ..Default::default()
    };
    let report = ondera_engine::export::stems(
        host.store.session(),
        &host.library,
        &out,
        &options,
        None,
        true,
        true,
    )
    .unwrap();
    assert_eq!(report.files.len(), 3);
    let read = |i: usize| -> Vec<f32> {
        hound::WavReader::open(&report.files[i].path)
            .unwrap()
            .samples::<f32>()
            .map(Result::unwrap)
            .collect()
    };
    let gain = fader_gain(0.5);
    // Each track's stem goes through the group's fader; the group's stem is both.
    assert!((read(0)[48_000] - 0.25 * gain).abs() < 1e-4);
    assert!((read(2)[48_000] - 0.5 * gain).abs() < 1e-4);
}

#[test]
fn routing_stays_out_of_files_that_do_not_use_it() {
    let (host, _) = song(1, vec![DC; 480]);
    let text = serde_json::to_string(host.store.session()).unwrap();
    assert!(!text.contains("\"output\""));
    assert!(!text.contains("\"bus\""));
}
