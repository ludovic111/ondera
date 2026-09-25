//! Audio Unit host (macOS). Units are created and torn down on the main
//! thread; `AuProcessor` renders on the audio thread.
#![allow(non_upper_case_globals)]

use crate::{plugin::*, Result};
use objc2::msg_send;
use objc2::runtime::{AnyClass, AnyObject};
use objc2_audio_toolbox::*;
use objc2_core_audio_types::*;
use objc2_core_foundation::*;
use std::{
    cell::UnsafeCell,
    ffi::{c_char, c_void, CStr},
    ptr::NonNull,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        Arc,
    },
};

type OSStatus = i32;
type Boolean = u8;

const TYPES: [u32; 4] = [
    kAudioUnitType_Effect,
    kAudioUnitType_MusicEffect,
    kAudioUnitType_MusicDevice,
    kAudioUnitType_Generator,
];
const FLAG_HAS_CFNAME: u32 = 1 << 27;
const FLAG_CFNAME_RELEASE: u32 = 1 << 4;
const FLAG_WRITABLE: u32 = 1 << 31;
const FLAG_LOGARITHMIC: u32 = 1 << 22;
const FLAG_EXPERT: u32 = 1 << 26;

fn fourcc(v: u32) -> String {
    let b = v.to_be_bytes();
    if b.iter().all(|c| c.is_ascii_graphic() || *c == b' ') {
        String::from_utf8_lossy(&b).trim().to_string()
    } else {
        format!("{v:08x}")
    }
}
fn description(kind: u32, sub: u32, manu: u32) -> AudioComponentDescription {
    AudioComponentDescription {
        componentType: kind,
        componentSubType: sub,
        componentManufacturer: manu,
        componentFlags: 0,
        componentFlagsMask: 0,
    }
}
unsafe fn cf_string(ptr: *const CFString) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        (*ptr).to_string()
    }
}
/// Every effect, music effect, instrument and generator the system knows.
pub fn scan() -> Vec<Descriptor> {
    let mut out = vec![];
    for kind in TYPES {
        let mut desc = description(kind, 0, 0);
        let mut component: AudioComponent = std::ptr::null_mut();
        loop {
            component = unsafe { AudioComponentFindNext(component, NonNull::from(&mut desc)) };
            if component.is_null() {
                break;
            }
            let mut found = description(0, 0, 0);
            let mut name: *const CFString = std::ptr::null();
            unsafe {
                if AudioComponentGetDescription(component, NonNull::from(&mut found)) != 0 {
                    continue;
                }
                if AudioComponentCopyName(component, NonNull::from(&mut name)) != 0 {
                    continue;
                }
            }
            let full = unsafe { cf_string(name) };
            if let Some(p) = NonNull::new(name as *mut CFString) {
                drop(unsafe { CFRetained::from_raw(p) });
            }
            let (vendor, title) = match full.split_once(": ") {
                Some((v, t)) => (v.to_string(), t.to_string()),
                None => (String::new(), full.clone()),
            };
            let instrument = matches!(
                found.componentType,
                kAudioUnitType_MusicDevice | kAudioUnitType_Generator
            );
            out.push(Descriptor {
                id: format!(
                    "au:{:08x}:{:08x}:{:08x}",
                    found.componentType, found.componentSubType, found.componentManufacturer
                ),
                format: Format::AudioUnit,
                name: title,
                vendor,
                path: format!(
                    "{}/{}/{}",
                    fourcc(found.componentType),
                    fourcc(found.componentSubType),
                    fourcc(found.componentManufacturer)
                ),
                instrument,
                effect: !instrument,
                category: match found.componentType {
                    kAudioUnitType_MusicEffect => "Music effect".into(),
                    kAudioUnitType_Generator => "Generator".into(),
                    _ => String::new(),
                },
            });
        }
    }
    out
}

/// Live transport numbers for the AU host callbacks.
#[derive(Default)]
struct HostState {
    beat: AtomicU64,
    tempo: AtomicU64,
    playing: AtomicBool,
    cycling: AtomicBool,
    cycle_start: AtomicU64,
    cycle_end: AtomicU64,
    sample: AtomicU64,
    numerator: AtomicU32,
    denominator: AtomicU32,
    bar_start: AtomicU64,
}
unsafe extern "C-unwind" fn beat_and_tempo(
    user: *mut c_void,
    beat: *mut f64,
    tempo: *mut f64,
) -> OSStatus {
    let s = &*(user as *const HostState);
    if !beat.is_null() {
        *beat = f64::from_bits(s.beat.load(Ordering::Relaxed));
    }
    if !tempo.is_null() {
        *tempo = f64::from_bits(s.tempo.load(Ordering::Relaxed));
    }
    0
}
unsafe extern "C-unwind" fn musical_time(
    user: *mut c_void,
    delta: *mut u32,
    numerator: *mut f32,
    denominator: *mut u32,
    downbeat: *mut f64,
) -> OSStatus {
    let s = &*(user as *const HostState);
    if !delta.is_null() {
        *delta = 0;
    }
    if !numerator.is_null() {
        *numerator = s.numerator.load(Ordering::Relaxed) as f32;
    }
    if !denominator.is_null() {
        *denominator = s.denominator.load(Ordering::Relaxed);
    }
    if !downbeat.is_null() {
        *downbeat = f64::from_bits(s.bar_start.load(Ordering::Relaxed));
    }
    0
}
unsafe extern "C-unwind" fn transport_state(
    user: *mut c_void,
    playing: *mut Boolean,
    changed: *mut Boolean,
    sample: *mut f64,
    cycling: *mut Boolean,
    cycle_start: *mut f64,
    cycle_end: *mut f64,
) -> OSStatus {
    let s = &*(user as *const HostState);
    if !playing.is_null() {
        *playing = s.playing.load(Ordering::Relaxed) as Boolean;
    }
    if !changed.is_null() {
        *changed = 0;
    }
    if !sample.is_null() {
        *sample = f64::from_bits(s.sample.load(Ordering::Relaxed));
    }
    if !cycling.is_null() {
        *cycling = s.cycling.load(Ordering::Relaxed) as Boolean;
    }
    if !cycle_start.is_null() {
        *cycle_start = f64::from_bits(s.cycle_start.load(Ordering::Relaxed));
    }
    if !cycle_end.is_null() {
        *cycle_end = f64::from_bits(s.cycle_end.load(Ordering::Relaxed));
    }
    0
}
/// The block the effect's input callback hands to the unit.
struct InputState {
    pointers: [*mut f32; 2],
    frames: usize,
}
unsafe extern "C-unwind" fn input_callback(
    refcon: NonNull<c_void>,
    _flags: NonNull<AudioUnitRenderActionFlags>,
    _time: NonNull<AudioTimeStamp>,
    _bus: u32,
    frames: u32,
    data: *mut AudioBufferList,
) -> OSStatus {
    let state = &*(refcon.as_ptr() as *const InputState);
    if data.is_null() {
        return -50;
    }
    let list = &mut *data;
    let count = list.mNumberBuffers as usize;
    if count != 2 || frames as usize > state.frames || state.pointers.iter().any(|p| p.is_null()) {
        return -50;
    }
    let buffers = std::slice::from_raw_parts_mut(list.mBuffers.as_mut_ptr(), count);
    let n = (frames as usize).min(state.frames);
    for (i, buffer) in buffers.iter_mut().enumerate() {
        let src = state.pointers[i.min(1)];
        let bytes = (n * 4) as u32;
        if buffer.mData.is_null() {
            buffer.mData = src as *mut c_void;
            buffer.mDataByteSize = bytes;
        } else {
            let n = n.min(buffer.mDataByteSize as usize / 4);
            std::ptr::copy_nonoverlapping(src, buffer.mData as *mut f32, n);
        }
    }
    0
}
#[repr(C)]
struct BufferList2 {
    count: u32,
    buffers: [AudioBuffer; 2],
}

struct Shared {
    rate: u32,
    unit: AudioUnit,
    kind: u32,
    host: Box<HostState>,
    input: Box<UnsafeCell<InputState>>,
    initialized: AtomicBool,
}
// SAFETY: AudioUnit instances are designed for one render thread plus the
// main thread setting properties; teardown happens on the main thread.
unsafe impl Send for Shared {}
unsafe impl Sync for Shared {}
impl Drop for Shared {
    fn drop(&mut self) {
        unsafe {
            if self.initialized.swap(false, Ordering::AcqRel) {
                AudioUnitUninitialize(self.unit);
            }
            AudioComponentInstanceDispose(self.unit);
        }
    }
}
fn check(status: OSStatus, what: &str) -> Result<()> {
    if status == 0 {
        Ok(())
    } else {
        Err(format!("{what} failed (OSStatus {status})"))
    }
}
unsafe fn set_property<T>(
    unit: AudioUnit,
    id: u32,
    scope: u32,
    element: u32,
    value: &T,
) -> OSStatus {
    AudioUnitSetProperty(
        unit,
        id,
        scope,
        element,
        value as *const T as *const c_void,
        std::mem::size_of::<T>() as u32,
    )
}
unsafe fn get_property<T>(
    unit: AudioUnit,
    id: u32,
    scope: u32,
    element: u32,
    value: &mut T,
) -> OSStatus {
    let mut size = std::mem::size_of::<T>() as u32;
    AudioUnitGetProperty(
        unit,
        id,
        scope,
        element,
        NonNull::new_unchecked(value as *mut T as *mut c_void),
        NonNull::from(&mut size),
    )
}
fn stream_format(rate: u32) -> AudioStreamBasicDescription {
    AudioStreamBasicDescription {
        mSampleRate: rate as f64,
        mFormatID: kAudioFormatLinearPCM,
        mFormatFlags: kAudioFormatFlagIsFloat
            | kAudioFormatFlagIsPacked
            | kAudioFormatFlagIsNonInterleaved,
        mBytesPerPacket: 4,
        mFramesPerPacket: 1,
        mBytesPerFrame: 4,
        mChannelsPerFrame: 2,
        mBitsPerChannel: 32,
        mReserved: 0,
    }
}

pub fn instantiate(plugin_id: &str, rate: u32) -> Result<Instance> {
    let (_, rest) = Format::parse(plugin_id).ok_or("Bad AU id")?;
    let parts: Vec<u32> = rest
        .split(':')
        .filter_map(|p| u32::from_str_radix(p, 16).ok())
        .collect();
    if parts.len() != 3 {
        return Err("Bad AU id".into());
    }
    let mut desc = description(parts[0], parts[1], parts[2]);
    let name = super::scan::lookup(plugin_id).map_or_else(|| rest.to_string(), |d| d.name);
    unsafe {
        let component = AudioComponentFindNext(std::ptr::null_mut(), NonNull::from(&mut desc));
        if component.is_null() {
            return Err(format!("{name} is not installed"));
        }
        let mut unit: AudioUnit = std::ptr::null_mut();
        check(
            AudioComponentInstanceNew(component, NonNull::from(&mut unit)),
            "Creating the Audio Unit",
        )?;
        let shared = Arc::new(Shared {
            rate,
            unit,
            kind: parts[0],
            host: Box::new(HostState::default()),
            input: Box::new(UnsafeCell::new(InputState {
                pointers: [std::ptr::null_mut(); 2],
                frames: 0,
            })),
            initialized: AtomicBool::new(false),
        });
        let format = stream_format(rate);
        let has_input = matches!(parts[0], kAudioUnitType_Effect | kAudioUnitType_MusicEffect);
        check(
            set_property(
                unit,
                kAudioUnitProperty_StreamFormat,
                kAudioUnitScope_Output,
                0,
                &format,
            ),
            "Setting the output format",
        )?;
        if has_input {
            check(
                set_property(
                    unit,
                    kAudioUnitProperty_StreamFormat,
                    kAudioUnitScope_Input,
                    0,
                    &format,
                ),
                "Setting the input format",
            )?;
            let callback = AURenderCallbackStruct {
                inputProc: Some(input_callback),
                inputProcRefCon: shared.input.get() as *mut c_void,
            };
            check(
                set_property(
                    unit,
                    kAudioUnitProperty_SetRenderCallback,
                    kAudioUnitScope_Input,
                    0,
                    &callback,
                ),
                "Connecting the input",
            )?;
        }
        let max = MAX_BLOCK as u32;
        set_property(
            unit,
            kAudioUnitProperty_MaximumFramesPerSlice,
            kAudioUnitScope_Global,
            0,
            &max,
        );
        let callbacks = HostCallbackInfo {
            hostUserData: &*shared.host as *const HostState as *mut c_void,
            beatAndTempoProc: Some(beat_and_tempo),
            musicalTimeLocationProc: Some(musical_time),
            transportStateProc: Some(transport_state),
            transportStateProc2: None,
        };
        set_property(
            unit,
            kAudioUnitProperty_HostCallbacks,
            kAudioUnitScope_Global,
            0,
            &callbacks,
        );
        check(AudioUnitInitialize(unit), "Initialising the Audio Unit")?;
        shared.initialized.store(true, Ordering::Release);
        let desc = super::scan::lookup(plugin_id).unwrap_or(Descriptor {
            id: plugin_id.into(),
            format: Format::AudioUnit,
            name: name.clone(),
            vendor: String::new(),
            path: String::new(),
            instrument: !has_input,
            effect: has_input,
            category: String::new(),
        });
        let editor = AuEditor::new(shared.clone(), desc);
        let processor = AuProcessor::new(shared, rate);
        Ok(Instance {
            editor: Box::new(editor),
            processor: Some(Box::new(processor)),
        })
    }
}

pub struct AuEditor {
    shared: Arc<Shared>,
    desc: Descriptor,
    params: Vec<ParamInfo>,
    view: *mut AnyObject,
    factory: *mut AnyObject,
}
impl AuEditor {
    fn new(shared: Arc<Shared>, desc: Descriptor) -> Self {
        let mut editor = Self {
            shared,
            desc,
            params: vec![],
            view: std::ptr::null_mut(),
            factory: std::ptr::null_mut(),
        };
        editor.read_params();
        editor
    }
    fn read_params(&mut self) {
        self.params.clear();
        let unit = self.shared.unit;
        unsafe {
            let mut size = 0u32;
            let mut writable: Boolean = 0;
            if AudioUnitGetPropertyInfo(
                unit,
                kAudioUnitProperty_ParameterList,
                kAudioUnitScope_Global,
                0,
                &mut size,
                &mut writable,
            ) != 0
            {
                return;
            }
            let count = (size as usize / 4).min(4096);
            let mut ids = vec![0u32; count];
            if count > 0 {
                let mut size = (count * 4) as u32;
                if AudioUnitGetProperty(
                    unit,
                    kAudioUnitProperty_ParameterList,
                    kAudioUnitScope_Global,
                    0,
                    NonNull::new_unchecked(ids.as_mut_ptr() as *mut c_void),
                    NonNull::from(&mut size),
                ) != 0
                {
                    return;
                }
            }
            for id in ids {
                let mut info: AudioUnitParameterInfo = std::mem::zeroed();
                if get_property(
                    unit,
                    kAudioUnitProperty_ParameterInfo,
                    kAudioUnitScope_Global,
                    id,
                    &mut info,
                ) != 0
                {
                    continue;
                }
                let flags = info.flags.0;
                if flags & FLAG_WRITABLE == 0 || flags & FLAG_EXPERT != 0 {
                    continue;
                }
                let name = if flags & FLAG_HAS_CFNAME != 0 && !info.cfNameString.is_null() {
                    let s = cf_string(info.cfNameString);
                    if flags & FLAG_CFNAME_RELEASE != 0 {
                        if let Some(p) = NonNull::new(info.cfNameString as *mut CFString) {
                            drop(CFRetained::from_raw(p));
                        }
                    }
                    s
                } else {
                    CStr::from_ptr(info.name.as_ptr() as *const c_char)
                        .to_string_lossy()
                        .into_owned()
                };
                let unit_code = info.unit.0;
                let (unit_name, steps) = match unit_code {
                    1 => (
                        "",
                        ((info.maxValue - info.minValue).round().max(0.0) as u32).min(10_000),
                    ),
                    2 => ("", 1),
                    3 => ("%", 0),
                    4 => ("s", 0),
                    8 => ("Hz", 0),
                    9 | 20 => ("ct", 0),
                    10 => ("st", 0),
                    13 => ("dB", 0),
                    15 => ("°", 0),
                    22 => ("BPM", 0),
                    24 => ("ms", 0),
                    _ => ("", 0),
                };
                let labels = if unit_code == 2 {
                    vec!["Off".into(), "On".into()]
                } else {
                    vec![]
                };
                self.params.push(ParamInfo {
                    id,
                    name,
                    min: info.minValue as f64,
                    max: info.maxValue.max(info.minValue + f32::EPSILON) as f64,
                    default: info.defaultValue as f64,
                    unit: unit_name.into(),
                    steps,
                    log: flags & FLAG_LOGARITHMIC != 0 && info.minValue > 0.0,
                    labels,
                });
            }
        }
    }
}
impl Editor for AuEditor {
    fn descriptor(&self) -> &Descriptor {
        &self.desc
    }
    fn params(&self) -> &[ParamInfo] {
        &self.params
    }
    fn value(&self, id: u32) -> Option<f64> {
        let mut value: f32 = 0.0;
        unsafe {
            (AudioUnitGetParameter(
                self.shared.unit,
                id,
                kAudioUnitScope_Global,
                0,
                NonNull::from(&mut value),
            ) == 0)
                .then_some(value as f64)
        }
    }
    fn set_value(&mut self, id: u32, value: f64) {
        unsafe {
            AudioUnitSetParameter(
                self.shared.unit,
                id,
                kAudioUnitScope_Global,
                0,
                value as f32,
                0,
            );
        }
    }
    fn save(&mut self) -> Option<Vec<u8>> {
        unsafe {
            let mut plist: *const CFPropertyList = std::ptr::null();
            if get_property(
                self.shared.unit,
                kAudioUnitProperty_ClassInfo,
                kAudioUnitScope_Global,
                0,
                &mut plist,
            ) != 0
                || plist.is_null()
            {
                return None;
            }
            let owned = CFRetained::from_raw(NonNull::new_unchecked(plist as *mut CFPropertyList));
            let data = CFPropertyListCreateData(
                None,
                Some(&owned),
                CFPropertyListFormat::BinaryFormat_v1_0,
                0,
                std::ptr::null_mut(),
            )?;
            Some(data.to_vec())
        }
    }
    fn load(&mut self, bytes: &[u8]) -> Result<()> {
        unsafe {
            let data = CFData::from_bytes(bytes);
            let plist = CFPropertyListCreateWithData(
                None,
                Some(&data),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
            .ok_or("Unreadable Audio Unit preset")?;
            let raw: *const CFPropertyList = CFRetained::as_ptr(&plist).as_ptr();
            check(
                set_property(
                    self.shared.unit,
                    kAudioUnitProperty_ClassInfo,
                    kAudioUnitScope_Global,
                    0,
                    &raw,
                ),
                "Restoring the Audio Unit preset",
            )
        }
    }
    fn has_gui(&self) -> bool {
        true
    }
    fn open_gui(&mut self, parent: ParentWindow) -> Result<(u32, u32)> {
        let ParentWindow::Cocoa(parent) = parent else {
            return Err("Audio Unit editors need a Cocoa window".into());
        };
        let unit = self.shared.unit;
        unsafe {
            let mut view: *mut AnyObject = std::ptr::null_mut();
            let mut size = 0u32;
            let mut writable: Boolean = 0;
            if AudioUnitGetPropertyInfo(
                unit,
                kAudioUnitProperty_CocoaUI,
                kAudioUnitScope_Global,
                0,
                &mut size,
                &mut writable,
            ) == 0
                && size as usize >= std::mem::size_of::<AudioUnitCocoaViewInfo>()
            {
                let mut raw = vec![0u8; size as usize];
                let mut got = size;
                if AudioUnitGetProperty(
                    unit,
                    kAudioUnitProperty_CocoaUI,
                    kAudioUnitScope_Global,
                    0,
                    NonNull::new_unchecked(raw.as_mut_ptr() as *mut c_void),
                    NonNull::from(&mut got),
                ) == 0
                {
                    let info = &*(raw.as_ptr() as *const AudioUnitCocoaViewInfo);
                    let url = info.mCocoaAUViewBundleLocation.as_ptr() as *mut AnyObject;
                    let class_name = info.mCocoaAUViewClass[0].as_ptr() as *mut AnyObject;
                    if let Some(bundle_class) = AnyClass::get(c"NSBundle") {
                        let bundle: *mut AnyObject = msg_send![bundle_class, bundleWithURL: url];
                        if !bundle.is_null() {
                            let _: bool = msg_send![bundle, load];
                            let factory_class: *const AnyClass =
                                msg_send![bundle, classNamed: class_name];
                            if !factory_class.is_null() {
                                let factory: *mut AnyObject = msg_send![&*factory_class, alloc];
                                let factory: *mut AnyObject = msg_send![factory, init];
                                if !factory.is_null() {
                                    let wanted = CGSize {
                                        width: 800.0,
                                        height: 500.0,
                                    };
                                    view = msg_send![factory, uiViewForAudioUnit: unit, withSize: wanted];
                                    self.factory = factory;
                                }
                            }
                        }
                    }
                    drop(CFRetained::from_raw(info.mCocoaAUViewBundleLocation));
                    drop(CFRetained::from_raw(info.mCocoaAUViewClass[0]));
                }
            }
            if view.is_null() {
                // Generic editor from CoreAudioKit.
                let bundle_class = AnyClass::get(c"NSBundle").ok_or("AppKit unavailable")?;
                let path = objc2_foundation::NSString::from_str(
                    "/System/Library/Frameworks/CoreAudioKit.framework",
                );
                let bundle: *mut AnyObject = msg_send![bundle_class, bundleWithPath: &*path];
                if !bundle.is_null() {
                    let _: bool = msg_send![bundle, load];
                }
                let generic = AnyClass::get(c"AUGenericView").ok_or("CoreAudioKit unavailable")?;
                let v: *mut AnyObject = msg_send![generic, alloc];
                view = msg_send![v, initWithAudioUnit: unit];
            }
            if view.is_null() {
                return Err("The Audio Unit did not provide an editor".into());
            }
            let _: () = msg_send![view, retain];
            let frame: CGRect = msg_send![view, frame];
            let (w, h) = (
                frame.size.width.max(200.0) as u32,
                frame.size.height.max(100.0) as u32,
            );
            let origin = CGRect {
                origin: CGPoint { x: 0.0, y: 0.0 },
                size: CGSize {
                    width: w as f64,
                    height: h as f64,
                },
            };
            let _: () = msg_send![view, setFrame: origin];
            let parent = parent as *mut AnyObject;
            let _: () = msg_send![parent, addSubview: view];
            self.view = view;
            Ok((w, h))
        }
    }
    fn close_gui(&mut self) {
        unsafe {
            if !self.view.is_null() {
                let _: () = msg_send![self.view, removeFromSuperview];
                let _: () = msg_send![self.view, release];
                self.view = std::ptr::null_mut();
            }
            if !self.factory.is_null() {
                let _: () = msg_send![self.factory, release];
                self.factory = std::ptr::null_mut();
            }
        }
    }
    fn set_gui_size(&mut self, width: u32, height: u32) {
        if self.view.is_null() {
            return;
        }
        let frame = CGRect {
            origin: CGPoint { x: 0.0, y: 0.0 },
            size: CGSize {
                width: width as f64,
                height: height as f64,
            },
        };
        unsafe {
            let _: () = msg_send![self.view, setFrame: frame];
        }
    }
    fn latency(&self) -> u32 {
        let mut seconds: f64 = 0.0;
        unsafe {
            if get_property(
                self.shared.unit,
                kAudioUnitProperty_Latency,
                kAudioUnitScope_Global,
                0,
                &mut seconds,
            ) == 0
            {
                return (seconds * self.shared.rate as f64).max(0.0) as u32;
            }
        }
        0
    }
    fn programs(&mut self) -> Vec<String> {
        unsafe { factory_presets(self.shared.unit) }
            .into_iter()
            .map(|p| p.1)
            .collect()
    }
    fn load_program(&mut self, index: usize) -> Result<()> {
        let presets = unsafe { factory_presets(self.shared.unit) };
        let (number, _, name) = presets.get(index).cloned().ok_or_else(|| {
            format!(
                "Program {index} does not exist ({} programs)",
                presets.len()
            )
        })?;
        let preset = AUPreset {
            presetNumber: number,
            presetName: name as *const CFString,
        };
        unsafe {
            check(
                set_property(
                    self.shared.unit,
                    kAudioUnitProperty_PresentPreset,
                    kAudioUnitScope_Global,
                    0,
                    &preset,
                ),
                "Loading the Audio Unit factory preset",
            )
        }
    }
}

/// The factory presets as (number, name, name pointer). The array stays with the Audio
/// Unit: some units return one they keep, so releasing it here could free it under them.
unsafe fn factory_presets(unit: AudioUnit) -> Vec<(i32, String, usize)> {
    extern "C" {
        fn CFArrayGetCount(array: *const c_void) -> isize;
        fn CFArrayGetValueAtIndex(array: *const c_void, index: isize) -> *const c_void;
    }
    let mut array: *const c_void = std::ptr::null();
    if get_property(
        unit,
        kAudioUnitProperty_FactoryPresets,
        kAudioUnitScope_Global,
        0,
        &mut array,
    ) != 0
        || array.is_null()
    {
        return Vec::new();
    }
    let count = CFArrayGetCount(array).clamp(0, 4096);
    (0..count)
        .filter_map(|i| {
            let preset = CFArrayGetValueAtIndex(array, i) as *const AUPreset;
            let preset = preset.as_ref()?;
            let name = if preset.presetName.is_null() {
                format!("Program {}", preset.presetNumber)
            } else {
                cf_string(preset.presetName)
            };
            Some((preset.presetNumber, name, preset.presetName as usize))
        })
        .collect()
}
impl Drop for AuEditor {
    fn drop(&mut self) {
        self.close_gui();
    }
}

pub struct AuProcessor {
    shared: Arc<Shared>,
    rate: u32,
    input: [Vec<f32>; 2],
    output: [Vec<f32>; 2],
    sample_time: f64,
}
unsafe impl Send for AuProcessor {}
impl AuProcessor {
    fn new(shared: Arc<Shared>, rate: u32) -> Self {
        Self {
            shared,
            rate,
            input: [vec![0.0; MAX_BLOCK], vec![0.0; MAX_BLOCK]],
            output: [vec![0.0; MAX_BLOCK], vec![0.0; MAX_BLOCK]],
            sample_time: 0.0,
        }
    }
}
impl Processor for AuProcessor {
    fn reset(&mut self) {
        unsafe {
            AudioUnitReset(self.shared.unit, kAudioUnitScope_Global, 0);
        }
    }
    fn process(
        &mut self,
        audio: &mut [[f32; 2]],
        events: &[Event],
        params: &[ParamChange],
        ctx: &ProcessContext,
    ) {
        let n = audio.len().min(MAX_BLOCK);
        if n == 0 || !self.shared.initialized.load(Ordering::Acquire) {
            return;
        }
        let unit = self.shared.unit;
        let host = &self.shared.host;
        host.beat
            .store(ctx.position_beats.to_bits(), Ordering::Relaxed);
        host.tempo.store(ctx.tempo.to_bits(), Ordering::Relaxed);
        host.playing.store(ctx.playing, Ordering::Relaxed);
        host.cycling.store(ctx.cycle.is_some(), Ordering::Relaxed);
        let (cs, ce) = ctx.cycle.unwrap_or((0.0, 0.0));
        host.cycle_start.store(cs.to_bits(), Ordering::Relaxed);
        host.cycle_end.store(ce.to_bits(), Ordering::Relaxed);
        host.sample.store(
            (ctx.position_seconds * self.rate as f64).to_bits(),
            Ordering::Relaxed,
        );
        host.numerator.store(ctx.numerator, Ordering::Relaxed);
        host.denominator.store(ctx.denominator, Ordering::Relaxed);
        host.bar_start
            .store(ctx.bar_start_beats.to_bits(), Ordering::Relaxed);
        for (k, frame) in audio[..n].iter().enumerate() {
            self.input[0][k] = frame[0];
            self.input[1][k] = frame[1];
        }
        unsafe {
            for change in params {
                AudioUnitSetParameter(
                    unit,
                    change.id,
                    kAudioUnitScope_Global,
                    0,
                    change.value as f32,
                    0,
                );
            }
            let accepts_notes = matches!(
                self.shared.kind,
                kAudioUnitType_MusicDevice | kAudioUnitType_MusicEffect
            );
            if accepts_notes {
                // Notes, controllers, pitch bend and pressure all travel as MIDI 1.0 bytes.
                for event in events {
                    if let Some([status, first, second]) = event.to_midi() {
                        MusicDeviceMIDIEvent(
                            unit,
                            status as u32,
                            first as u32,
                            second as u32,
                            (event.frame as usize).min(n - 1) as u32,
                        );
                    }
                }
            }
            // The input callback reads the block through this shared state.
            let input = self.shared.input.get();
            (*input).pointers = [self.input[0].as_mut_ptr(), self.input[1].as_mut_ptr()];
            (*input).frames = n;
            let mut list = BufferList2 {
                count: 2,
                buffers: [
                    AudioBuffer {
                        mNumberChannels: 1,
                        mDataByteSize: (n * 4) as u32,
                        mData: self.output[0].as_mut_ptr() as *mut c_void,
                    },
                    AudioBuffer {
                        mNumberChannels: 1,
                        mDataByteSize: (n * 4) as u32,
                        mData: self.output[1].as_mut_ptr() as *mut c_void,
                    },
                ],
            };
            let mut stamp: AudioTimeStamp = std::mem::zeroed();
            stamp.mSampleTime = self.sample_time;
            stamp.mFlags = AudioTimeStampFlags::SampleTimeValid;
            let mut flags = AudioUnitRenderActionFlags(0);
            let status = AudioUnitRender(
                unit,
                &mut flags,
                NonNull::from(&mut stamp),
                0,
                n as u32,
                NonNull::new_unchecked(&mut list as *mut BufferList2 as *mut AudioBufferList),
            );
            self.sample_time += n as f64;
            if status != 0 {
                return;
            }
            if list.count != 2
                || list.buffers.iter().any(|buffer| {
                    buffer.mData.is_null()
                        || buffer.mDataByteSize < (n * 4) as u32
                        || buffer.mNumberChannels != 1
                })
            {
                audio[..n].fill([0.0; 2]);
                return;
            }
            for (k, frame) in audio[..n].iter_mut().enumerate() {
                let l = *(list.buffers[0].mData as *const f32).add(k);
                let r = *(list.buffers[1].mData as *const f32).add(k);
                *frame = [
                    if l.is_finite() { l } else { 0.0 },
                    if r.is_finite() { r } else { 0.0 },
                ];
            }
        }
    }
}
