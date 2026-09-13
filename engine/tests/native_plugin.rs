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
fn example_bundle_exports_two_plugins_through_the_abi() {
    let entry = ondera_plugin_gain::ondera_plugin_entry();
    let tables = unsafe { native::tables_from_entry(entry) }.unwrap();
    let manifests = manifests_of(&tables);
    assert_eq!(
        manifests
            .iter()
            .map(|m| m.name.as_str())
            .collect::<Vec<_>>(),
        ["Trim", "Tilt EQ"]
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
    let changes = [
        ParamChange { id: 0, value: -6.0 },
        ParamChange { id: 2, value: 1.0 },
    ];
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
    assert_eq!(tables.len(), 24);
    let manifests = native::manifests(tables).unwrap();
    for manifest in &manifests {
        assert!(manifest.id.starts_with("org.ondera.stock."));
        assert_eq!(manifest.abi, ondera_plugin::ABI_VERSION);
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
    let Ok(target) = std::env::var("CARGO_TARGET_DIR").or_else(|_| {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.ancestors().nth(3).map(|p| p.to_path_buf()))
            .map(|p| p.to_string_lossy().into_owned())
            .ok_or(())
    }) else {
        return;
    };
    let library = std::path::Path::new(&target).join("debug").join(format!(
        "{}ondera_plugin_gain.{}",
        std::env::consts::DLL_PREFIX,
        native::library_extension()
    ));
    if !library.is_file() {
        eprintln!("skipping: {} is not built", library.display());
        return;
    }
    let descriptors = native::scan(&library).unwrap();
    assert_eq!(descriptors.len(), 2);
    assert_eq!(descriptors[1].id, "native:org.ondera.examples.tilt");
    assert_eq!(descriptors[1].format, ondera_engine::plugin::Format::Native);
    assert!(native::library_path(&library)
        .unwrap()
        .ends_with(library.file_name().unwrap()));
    assert!(native::looks_like_plugin(&library));
}
