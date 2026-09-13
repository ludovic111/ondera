//! Plugin bank and editor panels. Instances are created on the UI thread, their
//! processors mounted into the audio thread's rack, and their editors kept here
//! for parameters, state capture and native windows.

use crate::{
    app::{id, Ondera},
    native::NativeWindow,
    theme::*,
};
use eframe::egui::{self, vec2, Align2, Rect, Sense, Vec2};
use ondera_engine::{
    device::{Message, Retired},
    host,
    model::*,
    plugin::{Descriptor, Editor, Format, Processor},
    store::Command,
};
use std::collections::{BTreeMap, HashMap};

pub struct Loaded {
    pub key: String,
    pub plugin_id: String,
    pub name: String,
    pub slot: u32,
    pub editor: Box<dyn Editor>,
    /// Held until an output device exists to mount it.
    pub processor: Option<Box<dyn Processor>>,
    pub mounted: bool,
    pub retiring: bool,
    pub sent: BTreeMap<u32, f64>,
    pub baseline: BTreeMap<u32, f64>,
    pub blob: String,
    pub latency: u32,
    pub external: bool,
}
pub struct EditorWindow {
    pub native: Option<NativeWindow>,
    pub filter: String,
}
#[derive(Default)]
pub struct Bank {
    pub loaded: HashMap<String, Loaded>,
    pub failed: HashMap<String, String>,
    pub slots: HashMap<String, u32>,
    pub windows: HashMap<String, EditorWindow>,
    free: Vec<u32>,
    next_slot: u32,
    pub rate: u32,
}
impl Bank {
    fn allocate(&mut self) -> u32 {
        if let Some(slot) = self.free.pop() {
            return slot;
        }
        let slot = self.next_slot;
        self.next_slot += 1;
        slot
    }
}

/// Locate the strip that owns an insert or instrument by rack key.
pub fn find_insert(session: &Session, key: &str) -> Option<(String, Insert, bool)> {
    for (strip_id, strip) in &session.strips {
        if let Some(i) = strip.inserts.iter().find(|i| i.id == key) {
            return Some((strip_id.clone(), i.clone(), false));
        }
        if let Some(s) = &strip.synth {
            if s.id == key {
                return Some((strip_id.clone(), s.clone(), true));
            }
        }
    }
    None
}

pub fn format_icon(format: Format) -> &'static str {
    match format {
        Format::Stock => "Ondera",
        Format::Native => "Native",
        Format::Clap => "CLAP",
        Format::Vst3 => "VST3",
        Format::AudioUnit => "AU",
    }
}

impl Ondera {
    /// Make the loaded plugins match the session: create, mount, unload and
    /// forward parameter changes. External plugins load one per frame so the
    /// interface keeps painting their names while they initialise.
    pub(crate) fn reconcile_plugins(&mut self) {
        let session = self.store.snapshot();
        let rate = self.device.as_ref().map_or(48000, |d| d.sample_rate);
        if self.plugins.rate != rate && !self.plugins.loaded.is_empty() {
            self.unload_plugins();
        }
        self.plugins.rate = rate;
        let needs = session.needs();
        let needed: HashMap<&str, &Need> = needs.iter().map(|n| (n.key.as_str(), n)).collect();
        let mut changed = false;
        let stale: Vec<String> = self
            .plugins
            .loaded
            .values()
            .filter(|l| !l.retiring)
            .filter(|l| {
                needed
                    .get(l.key.as_str())
                    .is_none_or(|n| n.plugin != l.plugin_id || n.blob != l.blob)
            })
            .map(|l| l.key.clone())
            .collect();
        for key in stale {
            self.retire_plugin(&key);
            changed = true;
        }
        let mut loaded_external = false;
        self.plugins
            .failed
            .retain(|key, _| needed.contains_key(key.as_str()));
        for need in &needs {
            if self.plugins.loaded.contains_key(&need.key)
                || self.plugins.failed.contains_key(&need.key)
            {
                continue;
            }
            let external = !need.plugin.starts_with("stock:");
            if external && loaded_external {
                continue;
            }
            if external {
                self.status = format!("Loading {}…", need.name);
                loaded_external = true;
            }
            match host::instantiate(&need.plugin, &need.name, rate) {
                Ok(mut instance) => {
                    if !need.blob.is_empty() {
                        if let Err(error) = host::decode_blob(&need.blob)
                            .and_then(|bytes| instance.editor.load(&bytes))
                        {
                            self.plugins.failed.insert(need.key.clone(), error.clone());
                            self.error = Some(format!(
                                "{} state could not be restored: {error}",
                                need.name
                            ));
                            continue;
                        }
                    }
                    let baseline = instance
                        .editor
                        .params()
                        .iter()
                        .map(|p| (p.id, instance.editor.value(p.id).unwrap_or(p.default)))
                        .collect();
                    let slot = self.plugins.allocate();
                    let mut entry = Loaded {
                        key: need.key.clone(),
                        plugin_id: need.plugin.clone(),
                        name: need.name.clone(),
                        slot,
                        editor: instance.editor,
                        processor: instance.processor,
                        mounted: false,
                        retiring: false,
                        sent: BTreeMap::new(),
                        baseline,
                        blob: need.blob.clone(),
                        latency: 0,
                        external,
                    };
                    for (&id, &value) in &need.params {
                        entry.editor.set_value(id, value);
                    }
                    self.plugins.slots.insert(need.key.clone(), slot);
                    self.plugins.loaded.insert(need.key.clone(), entry);
                    changed = true;
                }
                Err(e) => {
                    self.plugins.failed.insert(need.key.clone(), e.clone());
                    self.error = Some(format!("{}: {e}", need.name));
                }
            }
        }
        for need in &needs {
            if let Some(entry) = self.plugins.loaded.get_mut(&need.key) {
                if !entry.mounted && !entry.retiring {
                    for (&id, &value) in &entry.baseline {
                        if entry.sent.contains_key(&id) && !need.params.contains_key(&id) {
                            entry.editor.set_value(id, value);
                        }
                    }
                    for (&id, &value) in &need.params {
                        entry.editor.set_value(id, value);
                    }
                    entry.sent = need.params.clone();
                }
            }
        }
        // Mount processors once a device exists; forward parameter changes.
        if let Some(device) = &mut self.device {
            for need in &needs {
                let Some(entry) = self.plugins.loaded.get_mut(&need.key) else {
                    continue;
                };
                if entry.retiring {
                    continue;
                }
                if let Some(processor) = entry.processor.take() {
                    match device.try_send(Message::Mount(entry.slot, processor)) {
                        Ok(()) => {
                            entry.mounted = true;
                            entry.sent.clear();
                            changed = true;
                        }
                        Err(Message::Mount(_, processor)) => {
                            entry.processor = Some(processor);
                            self.status = "Waiting for the audio queue to load the plugin…".into();
                            break;
                        }
                        Err(_) => unreachable!("try_send returns its input message"),
                    }
                }
                if entry.mounted {
                    // A first edit's undo removes the override. Restore its original
                    // preset value instead of leaving the last edit audible.
                    let reset: Vec<_> = entry
                        .sent
                        .keys()
                        .filter(|id| !need.params.contains_key(id))
                        .filter_map(|id| entry.baseline.get(id).map(|value| (*id, *value)))
                        .collect();
                    for (id, value) in reset {
                        if device
                            .send(Message::SetParam(entry.slot, id, value))
                            .is_ok()
                        {
                            entry.editor.set_value(id, value);
                            entry.sent.remove(&id);
                        }
                    }
                    for (&id, &value) in &need.params {
                        if entry.sent.get(&id) != Some(&value)
                            && device
                                .send(Message::SetParam(entry.slot, id, value))
                                .is_ok()
                        {
                            entry.sent.insert(id, value);
                            entry.editor.set_value(id, value);
                        }
                    }
                }
            }
        }
        if changed {
            self.sync_needed = true;
        }
        if loaded_external && self.status.starts_with("Loading ") {
            self.status = "Ready".into();
        }
    }
    fn retire_plugin(&mut self, key: &str) {
        let Some(entry) = self.plugins.loaded.get_mut(key) else {
            return;
        };
        self.plugins.slots.remove(key);
        entry.editor.close_gui();
        self.plugins.windows.remove(key);
        if entry.mounted {
            entry.retiring = true;
            let slot = entry.slot;
            if let Some(device) = &mut self.device {
                if device.send(Message::Unmount(slot)).is_err() {
                    // Try again on a later frame.
                    entry.retiring = false;
                    self.plugins.slots.insert(key.to_string(), slot);
                }
            } else {
                entry.mounted = false;
                entry.retiring = false;
                self.plugins.loaded.remove(key);
                self.plugins.free.push(slot);
            }
        } else {
            let slot = entry.slot;
            self.plugins.loaded.remove(key);
            self.plugins.free.push(slot);
        }
    }
    /// Reclaim graphs and processors the audio thread handed back.
    pub(crate) fn collect_retired(&mut self) {
        let Some(device) = &mut self.device else {
            return;
        };
        for retired in device.collect() {
            if let Retired::Processor(slot, processor) = retired {
                drop(processor);
                let key = self
                    .plugins
                    .loaded
                    .iter()
                    .find(|(_, l)| l.slot == slot && l.retiring)
                    .map(|(k, _)| k.clone());
                if let Some(key) = key {
                    self.plugins.loaded.remove(&key);
                    self.plugins.free.push(slot);
                }
            }
        }
    }
    /// Unmount everything, wait for the audio thread, then destroy instances
    /// here. Used before reconnecting the device and before quitting.
    pub(crate) fn unload_plugins(&mut self) {
        for entry in self.plugins.loaded.values_mut() {
            entry.editor.close_gui();
            entry.retiring = entry.mounted;
        }
        self.plugins.windows.clear();
        if let Some(device) = &mut self.device {
            let mut requested = device.send(Message::UnmountAll).is_ok();
            let started = std::time::Instant::now();
            while started.elapsed() < std::time::Duration::from_millis(600) {
                self.collect_retired();
                if self.plugins.loaded.values().all(|l| !l.mounted) {
                    break;
                }
                if !requested {
                    requested = self
                        .device
                        .as_mut()
                        .is_some_and(|d| d.send(Message::UnmountAll).is_ok());
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
        // A stalled/disconnected output cannot acknowledge retirement. Stop and join
        // its owning worker before destroying any editor that shares plugin state.
        if self.plugins.loaded.values().any(|entry| entry.mounted) {
            self.midi = None;
            self.device = None;
            self.synced_revision = None;
            self.status =
                "Audio output stopped while unloading plugins; reconnect output to resume.".into();
        }
        self.plugins.windows.clear();
        self.plugins.loaded.clear();
        self.plugins.slots.clear();
        self.plugins.free.clear();
        self.plugins.next_slot = 0;
        self.plugins.failed.clear();
        self.sync_needed = true;
    }
    /// Capture native editor state as one document edit so it participates in
    /// undo and the unsaved-changes prompt as well as save/export.
    pub(crate) fn capture_plugin_states(&mut self) {
        struct Capture {
            key: String,
            blob: String,
            baseline: BTreeMap<u32, f64>,
        }
        let mut updates = vec![];
        let mut strips = HashMap::new();
        let desired = self.store.snapshot();
        for entry in self.plugins.loaded.values_mut() {
            if !entry.external || entry.retiring {
                continue;
            }
            let Some((strip_id, insert, _)) = find_insert(&desired, &entry.key) else {
                continue;
            };
            if insert.blob != entry.blob {
                // A preset/state command may be waiting for reconciliation. Capturing
                // the old instance here would silently overwrite that requested state.
                continue;
            }
            let Some(bytes) = entry.editor.save() else {
                continue;
            };
            let blob = host::encode_blob(&bytes);
            let mut parameters = insert.params.clone();
            let mut automated = BTreeMap::new();
            for lane in &desired.automation {
                if let ondera_engine::automation::AutomationTarget::PluginParameter {
                    insert_id,
                    plugin_id,
                    parameter_id,
                    ..
                } = &lane.target
                {
                    if insert_id != &entry.key || plugin_id != &entry.plugin_id {
                        continue;
                    }
                    if lane.enabled && !lane.points.is_empty() {
                        automated.insert(*parameter_id, lane.manual_value);
                        parameters.entry(*parameter_id).or_insert(lane.manual_value);
                    } else {
                        // A disabled lane still needs its manual value on reopening;
                        // its original creation-time fallback may predate native edits.
                        parameters.entry(*parameter_id).or_insert_with(|| {
                            entry
                                .editor
                                .value(*parameter_id)
                                .filter(|value| value.is_finite())
                                .unwrap_or(lane.manual_value)
                        });
                    }
                }
            }
            for (&id, &value) in &insert.params {
                // An applied generic override may subsequently have changed in the
                // plugin's own editor. Persist that actual value beside the blob so
                // reopening cannot overwrite it with the earlier generic knob value.
                // Preserve commands which have not reached the loaded instance yet.
                if !automated.contains_key(&id) && entry.sent.get(&id) == Some(&value) {
                    if let Some(actual) = entry.editor.value(id).filter(|value| value.is_finite()) {
                        parameters.insert(id, actual);
                    }
                }
            }
            if blob == insert.blob && parameters == insert.params {
                continue;
            }
            let baseline = entry
                .editor
                .params()
                .iter()
                .filter(|parameter| !parameters.contains_key(&parameter.id))
                .map(|parameter| {
                    (
                        parameter.id,
                        entry
                            .editor
                            .value(parameter.id)
                            .unwrap_or(parameter.default),
                    )
                })
                .collect();
            let strip = strips
                .entry(strip_id.clone())
                .or_insert_with(|| desired.strips[&strip_id].clone());
            for insert in strip.inserts.iter_mut().chain(strip.synth.iter_mut()) {
                if insert.id == entry.key {
                    insert.blob = blob.clone();
                    insert.params = parameters.clone();
                }
            }
            updates.push(Capture {
                key: entry.key.clone(),
                blob,
                baseline,
            });
        }
        if updates.is_empty() {
            return;
        }
        self.store.set_gesture(false);
        let commands = strips
            .into_iter()
            .map(|(track, strip)| Command::SetStrip { track, strip })
            .collect();
        if let Err(e) = self.try_dispatch(Command::Batch(commands)) {
            self.error = Some(e);
        } else {
            for capture in updates {
                if let Some(entry) = self.plugins.loaded.get_mut(&capture.key) {
                    // The loaded instance already contains this state. Keep it alive,
                    // and let a later first-parameter undo restore the captured preset.
                    entry.blob = capture.blob;
                    entry.baseline.extend(capture.baseline);
                }
            }
        }
    }
    /// Per-frame plugin housekeeping: main-thread callbacks and native windows.
    pub(crate) fn idle_plugins(&mut self) {
        let mut closed = vec![];
        for (key, entry) in self.plugins.loaded.iter_mut() {
            entry.editor.idle();
            let latency = entry.editor.latency();
            if entry.latency != latency {
                entry.latency = latency;
                self.sync_needed = true;
            }
            if let Some(window) = self.plugins.windows.get_mut(key) {
                if let Some(native) = &window.native {
                    if !native.is_open() {
                        entry.editor.close_gui();
                        closed.push(key.clone());
                    } else if let Some((w, h)) = entry.editor.take_resize_request() {
                        native.set_size(w, h);
                        entry.editor.set_gui_size(w, h);
                    }
                }
            }
        }
        for key in closed {
            if let Some(window) = self.plugins.windows.get_mut(&key) {
                window.native = None;
            }
        }
    }
    pub(crate) fn open_plugin_window(&mut self, key: &str) {
        self.plugins
            .windows
            .entry(key.to_string())
            .or_insert(EditorWindow {
                native: None,
                filter: String::new(),
            });
    }
    pub(crate) fn toggle_native_window_public(&mut self, key: &str) {
        self.toggle_native_window(key);
    }
    fn toggle_native_window(&mut self, key: &str) {
        let Some(entry) = self.plugins.loaded.get_mut(key) else {
            return;
        };
        let Some(window) = self.plugins.windows.get_mut(key) else {
            return;
        };
        if let Some(native) = window.native.take() {
            entry.editor.close_gui();
            native.close();
            return;
        }
        match NativeWindow::new(&entry.name, 640, 420) {
            Ok(native) => match entry.editor.open_gui(native.parent()) {
                Ok((w, h)) => {
                    native.set_size(w, h);
                    native.focus();
                    window.native = Some(native);
                }
                Err(e) => {
                    native.close();
                    self.error = Some(e);
                }
            },
            Err(e) => self.error = Some(e),
        }
    }
    /// Replace one parameter of an insert or instrument, as an undoable edit.
    fn set_insert_param(&mut self, key: &str, param: u32, value: f64) {
        let Some((strip_id, _, is_synth)) = find_insert(self.store.session(), key) else {
            return;
        };
        let mut strip = self.store.session().strips[&strip_id].clone();
        let target = if is_synth {
            strip.synth.as_mut()
        } else {
            strip.inserts.iter_mut().find(|i| i.id == key)
        };
        if let Some(insert) = target {
            insert.params.insert(param, value);
        }
        self.dispatch(Command::SetStrip {
            track: strip_id,
            strip,
        });
    }
    /// Insert an effect on a strip (track, bus or master) in the first free slot.
    pub(crate) fn add_effect_to(&mut self, strip_id: &str, plugin_id: &str, name: &str) {
        let mut strip = self
            .store
            .session()
            .strips
            .get(strip_id)
            .cloned()
            .unwrap_or_default();
        let insert = Insert::new(id("insert"), plugin_id, name);
        let key = insert.id.clone();
        if let Some(empty) = strip.inserts.iter_mut().find(|i| i.is_empty()) {
            *empty = insert;
        } else if strip.inserts.len() < MAX_INSERTS {
            strip.inserts.push(insert);
        } else {
            self.error = Some(format!("All {MAX_INSERTS} insert slots are in use."));
            return;
        }
        self.dispatch(Command::SetStrip {
            track: strip_id.to_string(),
            strip,
        });
        self.open_plugin_window(&key);
    }
    /// Choose the instrument of a MIDI track: a stock preset or an external plugin.
    pub(crate) fn set_instrument(&mut self, track: &str, plugin_id: &str, name: &str) {
        let mut strip = self
            .store
            .session()
            .strips
            .get(track)
            .cloned()
            .unwrap_or_default();
        if let Some((Format::Stock, stock_name)) = Format::parse(plugin_id) {
            strip.instrument = stock_name.into();
            strip.synth = None;
        } else {
            strip.synth = Some(Insert::new(id("synth"), plugin_id, name));
        }
        self.dispatch(Command::SetStrip {
            track: track.to_string(),
            strip,
        });
    }
    /// The floating parameter panels and native window controls.
    pub(crate) fn plugin_windows(&mut self, ctx: &egui::Context) {
        let keys: Vec<String> = self.plugins.windows.keys().cloned().collect();
        for key in keys {
            let session = self.store.snapshot();
            let Some((strip_id, insert, is_synth)) = find_insert(&session, &key) else {
                self.plugins.windows.remove(&key);
                continue;
            };
            let mut open = true;
            let mut edits: Vec<(u32, f64)> = vec![];
            let mut toggle_native = false;
            let mut toggle_bypass = false;
            let mut remove = false;
            let title = format!("{} · {}", insert.name, strip_label(&session, &strip_id));
            egui::Window::new(title)
                .id(egui::Id::new(("plugin-window", &key)))
                .open(&mut open)
                .resizable(true)
                .default_width(420.0)
                .show(ctx, |ui| {
                    ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                    let Some(entry) = self.plugins.loaded.get(&key) else {
                        ui.label(text(
                            self.plugins
                                .failed
                                .get(&key)
                                .cloned()
                                .unwrap_or_else(|| "Loading…".into()),
                            FS_BODY,
                            Weight::Medium,
                            DIM,
                        ));
                        return;
                    };
                    let desc = entry.editor.descriptor().clone();
                    ui.horizontal(|ui| {
                        ui.label(text(
                            format!(
                                "{}{}",
                                format_icon(desc.format),
                                if desc.vendor.is_empty() {
                                    String::new()
                                } else {
                                    format!(" · {}", desc.vendor)
                                }
                            ),
                            FS_SECONDARY,
                            Weight::Medium,
                            FAINT,
                        ));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if !is_synth
                                && text_button(
                                    ui,
                                    if insert.state == "bypassed" {
                                        "Bypassed"
                                    } else {
                                        "On"
                                    },
                                    Face::from_flag(insert.state != "bypassed"),
                                )
                                .clicked()
                            {
                                toggle_bypass = true;
                            }
                            if entry.editor.has_gui() {
                                let native_open = self
                                    .plugins
                                    .windows
                                    .get(&key)
                                    .is_some_and(|w| w.native.is_some());
                                if text_button(
                                    ui,
                                    if native_open {
                                        "Close plugin window"
                                    } else {
                                        "Open plugin window"
                                    },
                                    Face::from_flag(native_open),
                                )
                                .clicked()
                                {
                                    toggle_native = true;
                                }
                            }
                            if !is_synth && text_button(ui, "Remove", Face::Raised).clicked() {
                                remove = true;
                            }
                        });
                    });
                    let params = entry.editor.params().to_vec();
                    if params.is_empty() {
                        ui.label(text(
                            "This plugin exposes no parameters; use its own window.",
                            FS_SECONDARY,
                            Weight::Medium,
                            FAINT,
                        ));
                        return;
                    }
                    let window = self.plugins.windows.get_mut(&key).unwrap();
                    if params.len() > 24 {
                        ui.horizontal(|ui| {
                            let (well, _) =
                                ui.allocate_exact_size(vec2(220.0, 24.0), Sense::hover());
                            well_input(ui.painter(), well, R_MD);
                            ui.scope_builder(
                                egui::UiBuilder::new().max_rect(well.shrink2(vec2(4.0, 2.0))),
                                |ui| {
                                    inline_edit(
                                        ui,
                                        ("plugin-filter", &key),
                                        &mut window.filter,
                                        font(FS_LIST, Weight::Medium),
                                        INK,
                                        well.width() - 8.0,
                                    );
                                },
                            );
                            ui.label(text(
                                format!("{} parameters", params.len()),
                                FS_SECONDARY,
                                Weight::Medium,
                                FAINT,
                            ));
                        });
                    }
                    let filter = window.filter.to_lowercase();
                    let visible: Vec<_> = params
                        .iter()
                        .filter(|p| filter.is_empty() || p.name.to_lowercase().contains(&filter))
                        .take(256)
                        .collect();
                    egui::ScrollArea::vertical()
                        .max_height(360.0)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = vec2(6.0, 10.0);
                                for p in visible {
                                    let current = insert
                                        .params
                                        .get(&p.id)
                                        .copied()
                                        .or_else(|| entry.editor.value(p.id))
                                        .unwrap_or(p.default);
                                    let (cell, _) =
                                        ui.allocate_exact_size(vec2(72.0, 78.0), Sense::hover());
                                    let painter = ui.painter();
                                    painter.text(
                                        cell.center_top() + vec2(0.0, 2.0),
                                        Align2::CENTER_TOP,
                                        truncate(&p.name, 11),
                                        font(FS_CAPS, Weight::Bold),
                                        FAINT,
                                    );
                                    let well = Rect::from_center_size(
                                        cell.center_bottom() - vec2(0.0, 9.0),
                                        vec2(70.0, 16.0),
                                    );
                                    well_value(painter, well, 3.0);
                                    painter.text(
                                        well.center(),
                                        Align2::CENTER_CENTER,
                                        truncate(&entry.editor.text(p.id, current), 12),
                                        mono_font(FS_MICRO),
                                        INK,
                                    );
                                    if !p.labels.is_empty() {
                                        let button = Rect::from_center_size(
                                            cell.center() + vec2(0.0, 2.0),
                                            vec2(64.0, 24.0),
                                        );
                                        let response = ui.interact(
                                            button,
                                            egui::Id::new(("choice", &key, p.id)),
                                            Sense::click(),
                                        );
                                        raised(ui.painter(), button, R_BUTTON);
                                        ui.painter().text(
                                            button.center(),
                                            Align2::CENTER_CENTER,
                                            "▾",
                                            font(FS_LIST, Weight::Bold),
                                            INK_CONTROL,
                                        );
                                        egui::Popup::menu(&response).show(|ui| {
                                            for (i, label) in p.labels.iter().enumerate() {
                                                let value = p.min
                                                    + i as f64 * (p.max - p.min)
                                                        / (p.labels.len() - 1).max(1) as f64;
                                                if ui
                                                    .selectable_label(
                                                        (current - value).abs() < 1e-6,
                                                        label,
                                                    )
                                                    .clicked()
                                                {
                                                    edits.push((p.id, value));
                                                }
                                            }
                                        });
                                        continue;
                                    }
                                    let mut t = p.normalize(current) as f32;
                                    let knob = Rect::from_center_size(
                                        cell.center() + vec2(0.0, 2.0),
                                        Vec2::splat(KNOB_LG + 4.0),
                                    );
                                    ui.scope_builder(egui::UiBuilder::new().max_rect(knob), |ui| {
                                        if knob_widget(
                                            ui,
                                            &mut t,
                                            0.0..=1.0,
                                            p.normalize(p.default) as f32,
                                            KNOB_LG,
                                        )
                                        .on_hover_text(&p.name)
                                        .changed()
                                        {
                                            edits.push((p.id, p.denormalize(t as f64)));
                                        }
                                    });
                                }
                            });
                        });
                });
            for (param, value) in edits {
                self.set_insert_param(&key, param, value);
            }
            if toggle_bypass {
                let mut strip = session.strips[&strip_id].clone();
                if let Some(i) = strip.inserts.iter_mut().find(|i| i.id == key) {
                    i.state = if i.state == "bypassed" {
                        "active"
                    } else {
                        "bypassed"
                    }
                    .into();
                }
                self.dispatch(Command::SetStrip {
                    track: strip_id.clone(),
                    strip,
                });
            }
            if toggle_native {
                self.toggle_native_window(&key);
            }
            if remove {
                let mut strip = session.strips[&strip_id].clone();
                strip.inserts.retain(|i| i.id != key);
                self.dispatch(Command::SetStrip {
                    track: strip_id.clone(),
                    strip,
                });
                open = false;
            }
            if !open {
                if let Some(mut window) = self.plugins.windows.remove(&key) {
                    if let Some(native) = window.native.take() {
                        if let Some(entry) = self.plugins.loaded.get_mut(&key) {
                            entry.editor.close_gui();
                        }
                        native.close();
                    }
                }
            }
        }
    }
    /// Instruments and effects available to the browser, stock first.
    pub(crate) fn catalog_entries(&self, instruments: bool) -> Vec<Descriptor> {
        self.catalog
            .iter()
            .filter(|d| if instruments { d.instrument } else { d.effect })
            .cloned()
            .collect()
    }
    pub(crate) fn scan_plugins(&mut self) {
        if self.scan_job.is_some() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        self.scan_job = Some(rx);
        self.status = "Scanning plugins…".into();
        std::thread::spawn(move || {
            let progress = tx.clone();
            let cache = host::scan::scan_all(|path| {
                let _ = progress.send(ScanEvent::Progress(path.to_string()));
            });
            let _ = tx.send(ScanEvent::Done(cache));
        });
    }
    pub(crate) fn poll_scan(&mut self) {
        let Some(rx) = &self.scan_job else {
            return;
        };
        loop {
            match rx.try_recv() {
                Ok(ScanEvent::Progress(path)) => {
                    let name = std::path::Path::new(&path)
                        .file_name()
                        .map(|f| f.to_string_lossy().into_owned())
                        .unwrap_or(path);
                    self.status = format!("Scanning {name}…");
                }
                Ok(ScanEvent::Done(cache)) => {
                    let failed: Vec<String> = cache
                        .entries
                        .iter()
                        .filter_map(|e| {
                            e.error.as_ref().map(|err| {
                                format!(
                                    "{}: {err}",
                                    std::path::Path::new(&e.path)
                                        .file_name()
                                        .map(|f| f.to_string_lossy().into_owned())
                                        .unwrap_or_default()
                                )
                            })
                        })
                        .collect();
                    self.catalog = host::scan::installed();
                    let external = self
                        .catalog
                        .iter()
                        .filter(|d| d.format != Format::Stock)
                        .count();
                    self.status = format!("{external} external plugins found");
                    self.scan_job = None;
                    self.plugins.failed.clear();
                    if !failed.is_empty() {
                        self.error = Some(format!(
                            "Some plugins could not be scanned:\n{}",
                            failed.join("\n")
                        ));
                    }
                    return;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => return,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.scan_job = None;
                    self.status = "Plugin scan stopped".into();
                    return;
                }
            }
        }
    }
}
pub enum ScanEvent {
    Progress(String),
    Done(host::scan::Cache),
}
pub fn strip_label(session: &Session, strip_id: &str) -> String {
    if is_bus(strip_id) {
        bus_name(strip_id).to_string()
    } else {
        session
            .tracks
            .iter()
            .find(|t| t.id == strip_id)
            .map(|t| t.name.clone())
            .unwrap_or_else(|| strip_id.to_string())
    }
}
pub fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Intent;
    use ondera_engine::{control::Host, store};

    /// Use the real stock state serializer as a deterministic native-editor stand-in:
    /// direct editor changes bypass the store just like an external plugin's own UI.
    fn captured_editor() -> (Ondera, String, u32, f64, f64) {
        let mut app = Ondera::from_session(store::empty(), None);
        let track = app.store.session().tracks[0].id.clone();
        app.add_effect_to(&track, "stock:Utility", "Utility");
        app.reconcile_plugins();
        let key = app.store.session().strips[&track].inserts[0].id.clone();
        let entry = app.plugins.loaded.get_mut(&key).unwrap();
        entry.external = true;
        let parameter = entry.editor.params()[0].clone();
        app.capture_plugin_states();
        app.store.mark_saved(app.store.revision);
        assert!(!app.store.dirty());
        (app, key, parameter.id, parameter.min, parameter.max)
    }

    #[test]
    fn native_only_edit_prompts_before_quit_and_capture_can_be_undone() {
        let (mut app, key, parameter, min, max) = captured_editor();
        let original = app.plugins.loaded[&key].editor.value(parameter).unwrap();
        let edited = if original == max { min } else { max };
        app.plugins
            .loaded
            .get_mut(&key)
            .unwrap()
            .editor
            .set_value(parameter, edited);
        assert!(
            !app.store.dirty(),
            "Native changes have not been captured yet"
        );
        app.request(Intent::Quit);
        assert!(matches!(app.intent, Some(Intent::Quit)));
        assert!(
            !app.closing,
            "The app must wait for the save/discard choice"
        );
        assert!(app.store.dirty());
        let captured = app.store.revision;
        app.capture_plugin_states();
        assert_eq!(
            app.store.revision, captured,
            "An unchanged capture is not another edit"
        );
        app.dispatch(Command::Undo);
        app.reconcile_plugins();
        assert!(!app.store.dirty());
        assert_eq!(
            app.plugins.loaded[&key].editor.value(parameter),
            Some(original)
        );
    }

    #[test]
    fn saved_native_preset_is_the_next_parameter_edits_undo_baseline() {
        let (mut app, key, parameter, min, max) = captured_editor();
        let preset = min + (max - min) * 0.25;
        let edited = min + (max - min) * 0.75;
        let slot = app.plugins.loaded[&key].slot;
        app.plugins
            .loaded
            .get_mut(&key)
            .unwrap()
            .editor
            .set_value(parameter, preset);
        app.capture_plugin_states();
        app.store.mark_saved(app.store.revision);
        app.reconcile_plugins();
        assert_eq!(
            app.plugins.loaded[&key].slot, slot,
            "Capturing should keep the live instance"
        );
        app.set_insert_param(&key, parameter, edited);
        app.reconcile_plugins();
        assert_eq!(
            app.plugins.loaded[&key].editor.value(parameter),
            Some(edited)
        );
        app.dispatch(Command::Undo);
        app.reconcile_plugins();
        assert_eq!(
            app.plugins.loaded[&key].editor.value(parameter),
            Some(preset)
        );
        assert!(!app.store.dirty());
        app.dispatch(Command::Redo);
        app.reconcile_plugins();
        assert_eq!(
            app.plugins.loaded[&key].editor.value(parameter),
            Some(edited)
        );
    }

    #[test]
    fn native_change_after_generic_override_survives_fresh_processing_and_undo() {
        use ondera_engine::plugin::{ParamChange, ProcessContext};

        let (mut app, key, parameter, min, max) = captured_editor();
        let generic = min + (max - min) * 0.25;
        let native = min + (max - min) * 0.75;
        app.set_insert_param(&key, parameter, generic);
        app.reconcile_plugins();
        app.plugins
            .loaded
            .get_mut(&key)
            .unwrap()
            .editor
            .set_value(parameter, native);
        app.capture_plugin_states();
        let captured = find_insert(app.store.session(), &key).unwrap().1;
        assert_eq!(captured.params[&parameter], native);
        app.reconcile_plugins();
        assert_eq!(
            app.plugins.loaded[&key].editor.value(parameter),
            Some(native)
        );

        // Restore into a fresh processor using the same state-plus-overrides order
        // as offline export, then compare real DSP output to the native knob value.
        let mut restored = host::instantiate(&captured.plugin_id(), &captured.name, 48000).unwrap();
        restored
            .editor
            .load(&host::decode_blob(&captured.blob).unwrap())
            .unwrap();
        let params: Vec<_> = captured
            .params
            .iter()
            .map(|(&id, &value)| ParamChange { id, value })
            .collect();
        let mut reference =
            host::instantiate(&captured.plugin_id(), &captured.name, 48000).unwrap();
        let mut actual = [[0.01, -0.02]; 256];
        let mut expected = actual;
        restored.processor.as_mut().unwrap().process(
            &mut actual,
            &[],
            &params,
            &ProcessContext::default(),
        );
        reference.processor.as_mut().unwrap().process(
            &mut expected,
            &[],
            &[ParamChange {
                id: parameter,
                value: native,
            }],
            &ProcessContext::default(),
        );
        assert_eq!(actual, expected);
        assert!(actual.iter().any(|frame| frame[0] != 0.0));

        app.dispatch(Command::Undo);
        app.reconcile_plugins();
        assert_eq!(
            app.plugins.loaded[&key].editor.value(parameter),
            Some(generic)
        );
        app.dispatch(Command::Redo);
        app.reconcile_plugins();
        assert_eq!(
            app.plugins.loaded[&key].editor.value(parameter),
            Some(native)
        );
    }

    #[test]
    fn native_capture_preserves_generic_overrides_not_yet_applied() {
        let (mut app, key, parameter, min, max) = captured_editor();
        let pending = min + (max - min) * 0.25;
        let native = min + (max - min) * 0.75;
        app.set_insert_param(&key, parameter, pending);
        app.plugins
            .loaded
            .get_mut(&key)
            .unwrap()
            .editor
            .set_value(parameter, native);
        app.capture_plugin_states();
        assert_eq!(
            find_insert(app.store.session(), &key).unwrap().1.params[&parameter],
            pending
        );
        app.reconcile_plugins();
        assert_eq!(
            app.plugins.loaded[&key].editor.value(parameter),
            Some(pending)
        );
    }

    #[test]
    fn capturing_read_automation_preserves_manual_parameter_values() {
        use ondera_engine::automation::{
            AutomationLane, AutomationPoint, AutomationTarget, Interpolation,
        };

        for explicit in [false, true] {
            let (mut app, key, parameter, min, max) = captured_editor();
            let base = app.plugins.loaded[&key].editor.value(parameter).unwrap();
            let manual = if explicit {
                min + (max - min) * 0.25
            } else {
                base
            };
            let modulated = min + (max - min) * 0.75;
            if explicit {
                app.set_insert_param(&key, parameter, manual);
                app.reconcile_plugins();
            }
            let (track, insert, _) = find_insert(app.store.session(), &key).unwrap();
            app.dispatch(Command::PutAutomation(AutomationLane {
                id: "test-lane".into(),
                name: "Gain".into(),
                target: AutomationTarget::PluginParameter {
                    track_id: track,
                    insert_id: key.clone(),
                    plugin_id: insert.plugin_id(),
                    parameter_id: parameter,
                },
                min,
                max,
                manual_value: base,
                interpolation: Interpolation::Linear,
                enabled: true,
                points: vec![AutomationPoint {
                    id: "test-point".into(),
                    beat: 0.0,
                    value: modulated,
                }],
            }));
            app.plugins
                .loaded
                .get_mut(&key)
                .unwrap()
                .editor
                .set_value(parameter, modulated);
            app.capture_plugin_states();
            assert_eq!(
                find_insert(app.store.session(), &key).unwrap().1.params[&parameter],
                manual
            );
            assert_eq!(app.store.session().automation[0].points[0].value, modulated);
            app.dispatch(Command::RemoveAutomation("test-lane".into()));
            app.reconcile_plugins();
            assert_eq!(
                app.plugins.loaded[&key].editor.value(parameter),
                Some(manual)
            );
        }
    }

    #[test]
    fn capture_does_not_replace_state_waiting_for_reconciliation() {
        let (mut app, key, parameter, min, max) = captured_editor();
        let entry = app.plugins.loaded.get_mut(&key).unwrap();
        let original = entry.editor.value(parameter).unwrap();
        entry
            .editor
            .set_value(parameter, if original == max { min } else { max });
        let pending_blob = host::encode_blob(&entry.editor.save().unwrap());
        entry.editor.set_value(parameter, original);
        let (track, _, _) = find_insert(app.store.session(), &key).unwrap();
        let mut strip = app.store.session().strips[&track].clone();
        strip.inserts[0].blob = pending_blob.clone();
        app.dispatch(Command::SetStrip { track, strip });
        let pending_revision = app.store.revision;
        app.capture_plugin_states();
        assert_eq!(app.store.revision, pending_revision);
        assert_eq!(
            find_insert(app.store.session(), &key).unwrap().1.blob,
            pending_blob
        );
    }

    #[test]
    fn first_parameter_edit_undo_restores_the_loaded_preset() {
        let mut app = Ondera::from_session(store::empty(), None);
        let track = app.store.session().tracks[0].id.clone();
        app.add_effect_to(&track, "stock:Utility", "Utility");
        app.reconcile_plugins();
        let key = app.store.session().strips[&track].inserts[0].id.clone();
        let entry = &app.plugins.loaded[&key];
        let p = entry.editor.params()[0].clone();
        let original = entry.editor.value(p.id).unwrap();
        let edited = if original == p.max { p.min } else { p.max };
        app.set_insert_param(&key, p.id, edited);
        app.reconcile_plugins();
        assert_eq!(app.plugins.loaded[&key].editor.value(p.id), Some(edited));
        app.dispatch(Command::Undo);
        app.reconcile_plugins();
        assert_eq!(app.plugins.loaded[&key].editor.value(p.id), Some(original));
        app.dispatch(Command::Redo);
        app.reconcile_plugins();
        assert_eq!(app.plugins.loaded[&key].editor.value(p.id), Some(edited));
    }

    #[test]
    fn live_registry_can_inspect_implicit_stock_instrument() {
        let mut app = Ondera::from_session(store::empty(), None);
        app.reconcile_plugins();
        let track = app
            .store
            .session()
            .tracks
            .iter()
            .find(|t| t.kind == "midi")
            .unwrap()
            .id
            .clone();
        let result = Host::plugin_parameters(&mut app, &track, None).unwrap();
        assert_eq!(result["pluginId"], "stock:Ondera Synth");
        assert!(!result["parameters"].as_array().unwrap().is_empty());
    }
}
