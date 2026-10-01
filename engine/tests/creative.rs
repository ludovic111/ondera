use ryolune_engine::{
    control::{self, Headless, Host},
    model::ClipData,
};
use serde_json::{json, Value};
fn call(h: &mut Headless, name: &str, args: Value) -> Value {
    control::call(h, name, &args, false).unwrap_or_else(|e| panic!("{name}: {e}"))
}
fn phrase() -> (Headless, Value, Value) {
    let mut h = Headless::new();
    let track = call(
        &mut h,
        "track.add",
        json!({"kind":"midi","name":"Test","instrument":"Drum Machine"}),
    )["id"]
        .clone();
    let clip = call(&mut h,"clip.create",json!({"trackId":track,"startBar":0,"lengthBars":1,"notes":[{"start":0,"length":0.125,"pitch":36,"velocity":127},{"start":1,"length":0.25,"pitch":38,"velocity":1},{"start":3.9,"length":0.1,"pitch":42,"velocity":90}]}))["id"].clone();
    (h, track, clip)
}
#[test]
fn humanization_preserves_note_identity_pitch_and_bounds_across_seeds() {
    for seed in 0..128 {
        let (mut h, _, clip) = phrase();
        let before = call(&mut h, "clip.get", json!({"clipId":clip}));
        call(
            &mut h,
            "clip.humanize",
            json!({"clipId":clip,"seed":seed,"timingMs":100,"velocity":32}),
        );
        h.store().session().validate().unwrap();
        let after = call(&mut h, "clip.get", json!({"clipId":clip}));
        let original = before["data"]["notes"].as_array().unwrap();
        for note in after["data"]["notes"].as_array().unwrap() {
            let old = original.iter().find(|old| old["id"] == note["id"]).unwrap();
            assert_eq!(note["pitch"], old["pitch"]);
            assert!(note["start"].as_f64().unwrap() >= 0.0);
            assert!(
                note["start"].as_f64().unwrap() + note["length"].as_f64().unwrap() <= 4.0 + 1e-9
            );
        }
        call(&mut h, "history.undo", json!({}));
        assert_eq!(call(&mut h, "clip.get", json!({"clipId":clip})), before);
    }
}
#[test]
fn scale_is_idempotent_and_reverse_is_an_involution() {
    let (mut h, _, clip) = phrase();
    call(
        &mut h,
        "clip.fitScale",
        json!({"clipId":clip,"root":0,"scale":"major"}),
    );
    let first = call(&mut h, "clip.get", json!({"clipId":clip}));
    call(
        &mut h,
        "clip.fitScale",
        json!({"clipId":clip,"root":0,"scale":"major"}),
    );
    assert_eq!(call(&mut h, "clip.get", json!({"clipId":clip})), first);
    for _ in 0..2 {
        call(&mut h, "clip.reverseMidi", json!({"clipId":clip}));
    }
    let final_clip = call(&mut h, "clip.get", json!({"clipId":clip}));
    for note in final_clip["data"]["notes"].as_array().unwrap() {
        let original = first["data"]["notes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == note["id"])
            .unwrap();
        assert!(
            (note["start"].as_f64().unwrap() - original["start"].as_f64().unwrap()).abs() < 1e-9
        );
        assert_eq!(note["pitch"], original["pitch"]);
    }
}
#[test]
fn repeats_use_unique_ids_and_undo_as_one_edit() {
    let (mut h, _, clip) = phrase();
    let before = h.store().session().clips.len();
    call(&mut h, "clip.repeat", json!({"clipId":clip,"count":3}));
    let s = h.store().session();
    assert_eq!(s.clips.len(), before + 3);
    let mut ids = std::collections::HashSet::new();
    for c in &s.clips {
        assert!(ids.insert(c.id.clone()));
        if let ClipData::Midi { notes, .. } = &c.data {
            for n in notes {
                assert!(ids.insert(n.id.clone()));
            }
        }
    }
    call(&mut h, "history.undo", json!({}));
    assert_eq!(h.store().session().clips.len(), before);
}
#[test]
fn takes_preserve_edits_across_switch_save_reopen_and_undo() {
    let (mut h, _, clip) = phrase();
    let original = call(&mut h, "take.create", json!({"name":"Original"}))["active"].clone();
    let variation = call(&mut h, "take.create", json!({"name":"Variation"}))["active"].clone();
    call(
        &mut h,
        "clip.transpose",
        json!({"clipId":clip,"semitones":12}),
    );
    let changed = call(&mut h, "clip.get", json!({"clipId":clip}));
    call(&mut h, "take.select", json!({"id":original}));
    assert_ne!(call(&mut h, "clip.get", json!({"clipId":clip})), changed);
    call(&mut h, "history.undo", json!({}));
    assert_eq!(call(&mut h, "clip.get", json!({"clipId":clip})), changed);
    call(&mut h, "history.redo", json!({}));
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("takes.ryolune");
    call(&mut h, "session.save", json!({"path":file}));
    let mut reopened = Headless::open(&file).unwrap();
    call(&mut reopened, "take.select", json!({"id":variation}));
    assert_eq!(
        call(&mut reopened, "clip.get", json!({"clipId":clip})),
        changed
    );
}
#[test]
fn invalid_parameter_batch_does_not_partially_change_the_plugin() {
    let (mut h, track, _) = phrase();
    let params = call(&mut h, "strip.parameters", json!({"trackId":track}));
    let p = &params["parameters"][0];
    let before = call(&mut h, "strip.get", json!({"trackId":track}));
    let mut values = serde_json::Map::new();
    values.insert(p["id"].to_string(), p["max"].clone());
    values.insert("4294967295".into(), json!(1));
    assert!(control::call(
        &mut h,
        "strip.setParameters",
        &json!({"trackId":track,"values":values}),
        false
    )
    .is_err());
    assert_eq!(call(&mut h, "strip.get", json!({"trackId":track})), before);
}
#[test]
fn inspection_explains_a_muted_solo_track_excluding_new_drums() {
    let (mut h, track, _) = phrase();
    let old = h.store().session().tracks[0].id.clone();
    call(&mut h, "track.setMute", json!({"trackId":old,"muted":true}));
    call(&mut h, "track.setSolo", json!({"trackId":old,"solo":true}));
    let inspect = call(&mut h, "session.inspect", json!({}));
    let drum = inspect["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == track)
        .unwrap();
    assert_eq!(drum["audibility"]["excludedBySolo"], true);
    call(&mut h, "track.setSolo", json!({"trackId":old,"solo":false}));
    let list = call(&mut h, "track.list", json!({}));
    assert_eq!(
        list.as_array()
            .unwrap()
            .iter()
            .find(|t| t["id"] == track)
            .unwrap()["audibility"]["excludedBySolo"],
        false
    );
}

#[test]
fn rhythm_is_audible_and_creation_is_one_undo_step() {
    let mut h = Headless::new();
    call(&mut h, "session.new", json!({}));
    let before = h.store().session().tracks.len();
    let made = call(
        &mut h,
        "rhythm.create",
        json!({"bars":2,"name":"Euclid","lanes":[
            {"steps":16,"pulses":4,"rotation":0,"pitch":36,"velocity":100},
            {"steps":16,"pulses":3,"rotation":2,"pitch":38,"velocity":90},
            {"steps":12,"pulses":7,"rotation":1,"pitch":42,"velocity":80}
        ]}),
    );
    assert_eq!(made["noteCount"], 28);
    let dir = tempfile::tempdir().unwrap();
    let render = call(
        &mut h,
        "session.exportAudio",
        json!({"path":dir.path().join("rhythm.wav"),"startBar":0,"endBar":2}),
    );
    assert!(render["peak"].as_f64().unwrap() > 0.01, "{render}");
    assert_eq!(render["clippedSamples"], 0);
    call(&mut h, "history.undo", json!({}));
    assert_eq!(h.store().session().tracks.len(), before);
}

/// A one-second 220 Hz tone as a WAV file.
fn tone(path: &std::path::Path) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 44_100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut wav = hound::WavWriter::create(path, spec).unwrap();
    for i in 0..44_100 {
        let s = (i as f32 * 220.0 * std::f32::consts::TAU / 44_100.0).sin() * 0.6;
        wav.write_sample((s * 32767.0) as i16).unwrap();
    }
    wav.finalize().unwrap();
}

#[test]
fn a_sound_becomes_a_playable_instrument_in_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let wav = dir.path().join("Hum.wav");
    tone(&wav);
    let mut h = Headless::new();
    call(&mut h, "session.new", json!({}));
    let before = h.store().session().tracks.len();
    let strip = call(
        &mut h,
        "strip.loadSample",
        json!({"path": wav, "rootNote": 57}),
    );
    assert_eq!(strip["synth"]["name"], "Sample Keys");
    assert!(!strip["synth"]["blob"].as_str().unwrap().is_empty());
    assert_eq!(strip["rootNote"], 57);
    let track = strip["trackId"].clone();
    assert_eq!(h.store().session().tracks.len(), before + 1);
    assert_eq!(
        h.store().session().tracks.last().unwrap().name,
        "Hum",
        "a new track is named after the sound"
    );
    call(
        &mut h,
        "clip.create",
        json!({"trackId": track, "startBar": 0, "lengthBars": 1,
        "notes": [{"start": 0, "length": 2, "pitch": 69, "velocity": 110}]}),
    );
    let render = call(
        &mut h,
        "session.exportAudio",
        json!({"path": dir.path().join("keys.wav"), "startBar": 0, "endBar": 1}),
    );
    assert!(render["peak"].as_f64().unwrap() > 0.05, "{render}");
    // Undo the clip, then the instrument: both the strip and its track go in one step.
    call(&mut h, "history.undo", json!({"steps": 2}));
    assert_eq!(h.store().session().tracks.len(), before);
    // A sound already in the song works too, and an audio track cannot take an instrument.
    let placed = call(&mut h, "session.importAudio", json!({"path": wav}));
    let source = placed["source"]["id"].clone();
    let audio_track = placed["clip"]["trackId"].clone();
    call(&mut h, "strip.loadSample", json!({"sourceId": source}));
    assert!(control::call(
        &mut h,
        "strip.loadSample",
        &json!({"sourceId": source, "trackId": audio_track}),
        false
    )
    .is_err());
    assert!(control::call(&mut h, "strip.loadSample", &json!({}), false).is_err());
}

#[test]
fn generation_requests_follow_the_song_and_need_a_connected_service() {
    use ryolune_engine::{control_generate as generate, settings::Settings};
    let mut h = Headless::new();
    call(&mut h, "session.new", json!({}));
    call(&mut h, "transport.setTempo", json!({"bpm": 90}));
    let session = h.store().session().clone();
    let mut settings = Settings::default();
    let none = generate::request_for(&settings, &session, &json!({"prompt": "dusty drums"}));
    assert!(none.unwrap_err().contains("not connected"));
    settings.generation.elevenlabs_api_key = "sk-test-key-1234".into();
    let r = generate::request_for(
        &settings,
        &session,
        &json!({"prompt": "dusty drums", "bars": 2}),
    )
    .unwrap();
    assert_eq!(r.kind, generate::GenKind::Loop);
    // Two bars of 4/4 at 90 BPM.
    assert!((r.seconds - 8.0 * 60.0 / 90.0).abs() < 1e-9);
    assert_eq!(r.fit_seconds, Some(r.seconds));
    assert!(r.prompt.contains("90 BPM"), "{}", r.prompt);
    assert_eq!(r.name, "Dusty drums");
    let keys = generate::request_for(
        &settings,
        &session,
        &json!({"prompt": "glass marimba", "kind": "instrument"}),
    )
    .unwrap();
    assert_eq!(keys.root_note, 60);
    assert!(keys.prompt.contains("single sustained note"));
    assert!(generate::request_for(
        &settings,
        &session,
        &json!({"prompt": "x", "kind": "song", "seconds": 900})
    )
    .is_err());
    assert!(generate::request_for(
        &settings,
        &session,
        &json!({"prompt": "x", "kind": "symphony"})
    )
    .is_err());
}
