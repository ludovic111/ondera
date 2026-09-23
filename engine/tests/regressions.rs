//! Regression tests for bugs found in the 0.8 quality pass, one test per bug.
use ondera_engine::control::{self, Headless, Host};
use serde_json::{json, Value};

fn call(host: &mut dyn Host, name: &str, params: Value) -> Value {
    control::call(host, name, &params, false).unwrap_or_else(|e| panic!("{name} {params}: {e}"))
}
fn fail(host: &mut dyn Host, name: &str, params: Value) -> String {
    control::call(host, name, &params, false)
        .err()
        .unwrap_or_else(|| panic!("{name} {params} should fail"))
}
fn midi_track(host: &mut dyn Host) -> String {
    call(host, "track.add", json!({"kind":"midi","name":"Keys"}))["id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// `strip.setInsert slot=0 bypassed=true` without `effect` emptied the slot, throwing the
/// effect and its settings away instead of bypassing it.
#[test]
fn bypassing_an_insert_by_slot_keeps_the_effect() {
    let mut host = Headless::new();
    let track = midi_track(&mut host);
    call(
        &mut host,
        "strip.setInsert",
        json!({"trackId":track,"slot":0,"effect":"Chorus"}),
    );
    let strip = call(
        &mut host,
        "strip.setInsert",
        json!({"trackId":track,"slot":0,"bypassed":true}),
    );
    assert_eq!(strip["inserts"][0]["effect"], "Chorus");
    assert_eq!(strip["inserts"][0]["state"], "bypassed");
    let strip = call(
        &mut host,
        "strip.setInsert",
        json!({"trackId":track,"slot":0,"bypassed":false}),
    );
    assert_eq!(strip["inserts"][0]["state"], "active");
    assert!(fail(
        &mut host,
        "strip.setInsert",
        json!({"trackId":track,"slot":1,"bypassed":true}),
    )
    .contains("empty"));
    let strip = call(
        &mut host,
        "strip.setInsert",
        json!({"trackId":track,"slot":0}),
    );
    assert_eq!(strip["inserts"][0]["state"], "empty");
}

/// `rhythm.preview path=…` could overwrite the open session file with a WAV, and an agent
/// with file operations off could write anywhere through it.
#[test]
fn rhythm_preview_respects_the_session_file_and_agent_permissions() {
    use ondera_engine::{control_app, settings::Permissions};
    let dir = tempfile::tempdir().unwrap();
    let song = dir.path().join("song.ondera");
    let mut host = Headless::new();
    call(&mut host, "session.save", json!({ "path": song }));
    let lanes = json!([{"steps":4,"pulses":4,"rotation":0,"pitch":36,"velocity":100}]);
    let error = fail(
        &mut host,
        "rhythm.preview",
        json!({ "lanes": lanes, "bars": 1, "path": song }),
    );
    assert!(error.contains("open session file"), "{error}");
    assert!(std::fs::read_to_string(&song).unwrap().starts_with('{'));
    let spec = control::spec("rhythm.preview").unwrap();
    assert!(spec.mutates, "a command that writes files is not read-only");
    let off = Permissions {
        file_operations: false,
        ..Permissions::default()
    };
    let with_path = json!({ "lanes": lanes, "bars": 1, "path": "/tmp/x.wav" });
    assert!(control_app::denied_for_agent_request("rhythm.preview", &with_path, &off).is_some());
    assert!(control_app::denied_for_agent_request(
        "ui.screenshot",
        &json!({"path":"/tmp/x.png"}),
        &off
    )
    .is_some());
    let without = json!({ "lanes": lanes, "bars": 1 });
    assert!(control_app::denied_for_agent_request("rhythm.preview", &without, &off).is_none());
    assert!(control_app::denied_for_agent_request(
        "rhythm.preview",
        &with_path,
        &Permissions::default()
    )
    .is_none());
}

/// `strip.getState` inside `session.batch` captured plugin state as an undo step of its own
/// in the window, which ended the batch's step and broke its rollback.
#[test]
fn strip_get_state_is_not_batchable() {
    let mut host = Headless::new();
    let track = midi_track(&mut host);
    let error = fail(
        &mut host,
        "session.batch",
        json!({"commands":[{"command":"strip.getState","params":{"trackId":track}}]}),
    );
    assert!(error.contains("strip.getState"), "{error}");
}

/// A failed atomic `session.batch` rolled the document back but emptied Redo: its first edit
/// had cleared the redo history, and the rollback did not bring it back.
#[test]
fn a_failed_atomic_batch_keeps_redo() {
    let mut host = Headless::new();
    call(&mut host, "session.rename", json!({"name":"One"}));
    call(&mut host, "session.rename", json!({"name":"Two"}));
    call(&mut host, "history.undo", json!({}));
    assert_eq!(call(&mut host, "history.info", json!({}))["canRedo"], true);
    let error = fail(
        &mut host,
        "session.batch",
        json!({"commands":[
            {"command":"session.rename","params":{"name":"Batch"}},
            {"command":"track.remove","params":{"trackId":"missing"}}
        ]}),
    );
    assert!(error.contains("rolled back"), "{error}");
    assert_eq!(call(&mut host, "session.info", json!({}))["name"], "One");
    assert_eq!(call(&mut host, "history.info", json!({}))["canRedo"], true);
    call(&mut host, "history.redo", json!({}));
    assert_eq!(call(&mut host, "session.info", json!({}))["name"], "Two");
}
