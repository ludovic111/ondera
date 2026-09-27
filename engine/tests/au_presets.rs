//! Audio Unit factory presets on Apple's own units, which every Mac has.
#![cfg(target_os = "macos")]
use ondera_engine::host;

/// AUDistortion (aufx dist appl) and AUMatrixReverb (aufx mrev appl).
const UNITS: [&str; 2] = [
    "au:61756678:64697374:6170706c",
    "au:61756678:6d726576:6170706c",
];

#[test]
fn a_loaded_factory_preset_is_reported_and_survives_the_state_round_trip() {
    for id in UNITS {
        let mut instance = host::instantiate(id, id, 48000).expect(id);
        let programs = instance.editor.programs();
        assert!(programs.len() > 2, "{id}: {programs:?}");
        // The host releases the list it reads (it used to leak one array a read); a unit
        // that kept it for itself would crash here after a few reads.
        for _ in 0..200 {
            assert_eq!(instance.editor.programs(), programs);
        }
        instance.editor.load_program(2).unwrap();
        assert_eq!(instance.editor.current_program(), Some(2), "{id}");
        let state = instance.editor.save().expect("state");
        let mut fresh = host::instantiate(id, id, 48000).unwrap();
        fresh.editor.load(&state).unwrap();
        assert_eq!(
            fresh.editor.current_program(),
            Some(2),
            "{id} after restore"
        );
        fresh.editor.load_program(1).unwrap();
        assert_eq!(fresh.editor.current_program(), Some(1));
        assert!(fresh.editor.load_program(programs.len()).is_err());
    }
}
