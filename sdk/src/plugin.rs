//! The plugin contract: metadata, parameters, events and the trait itself.

use serde::{Deserialize, Serialize};

/// Whether a plugin generates audio from notes or transforms audio.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Effect,
    Instrument,
}

/// Static identity of a plugin. `id` must be stable for the life of the plugin: it is
/// stored in every session that uses the plugin, as `native:<id>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Info {
    /// Reverse-DNS style identifier, for example `com.example.gain`.
    pub id: &'static str,
    pub name: &'static str,
    pub vendor: &'static str,
    pub version: &'static str,
    /// Browser grouping: Dynamics, EQ & Filter, Distortion, Modulation, Space & Time, Utility,
    /// Instrument, or any label of your own.
    pub category: &'static str,
    pub description: &'static str,
    pub kind: Kind,
}
impl Info {
    pub const fn effect(
        id: &'static str,
        name: &'static str,
        vendor: &'static str,
        category: &'static str,
    ) -> Self {
        Self {
            id,
            name,
            vendor,
            version: "1.0.0",
            category,
            description: "",
            kind: Kind::Effect,
        }
    }
    pub const fn instrument(id: &'static str, name: &'static str, vendor: &'static str) -> Self {
        Self {
            id,
            name,
            vendor,
            version: "1.0.0",
            category: "Instrument",
            description: "",
            kind: Kind::Instrument,
        }
    }
    pub const fn version(mut self, version: &'static str) -> Self {
        self.version = version;
        self
    }
    pub const fn describe(mut self, description: &'static str) -> Self {
        self.description = description;
        self
    }
}

/// One automatable parameter. Values are plain units (dB, Hz, %, ms); Ondera stores them
/// in the document and hands them back through `set_param`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct ParamSpec {
    pub name: &'static str,
    pub min: f64,
    pub max: f64,
    pub default: f64,
    pub unit: &'static str,
    /// 0 for continuous parameters; otherwise the number of discrete steps.
    pub steps: u32,
    /// Frequency-like parameters read better on a logarithmic knob.
    pub log: bool,
    /// Discrete parameters may name their values.
    pub labels: &'static [&'static str],
}
/// A continuous parameter.
pub const fn param(
    name: &'static str,
    min: f64,
    max: f64,
    default: f64,
    unit: &'static str,
) -> ParamSpec {
    ParamSpec {
        name,
        min,
        max,
        default,
        unit,
        steps: 0,
        log: false,
        labels: &[],
    }
}
/// A frequency in Hz on a logarithmic scale.
pub const fn hz(name: &'static str, min: f64, max: f64, default: f64) -> ParamSpec {
    ParamSpec {
        name,
        min,
        max,
        default,
        unit: "Hz",
        steps: 0,
        log: true,
        labels: &[],
    }
}
/// A choice among named values; the value is the index.
pub const fn choice(
    name: &'static str,
    labels: &'static [&'static str],
    default: usize,
) -> ParamSpec {
    ParamSpec {
        name,
        min: 0.0,
        max: (labels.len() - 1) as f64,
        default: default as f64,
        unit: "",
        steps: (labels.len() - 1) as u32,
        log: false,
        labels,
    }
}
/// An on/off switch (0 = off, 1 = on).
pub const fn switch(name: &'static str, default: bool) -> ParamSpec {
    choice(name, &["Off", "On"], default as usize)
}

/// A note on or off at `frame` within the current block. Instruments receive these
/// sorted by frame.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoteEvent {
    pub frame: u32,
    pub on: bool,
    pub pitch: u8,
    pub velocity: u8,
    pub channel: u8,
}

/// A parameter value the host applies before a block.
#[repr(C)]
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
    /// Cycle range in beats while cycling.
    pub cycle: Option<(f64, f64)>,
    pub bar_start_beats: f64,
}

/// A stereo instrument or effect. `process` runs on the audio thread: no allocation,
/// blocking, I/O or logging. Instruments receive silence and add their output; effects
/// transform the buffer in place. Parameter values arrive through `set_param` before the
/// block they apply to, in plain units, always within the declared range.
pub trait Plugin: Send + 'static {
    const INFO: Info;
    /// Parameter list, in id order. Called when the plugin is scanned and instantiated.
    fn params() -> Vec<ParamSpec>;
    fn new(sample_rate: f64) -> Self;
    fn set_param(&mut self, index: usize, value: f64);
    fn process(&mut self, audio: &mut [[f32; 2]], notes: &[NoteEvent], ctx: &ProcessContext);
    /// Silence tails and release voices.
    fn reset(&mut self) {}
    /// Latency in frames, for delay compensation.
    fn latency(&self) -> u32 {
        0
    }
}
