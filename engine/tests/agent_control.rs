//! What an agent needs from the registry: one overview of the song, tracks and clips by
//! name, and a plugin's parameters and programs by name, display text or 0-1 position.
use ondera_engine::{
    audio::Library,
    control::{self, Headless, Host},
    host::native,
    model::Insert,
    plugin::{Editor, Instance, ParamInfo},
    store::{Command, Store},
    Result,
};
use ondera_plugin::ffi;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn call(h: &mut dyn Host, name: &str, args: Value) -> Value {
    control::call(h, name, &args, false).unwrap_or_else(|e| panic!("{name}: {e}"))
}
fn fail(h: &mut dyn Host, name: &str, args: Value) -> String {
    control::call(h, name, &args, false).expect_err(name)
}
fn demo() -> Headless {
    let mut h = Headless::new();
    call(&mut h, "session.new", json!({"demo": true}));
    h
}

#[test]
fn the_overview_describes_the_whole_song_in_one_bounded_answer() {
    let mut h = demo();
    call(&mut h, "marker.add", json!({"bar": 4, "name": "Verse"}));
    call(
        &mut h,
        "track.setSolo",
        json!({"trackId": "Drums", "solo": true}),
    );
    let o = call(&mut h, "session.overview", json!({}));
    let text = o.to_string();
    assert!(text.len() < 16_000, "overview is {} bytes", text.len());
    assert_eq!(o["song"]["meter"], "4/4");
    assert_eq!(o["song"]["tempo"], 120.0);
    assert_eq!(o["sections"][0]["name"], "Verse");
    let bass = o["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "Bass")
        .unwrap();
    assert_eq!(bass["instrument"]["format"], "stock");
    assert_eq!(bass["inserts"][2]["bypassed"], true);
    assert_eq!(bass["sends"]["A · Reverb"], -12.0);
    assert_eq!(bass["clips"]["items"][1]["pitch"], "C2-G#3");
    assert!(bass["problems"][0]
        .as_str()
        .unwrap()
        .contains("silenced by solo on Drums"));
    assert_eq!(o["buses"]["bus-a"]["inserts"][0]["changed"]["Mix"], "100 %");
    assert_eq!(o["selection"]["track"]["name"], "Bass");
    assert!(o["history"]["canUndo"].as_bool().unwrap());
    assert!(o["next"]["parameters"].is_string());
    // Headless: no window to describe.
    assert!(o.get("window").is_none());

    let narrow = call(
        &mut h,
        "session.overview",
        json!({"trackId": "Keys", "maxClips": 1, "parameters": false}),
    );
    assert_eq!(narrow["tracks"].as_array().unwrap().len(), 1);
    assert_eq!(
        narrow["tracks"][0]["clips"]["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(narrow["truncated"].as_str().unwrap().contains("Keys"));
    assert!(fail(&mut h, "ui.state", json!({})).contains("running Ondera app"));
}

#[test]
fn tracks_clips_and_markers_answer_to_their_names() {
    let mut h = demo();
    let muted = call(
        &mut h,
        "track.setMute",
        json!({"trackId": "bass", "muted": true}),
    );
    assert_eq!(muted["mute"], true);
    call(
        &mut h,
        "track.setMute",
        json!({"trackId": "lead vox", "muted": true}),
    );
    assert!(call(&mut h, "track.list", json!({}))
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["name"] == "Lead Vox" && t["mute"] == true));
    let error = fail(
        &mut h,
        "track.setMute",
        json!({"trackId": "Bas", "muted": true}),
    );
    assert!(
        error.contains("Unknown track `Bas`") && error.contains("Did you mean Bass?"),
        "{error}"
    );
    assert!(error.contains("Drums (drums)"), "{error}");
    // Two tracks called the same must be told apart by id.
    call(&mut h, "track.add", json!({"kind": "midi", "name": "Bass"}));
    let error = fail(
        &mut h,
        "track.setMute",
        json!({"trackId": "Bass", "muted": false}),
    );
    assert!(error.contains("ambiguous"), "{error}");
    // Clips by name, buses by name.
    let clip = call(&mut h, "clip.get", json!({"clipId": "Bass verse"}));
    assert_eq!(clip["id"], "bass-2");
    assert!(fail(&mut h, "clip.get", json!({"clipId": "Bass vers"}))
        .contains("Did you mean Bass verse?"));
    assert_eq!(
        call(&mut h, "strip.get", json!({"trackId": "A · Reverb"}))["trackId"],
        "bus-a"
    );
    call(&mut h, "marker.add", json!({"bar": 8, "name": "Chorus"}));
    let moved = call(
        &mut h,
        "marker.move",
        json!({"markerId": "chorus", "bar": 6}),
    );
    assert_eq!(moved["bar"], 6.0, "{moved}");
    // A batch resolves each entry.
    call(
        &mut h,
        "session.batch",
        json!({"commands": [{"command": "track.setSolo", "params": {"trackId": "Keys", "solo": true}}]}),
    );
    assert!(call(&mut h, "track.list", json!({}))
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["name"] == "Keys" && t["solo"] == true));
}

#[test]
fn parameters_are_found_by_name_and_set_by_text_or_position_in_one_undo_step() {
    let mut h = demo();
    let found = call(
        &mut h,
        "strip.parameters",
        json!({"trackId": "Bass", "slot": 0, "query": "thresh"}),
    );
    assert_eq!(found["total"], 1);
    let p = &found["parameters"][0];
    assert_eq!(p["name"], "Threshold");
    assert_eq!(p["display"], "-18.0 dB");
    assert_eq!(p["automatable"], true);
    let set = call(
        &mut h,
        "strip.setParameter",
        json!({"trackId": "Bass", "slot": 0, "parameter": "threshold", "text": "-30 dB"}),
    );
    assert_eq!(set["changed"][0]["value"], -30.0);
    assert_eq!(set["changed"][0]["display"], "-30.0 dB");
    let set = call(
        &mut h,
        "strip.setParameter",
        json!({"trackId": "Bass", "slot": 0, "parameterId": 0, "normalized": 0.5}),
    );
    assert_eq!(set["changed"][0]["value"], -30.0);
    let changed = call(
        &mut h,
        "strip.parameters",
        json!({"trackId": "Bass", "slot": 0, "changed": true}),
    );
    assert!(changed["parameters"]
        .as_array()
        .unwrap()
        .iter()
        .all(|p| p["value"] != p["default"]));
    assert!(changed["parameters"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["name"] == "Threshold" && p["value"] == -30.0));
    call(&mut h, "history.undo", json!({}));
    call(&mut h, "history.undo", json!({}));
    let back = call(
        &mut h,
        "strip.parameters",
        json!({"trackId": "Bass", "slot": 0, "query": "threshold"}),
    );
    assert_eq!(
        back["parameters"][0]["value"], -18.0,
        "two edits, two undo steps"
    );

    assert!(fail(
        &mut h,
        "strip.setParameter",
        json!({"trackId": "Bass", "slot": 0, "parameter": "threshold", "value": -3, "text": "-3 dB"})
    )
    .contains("exactly one"));
    assert!(fail(
        &mut h,
        "strip.setParameter",
        json!({"trackId": "Bass", "slot": 0, "parameter": "threshold", "value": 20})
    )
    .contains("between -60 and 0"));
    assert!(fail(
        &mut h,
        "strip.setParameter",
        json!({"trackId": "Bass", "slot": 0, "parameter": "cutoff", "value": 1})
    )
    .contains("no parameter matching `cutoff`"));

    // Several at once, keyed by id or name, valued by number, text or position.
    let many = call(
        &mut h,
        "strip.setParameters",
        json!({"trackId": "Bass", "slot": 0, "values": {"0": -24, "Ratio": {"normalized": 1.0}}}),
    );
    assert_eq!(many["changed"].as_array().unwrap().len(), 2);
    let page = call(
        &mut h,
        "strip.parameters",
        json!({"trackId": "Bass", "slot": 0, "limit": 2}),
    );
    assert_eq!(page["parameters"].as_array().unwrap().len(), 2);
    assert_eq!(page["nextOffset"], 2);
}

#[test]
fn plugins_load_by_name_into_the_first_free_slot_and_leave_cleanly() {
    let mut h = demo();
    let loaded = call(
        &mut h,
        "strip.setPlugin",
        json!({"trackId": "Bass", "plugin": "space", "firstFreeSlot": true}),
    );
    assert_eq!(loaded["loaded"]["pluginId"], "stock:Space");
    assert_eq!(loaded["loaded"]["slot"], 3);
    let lane = call(
        &mut h,
        "automation.create",
        json!({"target": "pluginParameter", "trackId": "Bass", "slot": 3, "parameter": "mix"}),
    );
    let lane_id = lane["id"]
        .as_str()
        .or(lane["lane"]["id"].as_str())
        .map(str::to_string);
    let listed = call(
        &mut h,
        "strip.parameters",
        json!({"trackId": "Bass", "slot": 3, "query": "mix"}),
    );
    assert!(
        listed["parameters"][0]["automationLane"].is_string(),
        "{listed} {lane_id:?}"
    );
    let removed = call(
        &mut h,
        "strip.removeInsert",
        json!({"trackId": "Bass", "slot": 3}),
    );
    assert_eq!(removed["removed"], "Space");
    assert!(call(&mut h, "automation.list", json!({}))["lanes"]
        .as_array()
        .unwrap()
        .is_empty());
    call(&mut h, "history.undo", json!({}));
    assert_eq!(
        call(&mut h, "automation.list", json!({}))["lanes"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    assert!(fail(
        &mut h,
        "strip.setPlugin",
        json!({"trackId": "Bass", "plugin": "e"})
    )
    .contains("too short"));
    assert!(fail(
        &mut h,
        "strip.setPlugin",
        json!({"trackId": "Bass", "plugin": "Space"})
    )
    .contains("not an instrument"));
    assert!(fail(
        &mut h,
        "strip.setPlugin",
        json!({"trackId": "Drums", "plugin": "E-Piano"})
    )
    .contains("Only MIDI tracks have an instrument"));
    let piano = call(
        &mut h,
        "strip.setPlugin",
        json!({"trackId": "Keys", "plugin": "e-piano"}),
    );
    assert_eq!(piano["instrument"], "E-Piano Mk I");
    let described = call(
        &mut h,
        "plugin.describe",
        json!({"pluginId": "Space", "query": "mix"}),
    );
    assert_eq!(described["descriptor"]["id"], "stock:Space");
    assert_eq!(described["parameters"][0]["name"], "Mix");
}

#[test]
fn programs_list_presets_and_load_them_by_name() {
    let mut h = demo();
    let programs = call(&mut h, "strip.programs", json!({"trackId": "Bass"}));
    assert_eq!(programs["source"], "none");
    let presets: Vec<&str> = programs["presets"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|p| p["name"].as_str())
        .collect();
    assert!(presets.contains(&"Plucky bass"), "{presets:?}");
    let loaded = call(
        &mut h,
        "strip.setProgram",
        json!({"trackId": "Bass", "name": "Plucky bass"}),
    );
    assert_eq!(loaded["preset"], "Plucky bass");
    assert!(fail(
        &mut h,
        "strip.setProgram",
        json!({"trackId": "Bass", "index": 0})
    )
    .contains("0 programs"));
}

#[test]
fn display_text_reads_back_into_plain_values() {
    let db = ParamInfo {
        id: 0,
        name: "Gain".into(),
        min: -60.0,
        max: 12.0,
        default: 0.0,
        unit: "dB".into(),
        steps: 0,
        log: false,
        labels: vec![],
    };
    assert_eq!(db.parse_text("-6 dB"), Some(-6.0));
    assert_eq!(db.parse_text("+3"), Some(3.0));
    assert_eq!(db.parse_text("50%"), Some(-24.0));
    assert_eq!(db.parse_text("loud"), None);
    let hz = ParamInfo {
        unit: "Hz".into(),
        min: 20.0,
        max: 20_000.0,
        log: true,
        ..db.clone()
    };
    assert_eq!(hz.parse_text("2.5k"), Some(2500.0));
    assert_eq!(hz.parse_text("440 Hz"), Some(440.0));
    let mode = ParamInfo {
        min: 0.0,
        max: 2.0,
        steps: 2,
        unit: String::new(),
        labels: vec!["Room".into(), "Hall".into(), "Plate".into()],
        ..db.clone()
    };
    assert_eq!(mode.parse_text("hall"), Some(1.0));
    let switch = ParamInfo {
        min: 0.0,
        max: 1.0,
        steps: 1,
        unit: String::new(),
        labels: vec![],
        ..db
    };
    assert_eq!(switch.parse_text("On"), Some(1.0));
}

/// A host that keeps a plugin loaded the way the window does, so commands read and set it
/// through that instance instead of instantiating one: the example bundle's Trim, as an
/// external (native ABI) plugin that is in no scan cache.
struct Window {
    inner: Headless,
    key: String,
    instance: Instance,
}
impl Host for Window {
    fn store(&self) -> &Store {
        &self.inner.store
    }
    fn store_mut(&mut self) -> &mut Store {
        &mut self.inner.store
    }
    fn library(&self) -> &Library {
        &self.inner.library
    }
    fn library_mut(&mut self) -> &mut Library {
        &mut self.inner.library
    }
    fn path(&self) -> Option<&Path> {
        None
    }
    fn mode(&self) -> &'static str {
        "live"
    }
    fn playing(&self) -> bool {
        false
    }
    fn position(&self) -> f64 {
        0.0
    }
    fn play(&mut self) -> Result<()> {
        Err("no audio in tests".into())
    }
    fn stop(&mut self) -> Result<()> {
        Ok(())
    }
    fn locate(&mut self, _beats: f64) -> Result<()> {
        Ok(())
    }
    fn new_session(&mut self, demo: bool) -> Result<()> {
        self.inner.new_session(demo)
    }
    fn open(&mut self, path: &Path) -> Result<()> {
        Host::open(&mut self.inner, path)
    }
    fn save(&mut self, path: Option<&Path>) -> Result<PathBuf> {
        self.inner.save(path)
    }
    fn bounce(&mut self, path: &Path) -> Result<()> {
        self.inner.bounce(path)
    }
    fn loaded_editor(&mut self, insert_id: &str) -> Option<&mut dyn Editor> {
        (insert_id == self.key).then(|| self.instance.editor.as_mut() as &mut dyn Editor)
    }
    fn plugin_failures(&self) -> Vec<(String, String)> {
        vec![("insert-2".into(), "the bundle moved".into())]
    }
    fn live(&mut self, action: &str, _params: &Value) -> Result<Value> {
        match action {
            "ui.state" => Ok(json!({"panels": {"mixer": true}})),
            _ => Err(format!("{action} is not in this test window")),
        }
    }
}

fn trim() -> Instance {
    let tables =
        unsafe { native::tables_from_entry(ondera_plugin_gain::ondera_plugin_entry()) }.unwrap();
    let manifest = unsafe { ffi::read_manifest(tables[0]).unwrap() };
    let descriptor = ondera_engine::plugin::Descriptor {
        id: format!("native:{}", manifest.id),
        format: ondera_engine::plugin::Format::Native,
        name: manifest.name.clone(),
        vendor: manifest.vendor.clone(),
        path: "in-process".into(),
        instrument: false,
        effect: true,
        category: manifest.category.clone(),
    };
    native::instance_from(tables[0], &manifest, descriptor, 48000).unwrap()
}

#[test]
fn an_external_plugin_the_window_has_loaded_is_read_and_set_through_that_instance() {
    let mut inner = demo();
    let key = "trim-on-drums".to_string();
    let mut strip = inner
        .store
        .session()
        .strips
        .get("drums")
        .cloned()
        .unwrap_or_default();
    let trim_insert = Insert::new(key.clone(), "native:org.ondera.examples.trim", "Trim");
    match strip.inserts.first_mut() {
        Some(first) => *first = trim_insert,
        None => strip.inserts.push(trim_insert),
    }
    inner
        .store
        .dispatch(Command::SetStrip {
            track: "drums".into(),
            strip,
        })
        .unwrap();
    let mut w = Window {
        inner,
        key,
        instance: trim(),
    };
    let listed = call(
        &mut w,
        "strip.parameters",
        json!({"trackId": "Drums", "slot": 0}),
    );
    assert_eq!(listed["format"], "native");
    assert_eq!(listed["parameterCount"], 3);
    let set = call(
        &mut w,
        "strip.setParameters",
        json!({"trackId": "Drums", "slot": 0, "values": {"Gain": "-6 dB", "Invert": "On"}}),
    );
    let values: Vec<f64> = set["changed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["value"].as_f64().unwrap())
        .collect();
    assert_eq!(values, [-6.0, 1.0]);
    let o = call(&mut w, "session.overview", json!({"trackId": "drums"}));
    let insert = &o["tracks"][0]["inserts"][0];
    assert_eq!(insert["format"], "native");
    assert_eq!(insert["changed"]["Gain"], "-6.00 dB", "{insert}");
    assert!(o["tracks"][0]["problems"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p.as_str().unwrap().contains("not installed")));
    assert_eq!(o["window"]["panels"]["mixer"], true);
    let bass = call(&mut w, "session.overview", json!({"trackId": "bass"}));
    assert!(bass["tracks"][0]["problems"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p
            .as_str()
            .unwrap()
            .contains("failed to load: the bundle moved")));
}
