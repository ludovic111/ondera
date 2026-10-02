//! lsuite hand-offs end to end on a headless session: a mix lands on a kimchi project, a
//! cut from kimchi becomes audio and markers to score, and the shared command names run
//! their ryolune commands.
use ryolune_engine::control::{self, Headless};
use serde_json::{json, Value};

fn kimchi_project(library: &std::path::Path) -> std::path::PathBuf {
    let dir = library.join("projects/7d1c1e2a-0000-4000-8000-00000000000a");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("project.json"),
        json!({
            "id": "7d1c1e2a-0000-4000-8000-00000000000a", "name": "Teaser",
            "created_at": "2026-10-01T10:00:00Z", "updated_at": "2026-10-01T10:00:00Z",
            "settings": {"width": 1920, "height": 1080, "fps": 30.0, "background": "#000000", "sample_rate": 48000},
            "assets": [], "tracks": [], "markers": []
        })
        .to_string(),
    )
    .unwrap();
    dir
}

#[test]
fn a_mix_goes_onto_kimchi_and_a_cut_comes_back_to_score() {
    let scratch = tempfile::tempdir().unwrap();
    // Nothing of the person's: an empty suite folder, a kimchi library of our own.
    std::env::set_var("LSUITE_HOME", scratch.path().join("lsuite"));
    std::env::set_var("KIMCHI_LIBRARY", scratch.path().join("kimchi"));
    let project = kimchi_project(&scratch.path().join("kimchi"));

    let mut host = Headless::new();
    let track = control::call(
        &mut host,
        "track.add",
        &json!({"kind": "midi", "name": "Keys"}),
        false,
    )
    .unwrap();
    control::call(
        &mut host,
        "clip.create",
        &json!({"trackId": track["id"], "startBar": 0, "lengthBars": 1,
            "notes": [{"pitch": 60, "start": 0, "length": 1, "velocity": 100}]}),
        false,
    )
    .unwrap();
    let sent = control::call(
        &mut host,
        "export.toKimchi",
        &json!({"project": "teaser", "startSeconds": 1.5, "tailSeconds": 0}),
        false,
    )
    .unwrap();
    assert_eq!(sent["placed"], true, "{sent}");
    let doc: Value =
        serde_json::from_str(&std::fs::read_to_string(project.join("project.json")).unwrap())
            .unwrap();
    assert_eq!(doc["tracks"][0]["kind"], "audio");
    assert_eq!(doc["tracks"][0]["clips"][0]["start"], 1.5);
    let wav = doc["assets"][0]["path"].as_str().unwrap();
    assert!(std::path::Path::new(wav).is_file());

    // kimchi sends its cut back: the same audio, two markers and the cut's length.
    let manifest = scratch.path().join("cut.json");
    std::fs::write(
        &manifest,
        json!({"from": "kimchi", "kind": "cut", "name": "Teaser", "audio": wav,
            "durationSeconds": 4.0,
            "markers": [{"time": 0.0, "label": "Open"}, {"time": 2.0, "label": "Title"}]})
        .to_string(),
    )
    .unwrap();
    let scored = control::call(
        &mut host,
        "session.scoreCut",
        &json!({"manifest": manifest}),
        false,
    )
    .unwrap();
    assert_eq!(scored["markers"].as_array().unwrap().len(), 2);
    let session = host.store.session();
    assert!(session.markers.iter().any(|m| m.name == "Title"));
    assert!(session.transport.cycle);
    assert!(session.tracks.iter().any(|t| t.kind == "audio"));
    assert!(!manifest.exists(), "a scored hand-off is consumed");

    // Shared lsuite names run ryolune's commands.
    assert!(
        control::call(&mut host, "app.version", &json!({}), false).unwrap()["version"].is_string()
    );
    assert!(control::call(&mut host, "project.overview", &json!({}), false).is_ok());
    let suite = control::call(&mut host, "app.suite", &json!({}), false).unwrap();
    assert_eq!(suite["apps"], json!([]));
}
