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

/// Removing the selected note (or replacing every note) left `selection.noteId` naming a
/// note that no longer existed.
#[test]
fn removing_the_selected_note_clears_the_note_selection() {
    let mut host = Headless::new();
    let track = midi_track(&mut host);
    let clip = call(
        &mut host,
        "clip.create",
        json!({"trackId":track,"startBar":0,"lengthBars":1,
               "notes":[{"start":0,"length":1,"pitch":60},{"start":1,"length":1,"pitch":62}]}),
    );
    let clip_id = clip["id"].as_str().unwrap().to_string();
    let notes = call(&mut host, "note.list", json!({"clipId":clip_id}));
    let first = notes[0]["id"].as_str().unwrap().to_string();
    let second = notes[1]["id"].as_str().unwrap().to_string();
    call(
        &mut host,
        "clip.select",
        json!({"clipId":clip_id,"noteId":first}),
    );
    // Editing another note keeps the selection.
    call(
        &mut host,
        "note.update",
        json!({"clipId":clip_id,"noteId":second,"pitch":64}),
    );
    assert_eq!(
        call(&mut host, "session.info", json!({}))["selection"]["noteId"],
        first.as_str()
    );
    call(
        &mut host,
        "note.remove",
        json!({"clipId":clip_id,"noteId":first}),
    );
    let selection = &call(&mut host, "session.info", json!({}))["selection"];
    assert!(selection["noteId"].is_null(), "{selection}");
    assert_eq!(selection["clipId"], clip_id.as_str());
}

/// A file where two slots share an insert id (hand-merged, or written by a buggy tool) made
/// both tracks run through one plugin instance; an out-of-range piano-roll scroll from a file
/// left the roll empty.
#[test]
fn loading_repairs_shared_insert_ids_and_the_piano_roll_scroll() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("merged.ondera");
    let mut host = Headless::new();
    let a = midi_track(&mut host);
    let b = midi_track(&mut host);
    for track in [&a, &b] {
        call(
            &mut host,
            "strip.setInsert",
            json!({"trackId":track,"slot":0,"effect":"Chorus"}),
        );
    }
    call(&mut host, "session.save", json!({ "path": path }));
    let mut file: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let shared = file["session"]["strips"][&a]["inserts"][0]["id"].clone();
    file["session"]["strips"][&b]["inserts"][0]["id"] = shared;
    file["session"]["view"]["editorLowPitch"] = json!(250);
    std::fs::write(&path, file.to_string()).unwrap();
    let (session, _) = ondera_engine::document::load(&path).unwrap();
    let store = ondera_engine::store::Store::new(session).unwrap();
    let session = store.session();
    assert_ne!(
        session.strips[&a].inserts[0].id,
        session.strips[&b].inserts[0].id
    );
    let mut ids: Vec<&str> = session
        .strips
        .values()
        .flat_map(|s| s.inserts.iter().chain(s.synth.iter()))
        .map(|i| i.id.as_str())
        .collect();
    let count = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), count, "every slot has its own id");
    assert_eq!(session.view.editor_low_pitch, Some(108));
}

/// `clip.legato` failed with "Invalid MIDI note" when a shortened region left a note past its
/// end; names of 41 CJK characters were refused as longer than 120 "characters".
#[test]
fn legato_ignores_notes_past_the_region_and_names_count_characters() {
    let mut host = Headless::new();
    let track = midi_track(&mut host);
    let clip = call(
        &mut host,
        "clip.create",
        json!({"trackId":track,"startBar":0,"lengthBars":2,
               "notes":[{"start":0,"length":0.5,"pitch":60},{"start":6,"length":1,"pitch":62}]}),
    );
    let id = clip["id"].as_str().unwrap().to_string();
    call(
        &mut host,
        "clip.resize",
        json!({"clipId":id,"lengthBars":1}),
    );
    call(&mut host, "clip.legato", json!({"clipId":id}));
    let notes = call(&mut host, "note.list", json!({"clipId":id}));
    assert_eq!(notes[0]["length"], 4.0, "held to the region end");
    assert_eq!(notes[1]["start"], 6.0);
    assert_eq!(notes[1]["length"], 1.0);
    let name: String = "音".repeat(41);
    call(&mut host, "take.create", json!({ "name": name }));
    let lanes = json!([{"steps":4,"pulses":4,"rotation":0,"pitch":36,"velocity":100}]);
    call(
        &mut host,
        "rhythm.create",
        json!({ "lanes": lanes, "bars": 1, "name": "ドラム".repeat(40) }),
    );
}
