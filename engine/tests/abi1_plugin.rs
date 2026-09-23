//! Libraries built before plugin ABI 2 keep loading and sounding the same. The fixture is a
//! plugin frozen at the ABI 1 layout with no dependency on the SDK; see its `lib.rs`. This
//! file links the fixture instead of the example bundle: both export `ondera_plugin_entry`.
use ondera_engine::{
    host::native,
    plugin::{NoteEvent, ParamChange, ProcessContext},
};
use ondera_plugin::ffi;

fn descriptor(manifest: &ffi::Manifest, path: &str) -> ondera_engine::plugin::Descriptor {
    ondera_engine::plugin::Descriptor {
        id: format!("native:{}", manifest.id),
        format: ondera_engine::plugin::Format::Native,
        name: manifest.name.clone(),
        vendor: manifest.vendor.clone(),
        path: path.into(),
        instrument: false,
        effect: true,
        category: manifest.category.clone(),
    }
}
/// Default gain, a parameter change, a note and the transport all arrive as ABI 1 laid them out.
fn exercise(mut instance: ondera_engine::plugin::Instance) {
    assert_eq!(instance.editor.params().len(), 1);
    assert_eq!(instance.editor.value(0), Some(0.5));
    assert_eq!(instance.editor.latency(), 7);
    assert_eq!(instance.editor.tail_seconds(), 0.0);
    let mut processor = instance.processor.take().unwrap();
    let ctx = ProcessContext {
        tempo: 93.0,
        ..Default::default()
    };
    let mut audio = [[1.0f32, -1.0]; 64];
    processor.process(&mut audio, &[], &[], &ctx);
    assert_eq!(audio[63], [0.5, -0.5]);
    let note = NoteEvent {
        frame: 9,
        on: true,
        pitch: 61,
        velocity: 100,
        channel: 0,
    };
    let mut audio = [[1.0f32, -1.0]; 64];
    processor.process(
        &mut audio,
        &[note.into()],
        &[ParamChange::now(0, 2.0)],
        &ctx,
    );
    assert_eq!(audio[8], [2.0, -2.0]);
    assert_eq!(
        audio[9],
        [61.0, 93.0],
        "pitch and tempo read at their ABI 1 offsets"
    );
    // State is the parameter list, as it was.
    instance.editor.set_value(0, 1.5);
    assert_eq!(instance.editor.save().unwrap(), b"[1.5]");
    instance.editor.load(b"[0.25]").unwrap();
    let mut audio = [[1.0f32, -1.0]; 8];
    processor.process(&mut audio, &[], &[], &ctx);
    assert_eq!(audio[0], [0.25, -0.25]);
}

#[test]
fn an_abi_1_entry_is_accepted_and_runs_through_its_own_table() {
    let entry = ondera_abi1_fixture::ondera_plugin_entry() as *const ffi::Entry;
    let tables = unsafe { native::tables_from_entry(entry) }.unwrap();
    assert_eq!(tables.len(), 1);
    let manifest = unsafe { ffi::read_manifest(tables[0]) }.unwrap();
    assert_eq!(manifest.abi, 1);
    assert_eq!(manifest.name, "ABI 1 Halver");
    let instance = native::instance_from(
        tables[0],
        &manifest,
        descriptor(&manifest, "in-process"),
        48000,
    )
    .unwrap();
    exercise(instance);
}

/// Cargo builds the fixture's dynamic library along with its rlib because this test depends
/// on it, so unlike the example bundle's test this one does not skip.
#[test]
fn an_abi_1_dynamic_library_without_the_v2_symbol_loads_from_disk() {
    let exe = std::env::current_exe().unwrap();
    let deps = exe.parent().unwrap();
    let name = format!(
        "{}ondera_abi1_fixture.{}",
        std::env::consts::DLL_PREFIX,
        native::library_extension()
    );
    let library = [deps.join(&name), deps.parent().unwrap().join(&name)]
        .into_iter()
        .find(|p| p.is_file())
        .unwrap_or_else(|| panic!("{name} was not built next to {}", exe.display()));
    let bytes = std::fs::read(&library).unwrap();
    let has = |needle: &[u8]| bytes.windows(needle.len()).any(|w| w == needle);
    assert!(has(b"ondera_plugin_entry"));
    assert!(
        !has(b"ondera_plugin_entry_v2"),
        "the fixture must stay an ABI 1 library"
    );
    assert_eq!(native::abi_of(&library).unwrap(), 1);
    let descriptors = native::scan(&library).unwrap();
    assert_eq!(descriptors.len(), 1);
    assert_eq!(descriptors[0].id, "native:org.ondera.fixtures.abi1");
}
