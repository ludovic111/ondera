use ondera_engine::{
    audio::{AudioBuffer, Library},
    automation::{AutomationLane, AutomationPoint, AutomationTarget, Interpolation},
    control::{self, Headless},
    document,
    model::{Clip, ClipData, Insert, Source, Strip},
    plugin::Rack,
    render::{self, Renderer},
    store::{self, Command, Store},
};
use serde_json::json;
use std::{collections::HashMap, sync::Arc};
fn fixture() -> (ondera_engine::model::Session, Library) {
    let mut session = store::empty();
    session.tracks.truncate(1);
    session.tracks[0].kind = "audio".into();
    session.tracks[0].volume = 0.75;
    session.tracks[0].pan = 0.0;
    session.master_volume = 0.75;
    session.transport.tempo = 120.0;
    session.transport.metronome = false;
    session.strips.clear();
    let buffer = Arc::new(AudioBuffer::new(48000, vec![[0.2; 2]; 48000]).unwrap());
    session.sources.insert(
        "audio".into(),
        Source {
            id: "audio".into(),
            name: "Signal".into(),
            sample_rate: 48000,
            channels: 2,
            file_name: Some("signal.wav".into()),
            duration_seconds: 1.0,
            origin: "file".into(),
            seed: None,
            wave_kind: None,
        },
    );
    session.clips.push(Clip {
        id: "clip".into(),
        name: "Signal".into(),
        agent: false,
        track_id: session.tracks[0].id.clone(),
        start_bar: 0.0,
        length_bars: 0.5,
        data: ClipData::Audio {
            source_id: "audio".into(),
            offset_seconds: 0.0,
        },
    });
    (session, Library::from([("audio".into(), buffer)]))
}
fn lane(target: AutomationTarget, min: f64, max: f64, values: &[(f64, f64)]) -> AutomationLane {
    AutomationLane {
        id: "automation".into(),
        name: "Envelope".into(),
        target,
        min,
        max,
        manual_value: 0.0f64.clamp(min, max),
        interpolation: Interpolation::Linear,
        enabled: true,
        points: values
            .iter()
            .enumerate()
            .map(|(i, (beat, value))| AutomationPoint {
                id: format!("p{i}"),
                beat: *beat,
                value: *value,
            })
            .collect(),
    }
}
fn samples(mut renderer: Renderer, mut rack: Rack, count: usize) -> Vec<[f32; 2]> {
    renderer.playing = true;
    let mut audio = vec![[0.0; 2]; count];
    renderer.render(&mut rack, &mut audio);
    audio
}
#[test]
fn linear_volume_and_pan_automation_change_samples_without_changing_the_manual_fader() {
    let (mut session, library) = fixture();
    let track = session.tracks[0].id.clone();
    session.automation.push(lane(
        AutomationTarget::TrackVolume {
            track_id: track.clone(),
        },
        0.0,
        1.0,
        &[(0.0, 0.0), (0.1, 0.75)],
    ));
    let mut pan = lane(
        AutomationTarget::TrackPan { track_id: track },
        -100.0,
        100.0,
        &[(0.0, -100.0), (0.1, 100.0)],
    );
    pan.id = "pan".into();
    session.automation.push(pan);
    let renderer = Renderer::new(session.clone(), &library, 48000, &HashMap::new()).unwrap();
    let audio = samples(renderer, Rack::new(1), 2600);
    let gain = ondera_engine::model::fader_gain(0.375);
    assert!((audio[1200][0] - 0.2 * gain).abs() < 1e-5);
    assert!((audio[1200][0] - audio[1200][1]).abs() < 1e-5);
    assert!(audio[2450][0].abs() < 1e-6);
    assert!((audio[2450][1] - 0.2).abs() < 1e-5);
    assert_eq!(session.tracks[0].volume, 0.75);
}
#[test]
fn step_master_automation_switches_at_the_exact_sample_inside_a_block() {
    let (mut session, library) = fixture();
    let mut automation = lane(
        AutomationTarget::MasterVolume,
        0.0,
        1.0,
        &[(0.0, 0.75), (300.0 / 24000.0, 0.0)],
    );
    automation.interpolation = Interpolation::Step;
    session.automation.push(automation);
    let renderer = Renderer::new(session, &library, 48000, &HashMap::new()).unwrap();
    let audio = samples(renderer, Rack::new(1), 700);
    assert!((audio[299][0] - 0.2).abs() < 1e-5);
    assert_eq!(audio[300], [0.0; 2]);
    assert_eq!(audio[511], [0.0; 2]);
}
#[test]
fn automation_never_unmutes_a_muted_track() {
    let (mut session, library) = fixture();
    session.tracks[0].mute = true;
    session.automation.push(lane(
        AutomationTarget::TrackVolume {
            track_id: session.tracks[0].id.clone(),
        },
        0.0,
        1.0,
        &[(0.0, 1.0)],
    ));
    let renderer = Renderer::new(session, &library, 48000, &HashMap::new()).unwrap();
    assert!(samples(renderer, Rack::new(1), 1024)
        .iter()
        .flatten()
        .all(|sample| *sample == 0.0));
}

#[test]
fn delayed_audio_keeps_the_previous_cycles_automation_until_its_audio_crosses_the_boundary() {
    struct Delay {
        frames: [[f32; 2]; 64],
        position: usize,
    }
    impl ondera_engine::plugin::Processor for Delay {
        fn process(
            &mut self,
            audio: &mut [[f32; 2]],
            _: &[ondera_engine::plugin::NoteEvent],
            _: &[ondera_engine::plugin::ParamChange],
            _: &ondera_engine::plugin::ProcessContext,
        ) {
            for frame in audio {
                std::mem::swap(frame, &mut self.frames[self.position]);
                self.position = (self.position + 1) % 64;
            }
        }
    }
    for master in [false, true] {
        let (mut session, library) = fixture();
        let track = session.tracks[0].id.clone();
        session.transport.cycle = true;
        session.transport.cycle_start_bar = 0.0;
        session.transport.cycle_end_bar = 1024.0 / 24000.0 / 4.0;
        session.strips.insert(
            track.clone(),
            Strip {
                inserts: vec![Insert::new("delay".into(), "stock:Utility", "Utility")],
                ..Default::default()
            },
        );
        let target = if master {
            AutomationTarget::MasterVolume
        } else {
            AutomationTarget::TrackVolume { track_id: track }
        };
        let mut automation = lane(target, 0.0, 1.0, &[(0.0, 0.0), (900.0 / 24000.0, 0.75)]);
        automation.interpolation = Interpolation::Step;
        session.automation.push(automation);
        let mut renderer = Renderer::new(
            session,
            &library,
            48000,
            &HashMap::from([("delay".into(), 0)]),
        )
        .unwrap();
        renderer.set_latencies(&HashMap::from([(0, 64)])).unwrap();
        renderer.playing = true;
        let mut rack = Rack::new(1);
        rack.mount(
            0,
            Box::new(Delay {
                frames: [[0.0; 2]; 64],
                position: 0,
            }),
        );
        let mut audio = [[0.0; 2]; 2200];
        renderer.render(&mut rack, &mut audio);
        for cycle_start in [1024, 2048] {
            assert!(
                (audio[cycle_start + 32][0] - 0.2).abs() < 1e-5,
                "Previous loop's delayed tail lost automation: master={master}"
            );
            assert_eq!(
                audio[cycle_start + 80],
                [0.0; 2],
                "Cursor did not reset after latency-compensated wrap: master={master}"
            );
        }
    }
}
#[test]
fn stock_plugin_step_automation_lands_on_its_frame_inside_a_block_and_delete_restores_manual() {
    let (mut session, library) = fixture();
    let track = session.tracks[0].id.clone();
    let mut insert = Insert::new("utility".into(), "stock:Utility", "Utility");
    insert.params.insert(0, 0.0);
    session.strips.insert(
        track.clone(),
        Strip {
            inserts: vec![insert],
            ..Default::default()
        },
    );
    let mut automation = lane(
        AutomationTarget::PluginParameter {
            track_id: track,
            insert_id: "utility".into(),
            plugin_id: "stock:Utility".into(),
            parameter_id: 0,
        },
        -60.0,
        12.0,
        &[(0.0, 0.0), (300.0 / 24000.0, -12.0)],
    );
    automation.interpolation = Interpolation::Step;
    session.automation.push(automation);
    let (mut renderer, mut rack) = render::offline(&session, &library, 48000).unwrap();
    renderer.playing = true;
    let mut audio = [[0.0; 2]; 768];
    renderer.render(&mut rack, &mut audio);
    // The point sits on frame 300, inside the second 256-frame block.
    let quieter = 0.2 * 10f32.powf(-12.0 / 20.0);
    assert!((audio[299][0] - 0.2).abs() < 1e-5);
    assert!((audio[300][0] - quieter).abs() < 1e-5);
    assert!((audio[511][0] - quieter).abs() < 1e-5);
    session.automation.clear();
    let mut new = Renderer::new(
        session,
        &library,
        48000,
        &HashMap::from([("utility".into(), 0)]),
    )
    .unwrap();
    new.adopt(&renderer);
    let mut audio = [[0.0; 2]; 256];
    new.render(&mut rack, &mut audio);
    assert!((audio[0][0] - 0.2).abs() < 1e-5);
}
#[test]
fn lane_points_are_transactional_persistent_and_deleted_with_their_track() {
    let (session, library) = fixture();
    let track = session.tracks[0].id.clone();
    let mut store = Store::new(session).unwrap();
    let lane = lane(
        AutomationTarget::TrackVolume {
            track_id: track.clone(),
        },
        0.0,
        1.0,
        &[(0.0, 0.1), (4.0, 0.75)],
    );
    store
        .dispatch(Command::PutAutomation(lane.clone()))
        .unwrap();
    store.dispatch(Command::Undo).unwrap();
    assert!(store.session().automation.is_empty());
    store.dispatch(Command::Redo).unwrap();
    assert_eq!(store.session().automation[0], lane);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("automation.ondera");
    document::save(store.session(), &library, &path).unwrap();
    let (loaded, _) = document::load(&path).unwrap();
    assert_eq!(loaded.automation, vec![lane.clone()]);
    let before = store.revision;
    let mut invalid = lane;
    invalid.points[1].beat = 0.0;
    assert!(store.dispatch(Command::PutAutomation(invalid)).is_err());
    assert_eq!(store.revision, before);
    store.dispatch(Command::RemoveTrack(track)).unwrap();
    assert!(store.session().automation.is_empty());
    store.dispatch(Command::Undo).unwrap();
    assert_eq!(store.session().automation.len(), 1);
}
#[test]
fn deleting_automation_does_not_reset_a_replacement_plugin_in_the_same_slot() {
    let (mut session, library) = fixture();
    let track = session.tracks[0].id.clone();
    session.strips.insert(
        track.clone(),
        Strip {
            inserts: vec![Insert::new("original".into(), "stock:Utility", "Utility")],
            ..Default::default()
        },
    );
    session.automation.push(lane(
        AutomationTarget::PluginParameter {
            track_id: track.clone(),
            insert_id: "original".into(),
            plugin_id: "stock:Utility".into(),
            parameter_id: 0,
        },
        -60.0,
        12.0,
        &[(0.0, -12.0)],
    ));
    let (mut old, mut old_rack) = render::offline(&session, &library, 48000).unwrap();
    old.playing = true;
    old.render(&mut old_rack, &mut [[0.0; 2]; 256]);
    session.automation.clear();
    let mut replacement = Insert::new("replacement".into(), "stock:Utility", "Utility");
    replacement.params.insert(0, 6.0);
    session.strips.get_mut(&track).unwrap().inserts = vec![replacement];
    let (mut new, mut new_rack) = render::offline(&session, &library, 48000).unwrap();
    new.adopt(&old);
    let mut audio = [[0.0; 2]; 256];
    new.render(&mut new_rack, &mut audio);
    assert!((audio[0][0] - 0.2 * 10f32.powf(6.0 / 20.0)).abs() < 1e-5);
}
#[test]
fn replacing_a_plugin_removes_only_its_lanes_and_undo_restores_them() {
    let (mut session, _) = fixture();
    let track = session.tracks[0].id.clone();
    session.strips.insert(
        track.clone(),
        Strip {
            inserts: vec![Insert::new("effect".into(), "stock:Utility", "Utility")],
            ..Default::default()
        },
    );
    session.automation.push(lane(
        AutomationTarget::PluginParameter {
            track_id: track.clone(),
            insert_id: "effect".into(),
            plugin_id: "stock:Utility".into(),
            parameter_id: 0,
        },
        -60.0,
        12.0,
        &[(0.0, 0.0)],
    ));
    let mut store = Store::new(session).unwrap();
    store
        .dispatch(Command::SetStrip {
            track,
            strip: Strip::default(),
        })
        .unwrap();
    assert!(store.session().automation.is_empty());
    store.dispatch(Command::Undo).unwrap();
    assert_eq!(store.session().automation.len(), 1);
}
#[test]
fn registry_creates_moves_reads_and_removes_plugin_automation() {
    let mut host = Headless::new();
    let track = host.store.session().tracks[0].id.clone();
    control::call(
        &mut host,
        "strip.setPlugin",
        &json!({"trackId":track,"slot":0,"pluginId":"stock:Utility"}),
        false,
    )
    .unwrap();
    let result=control::call(&mut host,"automation.create",&json!({"target":"pluginParameter","trackId":track,"slot":0,"parameterId":0,"points":[{"beat":4,"value":-12},{"beat":0,"value":0}]}),false).unwrap();
    let id = result["lane"]["id"].as_str().unwrap();
    assert_eq!(result["lane"]["min"], -60.0);
    assert_eq!(result["lane"]["points"][0]["beat"], 0.0);
    let result = control::call(
        &mut host,
        "automation.setPoint",
        &json!({"laneId":id,"beat":2,"value":-6}),
        false,
    )
    .unwrap();
    assert_eq!(result["lane"]["points"].as_array().unwrap().len(), 3);
    control::call(
        &mut host,
        "automation.setInterpolation",
        &json!({"laneId":id,"interpolation":"step"}),
        false,
    )
    .unwrap();
    control::call(
        &mut host,
        "automation.setEnabled",
        &json!({"laneId":id,"enabled":false}),
        false,
    )
    .unwrap();
    assert_eq!(
        control::call(&mut host, "automation.list", &json!({}), false).unwrap()["lanes"][0]
            ["enabled"],
        false
    );
    assert!(control::call(
        &mut host,
        "automation.setPoint",
        &json!({"laneId":id,"beat":1,"value":999}),
        false
    )
    .is_err());
    control::call(&mut host, "automation.remove", &json!({"laneId":id}), false).unwrap();
    assert!(host.store.session().automation.is_empty());
}
#[test]
fn linear_step_and_range_tail_hold_have_defined_endpoint_behavior() {
    let (mut session, _) = fixture();
    let mut automation = lane(
        AutomationTarget::MasterVolume,
        0.0,
        1.0,
        &[(1.0, 0.2), (3.0, 0.8)],
    );
    assert_eq!(automation.value_at(0.0), Some(0.2));
    assert!((automation.value_at(2.0).unwrap() - 0.5).abs() < 1e-12);
    assert_eq!(automation.value_at(100.0), Some(0.8));
    automation.interpolation = Interpolation::Step;
    assert_eq!(automation.value_at(2.0), Some(0.2));
    automation.interpolation = Interpolation::Linear;
    session.automation.push(automation);
    ondera_engine::automation::hold_after(&mut session, 2.0);
    assert!((session.automation[0].value_at(5.0).unwrap() - 0.5).abs() < 1e-12);
}

/// Records every parameter change a plugin receives, at its absolute frame.
struct Changes {
    timed: bool,
    seen: Arc<std::sync::Mutex<Vec<(i64, f64)>>>,
}
impl ondera_engine::plugin::Processor for Changes {
    fn process(
        &mut self,
        _: &mut [[f32; 2]],
        _: &[ondera_engine::plugin::NoteEvent],
        params: &[ondera_engine::plugin::ParamChange],
        ctx: &ondera_engine::plugin::ProcessContext,
    ) {
        let mut seen = self.seen.lock().unwrap();
        for change in params {
            assert!(
                self.timed || change.frame == 0,
                "an untimed plugin got a later frame"
            );
            seen.push((ctx.sample_time + change.frame as i64, change.value));
        }
    }
    fn timed_params(&self) -> bool {
        self.timed
    }
}
fn automated_changes(
    timed: bool,
    interpolation: Interpolation,
    points: &[(f64, f64)],
    frames: usize,
    block: usize,
) -> Vec<(i64, f64)> {
    let (mut session, library) = fixture();
    let track = session.tracks[0].id.clone();
    session.strips.insert(
        track.clone(),
        Strip {
            inserts: vec![Insert::new("fx".into(), "clap:org.example.fx", "Fx")],
            ..Default::default()
        },
    );
    let beats: Vec<(f64, f64)> = points
        .iter()
        .map(|(frame, value)| (frame / 24000.0, *value))
        .collect();
    let mut automation = lane(
        AutomationTarget::PluginParameter {
            track_id: track,
            insert_id: "fx".into(),
            plugin_id: "clap:org.example.fx".into(),
            parameter_id: 7,
        },
        0.0,
        1.0,
        &beats,
    );
    automation.interpolation = interpolation;
    session.automation.push(automation);
    let mut renderer =
        Renderer::new(session, &library, 48000, &HashMap::from([("fx".into(), 0)])).unwrap();
    renderer.playing = true;
    let seen = Arc::new(std::sync::Mutex::new(vec![]));
    let mut rack = Rack::new(1);
    rack.mount(
        0,
        Box::new(Changes {
            timed,
            seen: seen.clone(),
        }),
    );
    let mut audio = vec![[0.0; 2]; block];
    for _ in 0..frames / block {
        renderer.render(&mut rack, &mut audio);
    }
    let seen = seen.lock().unwrap().clone();
    seen
}
#[test]
fn plugin_automation_reaches_the_plugin_on_breakpoints_and_every_grain_while_it_moves() {
    // 120 BPM at 48 kHz: 24,000 frames per beat. A ramp from frame 0 to frame 100, then held.
    let seen = automated_changes(
        true,
        Interpolation::Linear,
        &[(0.0, 0.0), (100.0, 1.0), (400.0, 1.0)],
        512,
        256,
    );
    let frames: Vec<i64> = seen.iter().map(|(frame, _)| *frame).collect();
    assert_eq!(frames, vec![0, 32, 64, 96, 100, 256]);
    for (frame, value) in &seen[..4] {
        assert!(
            (value - *frame as f64 / 100.0).abs() < 1e-9,
            "{frame}: {value}"
        );
    }
    assert_eq!(seen[4].1, 1.0);
    assert_eq!(seen[5].1, 1.0, "each block restates its first value");
}
#[test]
fn a_step_lands_on_its_exact_frame_for_timed_and_untimed_plugins() {
    for timed in [true, false] {
        let seen = automated_changes(
            timed,
            Interpolation::Step,
            &[(0.0, 0.25), (300.0, 0.75), (301.0, 0.5)],
            768,
            768,
        );
        assert_eq!(
            seen,
            vec![(0, 0.25), (256, 0.25), (300, 0.75), (301, 0.5), (512, 0.5)],
            "timed={timed}"
        );
    }
}
