//! Ondera native plugins: libraries built with the `ondera-plugin` SDK and loaded through
//! its C ABI. The stock library is linked into the engine and goes through the very same
//! vtables, so the ABI adapter below runs in every session, not only when a third-party
//! library is installed.
//!
//! Loading keeps a library resident for the life of the process (like the CLAP host), so
//! vtable references are `'static`. Instances are opaque pointers: the editor half owns the
//! parameter values and the processor half owns the instance on the audio thread. State
//! restores go through a lock-free handoff that the processor applies before its next block.

use crate::{
    plugin::{Descriptor, Editor, Format, Instance, ParamInfo, Processor},
    Result,
};
use ondera_plugin::{
    ffi::{self, Entry, EntryFn, Manifest, PluginVTable, RawContext},
    Kind, NoteEvent, ParamChange, ProcessContext, ENTRY_SYMBOL,
};
use std::{
    collections::HashMap,
    ffi::c_void,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex, OnceLock,
    },
};

struct Loaded {
    _library: libloading::Library,
    tables: Vec<&'static PluginVTable>,
}
// Vtables are immutable function tables and the library is never unloaded.
unsafe impl Send for Loaded {}
unsafe impl Sync for Loaded {}

/// Platform extension of a loadable library.
pub fn library_extension() -> &'static str {
    if cfg!(target_os = "macos") {
        "dylib"
    } else if cfg!(windows) {
        "dll"
    } else {
        "so"
    }
}
/// Whether a path looks like a native plugin: an `.onplug` bundle or a bare library.
pub fn looks_like_plugin(path: &Path) -> bool {
    path.extension().is_some_and(|e| {
        e.eq_ignore_ascii_case("onplug") || e.eq_ignore_ascii_case(library_extension())
    })
}
/// The library inside an `.onplug` bundle, or the file itself.
pub fn library_path(bundle: &Path) -> Result<PathBuf> {
    if !bundle.is_dir() {
        return Ok(bundle.to_path_buf());
    }
    let mut candidates = vec![];
    for dir in [bundle.join("Contents/MacOS"), bundle.to_path_buf()] {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut found: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .filter(|p| {
                p.extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case(library_extension()))
                    || dir.ends_with("Contents/MacOS")
            })
            .collect();
        found.sort();
        candidates.extend(found);
    }
    candidates
        .into_iter()
        .next()
        .ok_or_else(|| format!("No plugin library in {}", bundle.display()))
}

fn cache() -> &'static Mutex<HashMap<PathBuf, Arc<Loaded>>> {
    static LOADED: OnceLock<Mutex<HashMap<PathBuf, Arc<Loaded>>>> = OnceLock::new();
    LOADED.get_or_init(|| Mutex::new(HashMap::new()))
}
/// Read every vtable an entry exports, checking the ABI version first.
///
/// # Safety
/// `entry` must point at an `Entry` produced by the SDK's export macro.
unsafe fn tables_of(entry: *const Entry) -> Result<Vec<&'static PluginVTable>> {
    if entry.is_null() {
        return Err("Plugin entry is null".into());
    }
    let entry = &*entry;
    if entry.abi_version != ondera_plugin::ABI_VERSION {
        return Err(format!(
            "Plugin ABI {} is not supported by this Ondera (ABI {})",
            entry.abi_version,
            ondera_plugin::ABI_VERSION
        ));
    }
    if entry.plugin_count == 0 || entry.plugin_count > 256 {
        return Err("Plugin library exports no plugins or too many".into());
    }
    let mut tables = vec![];
    for index in 0..entry.plugin_count {
        let table = (entry.plugin)(index);
        if table.is_null() {
            return Err(format!("Plugin {index} has no vtable"));
        }
        tables.push(&*table);
    }
    Ok(tables)
}
fn load(bundle: &Path) -> Result<Arc<Loaded>> {
    let mut guard = cache().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(loaded) = guard.get(bundle) {
        return Ok(loaded.clone());
    }
    let binary = library_path(bundle)?;
    // SAFETY: loading a plugin binary runs its initialisers; this is inherent to hosting.
    let library = unsafe { libloading::Library::new(&binary) }
        .map_err(|e| format!("Cannot load {}: {e}", binary.display()))?;
    let tables = unsafe {
        let entry: libloading::Symbol<EntryFn> = library
            .get(format!("{ENTRY_SYMBOL}\0").as_bytes())
            .map_err(|e| format!("Not an Ondera plugin ({e})"))?;
        tables_of(entry())?
    };
    let loaded = Arc::new(Loaded {
        _library: library,
        tables,
    });
    guard.insert(bundle.to_path_buf(), loaded.clone());
    Ok(loaded)
}

fn descriptor_of(manifest: &Manifest, path: &Path) -> Descriptor {
    Descriptor {
        id: format!("native:{}", manifest.id),
        format: Format::Native,
        name: manifest.name.clone(),
        vendor: manifest.vendor.clone(),
        path: path.to_string_lossy().into_owned(),
        instrument: manifest.kind == Kind::Instrument,
        effect: manifest.kind == Kind::Effect,
        category: manifest.category.clone(),
    }
}
/// Probe a bundle: every plugin it exports. Runs in the `--scan-plugin` child process.
pub fn scan(bundle: &Path) -> Result<Vec<Descriptor>> {
    let loaded = load(bundle)?;
    let mut out = vec![];
    for table in &loaded.tables {
        let manifest = unsafe { ffi::read_manifest(table)? };
        out.push(descriptor_of(&manifest, bundle));
    }
    Ok(out)
}
/// Manifests of a static in-process table (the stock library, tests).
pub fn manifests(tables: &'static [PluginVTable]) -> Result<Vec<Manifest>> {
    tables
        .iter()
        .map(|table| unsafe { ffi::read_manifest(table) })
        .collect()
}
pub fn param_infos(manifest: &Manifest) -> Vec<ParamInfo> {
    manifest
        .params
        .iter()
        .enumerate()
        .map(|(i, p)| ParamInfo {
            id: i as u32,
            name: p.name.clone(),
            min: p.min,
            max: p.max,
            default: p.default,
            unit: p.unit.clone(),
            steps: p.steps,
            log: p.log,
            labels: p.labels.clone(),
        })
        .collect()
}

/// Instantiate a scanned native plugin by descriptor id.
pub fn instantiate(plugin_id: &str, rate: u32) -> Result<Instance> {
    let descriptor = super::scan::lookup(plugin_id)
        .ok_or_else(|| format!("Unknown native plugin `{plugin_id}`; rescan plugins"))?;
    let bundle = PathBuf::from(&descriptor.path);
    let loaded = load(&bundle)?;
    for table in &loaded.tables {
        let manifest = unsafe { ffi::read_manifest(table)? };
        if format!("native:{}", manifest.id) == plugin_id {
            return instance_from(table, &manifest, descriptor_of(&manifest, &bundle), rate);
        }
    }
    Err(format!(
        "{} no longer exports `{plugin_id}`",
        bundle.display()
    ))
}

/// Parameter values waiting for the audio thread after a state restore.
struct Shared {
    values: Vec<AtomicU64>,
    pending: AtomicBool,
}

/// Build an editor/processor pair around one vtable.
pub fn instance_from(
    table: &'static PluginVTable,
    manifest: &Manifest,
    descriptor: Descriptor,
    rate: u32,
) -> Result<Instance> {
    let params = param_infos(manifest);
    // SAFETY: the vtable comes from the SDK export macro or a static table built with it.
    let instance = unsafe { (table.create)(rate as f64) };
    if instance.is_null() {
        return Err(format!("{} could not be created", descriptor.name));
    }
    let values: Vec<f64> = params.iter().map(|p| p.default).collect();
    for (i, value) in values.iter().enumerate() {
        unsafe { (table.set_param)(instance, i as u32, *value) };
    }
    let latency = unsafe { (table.latency)(instance) };
    let shared = Arc::new(Shared {
        values: values.iter().map(|v| AtomicU64::new(v.to_bits())).collect(),
        pending: AtomicBool::new(false),
    });
    Ok(Instance {
        editor: Box::new(NativeEditor {
            descriptor,
            params,
            values,
            latency,
            shared: shared.clone(),
        }),
        processor: Some(Box::new(NativeProcessor {
            table,
            instance,
            shared,
        })),
    })
}

struct NativeEditor {
    descriptor: Descriptor,
    params: Vec<ParamInfo>,
    values: Vec<f64>,
    latency: u32,
    shared: Arc<Shared>,
}
impl Editor for NativeEditor {
    fn descriptor(&self) -> &Descriptor {
        &self.descriptor
    }
    fn params(&self) -> &[ParamInfo] {
        &self.params
    }
    fn value(&self, id: u32) -> Option<f64> {
        self.values.get(id as usize).copied()
    }
    fn set_value(&mut self, id: u32, value: f64) {
        if let Some(v) = self.values.get_mut(id as usize) {
            *v = value;
        }
    }
    fn latency(&self) -> u32 {
        self.latency
    }
    /// State is the parameter value list; the same JSON array the stock library has
    /// always written, so older sessions load unchanged.
    fn save(&mut self) -> Option<Vec<u8>> {
        serde_json::to_vec(&self.values).ok()
    }
    fn load(&mut self, bytes: &[u8]) -> Result<()> {
        let parsed: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|e| format!("Invalid plugin state: {e}"))?;
        let values = parsed
            .as_array()
            .or_else(|| parsed.get("values").and_then(|v| v.as_array()))
            .ok_or("Plugin state must be a list of parameter values")?;
        for (i, v) in values.iter().enumerate() {
            if let (Some(slot), Some(v)) = (self.values.get_mut(i), v.as_f64()) {
                if v.is_finite() {
                    *slot = v.clamp(self.params[i].min, self.params[i].max);
                }
            }
        }
        for (value, shared) in self.values.iter().zip(&self.shared.values) {
            shared.store(value.to_bits(), Ordering::Relaxed);
        }
        self.shared.pending.store(true, Ordering::Release);
        Ok(())
    }
}

struct NativeProcessor {
    table: &'static PluginVTable,
    instance: *mut c_void,
    shared: Arc<Shared>,
}
// The SDK requires `Plugin: Send`; the instance pointer is owned by exactly one processor.
unsafe impl Send for NativeProcessor {}
impl Processor for NativeProcessor {
    fn reset(&mut self) {
        unsafe { (self.table.reset)(self.instance) };
    }
    fn process(
        &mut self,
        audio: &mut [[f32; 2]],
        notes: &[NoteEvent],
        params: &[ParamChange],
        ctx: &ProcessContext,
    ) {
        if self.shared.pending.swap(false, Ordering::Acquire) {
            for (index, value) in self.shared.values.iter().enumerate() {
                unsafe {
                    (self.table.set_param)(
                        self.instance,
                        index as u32,
                        f64::from_bits(value.load(Ordering::Relaxed)),
                    )
                };
            }
        }
        for change in params {
            unsafe { (self.table.set_param)(self.instance, change.id, change.value) };
        }
        let raw = RawContext::from(ctx);
        unsafe {
            (self.table.process)(
                self.instance,
                audio.as_mut_ptr(),
                audio.len() as u32,
                notes.as_ptr(),
                notes.len() as u32,
                &raw,
            )
        };
    }
    fn latency(&self) -> u32 {
        unsafe { (self.table.latency)(self.instance) }
    }
}
impl Drop for NativeProcessor {
    fn drop(&mut self) {
        unsafe { (self.table.destroy)(self.instance) };
    }
}

/// Load a library from a raw entry pointer, for tests that link a plugin crate directly.
///
/// # Safety
/// `entry` must come from the SDK's `ondera_plugin_entry`.
pub unsafe fn tables_from_entry(entry: *const Entry) -> Result<Vec<&'static PluginVTable>> {
    tables_of(entry)
}
