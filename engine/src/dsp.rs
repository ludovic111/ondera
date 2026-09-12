//! Native synthesis and filter primitives shared by the stock plugins.

use std::f64::consts::TAU;

pub const INSTRUMENTS: [&str; 8] = [
    "Ondera Synth",
    "E-Piano Mk I",
    "Drum Machine",
    "Sampler",
    "Sub Bass 808",
    "Glass Keys",
    "Choir Pad",
    "Riser",
];
pub const EFFECTS: [&str; 16] = [
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
];

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
            _ => Self::Synth,
        }
    }
    pub fn release(self) -> f64 {
        match self {
            Self::Pad => 0.9,
            Self::Bell => 1.0,
            Self::Drums => 1.25,
            _ => 0.3,
        }
    }
    pub fn attack(self) -> f64 {
        match self {
            Self::Pad => 0.35,
            Self::Synth => 0.01,
            Self::Riser => 0.02,
            _ => 0.003,
        }
    }
    pub fn cutoff(self) -> f64 {
        match self {
            Self::Pad => 1200.0,
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
        }
    }
}

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
    pub fn sample(&mut self, p: &InstrumentParams, rate: f64) -> f32 {
        let age = self.age;
        self.age += 1.0 / rate;
        let cents = 1.0 + p.detune / 100.0 / 12.0 * 0.06;
        let hz = (440.0 * 2.0_f64.powf((self.pitch as f64 - 69.0) / 12.0)).min(rate * 0.4);
        let dt = hz / rate;
        self.phase = (self.phase + dt).fract();
        self.phase2 = (self.phase2 + dt * cents).fract();
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
                let alpha = (1.0 - (-TAU * p.cutoff.min(rate * 0.45) / rate).exp()) as f32;
                self.low += alpha * (osc as f32 - self.low);
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
        (value * env * gain * release) as f32 * self.velocity * p.level
    }
}
pub fn saw(t: f64, dt: f64) -> f64 {
    let correction = if t < dt {
        let x = t / dt;
        x + x - x * x - 1.0
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        x * x + x + x + 1.0
    } else {
        0.0
    };
    2.0 * t - 1.0 - correction
}
pub fn square(t: f64, dt: f64) -> f64 {
    saw(t, dt) - saw((t + 0.5).fract(), dt)
}
pub fn triangle(t: f64) -> f64 {
    (t * 4.0 - 2.0).abs() - 1.0
}

pub struct Delay {
    data: Vec<[f32; 2]>,
    cursor: usize,
}
impl Delay {
    pub fn new(seconds: f64, rate: u32) -> Self {
        Self {
            data: vec![[0.0; 2]; (seconds * rate as f64).ceil() as usize + 2],
            cursor: 0,
        }
    }
    pub fn len(&self) -> usize {
        self.data.len()
    }
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
    pub fn clear(&mut self) {
        self.data.fill([0.0; 2]);
    }
    pub fn read(&self, samples: f64) -> [f32; 2] {
        let pos = (self.cursor as f64 - samples).rem_euclid(self.data.len() as f64);
        let a = pos as usize;
        let b = (a + 1) % self.data.len();
        let t = (pos - a as f64) as f32;
        [
            self.data[a][0] * (1.0 - t) + self.data[b][0] * t,
            self.data[a][1] * (1.0 - t) + self.data[b][1] * t,
        ]
    }
    pub fn write(&mut self, v: [f32; 2]) {
        self.data[self.cursor] = v;
        self.cursor = (self.cursor + 1) % self.data.len();
    }
}

/// RBJ biquad with independent state per channel.
#[derive(Clone, Copy, Default)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z: [[f32; 2]; 2],
}
impl Biquad {
    fn set(&mut self, b0: f64, b1: f64, b2: f64, a0: f64, a1: f64, a2: f64) {
        self.b0 = (b0 / a0) as f32;
        self.b1 = (b1 / a0) as f32;
        self.b2 = (b2 / a0) as f32;
        self.a1 = (a1 / a0) as f32;
        self.a2 = (a2 / a0) as f32;
    }
    pub fn bypass(&mut self) {
        self.set(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
    }
    pub fn low_shelf(&mut self, rate: f64, hz: f64, gain_db: f64) {
        let a = 10f64.powf(gain_db / 40.0);
        let w = TAU * hz.clamp(10.0, rate * 0.45) / rate;
        let (s, c) = w.sin_cos();
        let alpha = s / 2.0 * (2.0_f64).sqrt();
        let sq = 2.0 * a.sqrt() * alpha;
        self.set(
            a * ((a + 1.0) - (a - 1.0) * c + sq),
            2.0 * a * ((a - 1.0) - (a + 1.0) * c),
            a * ((a + 1.0) - (a - 1.0) * c - sq),
            (a + 1.0) + (a - 1.0) * c + sq,
            -2.0 * ((a - 1.0) + (a + 1.0) * c),
            (a + 1.0) + (a - 1.0) * c - sq,
        );
    }
    pub fn high_shelf(&mut self, rate: f64, hz: f64, gain_db: f64) {
        let a = 10f64.powf(gain_db / 40.0);
        let w = TAU * hz.clamp(10.0, rate * 0.45) / rate;
        let (s, c) = w.sin_cos();
        let alpha = s / 2.0 * (2.0_f64).sqrt();
        let sq = 2.0 * a.sqrt() * alpha;
        self.set(
            a * ((a + 1.0) + (a - 1.0) * c + sq),
            -2.0 * a * ((a - 1.0) + (a + 1.0) * c),
            a * ((a + 1.0) + (a - 1.0) * c - sq),
            (a + 1.0) - (a - 1.0) * c + sq,
            2.0 * ((a - 1.0) - (a + 1.0) * c),
            (a + 1.0) - (a - 1.0) * c - sq,
        );
    }
    pub fn peaking(&mut self, rate: f64, hz: f64, gain_db: f64, q: f64) {
        let a = 10f64.powf(gain_db / 40.0);
        let w = TAU * hz.clamp(10.0, rate * 0.45) / rate;
        let (s, c) = w.sin_cos();
        let alpha = s / (2.0 * q.max(0.05));
        self.set(
            1.0 + alpha * a,
            -2.0 * c,
            1.0 - alpha * a,
            1.0 + alpha / a,
            -2.0 * c,
            1.0 - alpha / a,
        );
    }
    pub fn lowpass(&mut self, rate: f64, hz: f64, q: f64) {
        let w = TAU * hz.clamp(10.0, rate * 0.45) / rate;
        let (s, c) = w.sin_cos();
        let alpha = s / (2.0 * q.max(0.05));
        self.set(
            (1.0 - c) / 2.0,
            1.0 - c,
            (1.0 - c) / 2.0,
            1.0 + alpha,
            -2.0 * c,
            1.0 - alpha,
        );
    }
    pub fn highpass(&mut self, rate: f64, hz: f64, q: f64) {
        let w = TAU * hz.clamp(10.0, rate * 0.45) / rate;
        let (s, c) = w.sin_cos();
        let alpha = s / (2.0 * q.max(0.05));
        self.set(
            (1.0 + c) / 2.0,
            -(1.0 + c),
            (1.0 + c) / 2.0,
            1.0 + alpha,
            -2.0 * c,
            1.0 - alpha,
        );
    }
    pub fn allpass(&mut self, rate: f64, hz: f64, q: f64) {
        let w = TAU * hz.clamp(10.0, rate * 0.45) / rate;
        let (s, c) = w.sin_cos();
        let alpha = s / (2.0 * q.max(0.05));
        self.set(
            1.0 - alpha,
            -2.0 * c,
            1.0 + alpha,
            1.0 + alpha,
            -2.0 * c,
            1.0 - alpha,
        );
    }
    #[inline]
    pub fn process(&mut self, channel: usize, x: f32) -> f32 {
        let z = &mut self.z[channel];
        let y = self.b0 * x + z[0];
        z[0] = self.b1 * x - self.a1 * y + z[1];
        z[1] = self.b2 * x - self.a2 * y;
        if !y.is_finite() {
            *z = [0.0; 2];
            return 0.0;
        }
        y
    }
    pub fn clear(&mut self) {
        self.z = [[0.0; 2]; 2];
    }
}

/// Topology-preserving state variable filter (Zavalishin).
#[derive(Clone, Copy, Default)]
pub struct Svf {
    g: f32,
    k: f32,
    a1: f32,
    a2: f32,
    a3: f32,
    ic1: [f32; 2],
    ic2: [f32; 2],
}
impl Svf {
    pub fn set(&mut self, rate: f64, cutoff: f64, resonance: f64) {
        let g = (std::f64::consts::PI * cutoff.clamp(10.0, rate * 0.45) / rate).tan();
        let k = 2.0 - 1.9 * resonance.clamp(0.0, 1.0);
        self.g = g as f32;
        self.k = k as f32;
        self.a1 = (1.0 / (1.0 + g * (g + k))) as f32;
        self.a2 = (g * self.a1 as f64) as f32;
        self.a3 = (g * self.a2 as f64) as f32;
    }
    /// Returns (low, band, high).
    #[inline]
    pub fn process(&mut self, channel: usize, x: f32) -> (f32, f32, f32) {
        let v3 = x - self.ic2[channel];
        let v1 = self.a1 * self.ic1[channel] + self.a2 * v3;
        let v2 = self.ic2[channel] + self.a2 * self.ic1[channel] + self.a3 * v3;
        self.ic1[channel] = 2.0 * v1 - self.ic1[channel];
        self.ic2[channel] = 2.0 * v2 - self.ic2[channel];
        if !v2.is_finite() || !v1.is_finite() {
            self.ic1[channel] = 0.0;
            self.ic2[channel] = 0.0;
            return (0.0, 0.0, 0.0);
        }
        (v2, v1, x - self.k * v1 - v2)
    }
    pub fn clear(&mut self) {
        self.ic1 = [0.0; 2];
        self.ic2 = [0.0; 2];
    }
}

/// Smoothing coefficient for a time constant in seconds.
pub fn coef(rate: f64, seconds: f64) -> f32 {
    (1.0 - (-1.0 / (rate * seconds.max(1e-5))).exp()) as f32
}
pub fn db(gain: f32) -> f32 {
    if gain > 1e-9 {
        20.0 * gain.log10()
    } else {
        -180.0
    }
}
