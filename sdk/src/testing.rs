//! A bench for plugin authors: drive a plugin through the same C ABI the host uses and
//! make assertions about what comes out. Nothing here touches an audio device.
//!
//! ```
//! use ondera_plugin::{prelude::*, testing::Bench};
//! # struct Gain(f32);
//! # impl Plugin for Gain {
//! #     const INFO: Info = Info::effect("com.example.gain", "Gain", "Example", "Utility");
//! #     fn params() -> Vec<ParamSpec> { vec![param("Gain", -24.0, 24.0, 0.0, "dB")] }
//! #     fn new(_: f64) -> Self { Self(1.0) }
//! #     fn set_param(&mut self, _: usize, v: f64) { self.0 = db_to_gain(v) }
//! #     fn process(&mut self, audio: &mut [[f32; 2]], _: &[NoteEvent], _: &ProcessContext) {
//! #         for f in audio { f[0] *= self.0; f[1] *= self.0; }
//! #     }
//! # }
//! let mut bench = Bench::<Gain>::new(48_000.0);
//! bench.set("Gain", -6.0);
//! let out = bench.sine(440.0, 0.5, 0.1);
//! assert!((Bench::<Gain>::peak(&out) - 0.5 * 0.501).abs() < 0.01);
//! ```
use crate::{
    ffi::{self, PluginVTable, RawContext},
    plugin::{NoteEvent, ParamSpec, Plugin, ProcessContext},
    MAX_BLOCK,
};
use std::{ffi::c_void, marker::PhantomData};

/// One instance of `P` behind its vtable.
pub struct Bench<P: Plugin> {
    table: PluginVTable,
    instance: *mut c_void,
    specs: Vec<ParamSpec>,
    rate: f64,
    /// Transport handed to the plugin; advance or edit it between renders.
    pub context: ProcessContext,
    _plugin: PhantomData<P>,
}
impl<P: Plugin> Bench<P> {
    pub fn new(rate: f64) -> Self {
        let table = ffi::vtable::<P>();
        let instance = unsafe { (table.create)(rate) };
        assert!(!instance.is_null(), "{} failed to construct", P::INFO.name);
        let specs = P::params();
        for (index, spec) in specs.iter().enumerate() {
            unsafe { (table.set_param)(instance, index as u32, spec.default) };
        }
        Self {
            table,
            instance,
            specs,
            rate,
            context: ProcessContext {
                playing: true,
                tempo: 120.0,
                numerator: 4,
                denominator: 4,
                ..ProcessContext::default()
            },
            _plugin: PhantomData,
        }
    }
    /// Set a parameter by its display name. Panics on an unknown name or a value out of range,
    /// because a test that sets nothing proves nothing.
    pub fn set(&mut self, name: &str, value: f64) -> &mut Self {
        let index = self
            .specs
            .iter()
            .position(|s| s.name == name)
            .unwrap_or_else(|| panic!("{} has no parameter `{name}`", P::INFO.name));
        let spec = &self.specs[index];
        assert!(
            (spec.min..=spec.max).contains(&value),
            "{name} = {value} is outside {}..={}",
            spec.min,
            spec.max
        );
        unsafe { (self.table.set_param)(self.instance, index as u32, value) };
        self
    }
    pub fn reset(&mut self) {
        unsafe { (self.table.reset)(self.instance) };
    }
    pub fn latency(&self) -> u32 {
        unsafe { (self.table.latency)(self.instance) }
    }
    /// Process `audio` in place in host-sized blocks. Notes carry frame offsets from the
    /// start of `audio`; the transport position advances as it would while playing.
    pub fn process(&mut self, audio: &mut [[f32; 2]], notes: &[NoteEvent]) {
        let mut start = 0usize;
        for block in audio.chunks_mut(MAX_BLOCK) {
            let end = start + block.len();
            let local: Vec<NoteEvent> = notes
                .iter()
                .filter(|n| (start..end).contains(&(n.frame as usize)))
                .map(|n| NoteEvent {
                    frame: n.frame - start as u32,
                    ..*n
                })
                .collect();
            let raw = RawContext::from(&self.context);
            unsafe {
                (self.table.process)(
                    self.instance,
                    block.as_mut_ptr(),
                    block.len() as u32,
                    local.as_ptr(),
                    local.len() as u32,
                    &raw,
                );
            }
            let seconds = block.len() as f64 / self.rate;
            self.context.sample_time += block.len() as i64;
            self.context.position_seconds += seconds;
            self.context.position_beats += seconds * self.context.tempo / 60.0;
            start = end;
        }
    }
    /// Feed a sine of `amplitude` for `seconds` and return the result.
    pub fn sine(&mut self, hz: f64, amplitude: f32, seconds: f64) -> Vec<[f32; 2]> {
        let mut audio: Vec<[f32; 2]> = (0..(seconds * self.rate) as usize)
            .map(|i| {
                let v =
                    (std::f64::consts::TAU * hz * i as f64 / self.rate).sin() as f32 * amplitude;
                [v, v]
            })
            .collect();
        self.process(&mut audio, &[]);
        audio
    }
    /// Feed silence for `seconds`: what is left is the plugin's tail, or an instrument's notes.
    pub fn silence(&mut self, seconds: f64, notes: &[NoteEvent]) -> Vec<[f32; 2]> {
        let mut audio = vec![[0.0; 2]; (seconds * self.rate) as usize];
        self.process(&mut audio, notes);
        audio
    }
    /// Play one note for `held` seconds and keep rendering until `seconds`.
    pub fn note(&mut self, pitch: u8, velocity: u8, held: f64, seconds: f64) -> Vec<[f32; 2]> {
        let off = (held * self.rate) as u32;
        let on = NoteEvent {
            frame: 0,
            on: true,
            pitch,
            velocity,
            channel: 0,
        };
        self.silence(
            seconds,
            &[
                on,
                NoteEvent {
                    frame: off,
                    on: false,
                    ..on
                },
            ],
        )
    }
    pub fn peak(audio: &[[f32; 2]]) -> f32 {
        audio.iter().flatten().fold(0.0, |m, v| m.max(v.abs()))
    }
    pub fn rms(audio: &[[f32; 2]]) -> f32 {
        let sum: f64 = audio.iter().flatten().map(|v| (*v as f64).powi(2)).sum();
        (sum / (audio.len().max(1) * 2) as f64).sqrt() as f32
    }
    /// Every sample is a number and none is absurdly loud: the two ways a plugin ruins a mix.
    pub fn assert_sane(audio: &[[f32; 2]]) {
        for (i, frame) in audio.iter().enumerate() {
            for v in frame {
                assert!(
                    v.is_finite(),
                    "{} produced a non-finite sample at frame {i}",
                    P::INFO.name
                );
                assert!(v.abs() < 16.0, "{} produced {v} at frame {i}", P::INFO.name);
            }
        }
    }
}
impl<P: Plugin> Drop for Bench<P> {
    fn drop(&mut self) {
        unsafe { (self.table.destroy)(self.instance) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;

    struct Fragile {
        armed: bool,
    }
    impl Plugin for Fragile {
        const INFO: Info = Info::effect("test.fragile", "Fragile", "Test", "Utility");
        fn params() -> Vec<ParamSpec> {
            vec![switch("Explode", false)]
        }
        fn new(_: f64) -> Self {
            Self { armed: false }
        }
        fn set_param(&mut self, _: usize, value: f64) {
            self.armed = value >= 0.5;
        }
        fn process(&mut self, audio: &mut [[f32; 2]], _: &[NoteEvent], _: &ProcessContext) {
            assert!(!self.armed, "plugin bug");
            for frame in audio {
                frame[0] *= 0.5;
                frame[1] *= 0.5;
            }
        }
    }

    #[test]
    fn a_panicking_plugin_is_contained_and_passes_audio_through() {
        let mut bench = Bench::<Fragile>::new(48_000.0);
        let healthy = bench.sine(440.0, 1.0, 0.01);
        assert!((Bench::<Fragile>::peak(&healthy) - 0.5).abs() < 0.01);
        // Silence the default hook so the expected panic does not clutter the test log.
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        bench.set("Explode", 1.0);
        let after = bench.sine(440.0, 1.0, 0.01);
        std::panic::set_hook(hook);
        assert!(
            (Bench::<Fragile>::peak(&after) - 1.0).abs() < 0.01,
            "audio passes through"
        );
        // Poisoned for good: fixing the parameter does not revive undefined state.
        bench.set("Explode", 0.0);
        assert!((Bench::<Fragile>::peak(&bench.sine(440.0, 1.0, 0.01)) - 1.0).abs() < 0.01);
        assert_eq!(bench.latency(), 0);
    }

    #[test]
    fn notes_are_delivered_with_block_relative_offsets() {
        struct Counter(u32);
        impl Plugin for Counter {
            const INFO: Info = Info::instrument("test.counter", "Counter", "Test");
            fn params() -> Vec<ParamSpec> {
                vec![]
            }
            fn new(_: f64) -> Self {
                Self(0)
            }
            fn set_param(&mut self, _: usize, _: f64) {}
            fn process(&mut self, audio: &mut [[f32; 2]], notes: &[NoteEvent], _: &ProcessContext) {
                for n in notes {
                    assert!((n.frame as usize) < audio.len());
                    if n.on {
                        self.0 += 1;
                        audio[n.frame as usize][0] = 1.0;
                    }
                }
            }
        }
        let mut bench = Bench::<Counter>::new(48_000.0);
        let out = bench.note(60, 100, 0.05, 0.1);
        assert_eq!(out.iter().filter(|f| f[0] == 1.0).count(), 1);
        assert_eq!(out[0][0], 1.0);
    }
}
