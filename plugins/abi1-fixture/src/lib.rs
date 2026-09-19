//! A plugin as somebody built it against Ondera 0.7.0, the last release with plugin ABI 1.
//!
//! This crate must never depend on `ondera-plugin`. The `repr(C)` declarations below are a
//! copy of `sdk/src/ffi.rs` at tag `v0.7.0`, and the functions are written by hand, so the
//! library keeps the old layout whatever happens to the SDK afterwards. It exports only
//! `ondera_plugin_entry`, with `abi_version` 1. `engine/tests/native_plugin.rs` loads it; if
//! that test fails, a change broke every native plugin already installed on people's machines.
//!
//! Do not "update" this file when the SDK changes. That is the point of it.
#![allow(clippy::missing_safety_doc)]
use std::ffi::c_void;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RawContext {
    pub playing: bool,
    pub recording: bool,
    pub cycling: bool,
    pub tempo: f64,
    pub position_beats: f64,
    pub position_seconds: f64,
    pub sample_time: i64,
    pub numerator: u32,
    pub denominator: u32,
    pub cycle_start: f64,
    pub cycle_end: f64,
    pub bar_start_beats: f64,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NoteEvent {
    pub frame: u32,
    pub on: bool,
    pub pitch: u8,
    pub velocity: u8,
    pub channel: u8,
}
#[repr(C)]
pub struct PluginVTable {
    pub manifest: unsafe extern "C" fn(out: *mut *mut u8, len: *mut usize) -> i32,
    pub create: unsafe extern "C" fn(sample_rate: f64) -> *mut c_void,
    pub destroy: unsafe extern "C" fn(instance: *mut c_void),
    pub set_param: unsafe extern "C" fn(instance: *mut c_void, index: u32, value: f64),
    pub process: unsafe extern "C" fn(
        instance: *mut c_void,
        audio: *mut [f32; 2],
        frames: u32,
        notes: *const NoteEvent,
        note_count: u32,
        ctx: *const RawContext,
    ),
    pub reset: unsafe extern "C" fn(instance: *mut c_void),
    pub latency: unsafe extern "C" fn(instance: *mut c_void) -> u32,
    pub free_bytes: unsafe extern "C" fn(ptr: *mut u8, len: usize),
}
unsafe impl Sync for PluginVTable {}
#[repr(C)]
pub struct Entry {
    pub abi_version: u32,
    pub plugin_count: u32,
    pub plugin: unsafe extern "C" fn(index: u32) -> *const PluginVTable,
}
unsafe impl Sync for Entry {}

/// Halves the level by default, and beeps on each note so the note path is covered too.
struct Halver {
    gain: f32,
    tempo_seen: f64,
    beep: u32,
}

const MANIFEST: &str = r#"{"abi":1,"id":"org.ondera.fixtures.abi1","name":"ABI 1 Halver","vendor":"Ondera","version":"0.7.0","category":"Utility","description":"Frozen ABI 1 fixture.","kind":"effect","params":[{"name":"Gain","min":0.0,"max":2.0,"default":0.5,"unit":"","steps":0,"log":false,"labels":[]}]}"#;

unsafe extern "C" fn manifest(out: *mut *mut u8, len: *mut usize) -> i32 {
    let boxed = MANIFEST.as_bytes().to_vec().into_boxed_slice();
    *len = boxed.len();
    *out = Box::into_raw(boxed) as *mut u8;
    0
}
unsafe extern "C" fn create(_rate: f64) -> *mut c_void {
    Box::into_raw(Box::new(Halver {
        gain: 0.5,
        tempo_seen: 0.0,
        beep: 0,
    })) as *mut c_void
}
unsafe extern "C" fn destroy(instance: *mut c_void) {
    drop(Box::from_raw(instance as *mut Halver));
}
unsafe extern "C" fn set_param(instance: *mut c_void, index: u32, value: f64) {
    if index == 0 {
        (*(instance as *mut Halver)).gain = value as f32;
    }
}
unsafe extern "C" fn process(
    instance: *mut c_void,
    audio: *mut [f32; 2],
    frames: u32,
    notes: *const NoteEvent,
    note_count: u32,
    ctx: *const RawContext,
) {
    let plugin = &mut *(instance as *mut Halver);
    plugin.tempo_seen = (*ctx).tempo;
    let audio = std::slice::from_raw_parts_mut(audio, frames as usize);
    for frame in audio.iter_mut() {
        frame[0] *= plugin.gain;
        frame[1] *= plugin.gain;
    }
    for index in 0..note_count as usize {
        let note = *notes.add(index);
        if note.on {
            plugin.beep += 1;
            // Left carries the pitch, right the tempo the context reported: both only come
            // out right if NoteEvent and RawContext still have the ABI 1 layout.
            audio[note.frame as usize] = [note.pitch as f32, plugin.tempo_seen as f32];
        }
    }
}
unsafe extern "C" fn reset(instance: *mut c_void) {
    (*(instance as *mut Halver)).beep = 0;
}
unsafe extern "C" fn latency(_instance: *mut c_void) -> u32 {
    7
}
unsafe extern "C" fn free_bytes(ptr: *mut u8, len: usize) {
    drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len)));
}

static TABLE: PluginVTable = PluginVTable {
    manifest,
    create,
    destroy,
    set_param,
    process,
    reset,
    latency,
    free_bytes,
};
unsafe extern "C" fn plugin(index: u32) -> *const PluginVTable {
    if index == 0 {
        &TABLE
    } else {
        std::ptr::null()
    }
}
static ENTRY: Entry = Entry {
    abi_version: 1,
    plugin_count: 1,
    plugin,
};
#[no_mangle]
pub extern "C" fn ondera_plugin_entry() -> *const Entry {
    &ENTRY
}
