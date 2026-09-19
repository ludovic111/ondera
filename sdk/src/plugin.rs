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

/// What an [`Event`] carries. Plain numbers rather than a Rust enum, because the value crosses
/// the C ABI and a host may one day send a kind this SDK has not heard of.
pub mod event {
    pub const NOTE_OFF: u8 = 0;
    pub const NOTE_ON: u8 = 1;
    /// `key` is the controller number, `value` its 0-127 position.
    pub const CONTROL: u8 = 2;
    /// `bend` is -8192..=8191; `Event::bend_amount` gives -1..1.
    pub const PITCH_BEND: u8 = 3;
    /// `value` is the pressure on the whole channel.
    pub const CHANNEL_PRESSURE: u8 = 4;
    /// `key` is the pitch, `value` its pressure.
    pub const POLY_PRESSURE: u8 = 5;
}

/// One MIDI-like event at `frame` within the current block (ABI 2). Events arrive sorted
/// by frame. Ignore kinds you do not know.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Event {
    pub frame: u32,
    /// One of the [`event`] constants.
    pub kind: u8,
    pub channel: u8,
    /// Pitch for notes and poly pressure, controller number for `CONTROL`.
    pub key: u8,
    /// Velocity, controller value or pressure, 0-127.
    pub value: u8,
    /// Pitch bend, -8192..=8191; zero for every other kind.
    pub bend: i16,
    pub reserved: u16,
}
impl Event {
    pub const fn note_on(frame: u32, pitch: u8, velocity: u8) -> Self {
        Self::new(frame, event::NOTE_ON, 0, pitch, velocity, 0)
    }
    pub const fn note_off(frame: u32, pitch: u8) -> Self {
        Self::new(frame, event::NOTE_OFF, 0, pitch, 0, 0)
    }
    pub const fn control(frame: u32, controller: u8, value: u8) -> Self {
        Self::new(frame, event::CONTROL, 0, controller, value, 0)
    }
    /// `amount` from -1 (full down) to 1 (full up).
    pub fn pitch_bend(frame: u32, amount: f32) -> Self {
        let bend = (amount.clamp(-1.0, 1.0) * 8192.0)
            .round()
            .clamp(-8192.0, 8191.0) as i16;
        Self::new(frame, event::PITCH_BEND, 0, 0, 0, bend)
    }
    pub const fn channel_pressure(frame: u32, pressure: u8) -> Self {
        Self::new(frame, event::CHANNEL_PRESSURE, 0, 0, pressure, 0)
    }
    pub const fn poly_pressure(frame: u32, pitch: u8, pressure: u8) -> Self {
        Self::new(frame, event::POLY_PRESSURE, 0, pitch, pressure, 0)
    }
    pub const fn new(frame: u32, kind: u8, channel: u8, key: u8, value: u8, bend: i16) -> Self {
        Self {
            frame,
            kind,
            channel,
            key,
            value,
            bend,
            reserved: 0,
        }
    }
    pub const fn on_channel(mut self, channel: u8) -> Self {
        self.channel = channel;
        self
    }
    /// Pitch bend as -1..1.
    pub fn bend_amount(&self) -> f32 {
        (self.bend as f32 / 8192.0).clamp(-1.0, 1.0)
    }
    /// The note this event is, if it is one.
    pub fn as_note(&self) -> Option<NoteEvent> {
        (self.kind == event::NOTE_ON || self.kind == event::NOTE_OFF).then_some(NoteEvent {
            frame: self.frame,
            on: self.kind == event::NOTE_ON,
            pitch: self.key,
            velocity: self.value,
            channel: self.channel,
        })
    }
}
impl From<NoteEvent> for Event {
    fn from(note: NoteEvent) -> Self {
        Self::new(
            note.frame,
            if note.on {
                event::NOTE_ON
            } else {
                event::NOTE_OFF
            },
            note.channel,
            note.pitch,
            note.velocity,
            0,
        )
    }
}

/// A parameter value that takes effect at `frame` within the current block (ABI 2).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TimedParam {
    pub frame: u32,
    pub index: u32,
    pub value: f64,
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
    /// Latency in frames, for delay compensation. Return a new value whenever it changes:
    /// the host is told after the block in which it did, and rebuilds its compensation.
    fn latency(&self) -> u32 {
        0
    }
    /// The whole block as ABI 2 delivers it: every event kind, and parameter changes at their
    /// frames. The default serves a plugin written against `process`: it splits the block at
    /// each parameter change, so `set_param` lands on the right sample, and passes the notes
    /// on. Override it to read controllers, pitch bend and pressure. Audio thread.
    fn process_events(
        &mut self,
        audio: &mut [[f32; 2]],
        events: &[Event],
        params: &[TimedParam],
        ctx: &ProcessContext,
    ) {
        split_at_params(self, audio, events, params, ctx);
    }
    /// Anything worth keeping that is not a parameter. Called on the main thread, on an
    /// instance that never processes audio; parameters are saved by the host already.
    fn save(&self) -> Vec<u8> {
        Vec::new()
    }
    /// Restore what `save` wrote, possibly by an older version of the plugin. Main thread,
    /// before the instance reaches the audio thread, so it may allocate.
    fn load(&mut self, _state: &[u8]) -> Result<(), String> {
        Ok(())
    }
    /// How long the output keeps sounding after the input stops; `f64::INFINITY` for a
    /// plugin that never falls silent on its own.
    fn tail_seconds(&self) -> f64 {
        0.0
    }
}

/// Notes one sub-block can hold on the stack; hosts send far fewer.
const NOTE_BUFFER: usize = 512;

/// `process` run over the stretches between parameter changes. No allocation.
pub fn split_at_params<P: Plugin + ?Sized>(
    plugin: &mut P,
    audio: &mut [[f32; 2]],
    events: &[Event],
    params: &[TimedParam],
    ctx: &ProcessContext,
) {
    let frames = audio.len();
    let mut notes = [NoteEvent {
        frame: 0,
        on: false,
        pitch: 0,
        velocity: 0,
        channel: 0,
    }; NOTE_BUFFER];
    let (mut start, mut next_param, mut next_event) = (0usize, 0usize, 0usize);
    loop {
        while let Some(change) = params.get(next_param) {
            if change.frame as usize > start && start < frames {
                break;
            }
            if change.value.is_finite() {
                plugin.set_param(change.index as usize, change.value);
            }
            next_param += 1;
        }
        if start >= frames {
            break;
        }
        let end = params
            .get(next_param)
            .map_or(frames, |change| (change.frame as usize).min(frames));
        let mut count = 0;
        while let Some(event) = events.get(next_event) {
            if event.frame as usize >= end && end < frames {
                break;
            }
            if let Some(mut note) = event.as_note() {
                if count < NOTE_BUFFER {
                    note.frame = (note.frame as usize).clamp(start, end - 1) as u32 - start as u32;
                    notes[count] = note;
                    count += 1;
                }
            }
            next_event += 1;
        }
        let mut local = *ctx;
        if start > 0 {
            local.sample_time += start as i64;
        }
        plugin.process(&mut audio[start..end], &notes[..count], &local);
        start = end;
    }
}
