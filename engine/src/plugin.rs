//! Plugin abstraction shared by the stock Ondera processors and the external
//! CLAP, VST3 and Audio Unit hosts.
//!
//! Every insert and instrument is an `Instance`: an `Editor` half that stays on
//! the main thread (parameters, state, GUI) and a `Processor` half that is
//! mounted into the audio thread's `Rack`. The rack outlives graph rebuilds, so
//! reverb tails, synth voices and external plugin state survive every edit.

use crate::Result;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Stock,
    Clap,
    Vst3,
    #[serde(rename = "au")]
    AudioUnit,
}
impl Format {
    pub fn label(self) -> &'static str {
        match self {
            Format::Stock => "Ondera",
            Format::Clap => "CLAP",
            Format::Vst3 => "VST3",
            Format::AudioUnit => "AU",
        }
    }
    pub fn prefix(self) -> &'static str {
        match self {
            Format::Stock => "stock",
            Format::Clap => "clap",
            Format::Vst3 => "vst3",
            Format::AudioUnit => "au",
        }
    }
    pub fn parse(id: &str) -> Option<(Format, &str)> {
        let (prefix, rest) = id.split_once(':')?;
        let format = match prefix {
            "stock" => Format::Stock,
            "clap" => Format::Clap,
            "vst3" => Format::Vst3,
            "au" => Format::AudioUnit,
            _ => return None,
        };
        Some((format, rest))
    }
}

/// A plugin known to the browser. `id` is stable across scans:
/// `stock:<name>`, `clap:<plugin id>`, `vst3:<class id hex>` or `au:<type>:<subtype>:<manufacturer>`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Descriptor {
    pub id: String,
    pub format: Format,
    pub name: String,
    #[serde(default)]
    pub vendor: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub instrument: bool,
    #[serde(default = "yes")]
    pub effect: bool,
    #[serde(default)]
    pub category: String,
}
fn yes() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParamInfo {
    pub id: u32,
    pub name: String,
    pub min: f64,
    pub max: f64,
    pub default: f64,
    pub unit: String,
    /// 0 for continuous parameters; otherwise the number of discrete steps.
    pub steps: u32,
    /// Frequency-like parameters read better on a logarithmic knob.
    pub log: bool,
    /// Discrete parameters may name their values.
    pub labels: Vec<String>,
}
impl ParamInfo {
    pub fn normalize(&self, value: f64) -> f64 {
        let span = self.max - self.min;
        if span <= 0.0 {
            return 0.0;
        }
        if self.log && self.min > 0.0 {
            ((value / self.min).max(1e-9).ln() / (self.max / self.min).ln()).clamp(0.0, 1.0)
        } else {
            ((value - self.min) / span).clamp(0.0, 1.0)
        }
    }
    pub fn denormalize(&self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        let mut v = if self.log && self.min > 0.0 {
            self.min * (self.max / self.min).powf(t)
        } else {
            self.min + t * (self.max - self.min)
        };
        if self.steps > 0 {
            let step = (self.max - self.min) / self.steps as f64;
            v = self.min + ((v - self.min) / step).round() * step;
        }
        v.clamp(self.min, self.max)
    }
    pub fn text(&self, value: f64) -> String {
        if !self.labels.is_empty() {
            let index = ((value - self.min).round().max(0.0) as usize).min(self.labels.len() - 1);
            return self.labels[index].clone();
        }
        let magnitude = value.abs();
        let body = if self.steps > 0 || (value.fract() == 0.0 && magnitude >= 100.0) {
            format!("{}", value.round() as i64)
        } else if magnitude >= 100.0 {
            format!("{value:.0}")
        } else if magnitude >= 10.0 {
            format!("{value:.1}")
        } else {
            format!("{value:.2}")
        };
        if self.unit.is_empty() {
            body
        } else {
            format!("{body} {}", self.unit)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoteEvent {
    pub frame: u32,
    pub on: bool,
    pub pitch: u8,
    pub velocity: u8,
    pub channel: u8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParamChange {
    pub id: u32,
    pub value: f64,
}

/// Transport information for one block, valid at the first frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ProcessContext {
    pub playing: bool,
    pub recording: bool,
    pub tempo: f64,
    pub position_beats: f64,
    pub position_seconds: f64,
    pub sample_time: i64,
    pub numerator: u32,
    pub denominator: u32,
    pub cycle: Option<(f64, f64)>,
    pub bar_start_beats: f64,
}

/// Largest block handed to a processor. Device buffers are split accordingly.
pub const MAX_BLOCK: usize = 256;

/// The audio-thread half of a plugin. Nothing here may allocate, block or log.
pub trait Processor: Send {
    /// Called on the audio thread when the processor is mounted.
    fn start(&mut self) {}
    /// Called on the audio thread before the processor leaves the rack.
    fn stop(&mut self) {}
    /// Silence tails and release voices.
    fn reset(&mut self) {}
    /// Process stereo audio in place. Instruments receive silence and add their
    /// output; effects transform. `notes` and `params` are sorted by frame.
    fn process(
        &mut self,
        audio: &mut [[f32; 2]],
        notes: &[NoteEvent],
        params: &[ParamChange],
        ctx: &ProcessContext,
    );
    fn latency(&self) -> u32 {
        0
    }
}

/// An opaque parent window handle for native plugin editors.
#[derive(Clone, Copy, Debug)]
pub enum ParentWindow {
    Cocoa(*mut std::ffi::c_void),
    Win32(*mut std::ffi::c_void),
    X11(u64),
}

/// The main-thread half of a plugin.
pub trait Editor {
    fn descriptor(&self) -> &Descriptor;
    fn params(&self) -> &[ParamInfo];
    /// Current plain value as the plugin reports it.
    fn value(&self, id: u32) -> Option<f64>;
    fn text(&self, id: u32, value: f64) -> String {
        self.params()
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.text(value))
            .unwrap_or_default()
    }
    /// Apply a parameter value on the main thread (GUI synchronisation only;
    /// the audio thread receives the same change through the rack).
    fn set_value(&mut self, _id: u32, _value: f64) {}
    fn save(&mut self) -> Option<Vec<u8>>;
    fn load(&mut self, bytes: &[u8]) -> Result<()>;
    fn has_gui(&self) -> bool {
        false
    }
    /// Create and attach the native editor. Returns its preferred size.
    fn open_gui(&mut self, _parent: ParentWindow) -> Result<(u32, u32)> {
        Err("This plugin has no editor window".into())
    }
    fn close_gui(&mut self) {}
    /// A size the plugin requested from its host since the last call.
    fn take_resize_request(&mut self) -> Option<(u32, u32)> {
        None
    }
    fn set_gui_size(&mut self, _width: u32, _height: u32) {}
    /// Periodic main-thread housekeeping (CLAP `on_main_thread`, VST3 idle).
    fn idle(&mut self) {}
    fn latency(&self) -> u32 {
        0
    }
    /// True after the plugin reported parameter or state changes from its own GUI.
    fn take_dirty(&mut self) -> bool {
        false
    }
}

/// A live plugin: its editor and, until mounted, its processor.
pub struct Instance {
    pub editor: Box<dyn Editor>,
    pub processor: Option<Box<dyn Processor>>,
}

/// Bounded pending parameter changes per rack slot.
struct Pending {
    changes: Vec<ParamChange>,
    len: usize,
}

/// The audio thread's plugin bank, addressed by slot. Preallocated so mounting
/// and processing never allocate.
pub struct Rack {
    slots: Vec<Option<Box<dyn Processor>>>,
    pending: Vec<Pending>,
}
impl Rack {
    pub fn new(capacity: usize) -> Self {
        Self::with_parameter_capacity(capacity, 512)
    }
    /// Offline restores may enqueue many saved parameters before the first
    /// block; reserve their complete document values on the calling thread.
    pub fn with_parameter_capacity(capacity: usize, parameters: usize) -> Self {
        let mut slots = Vec::with_capacity(capacity);
        let mut pending = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            slots.push(None);
            pending.push(Pending {
                changes: vec![ParamChange { id: 0, value: 0.0 }; parameters.max(1)],
                len: 0,
            });
        }
        Self { slots, pending }
    }
    pub fn capacity(&self) -> usize {
        self.slots.len()
    }
    pub fn is_mounted(&self, slot: u32) -> bool {
        self.slots.get(slot as usize).is_some_and(|s| s.is_some())
    }
    /// Mount on the audio thread; returns the previous occupant for reclamation.
    pub fn mount(
        &mut self,
        slot: u32,
        mut processor: Box<dyn Processor>,
    ) -> Option<Box<dyn Processor>> {
        let Some(place) = self.slots.get_mut(slot as usize) else {
            return Some(processor);
        };
        processor.start();
        let mut old = place.replace(processor);
        if let Some(old) = old.as_mut() {
            old.stop();
        }
        self.pending[slot as usize].len = 0;
        old
    }
    pub fn unmount(&mut self, slot: u32) -> Option<Box<dyn Processor>> {
        let mut old = self.slots.get_mut(slot as usize)?.take();
        if let Some(old) = old.as_mut() {
            old.stop();
        }
        old
    }
    pub fn set_param(&mut self, slot: u32, id: u32, value: f64) {
        let Some(p) = self.pending.get_mut(slot as usize) else {
            return;
        };
        if let Some(existing) = p.changes[..p.len].iter_mut().find(|c| c.id == id) {
            existing.value = value;
        } else if p.len < p.changes.len() {
            p.changes[p.len] = ParamChange { id, value };
            p.len += 1;
        }
    }
    /// Returns false when the slot is empty so callers can pass audio through.
    pub fn process(
        &mut self,
        slot: u32,
        audio: &mut [[f32; 2]],
        notes: &[NoteEvent],
        ctx: &ProcessContext,
    ) -> bool {
        let index = slot as usize;
        let Some(Some(processor)) = self.slots.get_mut(index) else {
            return false;
        };
        let pending = &mut self.pending[index];
        processor.process(audio, notes, &pending.changes[..pending.len], ctx);
        pending.len = 0;
        true
    }
    pub fn latency(&self, slot: u32) -> u32 {
        self.slots
            .get(slot as usize)
            .and_then(|s| s.as_ref())
            .map_or(0, |p| p.latency())
    }
    pub fn reset(&mut self, slot: u32) {
        if let Some(Some(p)) = self.slots.get_mut(slot as usize) {
            p.reset();
        }
    }
    pub fn drain(&mut self) -> Vec<Box<dyn Processor>> {
        self.slots
            .iter_mut()
            .filter_map(|s| s.take())
            .map(|mut processor| {
                processor.stop();
                processor
            })
            .collect()
    }
}

impl Drop for Rack {
    fn drop(&mut self) {
        // Offline racks also own running processors. Stop processing before the
        // last instance reference deactivates and destroys the plugin.
        for processor in self.slots.iter_mut().flatten() {
            processor.stop();
        }
    }
}

/// A gain applied at a fixed frame count per second. Shared by stock DSP.
pub fn db_to_gain(db: f64) -> f32 {
    10f64.powf(db / 20.0) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parameter_normalisation_round_trips() {
        let log = ParamInfo {
            id: 0,
            name: "Cutoff".into(),
            min: 20.0,
            max: 20000.0,
            default: 1000.0,
            unit: "Hz".into(),
            steps: 0,
            log: true,
            labels: vec![],
        };
        for v in [20.0, 100.0, 1000.0, 20000.0] {
            assert!((log.denormalize(log.normalize(v)) - v).abs() < 1e-6 * v);
        }
        let stepped = ParamInfo {
            id: 1,
            name: "Mode".into(),
            min: 0.0,
            max: 2.0,
            default: 0.0,
            unit: String::new(),
            steps: 2,
            log: false,
            labels: vec!["Low".into(), "Band".into(), "High".into()],
        };
        assert_eq!(stepped.denormalize(0.4), 1.0);
        assert_eq!(stepped.text(2.0), "High");
    }
    #[test]
    fn format_ids_parse() {
        assert_eq!(
            Format::parse("clap:org.x.y"),
            Some((Format::Clap, "org.x.y"))
        );
        assert_eq!(Format::parse("bogus"), None);
    }
}
