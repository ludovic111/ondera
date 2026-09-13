//! Signal primitives shared by Ondera's stock plugins and available to every native plugin:
//! oscillators, a delay line, RBJ biquads, a state variable filter and smoothing helpers.

use std::f64::consts::TAU;

/// Band-limited sawtooth at phase `t` in 0..1 with phase increment `dt` (PolyBLEP).
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

/// A stereo delay line with linear interpolation. Allocated once in `new`.
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
    /// The frame `samples` frames ago (fractional).
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

/// One-pole parameter smoother: call `set` from `set_param`, `next` once per frame.
#[derive(Clone, Copy, Debug)]
pub struct Smoother {
    current: f32,
    target: f32,
    coefficient: f32,
}
impl Smoother {
    pub fn new(rate: f64, seconds: f64, value: f32) -> Self {
        Self {
            current: value,
            target: value,
            coefficient: coef(rate, seconds),
        }
    }
    pub fn set(&mut self, target: f32) {
        self.target = target;
    }
    pub fn jump(&mut self, value: f32) {
        self.current = value;
        self.target = value;
    }
    #[inline]
    pub fn next(&mut self) -> f32 {
        self.current += self.coefficient * (self.target - self.current);
        self.current
    }
    pub fn value(&self) -> f32 {
        self.current
    }
}

/// Smoothing coefficient for a time constant in seconds.
pub fn coef(rate: f64, seconds: f64) -> f32 {
    (1.0 - (-1.0 / (rate * seconds.max(1e-5))).exp()) as f32
}
/// Linear gain to decibels, floored at -180 dB.
pub fn db(gain: f32) -> f32 {
    if gain > 1e-9 {
        20.0 * gain.log10()
    } else {
        -180.0
    }
}
/// Decibels to linear gain.
pub fn db_to_gain(db: f64) -> f32 {
    10f64.powf(db / 20.0) as f32
}
