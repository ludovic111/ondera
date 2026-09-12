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
pub const EFFECTS: [&str; 6] = [
    "Ondera Comp",
    "Channel EQ",
    "Tape Sat",
    "Chorus",
    "Space",
    "Echo",
];

#[derive(Clone, Copy, Debug)]
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
}

#[derive(Clone, Copy)]
pub struct Voice {
    pub preset: Preset,
    pub pitch: u8,
    pub velocity: f32,
    pub duration: f64,
    phase: f64,
    phase2: f64,
    low: f32,
    seed: u32,
}
impl Voice {
    pub fn new(preset: Preset, pitch: u8, velocity: u8, duration: f64, age: f64) -> Self {
        let hz = 440.0 * 2.0_f64.powf((pitch as f64 - 69.0) / 12.0);
        Self {
            preset,
            pitch,
            velocity: velocity as f32 / 127.0,
            duration,
            phase: (hz * age).fract(),
            phase2: (hz * age * 1.004).fract(),
            low: 0.0,
            seed: 12345 + pitch as u32,
        }
    }
    pub fn sample(&mut self, age: f64, rate: f64) -> f32 {
        let hz = (440.0 * 2.0_f64.powf((self.pitch as f64 - 69.0) / 12.0)).min(rate * 0.4);
        let dt = hz / rate;
        self.phase = (self.phase + dt).fract();
        self.phase2 = (self.phase2 + dt * 1.004).fract();
        self.seed = self.seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = self.seed as f64 / 2147483648.0 - 1.0;
        let release = (-(age - self.duration).max(0.0) * 8.0 / self.preset.release()).exp();
        let (value, env, gain) = match self.preset {
            Preset::Synth | Preset::Pad => {
                let pad = matches!(self.preset, Preset::Pad);
                let osc = saw(self.phase, dt)
                    + if pad {
                        saw(self.phase2, dt * 1.004) * 0.5
                    } else {
                        0.0
                    };
                let cutoff = if pad { 1200.0 } else { 2200.0 };
                let alpha = (1.0 - (-TAU * cutoff / rate).exp()) as f32;
                self.low += alpha * (osc as f32 - self.low);
                let attack = if pad { 0.35 } else { 0.01 };
                (
                    self.low as f64,
                    (age / attack).min(1.0) * (0.6 + 0.4 * (-age * 8.0).exp()),
                    if pad { 0.28 } else { 0.32 },
                )
            }
            Preset::Sub => (
                (TAU * self.phase).sin(),
                (age / 0.005).min(1.0) * (0.9 + 0.1 * (-age * 10.0).exp()),
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
                    (age / 0.005).min(1.0)
                        * (0.25 + 0.75 * (-age * if bell { 0.9 } else { 1.8 }).exp()),
                    0.5,
                )
            }
            Preset::Pluck => (
                (self.phase * 2.0 - 1.0).abs() * 2.0 - 1.0,
                (age / 0.002).min(1.0) * (-age * 16.0).exp(),
                0.5,
            ),
            Preset::Riser => {
                self.low += 0.2 * (noise as f32 - self.low);
                (
                    noise - self.low as f64,
                    (age / self.duration.max(0.01)).min(1.0),
                    0.4,
                )
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
        (value * env * gain * release) as f32 * self.velocity
    }
}
fn saw(t: f64, dt: f64) -> f64 {
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
    fn read(&self, samples: f64) -> [f32; 2] {
        let pos = (self.cursor as f64 - samples).rem_euclid(self.data.len() as f64);
        let a = pos as usize;
        let b = (a + 1) % self.data.len();
        let t = (pos - a as f64) as f32;
        [
            self.data[a][0] * (1.0 - t) + self.data[b][0] * t,
            self.data[a][1] * (1.0 - t) + self.data[b][1] * t,
        ]
    }
    fn write(&mut self, v: [f32; 2]) {
        self.data[self.cursor] = v;
        self.cursor = (self.cursor + 1) % self.data.len();
    }
}
pub enum Effect {
    Comp {
        envelope: f32,
    },
    Eq {
        low: [f32; 2],
        high: [f32; 2],
    },
    Tape,
    Chorus {
        delay: Delay,
        phase: f64,
    },
    Echo {
        delay: Delay,
        seconds: f64,
        feedback: f32,
    },
    Space {
        lines: [Delay; 4],
        damp: [[f32; 2]; 4],
    },
}
impl Effect {
    pub fn new(name: &str, rate: u32) -> Option<Self> {
        Some(match name {
            "Ondera Comp" => Self::Comp { envelope: 0.0 },
            "Channel EQ" => Self::Eq {
                low: [0.0; 2],
                high: [0.0; 2],
            },
            "Tape Sat" => Self::Tape,
            "Chorus" => Self::Chorus {
                delay: Delay::new(0.05, rate),
                phase: 0.0,
            },
            "Echo" => Self::Echo {
                delay: Delay::new(0.51, rate),
                seconds: 0.375,
                feedback: 0.35,
            },
            "Space" => Self::Space {
                lines: [0.0297, 0.0371, 0.0411, 0.0437].map(|s| Delay::new(s, rate)),
                damp: [[0.0; 2]; 4],
            },
            _ => return None,
        })
    }
    pub fn process(&mut self, input: [f32; 2], rate: u32) -> [f32; 2] {
        match self {
            Self::Comp { envelope } => {
                let peak = input[0].abs().max(input[1].abs());
                let tau = if peak > *envelope { 0.005 } else { 0.15 };
                *envelope += (1.0 - (-1.0 / (rate as f32 * tau)).exp()) * (peak - *envelope);
                let gain = if *envelope > 0.1259 {
                    (0.1259 / *envelope).powf(0.75)
                } else {
                    1.0
                };
                input.map(|v| v * gain)
            }
            Self::Eq { low, high } => {
                let a = 1.0 - (-std::f32::consts::TAU * 120.0 / rate as f32).exp();
                let b = 1.0 - (-std::f32::consts::TAU * 6000.0 / rate as f32).exp();
                std::array::from_fn(|c| {
                    low[c] += a * (input[c] - low[c]);
                    high[c] += b * (input[c] - high[c]);
                    low[c] * 1.1885 + (high[c] - low[c]) * 0.7943 + (input[c] - high[c]) * 1.2589
                })
            }
            Self::Tape => input.map(|v| (v * 2.2).tanh() / 2.2_f32.tanh() * 0.8),
            Self::Chorus { delay, phase } => {
                *phase = (*phase + 0.6 / rate as f64).fract();
                let wet = delay.read((0.018 + 0.004 * (TAU * *phase).sin()) * rate as f64);
                delay.write(input);
                [input[0] + wet[0] * 0.5, input[1] + wet[1] * 0.5]
            }
            Self::Echo {
                delay,
                seconds,
                feedback,
            } => {
                let wet = delay.read(*seconds * rate as f64);
                delay.write([input[0] + wet[0] * *feedback, input[1] + wet[1] * *feedback]);
                [input[0] + wet[0] * 0.4, input[1] + wet[1] * 0.4]
            }
            Self::Space { lines, damp } => {
                let mut wet = [0.0; 2];
                for (i, line) in lines.iter_mut().enumerate() {
                    let tap = line.read((line.data.len() - 2) as f64);
                    for c in 0..2 {
                        damp[i][c] += 0.35 * (tap[c] - damp[i][c]);
                        wet[c] += tap[c] * 0.25;
                    }
                    line.write([input[0] + damp[i][1] * 0.78, input[1] + damp[i][0] * 0.78]);
                }
                [input[0] + wet[0] * 0.35, input[1] + wet[1] * 0.35]
            }
        }
    }
}
