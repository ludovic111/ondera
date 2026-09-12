//! The stock Ondera plugin library: sixteen effects and eight instruments,
//! all exposed through the same `Editor` / `Processor` pair as external plugins.

use crate::{dsp::*, plugin::*, Result};
use std::{
    f64::consts::TAU,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
};

struct Spec {
    name: &'static str,
    min: f64,
    max: f64,
    default: f64,
    unit: &'static str,
    steps: u32,
    log: bool,
    labels: &'static [&'static str],
}
const fn p(name: &'static str, min: f64, max: f64, default: f64, unit: &'static str) -> Spec {
    Spec {
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
const fn hz(name: &'static str, min: f64, max: f64, default: f64) -> Spec {
    Spec {
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
const fn choice(name: &'static str, labels: &'static [&'static str], default: usize) -> Spec {
    Spec {
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
const ON_OFF: &[&str] = &["Off", "On"];

fn effect_specs(name: &str) -> Vec<Spec> {
    match name {
        "Ondera Comp" => vec![
            p("Threshold", -60.0, 0.0, -18.0, "dB"),
            p("Ratio", 1.0, 20.0, 4.0, ":1"),
            p("Attack", 0.1, 100.0, 10.0, "ms"),
            p("Release", 10.0, 1000.0, 120.0, "ms"),
            p("Makeup", 0.0, 24.0, 0.0, "dB"),
            p("Mix", 0.0, 100.0, 100.0, "%"),
        ],
        "Channel EQ" => vec![
            p("Low Gain", -15.0, 15.0, 1.5, "dB"),
            hz("Low Freq", 40.0, 500.0, 120.0),
            p("Mid Gain", -15.0, 15.0, -2.0, "dB"),
            hz("Mid Freq", 200.0, 8000.0, 1000.0),
            p("Mid Q", 0.3, 5.0, 0.8, ""),
            p("High Gain", -15.0, 15.0, 2.0, "dB"),
            hz("High Freq", 2000.0, 16000.0, 6000.0),
        ],
        "Tape Sat" => vec![
            p("Drive", 0.0, 24.0, 6.0, "dB"),
            hz("Tone", 1000.0, 20000.0, 9000.0),
            p("Mix", 0.0, 100.0, 100.0, "%"),
            p("Output", -24.0, 6.0, -2.0, "dB"),
        ],
        "Chorus" => vec![
            p("Rate", 0.05, 5.0, 0.6, "Hz"),
            p("Depth", 0.0, 100.0, 40.0, "%"),
            p("Spread", 0.0, 100.0, 50.0, "%"),
            p("Mix", 0.0, 100.0, 50.0, "%"),
        ],
        "Space" => vec![
            p("Size", 0.0, 100.0, 55.0, "%"),
            p("Damp", 0.0, 100.0, 40.0, "%"),
            p("Pre-delay", 0.0, 100.0, 10.0, "ms"),
            p("Width", 0.0, 100.0, 100.0, "%"),
            p("Mix", 0.0, 100.0, 30.0, "%"),
        ],
        "Echo" => vec![
            choice(
                "Sync",
                &["Free", "1/16", "1/8T", "1/8", "1/8.", "1/4", "1/4.", "1/2"],
                3,
            ),
            p("Time", 10.0, 2000.0, 375.0, "ms"),
            p("Feedback", 0.0, 95.0, 35.0, "%"),
            hz("Tone", 500.0, 20000.0, 6000.0),
            choice("Ping-pong", ON_OFF, 0),
            p("Mix", 0.0, 100.0, 35.0, "%"),
        ],
        "Gate" => vec![
            p("Threshold", -80.0, 0.0, -40.0, "dB"),
            p("Attack", 0.1, 50.0, 1.0, "ms"),
            p("Hold", 0.0, 500.0, 50.0, "ms"),
            p("Release", 5.0, 1000.0, 100.0, "ms"),
            p("Range", -80.0, 0.0, -80.0, "dB"),
        ],
        "Limiter" => vec![
            p("Input", 0.0, 24.0, 0.0, "dB"),
            p("Ceiling", -20.0, 0.0, -0.3, "dB"),
            p("Release", 10.0, 1000.0, 80.0, "ms"),
        ],
        "Filter" => vec![
            choice("Type", &["Low-pass", "High-pass", "Band-pass"], 0),
            hz("Cutoff", 20.0, 20000.0, 1000.0),
            p("Resonance", 0.0, 100.0, 20.0, "%"),
            p("Drive", 0.0, 24.0, 0.0, "dB"),
        ],
        "Phaser" => vec![
            p("Rate", 0.02, 5.0, 0.3, "Hz"),
            p("Depth", 0.0, 100.0, 70.0, "%"),
            choice("Stages", &["2", "4", "6", "8", "10", "12"], 2),
            p("Feedback", -90.0, 90.0, 30.0, "%"),
            hz("Center", 200.0, 4000.0, 800.0),
            p("Mix", 0.0, 100.0, 50.0, "%"),
        ],
        "Tremolo" => vec![
            p("Rate", 0.1, 20.0, 4.0, "Hz"),
            p("Depth", 0.0, 100.0, 60.0, "%"),
            choice("Shape", &["Sine", "Triangle", "Square"], 0),
            p("Stereo", 0.0, 180.0, 0.0, "°"),
        ],
        "Bitcrusher" => vec![
            p("Bits", 2.0, 16.0, 8.0, "bit"),
            p("Downsample", 1.0, 64.0, 4.0, "x"),
            p("Mix", 0.0, 100.0, 100.0, "%"),
        ],
        "Stereo Width" => vec![
            p("Width", 0.0, 200.0, 120.0, "%"),
            p("Bass Mono", 0.0, 500.0, 0.0, "Hz"),
        ],
        "Utility" => vec![
            p("Gain", -60.0, 12.0, 0.0, "dB"),
            p("Pan", -100.0, 100.0, 0.0, ""),
            choice("Invert L", ON_OFF, 0),
            choice("Invert R", ON_OFF, 0),
            choice("Mono", ON_OFF, 0),
        ],
        "Overdrive" => vec![
            p("Drive", 0.0, 40.0, 12.0, "dB"),
            hz("Tone", 500.0, 12000.0, 4000.0),
            p("Mix", 0.0, 100.0, 100.0, "%"),
            p("Output", -24.0, 6.0, -6.0, "dB"),
        ],
        "Transient" => vec![
            p("Attack", -100.0, 100.0, 30.0, "%"),
            p("Sustain", -100.0, 100.0, 0.0, "%"),
        ],
        _ => vec![],
    }
}
fn instrument_specs(preset: Preset) -> Vec<Spec> {
    let d = InstrumentParams::for_preset(preset);
    let mut specs = vec![];
    match preset {
        Preset::Synth => {
            specs.push(choice("Wave", &["Saw", "Square", "Triangle"], 0));
            specs.push(hz("Cutoff", 100.0, 16000.0, d.cutoff));
            specs.push(p("Env Amount", 0.0, 100.0, d.env_amount * 100.0, "%"));
            specs.push(p("Attack", 1.0, 3000.0, d.attack * 1000.0, "ms"));
            specs.push(p("Decay", 5.0, 3000.0, d.decay * 1000.0, "ms"));
            specs.push(p("Sustain", 0.0, 100.0, d.sustain * 100.0, "%"));
            specs.push(p("Release", 10.0, 5000.0, d.release * 1000.0, "ms"));
        }
        Preset::Pad => {
            specs.push(hz("Cutoff", 100.0, 16000.0, d.cutoff));
            specs.push(p("Detune", 0.0, 50.0, d.detune, "ct"));
            specs.push(p("Attack", 1.0, 3000.0, d.attack * 1000.0, "ms"));
            specs.push(p("Release", 10.0, 5000.0, d.release * 1000.0, "ms"));
        }
        Preset::Drums => {}
        _ => {
            specs.push(p("Attack", 1.0, 3000.0, d.attack * 1000.0, "ms"));
            specs.push(p("Release", 10.0, 5000.0, d.release * 1000.0, "ms"));
        }
    }
    specs.push(p("Level", -24.0, 6.0, 0.0, "dB"));
    specs
}

fn infos(specs: &[Spec]) -> Vec<ParamInfo> {
    specs
        .iter()
        .enumerate()
        .map(|(i, s)| ParamInfo {
            id: i as u32,
            name: s.name.into(),
            min: s.min,
            max: s.max,
            default: s.default,
            unit: s.unit.into(),
            steps: s.steps,
            log: s.log,
            labels: s.labels.iter().map(|l| l.to_string()).collect(),
        })
        .collect()
}
pub fn is_stock(name: &str) -> bool {
    INSTRUMENTS.contains(&name) || EFFECTS.contains(&name)
}
pub fn descriptor(name: &str) -> Option<Descriptor> {
    let instrument = INSTRUMENTS.contains(&name);
    if !instrument && !EFFECTS.contains(&name) {
        return None;
    }
    Some(Descriptor {
        id: format!("stock:{name}"),
        format: Format::Stock,
        name: name.into(),
        vendor: "Ondera".into(),
        path: String::new(),
        instrument,
        effect: !instrument,
        category: if instrument {
            "Instrument".into()
        } else {
            category(name).into()
        },
    })
}
fn category(name: &str) -> &'static str {
    match name {
        "Ondera Comp" | "Gate" | "Limiter" | "Transient" => "Dynamics",
        "Channel EQ" | "Filter" => "EQ & Filter",
        "Tape Sat" | "Overdrive" | "Bitcrusher" => "Distortion",
        "Chorus" | "Phaser" | "Tremolo" => "Modulation",
        "Space" | "Echo" => "Space & Time",
        _ => "Utility",
    }
}
pub fn descriptors() -> Vec<Descriptor> {
    INSTRUMENTS
        .iter()
        .chain(EFFECTS.iter())
        .filter_map(|n| descriptor(n))
        .collect()
}
pub fn params(name: &str) -> Vec<ParamInfo> {
    if INSTRUMENTS.contains(&name) {
        infos(&instrument_specs(Preset::named(name)))
    } else {
        infos(&effect_specs(name))
    }
}

trait Dsp: Send {
    fn set(&mut self, index: usize, value: f64);
    fn process(&mut self, audio: &mut [[f32; 2]], notes: &[NoteEvent], ctx: &ProcessContext);
    fn reset(&mut self) {}
    fn latency(&self) -> u32 {
        0
    }
}
struct RestoredState {
    values: Vec<AtomicU64>,
    pending: AtomicBool,
}
struct StockProcessor {
    dsp: Box<dyn Dsp>,
    restored: Arc<RestoredState>,
}
impl Processor for StockProcessor {
    fn reset(&mut self) {
        self.dsp.reset();
    }
    fn process(
        &mut self,
        audio: &mut [[f32; 2]],
        notes: &[NoteEvent],
        params: &[ParamChange],
        ctx: &ProcessContext,
    ) {
        if self.restored.pending.swap(false, Ordering::Acquire) {
            for (index, value) in self.restored.values.iter().enumerate() {
                self.dsp
                    .set(index, f64::from_bits(value.load(Ordering::Relaxed)));
            }
        }
        for change in params {
            self.dsp.set(change.id as usize, change.value);
        }
        self.dsp.process(audio, notes, ctx);
    }
    fn latency(&self) -> u32 {
        self.dsp.latency()
    }
}
struct StockEditor {
    desc: Descriptor,
    params: Vec<ParamInfo>,
    values: Vec<f64>,
    latency: u32,
    restored: Arc<RestoredState>,
}
impl Editor for StockEditor {
    fn descriptor(&self) -> &Descriptor {
        &self.desc
    }
    fn latency(&self) -> u32 {
        self.latency
    }
    fn params(&self) -> &[ParamInfo] {
        &self.params
    }
    fn value(&self, id: u32) -> Option<f64> {
        self.values.get(id as usize).copied()
    }
    fn set_value(&mut self, id: u32, value: f64) {
        if let Some(v) = self.values.get_mut(id as usize) {
            *v = value;
        }
    }
    fn save(&mut self) -> Option<Vec<u8>> {
        serde_json::to_vec(&self.values).ok()
    }
    fn load(&mut self, bytes: &[u8]) -> Result<()> {
        let values: Vec<f64> = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        for (i, v) in values.into_iter().enumerate() {
            if let Some(slot) = self.values.get_mut(i) {
                if v.is_finite() {
                    *slot = v.clamp(self.params[i].min, self.params[i].max);
                }
            }
        }
        for (value, shared) in self.values.iter().zip(&self.restored.values) {
            shared.store(value.to_bits(), Ordering::Relaxed);
        }
        self.restored.pending.store(true, Ordering::Release);
        Ok(())
    }
}

/// Instantiate a stock plugin with every parameter at its default.
pub fn create(name: &str, rate: u32) -> Option<Instance> {
    let desc = descriptor(name)?;
    let params = params(name);
    let mut dsp: Box<dyn Dsp> = if desc.instrument {
        Box::new(StockInstrument::new(Preset::named(name), rate))
    } else {
        effect(name, rate)?
    };
    for (i, param) in params.iter().enumerate() {
        dsp.set(i, param.default);
    }
    let values: Vec<f64> = params.iter().map(|p| p.default).collect();
    let restored = Arc::new(RestoredState {
        values: values
            .iter()
            .map(|value| AtomicU64::new(value.to_bits()))
            .collect(),
        pending: AtomicBool::new(false),
    });
    Some(Instance {
        editor: Box::new(StockEditor {
            desc,
            params,
            values,
            latency: dsp.latency(),
            restored: restored.clone(),
        }),
        processor: Some(Box::new(StockProcessor { dsp, restored })),
    })
}
fn effect(name: &str, rate: u32) -> Option<Box<dyn Dsp>> {
    let r = rate as f64;
    Some(match name {
        "Ondera Comp" => Box::new(Comp::new(r)),
        "Channel EQ" => Box::new(Eq::new(r)),
        "Tape Sat" => Box::new(Saturator::new(r, false)),
        "Overdrive" => Box::new(Saturator::new(r, true)),
        "Chorus" => Box::new(Chorus::new(rate)),
        "Space" => Box::new(Space::new(rate)),
        "Echo" => Box::new(Echo::new(rate)),
        "Gate" => Box::new(Gate::new(r)),
        "Limiter" => Box::new(Limiter::new(rate)),
        "Filter" => Box::new(Filter::new(r)),
        "Phaser" => Box::new(Phaser::new(r)),
        "Tremolo" => Box::new(Tremolo::new(r)),
        "Bitcrusher" => Box::new(Bitcrusher::default()),
        "Stereo Width" => Box::new(Width::new(r)),
        "Utility" => Box::new(Utility::default()),
        "Transient" => Box::new(Transient::new(r)),
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Instruments
// ---------------------------------------------------------------------------

const VOICES: usize = 32;
struct StockInstrument {
    preset: Preset,
    params: InstrumentParams,
    voices: [Option<Voice>; VOICES],
    serial: u64,
    rate: f64,
    order: Vec<&'static str>,
}
impl StockInstrument {
    fn new(preset: Preset, rate: u32) -> Self {
        Self {
            preset,
            params: InstrumentParams::for_preset(preset),
            voices: [None; VOICES],
            serial: 0,
            rate: rate as f64,
            order: instrument_specs(preset).iter().map(|s| s.name).collect(),
        }
    }
    fn note_on(&mut self, pitch: u8, velocity: u8) {
        self.serial += 1;
        let voice = Voice::new(self.preset, pitch, velocity, self.serial);
        if let Some(slot) = self.voices.iter_mut().find(|v| v.is_none()) {
            *slot = Some(voice);
            return;
        }
        // Steal the oldest released voice, else the oldest voice.
        let victim = self
            .voices
            .iter()
            .enumerate()
            .filter_map(|(i, v)| v.map(|v| (i, v)))
            .min_by(|a, b| (a.1.held(), a.1.serial).cmp(&(b.1.held(), b.1.serial)))
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.voices[victim] = Some(voice);
    }
    fn note_off(&mut self, pitch: u8) {
        if let Some(v) = self
            .voices
            .iter_mut()
            .flatten()
            .filter(|v| v.pitch == pitch && v.held())
            .max_by_key(|v| v.serial)
        {
            v.release();
        }
    }
}
impl Dsp for StockInstrument {
    fn set(&mut self, index: usize, value: f64) {
        let Some(name) = self.order.get(index) else {
            return;
        };
        let p = &mut self.params;
        match *name {
            "Wave" => p.wave = value.round() as u8,
            "Cutoff" => p.cutoff = value,
            "Env Amount" => p.env_amount = value / 100.0,
            "Attack" => p.attack = value / 1000.0,
            "Decay" => p.decay = value / 1000.0,
            "Sustain" => p.sustain = value / 100.0,
            "Release" => p.release = value / 1000.0,
            "Detune" => p.detune = value,
            "Level" => p.level = db_to_gain(value),
            _ => {}
        }
    }
    fn reset(&mut self) {
        self.voices = [None; VOICES];
    }
    fn process(&mut self, audio: &mut [[f32; 2]], notes: &[NoteEvent], _ctx: &ProcessContext) {
        let mut next = 0;
        for (i, frame) in audio.iter_mut().enumerate() {
            while next < notes.len() && notes[next].frame as usize <= i {
                let n = notes[next];
                if n.on {
                    self.note_on(n.pitch, n.velocity);
                } else {
                    self.note_off(n.pitch);
                }
                next += 1;
            }
            let mut sum = 0.0;
            for slot in &mut self.voices {
                if let Some(v) = slot {
                    sum += v.sample(&self.params, self.rate);
                    if v.finished(&self.params) {
                        *slot = None;
                    }
                }
            }
            if !sum.is_finite() {
                sum = 0.0;
            }
            frame[0] += sum;
            frame[1] += sum;
        }
        // Notes scheduled at the block end.
        for n in &notes[next..] {
            if n.on {
                self.note_on(n.pitch, n.velocity);
            } else {
                self.note_off(n.pitch);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Effects
// ---------------------------------------------------------------------------

struct Comp {
    rate: f64,
    threshold: f32,
    ratio: f32,
    attack: f32,
    release: f32,
    makeup: f32,
    mix: f32,
    env: f32,
}
impl Comp {
    fn new(rate: f64) -> Self {
        Self {
            rate,
            threshold: -18.0,
            ratio: 4.0,
            attack: coef(rate, 0.01),
            release: coef(rate, 0.12),
            makeup: 1.0,
            mix: 1.0,
            env: 0.0,
        }
    }
}
impl Dsp for Comp {
    fn set(&mut self, index: usize, value: f64) {
        match index {
            0 => self.threshold = value as f32,
            1 => self.ratio = value.max(1.0) as f32,
            2 => self.attack = coef(self.rate, value / 1000.0),
            3 => self.release = coef(self.rate, value / 1000.0),
            4 => self.makeup = db_to_gain(value),
            5 => self.mix = (value / 100.0) as f32,
            _ => {}
        }
    }
    fn reset(&mut self) {
        self.env = 0.0;
    }
    fn process(&mut self, audio: &mut [[f32; 2]], _: &[NoteEvent], _: &ProcessContext) {
        for frame in audio {
            let peak = frame[0].abs().max(frame[1].abs());
            let over = (db(peak) - self.threshold).max(0.0);
            let target = over * (1.0 - 1.0 / self.ratio);
            let c = if target > self.env {
                self.attack
            } else {
                self.release
            };
            self.env += c * (target - self.env);
            let gain = db_to_gain(-self.env as f64) * self.makeup;
            for s in frame.iter_mut() {
                *s = *s * (1.0 - self.mix) + *s * gain * self.mix;
            }
        }
    }
}

struct Eq {
    rate: f64,
    values: [f64; 7],
    bands: [Biquad; 3],
    dirty: bool,
}
impl Eq {
    fn new(rate: f64) -> Self {
        Self {
            rate,
            values: [1.5, 120.0, -2.0, 1000.0, 0.8, 2.0, 6000.0],
            bands: [Biquad::default(); 3],
            dirty: true,
        }
    }
    fn update(&mut self) {
        let v = self.values;
        self.bands[0].low_shelf(self.rate, v[1], v[0]);
        self.bands[1].peaking(self.rate, v[3], v[2], v[4]);
        self.bands[2].high_shelf(self.rate, v[6], v[5]);
        self.dirty = false;
    }
}
impl Dsp for Eq {
    fn set(&mut self, index: usize, value: f64) {
        if let Some(v) = self.values.get_mut(index) {
            *v = value;
            self.dirty = true;
        }
    }
    fn reset(&mut self) {
        self.bands.iter_mut().for_each(Biquad::clear);
    }
    fn process(&mut self, audio: &mut [[f32; 2]], _: &[NoteEvent], _: &ProcessContext) {
        if self.dirty {
            self.update();
        }
        for frame in audio {
            for (c, s) in frame.iter_mut().enumerate() {
                let mut x = *s;
                for band in &mut self.bands {
                    x = band.process(c, x);
                }
                *s = x;
            }
        }
    }
}

struct Saturator {
    rate: f64,
    hard: bool,
    drive: f32,
    tone: f32,
    mix: f32,
    output: f32,
    lp: [f32; 2],
}
impl Saturator {
    fn new(rate: f64, hard: bool) -> Self {
        Self {
            rate,
            hard,
            drive: db_to_gain(6.0),
            tone: coef(rate, 1.0 / (TAU * 9000.0)),
            mix: 1.0,
            output: db_to_gain(-2.0),
            lp: [0.0; 2],
        }
    }
}
impl Dsp for Saturator {
    fn set(&mut self, index: usize, value: f64) {
        match index {
            0 => self.drive = db_to_gain(value),
            1 => self.tone = coef(self.rate, 1.0 / (TAU * value.max(20.0))),
            2 => self.mix = (value / 100.0) as f32,
            3 => self.output = db_to_gain(value),
            _ => {}
        }
    }
    fn reset(&mut self) {
        self.lp = [0.0; 2];
    }
    fn process(&mut self, audio: &mut [[f32; 2]], _: &[NoteEvent], _: &ProcessContext) {
        let norm = 1.0 / self.drive.max(1.0).powf(0.6);
        for frame in audio {
            for (c, s) in frame.iter_mut().enumerate() {
                let pre = *s * self.drive;
                let shaped = if self.hard {
                    pre / (1.0 + pre.abs())
                } else {
                    pre.tanh()
                } * norm;
                self.lp[c] += self.tone * (shaped - self.lp[c]);
                let wet = self.lp[c] * self.output;
                *s = *s * (1.0 - self.mix) + wet * self.mix;
            }
        }
    }
}

struct Chorus {
    rate: u32,
    delay: Delay,
    phase: f64,
    speed: f64,
    depth: f64,
    spread: f64,
    mix: f32,
}
impl Chorus {
    fn new(rate: u32) -> Self {
        Self {
            rate,
            delay: Delay::new(0.06, rate),
            phase: 0.0,
            speed: 0.6,
            depth: 0.4,
            spread: 0.5,
            mix: 0.5,
        }
    }
}
impl Dsp for Chorus {
    fn set(&mut self, index: usize, value: f64) {
        match index {
            0 => self.speed = value,
            1 => self.depth = value / 100.0,
            2 => self.spread = value / 100.0,
            3 => self.mix = (value / 100.0) as f32,
            _ => {}
        }
    }
    fn reset(&mut self) {
        self.delay.clear();
    }
    fn process(&mut self, audio: &mut [[f32; 2]], _: &[NoteEvent], _: &ProcessContext) {
        let r = self.rate as f64;
        for frame in audio {
            self.phase = (self.phase + self.speed / r).fract();
            let base = 0.016;
            let swing = 0.008 * self.depth;
            let l = self
                .delay
                .read((base + swing * (TAU * self.phase).sin()) * r);
            let ph = (self.phase + 0.5 * self.spread).fract();
            let rr = self.delay.read((base + swing * (TAU * ph).sin()) * r);
            self.delay.write(*frame);
            frame[0] += l[0] * self.mix;
            frame[1] += rr[1] * self.mix;
        }
    }
}

struct Comb {
    buffer: Vec<f32>,
    index: usize,
    store: f32,
}
impl Comb {
    fn new(len: usize) -> Self {
        Self {
            buffer: vec![0.0; len.max(1)],
            index: 0,
            store: 0.0,
        }
    }
    #[inline]
    fn process(&mut self, input: f32, feedback: f32, damp: f32) -> f32 {
        let out = self.buffer[self.index];
        self.store = out * (1.0 - damp) + self.store * damp;
        let next = input + self.store * feedback;
        self.buffer[self.index] = if next.is_finite() { next } else { 0.0 };
        self.index = (self.index + 1) % self.buffer.len();
        out
    }
}
struct Allpass {
    buffer: Vec<f32>,
    index: usize,
}
impl Allpass {
    fn new(len: usize) -> Self {
        Self {
            buffer: vec![0.0; len.max(1)],
            index: 0,
        }
    }
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let buffered = self.buffer[self.index];
        let out = -input + buffered;
        self.buffer[self.index] = input + buffered * 0.5;
        self.index = (self.index + 1) % self.buffer.len();
        out
    }
}
/// A Freeverb-style reverb: eight combs and four allpasses per channel.
struct Space {
    combs: [Vec<Comb>; 2],
    allpasses: [Vec<Allpass>; 2],
    predelay: Delay,
    rate: u32,
    feedback: f32,
    damp: f32,
    pre: f64,
    width: f32,
    mix: f32,
}
impl Space {
    fn new(rate: u32) -> Self {
        let scale = rate as f64 / 44100.0;
        let tune = |n: usize, offset: usize| ((n + offset) as f64 * scale) as usize;
        let combs = |offset| {
            [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617]
                .iter()
                .map(|&n| Comb::new(tune(n, offset)))
                .collect()
        };
        let allpasses = |offset| {
            [556, 441, 341, 225]
                .iter()
                .map(|&n| Allpass::new(tune(n, offset)))
                .collect()
        };
        Self {
            combs: [combs(0), combs(23)],
            allpasses: [allpasses(0), allpasses(23)],
            predelay: Delay::new(0.11, rate),
            rate,
            feedback: 0.7 + 0.28 * 0.55,
            damp: 0.4 * 0.4,
            pre: 0.01,
            width: 1.0,
            mix: 0.3,
        }
    }
}
impl Dsp for Space {
    fn set(&mut self, index: usize, value: f64) {
        match index {
            0 => self.feedback = (0.7 + 0.28 * value / 100.0) as f32,
            1 => self.damp = (0.4 * value / 100.0) as f32,
            2 => self.pre = value / 1000.0,
            3 => self.width = (value / 100.0) as f32,
            4 => self.mix = (value / 100.0) as f32,
            _ => {}
        }
    }
    fn reset(&mut self) {
        for c in self.combs.iter_mut().flatten() {
            c.buffer.fill(0.0);
            c.store = 0.0;
        }
        for a in self.allpasses.iter_mut().flatten() {
            a.buffer.fill(0.0);
        }
        self.predelay.clear();
    }
    fn process(&mut self, audio: &mut [[f32; 2]], _: &[NoteEvent], _: &ProcessContext) {
        let pre = self.pre * self.rate as f64;
        let wet1 = self.mix * (self.width / 2.0 + 0.5);
        let wet2 = self.mix * ((1.0 - self.width) / 2.0);
        for frame in audio {
            let delayed = self.predelay.read(pre);
            self.predelay.write(*frame);
            let input = (delayed[0] + delayed[1]) * 0.015;
            let mut out = [0.0f32; 2];
            for (c, out_c) in out.iter_mut().enumerate() {
                let mut acc = 0.0;
                for comb in &mut self.combs[c] {
                    acc += comb.process(input, self.feedback, self.damp);
                }
                for ap in &mut self.allpasses[c] {
                    acc = ap.process(acc);
                }
                *out_c = acc;
            }
            frame[0] += out[0] * wet1 + out[1] * wet2;
            frame[1] += out[1] * wet1 + out[0] * wet2;
        }
    }
}

struct Echo {
    rate: u32,
    lines: [Delay; 2],
    sync: usize,
    time_ms: f64,
    feedback: f32,
    tone: f32,
    pingpong: bool,
    mix: f32,
    lp: [f32; 2],
    current: f64,
}
impl Echo {
    fn new(rate: u32) -> Self {
        Self {
            rate,
            lines: [Delay::new(2.1, rate), Delay::new(2.1, rate)],
            sync: 3,
            time_ms: 375.0,
            feedback: 0.35,
            tone: coef(rate as f64, 1.0 / (TAU * 6000.0)),
            pingpong: false,
            mix: 0.35,
            lp: [0.0; 2],
            current: 0.0,
        }
    }
    fn seconds(&self, tempo: f64) -> f64 {
        let beat = 60.0 / tempo.max(20.0);
        match self.sync {
            1 => beat / 4.0,
            2 => beat / 3.0,
            3 => beat / 2.0,
            4 => beat * 0.75,
            5 => beat,
            6 => beat * 1.5,
            7 => beat * 2.0,
            _ => self.time_ms / 1000.0,
        }
        .clamp(0.005, 2.0)
    }
}
impl Dsp for Echo {
    fn set(&mut self, index: usize, value: f64) {
        match index {
            0 => self.sync = value.round().max(0.0) as usize,
            1 => self.time_ms = value,
            2 => self.feedback = (value / 100.0) as f32,
            3 => self.tone = coef(self.rate as f64, 1.0 / (TAU * value.max(20.0))),
            4 => self.pingpong = value >= 0.5,
            5 => self.mix = (value / 100.0) as f32,
            _ => {}
        }
    }
    fn reset(&mut self) {
        self.lines.iter_mut().for_each(Delay::clear);
        self.lp = [0.0; 2];
    }
    fn process(&mut self, audio: &mut [[f32; 2]], _: &[NoteEvent], ctx: &ProcessContext) {
        let target = self.seconds(ctx.tempo) * self.rate as f64;
        if self.current == 0.0 {
            self.current = target;
        }
        let glide = coef(self.rate as f64, 0.05) as f64;
        for frame in audio {
            self.current += glide * (target - self.current);
            let l = self.lines[0].read(self.current)[0];
            let r = self.lines[1].read(self.current)[1];
            self.lp[0] += self.tone * (l - self.lp[0]);
            self.lp[1] += self.tone * (r - self.lp[1]);
            let (fl, fr) = (self.lp[0] * self.feedback, self.lp[1] * self.feedback);
            if self.pingpong {
                let mono = (frame[0] + frame[1]) * 0.5;
                self.lines[0].write([mono + fr, 0.0]);
                self.lines[1].write([0.0, fl]);
            } else {
                self.lines[0].write([frame[0] + fl, 0.0]);
                self.lines[1].write([0.0, frame[1] + fr]);
            }
            frame[0] += l * self.mix;
            frame[1] += r * self.mix;
        }
    }
}

struct Gate {
    rate: f64,
    threshold: f32,
    attack: f32,
    hold: u32,
    release: f32,
    range: f32,
    env: f32,
    gain: f32,
    held: u32,
}
impl Gate {
    fn new(rate: f64) -> Self {
        Self {
            rate,
            threshold: db_to_gain(-40.0),
            attack: coef(rate, 0.001),
            hold: (rate * 0.05) as u32,
            release: coef(rate, 0.1),
            range: 0.0001,
            env: 0.0,
            gain: 0.0,
            held: 0,
        }
    }
}
impl Dsp for Gate {
    fn set(&mut self, index: usize, value: f64) {
        match index {
            0 => self.threshold = db_to_gain(value),
            1 => self.attack = coef(self.rate, value / 1000.0),
            2 => self.hold = (self.rate * value / 1000.0) as u32,
            3 => self.release = coef(self.rate, value / 1000.0),
            4 => self.range = db_to_gain(value),
            _ => {}
        }
    }
    fn reset(&mut self) {
        self.env = 0.0;
        self.gain = 0.0;
    }
    fn process(&mut self, audio: &mut [[f32; 2]], _: &[NoteEvent], _: &ProcessContext) {
        let follow = coef(self.rate, 0.0005);
        let decay = coef(self.rate, 0.02);
        for frame in audio {
            let peak = frame[0].abs().max(frame[1].abs());
            self.env += if peak > self.env { follow } else { decay } * (peak - self.env);
            let open = if self.env > self.threshold {
                self.held = self.hold;
                true
            } else if self.held > 0 {
                self.held -= 1;
                true
            } else {
                false
            };
            let target = if open { 1.0 } else { self.range };
            let c = if target > self.gain {
                self.attack
            } else {
                self.release
            };
            self.gain += c * (target - self.gain);
            frame[0] *= self.gain;
            frame[1] *= self.gain;
        }
    }
}

struct Limiter {
    rate: f64,
    input: f32,
    ceiling: f32,
    release: f32,
    delay: Delay,
    lookahead: usize,
    gain: f32,
}
impl Limiter {
    fn new(rate: u32) -> Self {
        Self {
            rate: rate as f64,
            input: 1.0,
            ceiling: db_to_gain(-0.3),
            release: coef(rate as f64, 0.08),
            delay: Delay::new(0.002, rate),
            lookahead: (rate / 1000).max(1) as usize,
            gain: 1.0,
        }
    }
}
impl Dsp for Limiter {
    fn set(&mut self, index: usize, value: f64) {
        match index {
            0 => self.input = db_to_gain(value),
            1 => self.ceiling = db_to_gain(value),
            2 => self.release = coef(self.rate, value / 1000.0),
            _ => {}
        }
    }
    fn reset(&mut self) {
        self.delay.clear();
        self.gain = 1.0;
    }
    fn latency(&self) -> u32 {
        self.lookahead as u32
    }
    fn process(&mut self, audio: &mut [[f32; 2]], _: &[NoteEvent], _: &ProcessContext) {
        for frame in audio {
            let x = [frame[0] * self.input, frame[1] * self.input];
            let peak = x[0].abs().max(x[1].abs());
            let needed = if peak > self.ceiling {
                self.ceiling / peak
            } else {
                1.0
            };
            if needed < self.gain {
                self.gain = needed;
            } else {
                self.gain += self.release * (needed - self.gain);
            }
            let delayed = self.delay.read(self.lookahead as f64);
            self.delay.write(x);
            frame[0] = (delayed[0] * self.gain).clamp(-self.ceiling, self.ceiling);
            frame[1] = (delayed[1] * self.gain).clamp(-self.ceiling, self.ceiling);
        }
    }
}

struct Filter {
    rate: f64,
    kind: u8,
    cutoff: f64,
    resonance: f64,
    drive: f32,
    svf: Svf,
}
impl Filter {
    fn new(rate: f64) -> Self {
        let mut f = Self {
            rate,
            kind: 0,
            cutoff: 1000.0,
            resonance: 0.2,
            drive: 1.0,
            svf: Svf::default(),
        };
        f.svf.set(rate, f.cutoff, f.resonance);
        f
    }
}
impl Dsp for Filter {
    fn set(&mut self, index: usize, value: f64) {
        match index {
            0 => self.kind = value.round() as u8,
            1 => self.cutoff = value,
            2 => self.resonance = value / 100.0,
            3 => self.drive = db_to_gain(value),
            _ => {}
        }
        self.svf.set(self.rate, self.cutoff, self.resonance);
    }
    fn reset(&mut self) {
        self.svf.clear();
    }
    fn process(&mut self, audio: &mut [[f32; 2]], _: &[NoteEvent], _: &ProcessContext) {
        for frame in audio {
            for (c, s) in frame.iter_mut().enumerate() {
                let x = if self.drive > 1.0 {
                    (*s * self.drive).tanh()
                } else {
                    *s
                };
                let (lp, bp, hp) = self.svf.process(c, x);
                *s = match self.kind {
                    1 => hp,
                    2 => bp,
                    _ => lp,
                };
            }
        }
    }
}

struct Phaser {
    rate: f64,
    speed: f64,
    depth: f64,
    stages: usize,
    feedback: f32,
    center: f64,
    mix: f32,
    phase: f64,
    state: [[f32; 2]; 12],
    last: [f32; 2],
}
impl Phaser {
    fn new(rate: f64) -> Self {
        Self {
            rate,
            speed: 0.3,
            depth: 0.7,
            stages: 6,
            feedback: 0.3,
            center: 800.0,
            mix: 0.5,
            phase: 0.0,
            state: [[0.0; 2]; 12],
            last: [0.0; 2],
        }
    }
}
impl Dsp for Phaser {
    fn set(&mut self, index: usize, value: f64) {
        match index {
            0 => self.speed = value,
            1 => self.depth = value / 100.0,
            2 => self.stages = ((value.round() as usize + 1) * 2).clamp(2, 12),
            3 => self.feedback = (value / 100.0) as f32,
            4 => self.center = value,
            5 => self.mix = (value / 100.0) as f32,
            _ => {}
        }
    }
    fn reset(&mut self) {
        self.state = [[0.0; 2]; 12];
        self.last = [0.0; 2];
    }
    fn process(&mut self, audio: &mut [[f32; 2]], _: &[NoteEvent], _: &ProcessContext) {
        for frame in audio {
            self.phase = (self.phase + self.speed / self.rate).fract();
            let lfo = (TAU * self.phase).sin();
            let hz =
                (self.center * 2f64.powf(lfo * self.depth * 2.0)).clamp(20.0, self.rate * 0.45);
            let w = (std::f64::consts::PI * hz / self.rate).tan();
            let c = ((1.0 - w) / (1.0 + w)) as f32;
            for ch in 0..2 {
                let mut x = frame[ch] + self.last[ch] * self.feedback;
                for stage in 0..self.stages {
                    let z = &mut self.state[stage][ch];
                    let y = -c * x + *z;
                    *z = x + c * y;
                    x = y;
                }
                if !x.is_finite() {
                    x = 0.0;
                    self.state.iter_mut().for_each(|s| s[ch] = 0.0);
                }
                self.last[ch] = x;
                frame[ch] = frame[ch] * (1.0 - self.mix) + x * self.mix;
            }
        }
    }
}

struct Tremolo {
    rate: f64,
    speed: f64,
    depth: f32,
    shape: u8,
    stereo: f64,
    phase: f64,
}
impl Tremolo {
    fn new(rate: f64) -> Self {
        Self {
            rate,
            speed: 4.0,
            depth: 0.6,
            shape: 0,
            stereo: 0.0,
            phase: 0.0,
        }
    }
    fn wave(&self, phase: f64) -> f32 {
        let v = match self.shape {
            1 => triangle(phase),
            2 => {
                if phase < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            _ => (TAU * phase).sin(),
        };
        ((v + 1.0) * 0.5) as f32
    }
}
impl Dsp for Tremolo {
    fn set(&mut self, index: usize, value: f64) {
        match index {
            0 => self.speed = value,
            1 => self.depth = (value / 100.0) as f32,
            2 => self.shape = value.round() as u8,
            3 => self.stereo = value / 360.0,
            _ => {}
        }
    }
    fn process(&mut self, audio: &mut [[f32; 2]], _: &[NoteEvent], _: &ProcessContext) {
        for frame in audio {
            self.phase = (self.phase + self.speed / self.rate).fract();
            let l = 1.0 - self.depth * (1.0 - self.wave(self.phase));
            let r = 1.0 - self.depth * (1.0 - self.wave((self.phase + self.stereo).fract()));
            frame[0] *= l;
            frame[1] *= r;
        }
    }
}

#[derive(Default)]
struct Bitcrusher {
    bits: f32,
    factor: u32,
    mix: f32,
    counter: u32,
    held: [f32; 2],
}
impl Dsp for Bitcrusher {
    fn set(&mut self, index: usize, value: f64) {
        match index {
            0 => self.bits = value as f32,
            1 => self.factor = value.round().max(1.0) as u32,
            2 => self.mix = (value / 100.0) as f32,
            _ => {}
        }
    }
    fn process(&mut self, audio: &mut [[f32; 2]], _: &[NoteEvent], _: &ProcessContext) {
        let step = 2f32.powf(self.bits.max(1.0) - 1.0);
        for frame in audio {
            if self.counter == 0 {
                self.held = frame.map(|s| (s * step).round() / step);
            }
            self.counter = (self.counter + 1) % self.factor.max(1);
            for (c, s) in frame.iter_mut().enumerate() {
                *s = *s * (1.0 - self.mix) + self.held[c] * self.mix;
            }
        }
    }
}

struct Width {
    rate: f64,
    width: f32,
    bass: f32,
    lp: f32,
}
impl Width {
    fn new(rate: f64) -> Self {
        Self {
            rate,
            width: 1.2,
            bass: 0.0,
            lp: 0.0,
        }
    }
}
impl Dsp for Width {
    fn set(&mut self, index: usize, value: f64) {
        match index {
            0 => self.width = (value / 100.0) as f32,
            1 => {
                self.bass = if value < 1.0 {
                    0.0
                } else {
                    coef(self.rate, 1.0 / (TAU * value))
                }
            }
            _ => {}
        }
    }
    fn process(&mut self, audio: &mut [[f32; 2]], _: &[NoteEvent], _: &ProcessContext) {
        for frame in audio {
            let mid = (frame[0] + frame[1]) * 0.5;
            let mut side = (frame[0] - frame[1]) * 0.5;
            if self.bass > 0.0 {
                self.lp += self.bass * (side - self.lp);
                side -= self.lp;
            }
            side *= self.width;
            frame[0] = mid + side;
            frame[1] = mid - side;
        }
    }
}

#[derive(Default)]
struct Utility {
    gain: f32,
    pan: f32,
    invert: [bool; 2],
    mono: bool,
}
impl Dsp for Utility {
    fn set(&mut self, index: usize, value: f64) {
        match index {
            0 => self.gain = db_to_gain(value),
            1 => self.pan = (value / 100.0) as f32,
            2 => self.invert[0] = value >= 0.5,
            3 => self.invert[1] = value >= 0.5,
            4 => self.mono = value >= 0.5,
            _ => {}
        }
    }
    fn process(&mut self, audio: &mut [[f32; 2]], _: &[NoteEvent], _: &ProcessContext) {
        let left = (1.0 - self.pan.max(0.0)).sqrt();
        let right = (1.0 + self.pan.min(0.0)).sqrt();
        for frame in audio {
            if self.mono {
                let m = (frame[0] + frame[1]) * 0.5;
                *frame = [m, m];
            }
            for (c, s) in frame.iter_mut().enumerate() {
                if self.invert[c] {
                    *s = -*s;
                }
            }
            frame[0] *= self.gain * left;
            frame[1] *= self.gain * right;
        }
    }
}

struct Transient {
    rate: f64,
    attack: f32,
    sustain: f32,
    fast: f32,
    slow: f32,
}
impl Transient {
    fn new(rate: f64) -> Self {
        Self {
            rate,
            attack: 0.3,
            sustain: 0.0,
            fast: 0.0,
            slow: 0.0,
        }
    }
}
impl Dsp for Transient {
    fn set(&mut self, index: usize, value: f64) {
        match index {
            0 => self.attack = (value / 100.0) as f32,
            1 => self.sustain = (value / 100.0) as f32,
            _ => {}
        }
    }
    fn reset(&mut self) {
        self.fast = 0.0;
        self.slow = 0.0;
    }
    fn process(&mut self, audio: &mut [[f32; 2]], _: &[NoteEvent], _: &ProcessContext) {
        let fast_up = coef(self.rate, 0.0005);
        let fast_down = coef(self.rate, 0.02);
        let slow_up = coef(self.rate, 0.02);
        let slow_down = coef(self.rate, 0.15);
        for frame in audio {
            let peak = frame[0].abs().max(frame[1].abs());
            self.fast += if peak > self.fast { fast_up } else { fast_down } * (peak - self.fast);
            self.slow += if peak > self.slow { slow_up } else { slow_down } * (peak - self.slow);
            let diff = db(self.fast) - db(self.slow);
            let attack_db = self.attack * diff.clamp(0.0, 24.0);
            let sustain_db = self.sustain * (-diff).clamp(0.0, 24.0);
            let gain = db_to_gain((attack_db + sustain_db) as f64).clamp(0.05, 8.0);
            frame[0] *= gain;
            frame[1] *= gain;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_stock_plugin_instantiates_with_matching_parameters() {
        for name in INSTRUMENTS.iter().chain(EFFECTS.iter()) {
            let instance = create(name, 48000).expect(name);
            assert_eq!(instance.editor.params().len(), params(name).len());
            assert!(instance.processor.is_some());
        }
    }
}
