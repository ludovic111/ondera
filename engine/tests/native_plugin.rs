//! The native plugin path: the example bundle through the in-process ABI, the stock library
//! through the same vtables, and the built dynamic library through the scanner when present.
use ondera_engine::{
    host::native,
    plugin::{ParamChange, ProcessContext},
    stock,
};
use ondera_plugin::ffi;

fn manifests_of(tables: &[&'static ffi::PluginVTable]) -> Vec<ffi::Manifest> {
    tables
        .iter()
        .map(|t| unsafe { ffi::read_manifest(t).unwrap() })
        .collect()
}

#[test]
fn example_bundle_exports_its_plugins_through_the_abi() {
    let entry = ondera_plugin_gain::ondera_plugin_entry();
    let tables = unsafe { native::tables_from_entry(entry) }.unwrap();
    let manifests = manifests_of(&tables);
    assert_eq!(
        manifests
            .iter()
            .map(|m| m.name.as_str())
            .collect::<Vec<_>>(),
        ["Trim", "Tilt EQ", "Bend Sine"]
    );
    assert_eq!(manifests[0].id, "org.ondera.examples.trim");
    assert_eq!(manifests[0].kind, ondera_plugin::Kind::Effect);
    assert_eq!(manifests[0].params[2].labels, ["Off", "On"]);
    let descriptor = ondera_engine::plugin::Descriptor {
        id: format!("native:{}", manifests[0].id),
        format: ondera_engine::plugin::Format::Native,
        name: manifests[0].name.clone(),
        vendor: manifests[0].vendor.clone(),
        path: "in-process".into(),
        instrument: false,
        effect: true,
        category: manifests[0].category.clone(),
    };
    let mut instance = native::instance_from(tables[0], &manifests[0], descriptor, 48000).unwrap();
    assert_eq!(instance.editor.params().len(), 3);
    assert_eq!(instance.editor.value(0), Some(0.0));
    let mut processor = instance.processor.take().unwrap();
    let mut audio = [[0.5f32, -0.5]; 256];
    // A -6 dB trim with the polarity inverted, after the smoother settles.
    let changes = [ParamChange::now(0, -6.0), ParamChange::now(2, 1.0)];
    for _ in 0..40 {
        audio = [[0.5, -0.5]; 256];
        processor.process(&mut audio, &[], &changes, &ProcessContext::default());
    }
    let expected = 0.5 * 10f32.powf(-6.0 / 20.0);
    assert!((audio[255][0] + expected).abs() < 1e-3, "{}", audio[255][0]);
    assert!((audio[255][1] - expected).abs() < 1e-3);
    // State round trip through the editor half.
    instance.editor.set_value(1, 40.0);
    let saved = instance.editor.save().unwrap();
    let mut fresh = native::instance_from(
        tables[0],
        &manifests[0],
        instance.editor.descriptor().clone(),
        48000,
    )
    .unwrap();
    fresh.editor.load(&saved).unwrap();
    assert_eq!(fresh.editor.value(1), Some(40.0));
    assert!(fresh.editor.load(b"not json").is_err());
}

#[test]
fn stock_library_serves_every_plugin_through_the_same_abi() {
    let tables = stock::tables();
    assert_eq!(tables.len(), 34);
    let manifests = native::manifests(tables).unwrap();
    for manifest in &manifests {
        assert!(manifest.id.starts_with("org.ondera.stock."));
        // The manifest format is ABI 1's, which is what lets ABI 1 hosts read ABI 2 plugins.
        assert_eq!(manifest.abi, ondera_plugin::BASE_ABI_VERSION);
        assert!(!manifest.description.is_empty(), "{}", manifest.name);
        let descriptor = stock::descriptor(&manifest.name).unwrap();
        assert_eq!(
            descriptor.instrument,
            manifest.kind == ondera_plugin::Kind::Instrument
        );
    }
    // Legacy stock state (a bare JSON array) still loads.
    let mut instance = stock::create("Utility", 44100).unwrap();
    instance.editor.load(b"[-6.0, 20.0, 1, 0, 0]").unwrap();
    assert_eq!(instance.editor.value(0), Some(-6.0));
    assert_eq!(instance.editor.value(2), Some(1.0));
    let mut processor = instance.processor.take().unwrap();
    let mut audio = [[1.0f32, 1.0]; 64];
    processor.process(&mut audio, &[], &[], &ProcessContext::default());
    assert!(
        audio[0][0] < 0.0,
        "restored polarity inversion reaches the audio thread"
    );
}

#[test]
fn built_dynamic_library_scans_and_instantiates_when_present() {
    // Cargo builds the bundle's dynamic library with its rlib, next to this test binary;
    // the copy one level up is only refreshed by a workspace build and may be stale.
    let exe = std::env::current_exe().unwrap();
    let name = format!(
        "{}ondera_plugin_gain.{}",
        std::env::consts::DLL_PREFIX,
        native::library_extension()
    );
    let Some(library) = exe
        .parent()
        .map(|deps| deps.join(&name))
        .filter(|p| p.is_file())
    else {
        eprintln!("skipping: {name} is not built");
        return;
    };
    let descriptors = native::scan(&library).unwrap();
    assert_eq!(descriptors.len(), 3);
    assert_eq!(native::abi_of(&library).unwrap(), 2);
    assert_eq!(descriptors[1].id, "native:org.ondera.examples.tilt");
    assert_eq!(descriptors[1].format, ondera_engine::plugin::Format::Native);
    assert!(native::library_path(&library)
        .unwrap()
        .ends_with(library.file_name().unwrap()));
    assert!(native::looks_like_plugin(&library));
}

fn bend_sine(rate: u32) -> ondera_engine::plugin::Instance {
    let entry = ondera_plugin_gain::ondera_plugin_entry_v2();
    let tables = unsafe { native::tables_from_entry_v2(entry) }.unwrap();
    assert_eq!(tables.len(), 3);
    assert!(tables.iter().all(|t| t.abi() == 2));
    let manifest = unsafe { ffi::read_manifest(tables[2].base) }.unwrap();
    assert_eq!(manifest.name, "Bend Sine");
    let descriptor = ondera_engine::plugin::Descriptor {
        id: format!("native:{}", manifest.id),
        format: ondera_engine::plugin::Format::Native,
        name: manifest.name.clone(),
        vendor: manifest.vendor.clone(),
        path: "in-process".into(),
        instrument: true,
        effect: false,
        category: manifest.category.clone(),
    };
    native::instance_from(tables[2], &manifest, descriptor, rate).unwrap()
}
/// Zero crossings per second of the left channel: the pitch that was played.
fn pitch_of(audio: &[[f32; 2]], rate: f64) -> f64 {
    let crossings = audio
        .windows(2)
        .filter(|w| w[0][0] <= 0.0 && w[1][0] > 0.0)
        .count();
    crossings as f64 * rate / audio.len() as f64
}
fn render(
    processor: &mut Box<dyn ondera_engine::plugin::Processor>,
    blocks: usize,
    first: &[ondera_engine::plugin::NoteEvent],
    params: &[ParamChange],
) -> Vec<[f32; 2]> {
    let mut out = vec![];
    for block in 0..blocks {
        let mut audio = [[0.0f32; 2]; 256];
        let (notes, params) = if block == 0 {
            (first, params)
        } else {
            (&[][..], &[][..])
        };
        processor.process(&mut audio, notes, params, &ProcessContext::default());
        out.extend_from_slice(&audio);
    }
    out
}

#[test]
fn an_abi_2_plugin_keeps_state_of_its_own_in_the_insert_blob() {
    use base64::Engine as _;
    let a4 = [ondera_engine::plugin::NoteEvent {
        frame: 0,
        on: true,
        pitch: 69,
        velocity: 100,
        channel: 0,
    }];
    let mut plain = bend_sine(48000);
    assert_eq!(plain.editor.tail_seconds(), 0.25);
    // Nothing but parameters: the state is the bare list older sessions hold.
    assert_eq!(plain.editor.save().unwrap(), b"[-6.0,0.0]");
    let mut processor = plain.processor.take().unwrap();
    let tuned_to_440 = pitch_of(&render(&mut processor, 200, &a4, &[]), 48000.0);
    assert!((tuned_to_440 - 440.0).abs() < 2.0, "{tuned_to_440}");

    // A is retuned a whole tone up (200 cents) by state no parameter can express.
    let state = "bendsine 1\n0 0 0 0 0 0 0 0 0 200 0 0";
    let blob = serde_json::to_vec(&serde_json::json!({
        "values": [-12.0, 0.0],
        "state": base64::engine::general_purpose::STANDARD.encode(state),
    }))
    .unwrap();
    plain.editor.load(&blob).unwrap();
    assert_eq!(plain.editor.value(0), Some(-12.0));
    let retuned = pitch_of(&render(&mut processor, 200, &a4, &[]), 48000.0);
    let expected = 440.0 * 2f64.powf(2.0 / 12.0);
    assert!((retuned - expected).abs() < 2.0, "{retuned} vs {expected}");
    // The instance the audio thread gave back is collected on this thread.
    plain.editor.idle();

    // Save and load again: the blob round-trips, values and state.
    let saved = plain.editor.save().unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&saved).unwrap();
    assert_eq!(parsed["values"][0], -12.0);
    let mut fresh = bend_sine(48000);
    fresh.editor.load(&saved).unwrap();
    let mut fresh_processor = fresh.processor.take().unwrap();
    let again = pitch_of(&render(&mut fresh_processor, 200, &a4, &[]), 48000.0);
    assert!((again - expected).abs() < 2.0);

    // State the plugin refuses changes nothing, not even the parameter values beside it.
    let bad = serde_json::to_vec(&serde_json::json!({
        "values": [0.0, 1.0],
        "state": base64::engine::general_purpose::STANDARD.encode("bendsine 9\n"),
    }))
    .unwrap();
    assert!(fresh.editor.load(&bad).unwrap_err().contains("saved state"));
    assert_eq!(fresh.editor.value(0), Some(-12.0));
}

#[test]
fn a_latency_change_reaches_the_editor_after_the_block_it_happened_in() {
    let mut instance = bend_sine(48000);
    let mut processor = instance.processor.take().unwrap();
    assert_eq!(instance.editor.latency(), 0);
    render(&mut processor, 1, &[], &[ParamChange::now(1, 1.0)]);
    assert_eq!(instance.editor.latency(), 64);
    render(&mut processor, 1, &[], &[ParamChange::now(1, 0.0)]);
    assert_eq!(instance.editor.latency(), 0);
}
