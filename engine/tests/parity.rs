//! Commands that give scripts the edits a hand can make in the window.
use ondera_engine::control::{self, Headless, Host};
use serde_json::{json, Value};

fn call(h: &mut Headless, name: &str, args: Value) -> Value {
    control::call(h, name, &args, false).unwrap_or_else(|e| panic!("{name}: {e}"))
}
fn midi_clip(h: &mut Headless) -> Value {
    let track = call(h, "track.add", json!({"kind":"midi","name":"Keys"}))["id"].clone();
    call(
        h,
        "clip.create",
        json!({"trackId":track,"startBar":2,"lengthBars":2,"notes":[
            {"start":0,"length":1,"pitch":60},{"start":4,"length":1,"pitch":64}]}),
    )["id"]
        .clone()
}

#[test]
fn trimming_the_left_edge_keeps_notes_on_their_bars() {
    let mut h = Headless::new();
    let clip = midi_clip(&mut h);
    let trimmed = call(&mut h, "clip.trim", json!({"clipId":clip,"startBar":3}));
    assert_eq!(trimmed["startBar"], 3.0);
    assert_eq!(trimmed["lengthBars"], 1.0);
    let notes = call(&mut h, "note.list", json!({"clipId":clip}));
    let notes = notes.as_array().unwrap();
    // The note on bar 2 is gone; the one on bar 3 now opens the clip.
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0]["pitch"], 64);
    assert_eq!(notes[0]["start"], 0.0);
    call(&mut h, "history.undo", json!({}));
    assert_eq!(
        call(&mut h, "note.list", json!({"clipId":clip}))
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(control::call(
        &mut h,
        "clip.trim",
        &json!({"clipId":clip,"startBar":4}),
        false
    )
    .is_err());
}

#[test]
fn deselect_clears_clip_and_note_but_keeps_the_track() {
    let mut h = Headless::new();
    let clip = midi_clip(&mut h);
    call(&mut h, "clip.select", json!({"clipId":clip}));
    let selection = call(&mut h, "clip.deselect", json!({}));
    assert!(selection["clipId"].is_null() && selection["noteId"].is_null());
    assert!(selection["trackId"].is_string());
}

#[test]
fn a_batch_is_one_undo_step() {
    let mut h = Headless::new();
    let clip = midi_clip(&mut h);
    let depth = call(&mut h, "history.info", json!({}))["undoDepth"].clone();
    let commands: Vec<Value> = (0..50)
        .map(|i| json!({"command":"note.add","params":{"clipId":clip,"start":i as f64 * 0.1,"length":0.1,"pitch":72}}))
        .collect();
    let reply = call(&mut h, "session.batch", json!({"commands":commands}));
    assert_eq!(reply["count"], 50);
    assert_eq!(
        call(&mut h, "note.list", json!({"clipId":clip}))
            .as_array()
            .unwrap()
            .len(),
        52
    );
    call(&mut h, "history.undo", json!({}));
    assert_eq!(
        call(&mut h, "note.list", json!({"clipId":clip}))
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(call(&mut h, "history.info", json!({}))["undoDepth"], depth);
}

#[test]
fn a_failing_atomic_batch_leaves_no_trace() {
    let mut h = Headless::new();
    let clip = midi_clip(&mut h);
    let before = call(&mut h, "clip.get", json!({"clipId":clip}));
    let depth = h.store().undo_depth();
    let error = control::call(
        &mut h,
        "session.batch",
        &json!({"commands":[
            {"command":"clip.rename","params":{"clipId":clip,"name":"Changed"}},
            {"command":"clip.rename","params":{"clipId":"missing","name":"x"}}]}),
        false,
    )
    .unwrap_err();
    assert!(
        error.contains("entry 1") && error.contains("rolled back"),
        "{error}"
    );
    assert_eq!(call(&mut h, "clip.get", json!({"clipId":clip})), before);
    assert_eq!(h.store().undo_depth(), depth);
    assert!(!h.store().can_redo());
}

#[test]
fn a_batch_refuses_history_files_and_itself() {
    let mut h = Headless::new();
    for name in ["history.undo", "session.save", "session.batch", "app.quit"] {
        let error = control::call(
            &mut h,
            "session.batch",
            &json!({"commands":[{"command":name,"params":{}}]}),
            false,
        )
        .unwrap_err();
        assert!(
            error.contains("cannot run inside a batch"),
            "{name}: {error}"
        );
    }
}
