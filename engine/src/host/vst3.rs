//! VST3 host built on the `vst3` COM bindings. The component, controller and
//! view are driven from the main thread; `Vst3Processor` runs on the audio thread.

use crate::{
    plugin::{
        Descriptor, Editor, Format, Instance, NoteEvent, ParamChange, ParamInfo, ParentWindow,
        Processor, MAX_BLOCK,
    },
    Result,
};
use std::{
    cell::UnsafeCell,
    collections::HashMap,
    ffi::{c_char, c_void},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, AtomicUsize, Ordering},
        Arc, Mutex, OnceLock,
    },
};
use vst3::{Class, ComPtr, ComWrapper, Interface, Steinberg::Vst::*, Steinberg::*};

// The bindings type these enums as `c_uint` on Unix and `c_int` on Windows, so the cast is
// required on one platform and a no-op on the other.
#[allow(clippy::unnecessary_cast)]
const REALTIME: int32 = ProcessModes_::kRealtime as int32;
#[allow(clippy::unnecessary_cast)]
const SAMPLE32: int32 = SymbolicSampleSizes_::kSample32 as int32;

// ---------------------------------------------------------------------------
// Module loading
// ---------------------------------------------------------------------------

struct Module {
    _library: libloading::Library,
    factory: ComPtr<IPluginFactory>,
    #[cfg(target_os = "macos")]
    _bundle: Option<objc2_core_foundation::CFRetained<objc2_core_foundation::CFBundle>>,
}
unsafe impl Send for Module {}
unsafe impl Sync for Module {}

fn binary_path(bundle: &Path) -> Result<PathBuf> {
    if bundle.is_file() {
        return Ok(bundle.to_path_buf());
    }
    #[cfg(target_os = "macos")]
    let dir = bundle.join("Contents/MacOS");
    #[cfg(target_os = "windows")]
    let dir = bundle.join(if cfg!(target_arch = "aarch64") {
        "Contents/arm64-win"
    } else {
        "Contents/x86_64-win"
    });
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let dir = bundle.join(if cfg!(target_arch = "aarch64") {
        "Contents/aarch64-linux"
    } else {
        "Contents/x86_64-linux"
    });
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    entries.sort();
    entries
        .into_iter()
        .next()
        .ok_or_else(|| format!("No plugin binary in {}", dir.display()))
}
fn load(bundle: &Path) -> Result<Arc<Module>> {
    static LOADED: OnceLock<Mutex<HashMap<PathBuf, Arc<Module>>>> = OnceLock::new();
    let cache = LOADED.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(module) = guard.get(bundle) {
        return Ok(module.clone());
    }
    let binary = binary_path(bundle)?;
    // SAFETY: loading a plugin binary runs its initialisers; inherent to hosting.
    let library = unsafe { libloading::Library::new(&binary) }
        .map_err(|e| format!("Cannot load {}: {e}", binary.display()))?;
    #[cfg(target_os = "macos")]
    let cf_bundle;
    unsafe {
        #[cfg(target_os = "macos")]
        {
            use objc2_core_foundation::{CFBundle, CFString, CFURLPathStyle, CFURL};
            let path = CFString::from_str(&bundle.to_string_lossy());
            let url = CFURL::with_file_system_path(
                None,
                Some(&path),
                CFURLPathStyle::CFURLPOSIXPathStyle,
                true,
            )
            .ok_or("Bad bundle path")?;
            let b = CFBundle::new(None, Some(&url)).ok_or("Cannot read the VST3 bundle")?;
            if let Ok(entry) =
                library.get::<unsafe extern "C" fn(*mut c_void) -> bool>(b"bundleEntry\0")
            {
                let raw = objc2_core_foundation::CFRetained::as_ptr(&b).as_ptr() as *mut c_void;
                if !entry(raw) {
                    return Err("VST3 bundleEntry failed".into());
                }
            }
            cf_bundle = Some(b);
        }
        #[cfg(target_os = "windows")]
        {
            if let Ok(entry) = library.get::<unsafe extern "system" fn() -> bool>(b"InitDll\0") {
                if !entry() {
                    return Err("VST3 InitDll failed".into());
                }
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            if let Ok(entry) =
                library.get::<unsafe extern "C" fn(*mut c_void) -> bool>(b"ModuleEntry\0")
            {
                if !entry(std::ptr::null_mut()) {
                    return Err("VST3 ModuleEntry failed".into());
                }
            }
        }
    }
    let factory = unsafe {
        let get = library
            .get::<unsafe extern "system" fn() -> *mut IPluginFactory>(b"GetPluginFactory\0")
            .map_err(|e| format!("Not a VST3 plugin ({e})"))?;
        ComPtr::from_raw(get()).ok_or("VST3 factory missing")?
    };
    let module = Arc::new(Module {
        _library: library,
        factory,
        #[cfg(target_os = "macos")]
        _bundle: cf_bundle,
    });
    guard.insert(bundle.to_path_buf(), module.clone());
    Ok(module)
}
fn cstr(bytes: &[c_char]) -> String {
    let end = bytes.iter().position(|&c| c == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end].iter().map(|&c| c as u8).collect::<Vec<_>>()).into_owned()
}
fn wstr(chars: &[TChar]) -> String {
    let end = chars.iter().position(|&c| c == 0).unwrap_or(chars.len());
    String::from_utf16_lossy(&chars[..end])
}
fn write_wstr(src: &str, dst: &mut [TChar]) {
    let mut len = 0;
    for (s, d) in src.encode_utf16().zip(dst.iter_mut()) {
        *d = s;
        len += 1;
    }
    if len < dst.len() {
        dst[len] = 0;
    } else if let Some(last) = dst.last_mut() {
        *last = 0;
    }
}
fn guid(tuid: &TUID) -> [u8; 16] {
    tuid.map(|c| c as u8)
}
fn cid_hex(cid: &TUID) -> String {
    cid.iter().map(|b| format!("{:02x}", *b as u8)).collect()
}
fn cid_parse(hex: &str) -> Option<TUID> {
    if hex.len() != 32 {
        return None;
    }
    let mut out = [0 as c_char; 16];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()? as c_char;
    }
    Some(out)
}

/// Probe a bundle: every audio module class it exports.
pub fn scan(bundle: &Path) -> Result<Vec<Descriptor>> {
    let module = load(bundle)?;
    let mut out = vec![];
    unsafe {
        let factory = &module.factory;
        let mut info: PFactoryInfo = std::mem::zeroed();
        factory.getFactoryInfo(&mut info);
        let vendor = cstr(&info.vendor);
        let factory2 = factory.cast::<IPluginFactory2>();
        for i in 0..factory.countClasses() {
            let (cid, category, name, sub, class_vendor) = if let Some(f2) = &factory2 {
                let mut info: PClassInfo2 = std::mem::zeroed();
                if f2.getClassInfo2(i, &mut info) != kResultOk {
                    continue;
                }
                (
                    info.cid,
                    cstr(&info.category),
                    cstr(&info.name),
                    cstr(&info.subCategories),
                    cstr(&info.vendor),
                )
            } else {
                let mut info: PClassInfo = std::mem::zeroed();
                if factory.getClassInfo(i, &mut info) != kResultOk {
                    continue;
                }
                (
                    info.cid,
                    cstr(&info.category),
                    cstr(&info.name),
                    String::new(),
                    String::new(),
                )
            };
            if category != "Audio Module Class" {
                continue;
            }
            let instrument = sub.contains("Instrument");
            out.push(Descriptor {
                id: format!("vst3:{}", cid_hex(&cid)),
                format: Format::Vst3,
                name,
                vendor: if class_vendor.is_empty() {
                    vendor.clone()
                } else {
                    class_vendor
                },
                path: bundle.to_string_lossy().into_owned(),
                instrument,
                effect: !instrument || sub.contains("Fx"),
                category: sub
                    .split('|')
                    .find(|s| !["Fx", "Instrument", "Stereo", "Mono"].contains(s))
                    .unwrap_or_default()
                    .to_string(),
            });
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Host-side COM classes
// ---------------------------------------------------------------------------

enum Attr {
    Int(i64),
    Float(f64),
    Str(Vec<u16>),
    Bin(Vec<u8>),
}
struct AttributeList {
    values: Mutex<HashMap<String, Attr>>,
}
impl Class for AttributeList {
    type Interfaces = (IAttributeList,);
}
unsafe fn attr_key(id: IAttrID) -> String {
    if id.is_null() {
        String::new()
    } else {
        std::ffi::CStr::from_ptr(id).to_string_lossy().into_owned()
    }
}
impl IAttributeListTrait for AttributeList {
    unsafe fn setInt(&self, id: IAttrID, value: int64) -> tresult {
        self.values
            .lock()
            .unwrap()
            .insert(attr_key(id), Attr::Int(value));
        kResultOk
    }
    unsafe fn getInt(&self, id: IAttrID, value: *mut int64) -> tresult {
        match self.values.lock().unwrap().get(&attr_key(id)) {
            Some(Attr::Int(v)) => {
                *value = *v;
                kResultOk
            }
            _ => kResultFalse,
        }
    }
    unsafe fn setFloat(&self, id: IAttrID, value: f64) -> tresult {
        self.values
            .lock()
            .unwrap()
            .insert(attr_key(id), Attr::Float(value));
        kResultOk
    }
    unsafe fn getFloat(&self, id: IAttrID, value: *mut f64) -> tresult {
        match self.values.lock().unwrap().get(&attr_key(id)) {
            Some(Attr::Float(v)) => {
                *value = *v;
                kResultOk
            }
            _ => kResultFalse,
        }
    }
    unsafe fn setString(&self, id: IAttrID, string: *const TChar) -> tresult {
        let mut chars = vec![];
        let mut p = string;
        while !p.is_null() && *p != 0 && chars.len() < 65536 {
            chars.push(*p);
            p = p.add(1);
        }
        self.values
            .lock()
            .unwrap()
            .insert(attr_key(id), Attr::Str(chars));
        kResultOk
    }
    unsafe fn getString(&self, id: IAttrID, string: *mut TChar, size_in_bytes: uint32) -> tresult {
        match self.values.lock().unwrap().get(&attr_key(id)) {
            Some(Attr::Str(v)) => {
                let capacity = size_in_bytes as usize / 2;
                if capacity == 0 {
                    return kResultFalse;
                }
                let n = v.len().min(capacity - 1);
                std::ptr::copy_nonoverlapping(v.as_ptr(), string, n);
                *string.add(n) = 0;
                kResultOk
            }
            _ => kResultFalse,
        }
    }
    unsafe fn setBinary(&self, id: IAttrID, data: *const c_void, size_in_bytes: uint32) -> tresult {
        let bytes = if data.is_null() {
            vec![]
        } else {
            std::slice::from_raw_parts(data as *const u8, size_in_bytes as usize).to_vec()
        };
        self.values
            .lock()
            .unwrap()
            .insert(attr_key(id), Attr::Bin(bytes));
        kResultOk
    }
    unsafe fn getBinary(
        &self,
        id: IAttrID,
        data: *mut *const c_void,
        size_in_bytes: *mut uint32,
    ) -> tresult {
        match self.values.lock().unwrap().get(&attr_key(id)) {
            Some(Attr::Bin(v)) => {
                *data = v.as_ptr() as *const c_void;
                *size_in_bytes = v.len() as u32;
                kResultOk
            }
            _ => kResultFalse,
        }
    }
}
struct Message {
    id: Mutex<std::ffi::CString>,
    attributes: ComWrapper<AttributeList>,
}
impl Class for Message {
    type Interfaces = (IMessage,);
}
impl IMessageTrait for Message {
    unsafe fn getMessageID(&self) -> FIDString {
        self.id.lock().unwrap().as_ptr()
    }
    unsafe fn setMessageID(&self, id: FIDString) {
        if !id.is_null() {
            *self.id.lock().unwrap() = std::ffi::CStr::from_ptr(id).to_owned();
        }
    }
    unsafe fn getAttributes(&self) -> *mut IAttributeList {
        self.attributes
            .as_com_ref::<IAttributeList>()
            .map_or(std::ptr::null_mut(), |r| r.as_ptr())
    }
}
struct HostApp;
impl Class for HostApp {
    type Interfaces = (IHostApplication,);
}
impl IHostApplicationTrait for HostApp {
    unsafe fn getName(&self, name: *mut String128) -> tresult {
        write_wstr("Ondera", &mut *name);
        kResultOk
    }
    unsafe fn createInstance(
        &self,
        cid: *mut TUID,
        iid: *mut TUID,
        obj: *mut *mut c_void,
    ) -> tresult {
        let wanted = guid(&*iid);
        let class = guid(&*cid);
        if wanted == IMessage::IID || class == IMessage::IID {
            let message = ComWrapper::new(Message {
                id: Mutex::new(std::ffi::CString::default()),
                attributes: ComWrapper::new(AttributeList {
                    values: Mutex::new(HashMap::new()),
                }),
            });
            if let Some(ptr) = message.to_com_ptr::<IMessage>() {
                *obj = ptr.into_raw() as *mut c_void;
                return kResultOk;
            }
        }
        if wanted == IAttributeList::IID || class == IAttributeList::IID {
            let list = ComWrapper::new(AttributeList {
                values: Mutex::new(HashMap::new()),
            });
            if let Some(ptr) = list.to_com_ptr::<IAttributeList>() {
                *obj = ptr.into_raw() as *mut c_void;
                return kResultOk;
            }
        }
        *obj = std::ptr::null_mut();
        kNotImplemented
    }
}
/// Receives parameter edits from the plugin's own editor.
struct Handler {
    edits: Mutex<Vec<(u32, f64)>>,
    flags: AtomicU32,
}
const RESTART_PARAMS: u32 = 1;
const RESTART_RELOAD: u32 = 2;
impl Class for Handler {
    type Interfaces = (IComponentHandler,);
}
impl IComponentHandlerTrait for Handler {
    unsafe fn beginEdit(&self, _id: ParamID) -> tresult {
        kResultOk
    }
    unsafe fn performEdit(&self, id: ParamID, value: ParamValue) -> tresult {
        if let Ok(mut edits) = self.edits.lock() {
            if edits.len() < 4096 {
                edits.push((id, value));
            }
        }
        kResultOk
    }
    unsafe fn endEdit(&self, _id: ParamID) -> tresult {
        kResultOk
    }
    unsafe fn restartComponent(&self, flags: int32) -> tresult {
        if flags & RestartFlags_::kParamValuesChanged != 0 {
            self.flags.fetch_or(RESTART_PARAMS, Ordering::Relaxed);
        }
        if flags
            & (RestartFlags_::kReloadComponent
                | RestartFlags_::kLatencyChanged
                | RestartFlags_::kIoChanged)
            != 0
        {
            self.flags.fetch_or(RESTART_RELOAD, Ordering::Relaxed);
        }
        kResultOk
    }
}
struct PlugFrame {
    resize: Mutex<Option<(u32, u32)>>,
}
impl Class for PlugFrame {
    type Interfaces = (IPlugFrame,);
}
impl IPlugFrameTrait for PlugFrame {
    unsafe fn resizeView(&self, view: *mut IPlugView, new_size: *mut ViewRect) -> tresult {
        let r = *new_size;
        if let Ok(mut resize) = self.resize.lock() {
            *resize = Some((
                (r.right - r.left).max(1) as u32,
                (r.bottom - r.top).max(1) as u32,
            ));
        }
        if let Some(view) = vst3::ComRef::from_raw(view) {
            view.onSize(new_size);
        }
        kResultOk
    }
}
struct BStream {
    data: Mutex<(Vec<u8>, usize)>,
}
impl Class for BStream {
    type Interfaces = (IBStream,);
}
impl IBStreamTrait for BStream {
    unsafe fn read(&self, buffer: *mut c_void, num_bytes: int32, num_read: *mut int32) -> tresult {
        let mut guard = self.data.lock().unwrap();
        let (data, pos) = &mut *guard;
        let n = (num_bytes.max(0) as usize).min(data.len().saturating_sub(*pos));
        std::ptr::copy_nonoverlapping(data.as_ptr().add(*pos), buffer as *mut u8, n);
        *pos += n;
        if !num_read.is_null() {
            *num_read = n as i32;
        }
        kResultOk
    }
    unsafe fn write(
        &self,
        buffer: *mut c_void,
        num_bytes: int32,
        num_written: *mut int32,
    ) -> tresult {
        let mut guard = self.data.lock().unwrap();
        let (data, pos) = &mut *guard;
        let n = num_bytes.max(0) as usize;
        if *pos + n > 256 * 1024 * 1024 {
            return kResultFalse;
        }
        if data.len() < *pos + n {
            data.resize(*pos + n, 0);
        }
        std::ptr::copy_nonoverlapping(buffer as *const u8, data.as_mut_ptr().add(*pos), n);
        *pos += n;
        if !num_written.is_null() {
            *num_written = n as i32;
        }
        kResultOk
    }
    unsafe fn seek(&self, pos: int64, mode: int32, result: *mut int64) -> tresult {
        let mut guard = self.data.lock().unwrap();
        let (data, current) = &mut *guard;
        let base = match mode as IBStream_::IStreamSeekMode {
            IBStream_::IStreamSeekMode_::kIBSeekCur => *current as i64,
            IBStream_::IStreamSeekMode_::kIBSeekEnd => data.len() as i64,
            _ => 0,
        };
        *current = (base + pos).clamp(0, data.len() as i64) as usize;
        if !result.is_null() {
            *result = *current as i64;
        }
        kResultOk
    }
    unsafe fn tell(&self, pos: *mut int64) -> tresult {
        *pos = self.data.lock().unwrap().1 as i64;
        kResultOk
    }
}
/// Points one queue holds per block: automation sends one every 32 frames and one on each
/// breakpoint, eight or nine in a 256-frame block. Past the limit, the last point takes the
/// latest value.
const QUEUE_POINTS: usize = 16;
/// Parameter changes for one block: one queue per parameter, its points in offset order.
struct ParamQueue {
    id: AtomicU32,
    count: AtomicUsize,
    offsets: [AtomicI32; QUEUE_POINTS],
    values: [AtomicU64; QUEUE_POINTS],
}
impl ParamQueue {
    fn new() -> Self {
        Self {
            id: AtomicU32::new(0),
            count: AtomicUsize::new(0),
            offsets: std::array::from_fn(|_| AtomicI32::new(0)),
            values: std::array::from_fn(|_| AtomicU64::new(0)),
        }
    }
    /// Start the queue over for `id`.
    fn reset(&self, id: ParamID) {
        self.id.store(id, Ordering::Relaxed);
        self.count.store(0, Ordering::Relaxed);
    }
    fn push(&self, offset: i32, value: f64) -> usize {
        let count = self.count.load(Ordering::Relaxed);
        let index = count.min(QUEUE_POINTS - 1);
        self.offsets[index].store(offset, Ordering::Relaxed);
        self.values[index].store(value.to_bits(), Ordering::Relaxed);
        self.count
            .store((count + 1).min(QUEUE_POINTS), Ordering::Relaxed);
        index
    }
}
impl Class for ParamQueue {
    type Interfaces = (IParamValueQueue,);
}
impl IParamValueQueueTrait for ParamQueue {
    unsafe fn getParameterId(&self) -> ParamID {
        self.id.load(Ordering::Relaxed)
    }
    unsafe fn getPointCount(&self) -> int32 {
        self.count.load(Ordering::Relaxed) as int32
    }
    unsafe fn getPoint(
        &self,
        index: int32,
        sample_offset: *mut int32,
        value: *mut ParamValue,
    ) -> tresult {
        if index < 0 || index as usize >= self.count.load(Ordering::Relaxed) {
            return kResultFalse;
        }
        *sample_offset = self.offsets[index as usize].load(Ordering::Relaxed);
        *value = f64::from_bits(self.values[index as usize].load(Ordering::Relaxed));
        kResultOk
    }
    unsafe fn addPoint(
        &self,
        sample_offset: int32,
        value: ParamValue,
        index: *mut int32,
    ) -> tresult {
        let at = self.push(sample_offset, value);
        if !index.is_null() {
            *index = at as int32;
        }
        kResultOk
    }
}
struct ParameterChanges {
    queues: Vec<ComWrapper<ParamQueue>>,
    count: AtomicUsize,
}
impl Class for ParameterChanges {
    type Interfaces = (IParameterChanges,);
}
impl IParameterChangesTrait for ParameterChanges {
    unsafe fn getParameterCount(&self) -> int32 {
        self.count.load(Ordering::Relaxed) as i32
    }
    unsafe fn getParameterData(&self, index: int32) -> *mut IParamValueQueue {
        self.queues
            .get(index as usize)
            .filter(|_| (index as usize) < self.count.load(Ordering::Relaxed))
            .and_then(|q| q.as_com_ref::<IParamValueQueue>())
            .map_or(std::ptr::null_mut(), |r| r.as_ptr())
    }
    unsafe fn addParameterData(
        &self,
        id: *const ParamID,
        index: *mut int32,
    ) -> *mut IParamValueQueue {
        // Output changes from the plugin: reuse a free queue if any.
        let count = self.count.load(Ordering::Relaxed);
        if count >= self.queues.len() || id.is_null() {
            return std::ptr::null_mut();
        }
        self.queues[count].reset(*id);
        self.count.store(count + 1, Ordering::Relaxed);
        if !index.is_null() {
            *index = count as i32;
        }
        self.queues[count]
            .as_com_ref::<IParamValueQueue>()
            .map_or(std::ptr::null_mut(), |r| r.as_ptr())
    }
}
struct EventList {
    events: UnsafeCell<Vec<Event>>,
}
// SAFETY: only the audio thread touches the list, between filling it and the
// end of `process`.
unsafe impl Sync for EventList {}
impl Class for EventList {
    type Interfaces = (IEventList,);
}
impl IEventListTrait for EventList {
    unsafe fn getEventCount(&self) -> int32 {
        (*self.events.get()).len() as i32
    }
    unsafe fn getEvent(&self, index: int32, e: *mut Event) -> tresult {
        let events: &Vec<Event> = &*self.events.get();
        match events.get(index as usize) {
            Some(event) => {
                *e = *event;
                kResultOk
            }
            None => kResultFalse,
        }
    }
    unsafe fn addEvent(&self, _e: *mut Event) -> tresult {
        kResultFalse
    }
}

// ---------------------------------------------------------------------------
// Shared instance
// ---------------------------------------------------------------------------

struct Shared {
    _module: Arc<Module>,
    component: ComPtr<IComponent>,
    processor: ComPtr<IAudioProcessor>,
    controller: ComPtr<IEditController>,
    separate_controller: bool,
    _host: ComWrapper<HostApp>,
    handler: ComWrapper<Handler>,
    in_channels: usize,
    out_channels: usize,
    has_event_input: bool,
    active: AtomicBool,
}
// SAFETY: VST3 objects are shared between the main thread (controller, state,
// view) and the audio thread (process), which is the API's own contract.
unsafe impl Send for Shared {}
unsafe impl Sync for Shared {}
impl Drop for Shared {
    fn drop(&mut self) {
        unsafe {
            if self.active.swap(false, Ordering::AcqRel) {
                self.component.setActive(0);
            }
            if self.separate_controller {
                if let (Some(a), Some(b)) = (
                    self.component.cast::<IConnectionPoint>(),
                    self.controller.cast::<IConnectionPoint>(),
                ) {
                    a.disconnect(b.as_ptr());
                    b.disconnect(a.as_ptr());
                }
                self.controller.terminate();
            }
            self.component.terminate();
        }
    }
}

pub fn instantiate(plugin_id: &str, name: &str, rate: u32) -> Result<Instance> {
    let desc = super::scan::lookup(plugin_id)
        .ok_or_else(|| format!("{name} is not installed or has not been scanned"))?;
    instantiate_from(&desc, rate)
}
pub fn instantiate_from(desc: &Descriptor, rate: u32) -> Result<Instance> {
    let module = load(Path::new(&desc.path))?;
    let (_, hex) = Format::parse(&desc.id).ok_or("Bad VST3 id")?;
    let cid = cid_parse(hex).ok_or("Bad VST3 class id")?;
    let host = ComWrapper::new(HostApp);
    let host_unknown = host
        .to_com_ptr::<IHostApplication>()
        .ok_or("Host application interface missing")?;
    unsafe {
        let mut obj: *mut c_void = std::ptr::null_mut();
        if module.factory.createInstance(
            cid.as_ptr(),
            IComponent::IID.as_ptr() as FIDString,
            &mut obj,
        ) != kResultOk
            || obj.is_null()
        {
            return Err(format!("{} could not be created", desc.name));
        }
        let component = ComPtr::from_raw(obj as *mut IComponent).ok_or("Null component")?;
        if component.initialize(host_unknown.as_ptr() as *mut FUnknown) != kResultOk {
            return Err(format!("{} failed to initialise", desc.name));
        }
        let (controller, separate) = match component.cast::<IEditController>() {
            Some(c) => (c, false),
            None => {
                let mut ccid: TUID = [0; 16];
                if component.getControllerClassId(&mut ccid) != kResultOk {
                    component.terminate();
                    return Err(format!("{} has no edit controller", desc.name));
                }
                let mut obj: *mut c_void = std::ptr::null_mut();
                if module.factory.createInstance(
                    ccid.as_ptr(),
                    IEditController::IID.as_ptr() as FIDString,
                    &mut obj,
                ) != kResultOk
                    || obj.is_null()
                {
                    component.terminate();
                    return Err(format!("{} controller could not be created", desc.name));
                }
                let controller =
                    ComPtr::from_raw(obj as *mut IEditController).ok_or("Null controller")?;
                if controller.initialize(host_unknown.as_ptr() as *mut FUnknown) != kResultOk {
                    component.terminate();
                    return Err(format!("{} controller failed to initialise", desc.name));
                }
                if let (Some(a), Some(b)) = (
                    component.cast::<IConnectionPoint>(),
                    controller.cast::<IConnectionPoint>(),
                ) {
                    a.connect(b.as_ptr());
                    b.connect(a.as_ptr());
                }
                (controller, true)
            }
        };
        let handler = ComWrapper::new(Handler {
            edits: Mutex::new(vec![]),
            flags: AtomicU32::new(0),
        });
        if let Some(h) = handler.to_com_ptr::<IComponentHandler>() {
            controller.setComponentHandler(h.as_ptr());
        }
        let processor = component
            .cast::<IAudioProcessor>()
            .ok_or_else(|| format!("{} cannot process audio", desc.name))?;
        // Buses: main stereo in/out plus the first event input.
        let audio_in = component.getBusCount(
            MediaTypes_::kAudio as MediaType,
            BusDirections_::kInput as BusDirection,
        );
        let audio_out = component.getBusCount(
            MediaTypes_::kAudio as MediaType,
            BusDirections_::kOutput as BusDirection,
        );
        let event_in = component.getBusCount(
            MediaTypes_::kEvent as MediaType,
            BusDirections_::kInput as BusDirection,
        );
        if !(0..=64).contains(&audio_in) || !(0..=64).contains(&audio_out) {
            return Err(format!(
                "{} declares an unsupported number of audio buses",
                desc.name
            ));
        }
        let mut ins = vec![SpeakerArr::kStereo; audio_in.max(0) as usize];
        let mut outs = vec![SpeakerArr::kStereo; audio_out.max(0) as usize];
        processor.setBusArrangements(
            ins.as_mut_ptr(),
            ins.len() as i32,
            outs.as_mut_ptr(),
            outs.len() as i32,
        );
        let channels = |dir: BusDirections| -> Result<usize> {
            let mut info: BusInfo = std::mem::zeroed();
            if component.getBusInfo(
                MediaTypes_::kAudio as MediaType,
                dir as BusDirection,
                0,
                &mut info,
            ) == kResultOk
            {
                if !(0..=16).contains(&info.channelCount) {
                    return Err(format!(
                        "{} requires an unsupported {}-channel audio bus",
                        desc.name, info.channelCount
                    ));
                }
                Ok(info.channelCount as usize)
            } else {
                Err(format!("{} did not describe its audio bus", desc.name))
            }
        };
        let in_channels = if audio_in > 0 {
            channels(BusDirections_::kInput)?
        } else {
            0
        };
        let out_channels = if audio_out > 0 {
            channels(BusDirections_::kOutput)?
        } else {
            0
        };
        for i in 0..audio_in {
            component.activateBus(
                MediaTypes_::kAudio as MediaType,
                BusDirections_::kInput as BusDirection,
                i,
                (i == 0) as u8,
            );
        }
        for i in 0..audio_out {
            component.activateBus(
                MediaTypes_::kAudio as MediaType,
                BusDirections_::kOutput as BusDirection,
                i,
                (i == 0) as u8,
            );
        }
        if event_in > 0 {
            component.activateBus(
                MediaTypes_::kEvent as MediaType,
                BusDirections_::kInput as BusDirection,
                0,
                1,
            );
        }
        let mut setup = ProcessSetup {
            processMode: REALTIME,
            symbolicSampleSize: SAMPLE32,
            maxSamplesPerBlock: MAX_BLOCK as i32,
            sampleRate: rate as f64,
        };
        if processor.setupProcessing(&mut setup) != kResultOk {
            return Err(format!("{} rejected {rate} Hz processing", desc.name));
        }
        if component.setActive(1) != kResultOk {
            return Err(format!("{} could not be activated", desc.name));
        }
        // Give the controller the component's initial state.
        let stream = ComWrapper::new(BStream {
            data: Mutex::new((vec![], 0)),
        });
        if let Some(s) = stream.to_com_ptr::<IBStream>() {
            if component.getState(s.as_ptr()) == kResultOk {
                stream.data.lock().unwrap().1 = 0;
                controller.setComponentState(s.as_ptr());
            }
        }
        let shared = Arc::new(Shared {
            _module: module,
            component,
            processor,
            controller,
            separate_controller: separate,
            _host: host,
            handler,
            in_channels,
            out_channels,
            has_event_input: event_in > 0,
            active: AtomicBool::new(true),
        });
        let editor = Vst3Editor::new(shared.clone(), desc.clone());
        let processor = Vst3Processor::new(shared, rate);
        Ok(Instance {
            editor: Box::new(editor),
            processor: Some(Box::new(processor)),
        })
    }
}

// ---------------------------------------------------------------------------
// Editor
// ---------------------------------------------------------------------------

pub struct Vst3Editor {
    shared: Arc<Shared>,
    desc: Descriptor,
    params: Vec<ParamInfo>,
    view: Option<ComPtr<IPlugView>>,
    frame: ComWrapper<PlugFrame>,
    dirty: bool,
}
impl Vst3Editor {
    fn new(shared: Arc<Shared>, desc: Descriptor) -> Self {
        let mut editor = Self {
            shared,
            desc,
            params: vec![],
            view: None,
            frame: ComWrapper::new(PlugFrame {
                resize: Mutex::new(None),
            }),
            dirty: false,
        };
        editor.read_params();
        editor
    }
    fn read_params(&mut self) {
        self.params.clear();
        unsafe {
            let controller = &self.shared.controller;
            let count = controller.getParameterCount().clamp(0, 8192);
            for i in 0..count {
                let mut info: ParameterInfo = std::mem::zeroed();
                if controller.getParameterInfo(i, &mut info) != kResultOk {
                    continue;
                }
                if info.flags & ParameterInfo_::ParameterFlags_::kIsHidden != 0 {
                    continue;
                }
                let steps = info.stepCount.clamp(0, 100_000) as u32;
                let mut labels = vec![];
                if steps > 0 && steps <= 32 {
                    for s in 0..=steps {
                        labels.push(self.string_for(info.id, s as f64 / steps as f64));
                    }
                }
                self.params.push(ParamInfo {
                    id: info.id,
                    name: wstr(&info.title),
                    min: 0.0,
                    max: 1.0,
                    default: info.defaultNormalizedValue,
                    unit: wstr(&info.units),
                    steps,
                    log: false,
                    labels,
                });
            }
        }
    }
    fn string_for(&self, id: u32, normalized: f64) -> String {
        unsafe {
            let mut out: String128 = [0; 128];
            if self
                .shared
                .controller
                .getParamStringByValue(id, normalized, &mut out)
                == kResultOk
            {
                wstr(&out)
            } else {
                format!(
                    "{:.2}",
                    self.shared
                        .controller
                        .normalizedParamToPlain(id, normalized)
                )
            }
        }
    }
    fn state_of(&self, save: impl Fn(*mut IBStream) -> tresult) -> Option<Vec<u8>> {
        let stream = ComWrapper::new(BStream {
            data: Mutex::new((vec![], 0)),
        });
        let ptr = stream.to_com_ptr::<IBStream>()?;
        if save(ptr.as_ptr()) != kResultOk {
            return None;
        }
        let data = stream.data.lock().ok()?.0.clone();
        Some(data)
    }
}
#[cfg(target_os = "macos")]
const PLATFORM: FIDString = kPlatformTypeNSView;
#[cfg(target_os = "windows")]
const PLATFORM: FIDString = kPlatformTypeHWND;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const PLATFORM: FIDString = kPlatformTypeX11EmbedWindowID;

impl Editor for Vst3Editor {
    fn descriptor(&self) -> &Descriptor {
        &self.desc
    }
    fn params(&self) -> &[ParamInfo] {
        &self.params
    }
    fn value(&self, id: u32) -> Option<f64> {
        Some(unsafe { self.shared.controller.getParamNormalized(id) })
    }
    fn text(&self, id: u32, value: f64) -> String {
        self.string_for(id, value)
    }
    fn set_value(&mut self, id: u32, value: f64) {
        unsafe {
            self.shared
                .controller
                .setParamNormalized(id, value.clamp(0.0, 1.0));
        }
    }
    fn save(&mut self) -> Option<Vec<u8>> {
        let component = self.state_of(|s| unsafe { self.shared.component.getState(s) })?;
        let controller = self
            .state_of(|s| unsafe { self.shared.controller.getState(s) })
            .unwrap_or_default();
        let mut out = Vec::with_capacity(component.len() + controller.len() + 8);
        out.extend_from_slice(&(component.len() as u32).to_le_bytes());
        out.extend_from_slice(&component);
        out.extend_from_slice(&(controller.len() as u32).to_le_bytes());
        out.extend_from_slice(&controller);
        Some(out)
    }
    fn load(&mut self, bytes: &[u8]) -> Result<()> {
        let read = |at: usize| -> Result<(usize, &[u8])> {
            let len = u32::from_le_bytes(
                bytes
                    .get(at..at + 4)
                    .ok_or("Truncated state")?
                    .try_into()
                    .unwrap(),
            ) as usize;
            let data = bytes.get(at + 4..at + 4 + len).ok_or("Truncated state")?;
            Ok((at + 4 + len, data))
        };
        let (next, component) = read(0)?;
        let controller = read(next).map(|(_, c)| c).unwrap_or(&[]);
        let load = |data: &[u8], f: &dyn Fn(*mut IBStream) -> tresult| -> bool {
            let stream = ComWrapper::new(BStream {
                data: Mutex::new((data.to_vec(), 0)),
            });
            stream
                .to_com_ptr::<IBStream>()
                .is_some_and(|s| f(s.as_ptr()) == kResultOk)
        };
        if !load(component, &|s| unsafe { self.shared.component.setState(s) }) {
            return Err(format!("{} rejected its saved state", self.desc.name));
        }
        load(component, &|s| unsafe {
            self.shared.controller.setComponentState(s)
        });
        if !controller.is_empty() {
            load(controller, &|s| unsafe {
                self.shared.controller.setState(s)
            });
        }
        Ok(())
    }
    fn has_gui(&self) -> bool {
        unsafe {
            let view = self.shared.controller.createView(ViewType::kEditor);
            match ComPtr::from_raw(view) {
                Some(view) => view.isPlatformTypeSupported(PLATFORM) == kResultTrue,
                None => false,
            }
        }
    }
    fn open_gui(&mut self, parent: ParentWindow) -> Result<(u32, u32)> {
        unsafe {
            let view = ComPtr::from_raw(self.shared.controller.createView(ViewType::kEditor))
                .ok_or("The plugin has no editor")?;
            if view.isPlatformTypeSupported(PLATFORM) != kResultTrue {
                return Err("The plugin editor does not support this window system".into());
            }
            if let Some(frame) = self.frame.to_com_ptr::<IPlugFrame>() {
                view.setFrame(frame.as_ptr());
            }
            let handle = match parent {
                ParentWindow::Cocoa(p) | ParentWindow::Win32(p) => p,
                ParentWindow::X11(x) => x as usize as *mut c_void,
            };
            if view.attached(handle, PLATFORM) != kResultOk {
                return Err("The plugin refused the host window".into());
            }
            let mut rect = ViewRect {
                left: 0,
                top: 0,
                right: 600,
                bottom: 400,
            };
            view.getSize(&mut rect);
            self.view = Some(view);
            Ok((
                (rect.right - rect.left).max(1) as u32,
                (rect.bottom - rect.top).max(1) as u32,
            ))
        }
    }
    fn close_gui(&mut self) {
        if let Some(view) = self.view.take() {
            unsafe {
                view.removed();
                view.setFrame(std::ptr::null_mut());
            }
        }
    }
    fn take_resize_request(&mut self) -> Option<(u32, u32)> {
        self.frame.resize.lock().ok()?.take()
    }
    fn set_gui_size(&mut self, width: u32, height: u32) {
        if let Some(view) = &self.view {
            let mut rect = ViewRect {
                left: 0,
                top: 0,
                right: width as i32,
                bottom: height as i32,
            };
            unsafe {
                view.onSize(&mut rect);
            }
        }
    }
    fn idle(&mut self) {
        let flags = self.shared.handler.flags.swap(0, Ordering::AcqRel);
        if flags & RESTART_PARAMS != 0 {
            self.dirty = true;
        }
        if let Ok(mut edits) = self.shared.handler.edits.lock() {
            if !edits.is_empty() {
                self.dirty = true;
                edits.clear();
            }
        }
    }
    fn latency(&self) -> u32 {
        unsafe { self.shared.processor.getLatencySamples() }
    }
    fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }
}
impl Drop for Vst3Editor {
    fn drop(&mut self) {
        self.close_gui();
    }
}

// ---------------------------------------------------------------------------
// Processor
// ---------------------------------------------------------------------------

pub struct Vst3Processor {
    shared: Arc<Shared>,
    rate: u32,
    in_channels: Vec<Vec<f32>>,
    out_channels: Vec<Vec<f32>>,
    in_ptrs: Vec<*mut f32>,
    out_ptrs: Vec<*mut f32>,
    changes: ComWrapper<ParameterChanges>,
    out_changes: ComWrapper<ParameterChanges>,
    events: ComWrapper<EventList>,
    context: ProcessContext,
    processing: bool,
    edits: Vec<(u32, f64)>,
}
unsafe impl Send for Vst3Processor {}
impl Vst3Processor {
    fn new(shared: Arc<Shared>, rate: u32) -> Self {
        let mut in_channels: Vec<Vec<f32>> = (0..shared.in_channels)
            .map(|_| vec![0.0; MAX_BLOCK])
            .collect();
        let mut out_channels: Vec<Vec<f32>> = (0..shared.out_channels)
            .map(|_| vec![0.0; MAX_BLOCK])
            .collect();
        let in_ptrs = in_channels.iter_mut().map(|c| c.as_mut_ptr()).collect();
        let out_ptrs = out_channels.iter_mut().map(|c| c.as_mut_ptr()).collect();
        let queues = |n: usize| (0..n).map(|_| ComWrapper::new(ParamQueue::new())).collect();
        let parameter_count =
            unsafe { shared.controller.getParameterCount().clamp(0, 8192) as usize };
        Self {
            shared,
            rate,
            in_channels,
            out_channels,
            in_ptrs,
            out_ptrs,
            changes: ComWrapper::new(ParameterChanges {
                queues: queues(parameter_count.max(128)),
                count: AtomicUsize::new(0),
            }),
            out_changes: ComWrapper::new(ParameterChanges {
                queues: queues(32),
                count: AtomicUsize::new(0),
            }),
            events: ComWrapper::new(EventList {
                events: UnsafeCell::new(Vec::with_capacity(1024)),
            }),
            context: unsafe { std::mem::zeroed() },
            processing: false,
            edits: Vec::with_capacity(256),
        }
    }
}
impl Processor for Vst3Processor {
    fn start(&mut self) {
        if !self.processing && self.shared.active.load(Ordering::Acquire) {
            unsafe {
                self.shared.processor.setProcessing(1);
            }
            self.processing = true;
        }
    }
    fn stop(&mut self) {
        if self.processing {
            unsafe {
                self.shared.processor.setProcessing(0);
            }
            self.processing = false;
        }
    }
    fn latency(&self) -> u32 {
        0
    }
    /// Each queue point carries its sample offset.
    fn timed_params(&self) -> bool {
        true
    }
    fn process(
        &mut self,
        audio: &mut [[f32; 2]],
        notes: &[NoteEvent],
        params: &[ParamChange],
        ctx: &crate::plugin::ProcessContext,
    ) {
        if !self.processing {
            self.start();
            if !self.processing {
                return;
            }
        }
        let n = audio.len().min(MAX_BLOCK);
        if n == 0 {
            return;
        }
        // Parameter changes: the host's, one queue per parameter with a point at each change's
        // frame, then edits made in the plugin GUI.
        let mut count = 0;
        let last = n as u32 - 1;
        for change in params {
            let queues = &self.changes.queues;
            let queue = match queues[..count]
                .iter()
                .find(|q| q.id.load(Ordering::Relaxed) == change.id)
            {
                Some(queue) => queue,
                None if count < queues.len() => {
                    queues[count].reset(change.id);
                    count += 1;
                    &queues[count - 1]
                }
                None => continue,
            };
            queue.push(change.frame.min(last) as i32, change.value.clamp(0.0, 1.0));
        }
        if let Ok(mut edits) = self.shared.handler.edits.try_lock() {
            self.edits.clear();
            self.edits
                .extend(edits.drain(..).take(self.edits.capacity()));
        }
        for (id, value) in self.edits.drain(..) {
            if count >= self.changes.queues.len() {
                break;
            }
            let q = &self.changes.queues[count];
            q.reset(id);
            q.push(0, value);
            count += 1;
        }
        self.changes.count.store(count, Ordering::Relaxed);
        self.out_changes.count.store(0, Ordering::Relaxed);
        unsafe {
            let events = &mut *self.events.events.get();
            events.clear();
            if self.shared.has_event_input {
                for note in notes {
                    if events.len() >= events.capacity() {
                        break;
                    }
                    let mut e: Event = std::mem::zeroed();
                    e.busIndex = 0;
                    e.sampleOffset = (note.frame as usize).min(n - 1) as i32;
                    e.flags = 0;
                    if note.on {
                        e.r#type = Event_::EventTypes_::kNoteOnEvent as u16;
                        e.__field0.noteOn = NoteOnEvent {
                            channel: note.channel as i16,
                            pitch: note.pitch as i16,
                            tuning: 0.0,
                            velocity: note.velocity as f32 / 127.0,
                            length: 0,
                            noteId: -1,
                        };
                    } else {
                        e.r#type = Event_::EventTypes_::kNoteOffEvent as u16;
                        e.__field0.noteOff = NoteOffEvent {
                            channel: note.channel as i16,
                            pitch: note.pitch as i16,
                            velocity: 0.0,
                            noteId: -1,
                            tuning: 0.0,
                        };
                    }
                    events.push(e);
                }
            }
        }
        let mono = self.in_channels.len() == 1;
        for (c, channel) in self.in_channels.iter_mut().enumerate() {
            if mono {
                for (k, frame) in audio[..n].iter().enumerate() {
                    channel[k] = (frame[0] + frame[1]) * 0.5;
                }
            } else if c < 2 {
                for (k, frame) in audio[..n].iter().enumerate() {
                    channel[k] = frame[c];
                }
            } else {
                channel[..n].fill(0.0);
            }
        }
        for channel in &mut self.out_channels {
            channel[..n].fill(0.0);
        }
        let mut input = AudioBusBuffers {
            numChannels: self.in_channels.len() as i32,
            silenceFlags: 0,
            __field0: AudioBusBuffers__type0 {
                channelBuffers32: self.in_ptrs.as_mut_ptr(),
            },
        };
        let mut output = AudioBusBuffers {
            numChannels: self.out_channels.len() as i32,
            silenceFlags: 0,
            __field0: AudioBusBuffers__type0 {
                channelBuffers32: self.out_ptrs.as_mut_ptr(),
            },
        };
        let mut state = ProcessContext_::StatesAndFlags_::kTempoValid
            | ProcessContext_::StatesAndFlags_::kTimeSigValid
            | ProcessContext_::StatesAndFlags_::kProjectTimeMusicValid
            | ProcessContext_::StatesAndFlags_::kBarPositionValid
            | ProcessContext_::StatesAndFlags_::kContTimeValid;
        if ctx.playing {
            state |= ProcessContext_::StatesAndFlags_::kPlaying;
        }
        if ctx.recording {
            state |= ProcessContext_::StatesAndFlags_::kRecording;
        }
        if ctx.cycle.is_some() {
            state |= ProcessContext_::StatesAndFlags_::kCycleValid
                | ProcessContext_::StatesAndFlags_::kCycleActive;
        }
        let (cycle_start, cycle_end) = ctx.cycle.unwrap_or((0.0, 0.0));
        // `DefaultEnumType` is signed on Windows and unsigned elsewhere.
        #[allow(clippy::unnecessary_cast)]
        {
            self.context.state = state as u32;
        }
        self.context.sampleRate = self.rate as f64;
        self.context.projectTimeSamples = (ctx.position_seconds * self.rate as f64) as i64;
        self.context.continousTimeSamples = ctx.sample_time;
        self.context.projectTimeMusic = ctx.position_beats;
        self.context.barPositionMusic = ctx.bar_start_beats;
        self.context.cycleStartMusic = cycle_start;
        self.context.cycleEndMusic = cycle_end;
        self.context.tempo = ctx.tempo;
        self.context.timeSigNumerator = ctx.numerator as i32;
        self.context.timeSigDenominator = ctx.denominator as i32;
        let changes = self
            .changes
            .as_com_ref::<IParameterChanges>()
            .map_or(std::ptr::null_mut(), |r| r.as_ptr());
        let out_changes = self
            .out_changes
            .as_com_ref::<IParameterChanges>()
            .map_or(std::ptr::null_mut(), |r| r.as_ptr());
        let events = self
            .events
            .as_com_ref::<IEventList>()
            .map_or(std::ptr::null_mut(), |r| r.as_ptr());
        let mut data = ProcessData {
            processMode: REALTIME,
            symbolicSampleSize: SAMPLE32,
            numSamples: n as i32,
            numInputs: if self.in_channels.is_empty() { 0 } else { 1 },
            numOutputs: if self.out_channels.is_empty() { 0 } else { 1 },
            inputs: if self.in_channels.is_empty() {
                std::ptr::null_mut()
            } else {
                &mut input
            },
            outputs: if self.out_channels.is_empty() {
                std::ptr::null_mut()
            } else {
                &mut output
            },
            inputParameterChanges: changes,
            outputParameterChanges: out_changes,
            inputEvents: events,
            outputEvents: std::ptr::null_mut(),
            processContext: &mut self.context,
        };
        let result = unsafe { self.shared.processor.process(&mut data) };
        if result != kResultOk || self.out_channels.is_empty() {
            if self.out_channels.is_empty() {
                audio[..n].fill([0.0; 2]);
            }
            return;
        }
        for (k, frame) in audio[..n].iter_mut().enumerate() {
            let l = self.out_channels[0][k];
            let r = self.out_channels.get(1).map_or(l, |c| c[k]);
            *frame = [
                if l.is_finite() { l } else { 0.0 },
                if r.is_finite() { r } else { 0.0 },
            ];
        }
    }
}
