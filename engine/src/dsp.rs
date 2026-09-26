//! Native synthesis primitives for the stock instruments. Filters, delays and gain helpers
//! come from the plugin SDK so third-party native plugins share the same building blocks.

pub use ondera_plugin::dsp::{
    coef, db, db_to_gain, saw, square, triangle, Biquad, Delay, Smoother, Svf,
};
use std::f64::consts::TAU;

pub const INSTRUMENTS: [&str; 11] = [
    "Ondera Synth",
    "E-Piano Mk I",
    "Drum Machine",
    "Sampler",
    "Sub Bass 808",
    "Glass Keys",
    "Choir Pad",
    "Riser",
    "Tonewheel Organ",
    "String Ensemble",
    "Analog Bass",
];
pub const EFFECTS: [&str; 23] = [
    "Ondera Comp",
    "Channel EQ",
    "Tape Sat",
    "Chorus",
    "Space",
    "Echo",
    "Gate",
    "Limiter",
    "Filter",
    "Phaser",
    "Tremolo",
    "Bitcrusher",
    "Stereo Width",
    "Utility",
    "Overdrive",
    "Transient",
    "Flanger",
    "Auto Pan",
    "Auto Filter",
    "De-Esser",
    "Lo-Fi",
    "Pitch Shift",
    "Pump",
];

/// Every stock voice is scaled by this, so a chord or a full drum hit at unity gain leaves
/// room on the master instead of clipping it (a five-note E-piano chord at velocity 110 peaked
/// at +5 dBFS, the Four Floor loop at +3 dBFS).
pub const HEADROOM: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    Synth,
    Piano,
    Drums,
    Pluck,
    Sub,
    Bell,
    Pad,
    Riser,
    Organ,
    Strings,
    Bass,
}
impl Preset {
    pub fn named(name: &str) -> Self {
        match name {
            "E-Piano Mk I" => Self::Piano,
            "Drum Machine" => Self::Drums,
            "Sampler" => Self::Pluck,
            "Sub Bass 808" => Self::Sub,
            "Glass Keys" => Self::Bell,
            "Choir Pad" => Self::Pad,
            "Riser" => Self::Riser,
            "Tonewheel Organ" => Self::Organ,
            "String Ensemble" => Self::Strings,
            "Analog Bass" => Self::Bass,
            _ => Self::Synth,
        }
    }
    pub fn release(self) -> f64 {
        match self {
            Self::Pad => 0.9,
            Self::Strings => 0.7,
            Self::Organ => 0.06,
            Self::Bass => 0.12,
            Self::Bell => 1.0,
            Self::Drums => 1.25,
            _ => 0.3,
        }
    }
    pub fn attack(self) -> f64 {
        match self {
            Self::Pad => 0.35,
            Self::Strings => 0.18,
            Self::Synth => 0.01,
            Self::Riser => 0.02,
            _ => 0.003,
        }
    }
    pub fn cutoff(self) -> f64 {
        match self {
            Self::Pad => 1200.0,
            Self::Strings => 3200.0,
            Self::Bass => 700.0,
            Self::Synth => 2200.0,
            _ => 16000.0,
        }
    }
}

/// Instrument settings, in plain units. Stock instruments derive these from
/// their parameters; the defaults reproduce the original presets.
#[derive(Clone, Copy, Debug)]
pub struct InstrumentParams {
    pub level: f32,
    pub attack: f64,
    pub decay: f64,
    pub sustain: f64,
    pub release: f64,
    pub cutoff: f64,
    pub env_amount: f64,
    pub detune: f64,
    pub wave: u8,
    /// Frequency multiplier from pitch bend; 1 when the wheel is centred.
    pub pitch_ratio: f64,
    /// Vibrato depth 0-1 from the mod wheel or pressure; about ten cents at full depth.
    pub vibrato: f64,
}
impl InstrumentParams {
    pub fn for_preset(preset: Preset) -> Self {
        Self {
            level: 1.0,
            attack: preset.attack(),
            decay: 0.25,
            sustain: 0.6,
            release: preset.release(),
            cutoff: preset.cutoff(),
            env_amount: 0.4,
            detune: 8.0,
            wave: 0,
            pitch_ratio: 1.0,
            vibrato: 0.0,
        }
    }
}

/// Mod-wheel vibrato: rate in Hz and depth as a frequency ratio (about ten cents).
const VIBRATO_HZ: f64 = 5.5;
const VIBRATO_DEPTH: f64 = 0.0058;

#[derive(Clone, Copy)]
pub struct Voice {
    pub preset: Preset,
    pub pitch: u8,
    pub velocity: f32,
    pub age: f64,
    pub released: Option<f64>,
    pub serial: u64,
    phase: f64,
    phase2: f64,
    low: f32,
    seed: u32,
    frequency_pitch: u8,
    frequency: f64,
    pad_rate: f64,
    pad_cutoff: f64,
    pad_alpha: f32,
}
impl Voice {
    pub fn new(preset: Preset, pitch: u8, velocity: u8, serial: u64) -> Self {
        Self {
            preset,
            pitch,
            velocity: velocity as f32 / 127.0,
            age: 0.0,
            released: None,
            serial,
            phase: 0.0,
            phase2: 0.0,
            low: 0.0,
            seed: 12345 + pitch as u32 + serial as u32 * 7,
            frequency_pitch: pitch,
            frequency: 440.0 * 2.0_f64.powf((pitch as f64 - 69.0) / 12.0),
            pad_rate: 0.0,
            pad_cutoff: 0.0,
            pad_alpha: 0.0,
        }
    }
    pub fn release(&mut self) {
        if self.released.is_none() {
            self.released = Some(self.age);
        }
    }
    pub fn held(&self) -> bool {
        self.released.is_none()
    }
    pub fn finished(&self, p: &InstrumentParams) -> bool {
        match self.preset {
            Preset::Drums => self.age > 2.5,
            _ => self
                .released
                .is_some_and(|t| self.age - t > p.release * 1.6 + 0.01),
        }
    }
    /// One mono sample; advances the voice by one frame.
    #[inline]
    pub fn sample(&mut self, p: &InstrumentParams, rate: f64) -> f32 {
        // Pitch and pad cutoff stay constant for many samples. Keep the same
        // formulas and invalidate on changes, including public pitch edits and
        // sample-rate changes; modulation and envelopes remain sample-accurate.
        if self.frequency_pitch != self.pitch {
            self.frequency_pitch = self.pitch;
            self.frequency = 440.0 * 2.0_f64.powf((self.pitch as f64 - 69.0) / 12.0);
        }
        if self.preset == Preset::Pad && (self.pad_rate != rate || self.pad_cutoff != p.cutoff) {
            self.pad_rate = rate;
            self.pad_cutoff = p.cutoff;
            self.pad_alpha = (1.0 - (-TAU * p.cutoff.min(rate * 0.45) / rate).exp()) as f32;
        }
        let mut hz = self.frequency * p.pitch_ratio;
        if p.vibrato > 0.0 {
            hz *= 1.0 + p.vibrato * VIBRATO_DEPTH * (TAU * VIBRATO_HZ * self.age).sin();
        }
        let hz = hz.min(rate * 0.4);
        self.sample_with_coefficients(p, rate, hz, self.pad_alpha)
    }
    #[inline]
    fn sample_with_coefficients(
        &mut self,
        p: &InstrumentParams,
        rate: f64,
        hz: f64,
        pad_alpha: f32,
    ) -> f32 {
        let age = self.age;
        self.age += 1.0 / rate;
        let cents = 1.0 + p.detune / 100.0 / 12.0 * 0.06;
        let dt = hz / rate;
        self.phase = (self.phase + dt).fract();
        // The bass runs its second oscillator an octave down; everything else detunes it.
        let second = if self.preset == Preset::Bass {
            0.5
        } else {
            cents
        };
        self.phase2 = (self.phase2 + dt * second).fract();
        self.seed = self.seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = self.seed as f64 / 2147483648.0 - 1.0;
        let release = match (self.preset, self.released) {
            (Preset::Drums, _) | (_, None) => 1.0,
            (_, Some(t)) => (-(age - t) * 8.0 / p.release.max(0.005)).exp(),
        };
        let attack = (age / p.attack.max(0.0005)).min(1.0);
        let (value, env, gain) = match self.preset {
            Preset::Synth => {
                let osc = match p.wave {
                    1 => square(self.phase, dt),
                    2 => triangle(self.phase),
                    _ => saw(self.phase, dt),
                };
                // ADSR with the envelope opening the filter.
                let adsr = if age < p.attack {
                    attack
                } else {
                    let d = (age - p.attack) / p.decay.max(0.001);
                    p.sustain + (1.0 - p.sustain) * (-d * 4.0).exp()
                };
                let cutoff = (p.cutoff * (1.0 + p.env_amount * 4.0 * adsr)).min(rate * 0.45);
                let alpha = (1.0 - (-TAU * cutoff / rate).exp()) as f32;
                self.low += alpha * (osc as f32 - self.low);
                (self.low as f64, adsr, 0.32)
            }
            Preset::Pad => {
                let osc = saw(self.phase, dt) + saw(self.phase2, dt * cents) * 0.5;
                self.low += pad_alpha * (osc as f32 - self.low);
                (
                    self.low as f64,
                    attack * (0.6 + 0.4 * (-age * 8.0).exp()),
                    0.28,
                )
            }
            Preset::Sub => (
                (TAU * self.phase).sin(),
                attack * (0.9 + 0.1 * (-age * 10.0).exp()),
                0.7,
            ),
            Preset::Piano | Preset::Bell => {
                let bell = matches!(self.preset, Preset::Bell);
                let ratio = if bell { 3.5 } else { 1.0 };
                let modulation = (TAU * hz * ratio * age).sin()
                    * if bell { 2.5 } else { 1.8 }
                    * (-age * 2.0).exp();
                (
                    (TAU * self.phase + modulation).sin(),
                    attack * (0.25 + 0.75 * (-age * if bell { 0.9 } else { 1.8 }).exp()),
                    0.5,
                )
            }
            Preset::Organ => {
                // Drawbars 16', 8', 5 1/3', 4' and 2 2/3' with a touch of key click.
                let t = TAU * self.phase;
                let tone = (t * 0.5).sin() * 0.55
                    + t.sin()
                    + (t * 1.5).sin() * 0.35
                    + (t * 2.0).sin() * 0.5
                    + (t * 3.0).sin() * 0.22;
                let click = noise * 0.25 * (-age * 220.0).exp();
                (tone * 0.4 + click, attack, 0.42)
            }
            Preset::Strings => {
                let vibrato = 1.0 + 0.0025 * (TAU * 5.2 * age).sin() * (age * 2.0).min(1.0);
                let osc = saw(self.phase, dt)
                    + saw(self.phase2, dt * cents) * 0.8
                    + saw((self.phase * vibrato + 0.37).fract(), dt) * 0.6;
                let cutoff = p.cutoff.min(rate * 0.45);
                let alpha = (1.0 - (-TAU * cutoff / rate).exp()) as f32;
                self.low += alpha * (osc as f32 * 0.42 - self.low);
                (
                    self.low as f64,
                    attack * (0.85 + 0.15 * (-age * 3.0).exp()),
                    0.4,
                )
            }
            Preset::Bass => {
                let osc = saw(self.phase, dt) * 0.7 + square(self.phase2, dt * 0.5) * 0.6;
                let adsr = if age < p.attack {
                    attack
                } else {
                    let d = (age - p.attack) / p.decay.max(0.001);
                    p.sustain + (1.0 - p.sustain) * (-d * 4.0).exp()
                };
                let cutoff = (p.cutoff * (1.0 + p.env_amount * 6.0 * adsr)).min(rate * 0.45);
                let alpha = (1.0 - (-TAU * cutoff / rate).exp()) as f32;
                self.low += alpha * (osc as f32 - self.low);
                ((self.low as f64 * 1.4).tanh(), adsr, 0.55)
            }
            Preset::Pluck => (
                (self.phase * 2.0 - 1.0).abs() * 2.0 - 1.0,
                attack * (-age * 16.0).exp(),
                0.5,
            ),
            Preset::Riser => {
                self.low += 0.2 * (noise as f32 - self.low);
                let rise = (age / 4.0).min(1.0);
                (noise - self.low as f64, attack * (0.15 + 0.85 * rise), 0.4)
            }
            Preset::Drums => {
                let class = self.pitch % 12;
                if self.pitch == 35 || class == 0 {
                    let phase = 45.0 * age + 115.0 / 18.0 * (1.0 - (-age * 18.0).exp());
                    ((TAU * phase).sin(), (-age * 15.0).exp(), 0.9)
                } else if class == 2 || class == 3 || class == 4 {
                    (
                        noise * 0.6 + (TAU * 190.0 * age).sin() * 0.4,
                        (-age * 25.0).exp(),
                        0.8,
                    )
                } else {
                    self.low += 0.65 * (noise as f32 - self.low);
                    (
                        noise - self.low as f64,
                        (-age
                            * if class == 1 {
                                5.0
                            } else if class == 10 {
                                16.0
                            } else {
                                70.0
                            })
                        .exp(),
                        0.65,
                    )
                }
            }
        };
        (value * env * gain * release) as f32 * self.velocity * p.level * HEADROOM
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_voices_match_per_sample_coefficients_through_note_and_parameter_changes() {
        for preset in [
            Preset::Synth,
            Preset::Piano,
            Preset::Drums,
            Preset::Pluck,
            Preset::Sub,
            Preset::Bell,
            Preset::Pad,
            Preset::Riser,
            Preset::Organ,
            Preset::Strings,
            Preset::Bass,
        ] {
            for pitch in [0, 35, 38, 42, 60, 69, 84, 127] {
                let mut cached = Voice::new(preset, pitch, 97, 11);
                let mut reference = cached;
                let mut params = InstrumentParams::for_preset(preset);
                for frame in 0..8192 {
                    // Invalidate at non-block-aligned positions and while released.
                    // High notes also exercise the sample-rate frequency clamp.
                    let rate = if frame < 1537 {
                        44100.0
                    } else if frame < 4099 {
                        96000.0
                    } else {
                        48000.0
                    };
                    if frame == 257 || frame == 5003 {
                        params.cutoff = if frame == 257 { 731.25 } else { 19000.0 };
                        params.detune = 31.5;
                        params.attack = 0.007;
                        params.release = 0.12;
                    }
                    if frame == 2017 {
                        cached.pitch = pitch.wrapping_add(7);
                        reference.pitch = cached.pitch;
                    }
                    if frame == 3001 {
                        cached.release();
                        reference.release();
                    }
                    // This is the original per-sample coefficient calculation;
                    // both paths feed the unchanged synthesis arithmetic.
                    let hz = (440.0 * 2.0_f64.powf((reference.pitch as f64 - 69.0) / 12.0))
                        .min(rate * 0.4);
                    let alpha = (1.0 - (-TAU * params.cutoff.min(rate * 0.45) / rate).exp()) as f32;
                    let expected = reference.sample_with_coefficients(&params, rate, hz, alpha);
                    let actual = cached.sample(&params, rate);
                    assert_eq!(
                        actual.to_bits(),
                        expected.to_bits(),
                        "{preset:?}, pitch {pitch}, frame {frame}"
                    );
                }
            }
        }
    }
}
