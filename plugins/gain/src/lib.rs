//! Two small effects that show how an Ondera native plugin is written. Build this crate
//! (`cargo build -p ondera-plugin-gain --release`), copy the resulting library into an
//! Ondera plugin directory (Settings > Plugins lists them) and rescan.

use ondera_plugin::{export_plugins, prelude::*};

/// Gain, pan and polarity: the smallest useful effect.
pub struct Trim {
    gain: Smoother,
    pan: f32,
    invert: bool,
}
impl Plugin for Trim {
    const INFO: Info = Info::effect(
        "org.ondera.examples.trim",
        "Trim",
        "Ondera Examples",
        "Utility",
    )
    .describe("Gain, pan and polarity with click-free smoothing.");
    fn params() -> Vec<ParamSpec> {
        vec![
            param("Gain", -60.0, 12.0, 0.0, "dB"),
            param("Pan", -100.0, 100.0, 0.0, ""),
            switch("Invert", false),
        ]
    }
    fn new(rate: f64) -> Self {
        Self {
            gain: Smoother::new(rate, 0.01, 1.0),
            pan: 0.0,
            invert: false,
        }
    }
    fn set_param(&mut self, index: usize, value: f64) {
        match index {
            0 => self.gain.set(db_to_gain(value)),
            1 => self.pan = (value / 100.0) as f32,
            2 => self.invert = value >= 0.5,
            _ => {}
        }
    }
    fn process(&mut self, audio: &mut [[f32; 2]], _: &[NoteEvent], _: &ProcessContext) {
        let left = (1.0 - self.pan.max(0.0)).sqrt();
        let right = (1.0 + self.pan.min(0.0)).sqrt();
        let sign = if self.invert { -1.0 } else { 1.0 };
        for frame in audio {
            let gain = self.gain.next() * sign;
            frame[0] *= gain * left;
            frame[1] *= gain * right;
        }
    }
}

/// A tilt equaliser: one knob tips the spectrum around a pivot frequency.
pub struct TiltEq {
    rate: f64,
    tilt: f64,
    pivot: f64,
    low: Biquad,
    high: Biquad,
    dirty: bool,
}
impl Plugin for TiltEq {
    const INFO: Info = Info::effect(
        "org.ondera.examples.tilt",
        "Tilt EQ",
        "Ondera Examples",
        "EQ & Filter",
    )
    .describe("Tips the spectrum darker or brighter around a pivot.");
    fn params() -> Vec<ParamSpec> {
        vec![
            param("Tilt", -12.0, 12.0, 0.0, "dB"),
            hz("Pivot", 200.0, 5000.0, 800.0),
        ]
    }
    fn new(rate: f64) -> Self {
        Self {
            rate,
            tilt: 0.0,
            pivot: 800.0,
            low: Biquad::default(),
            high: Biquad::default(),
            dirty: true,
        }
    }
    fn set_param(&mut self, index: usize, value: f64) {
        match index {
            0 => self.tilt = value,
            1 => self.pivot = value,
            _ => return,
        }
        self.dirty = true;
    }
    fn reset(&mut self) {
        self.low.clear();
        self.high.clear();
    }
    fn process(&mut self, audio: &mut [[f32; 2]], _: &[NoteEvent], _: &ProcessContext) {
        if self.dirty {
            self.low.low_shelf(self.rate, self.pivot, -self.tilt / 2.0);
            self.high.high_shelf(self.rate, self.pivot, self.tilt / 2.0);
            self.dirty = false;
        }
        for frame in audio {
            for (c, s) in frame.iter_mut().enumerate() {
                *s = self.high.process(c, self.low.process(c, *s));
            }
        }
    }
}

export_plugins!(Trim, TiltEq);
