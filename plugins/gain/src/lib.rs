//! Two small effects and one instrument that show how an Ondera native plugin is written.
//! `Trim` and `Tilt EQ` use only what ABI 1 had; `Bend Sine` uses what ABI 2 added: pitch
//! bend, the mod wheel, pressure, parameter changes inside a block, state of its own, a tail
//! and a latency that changes. Build this crate
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
            let gain = self.gain.step() * sign;
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

/// A one-voice sine that listens to everything ABI 2 delivers.
pub struct BendSine {
    rate: f64,
    phase: f64,
    vibrato_phase: f64,
    pitch: Option<u8>,
    level: f32,
    target: f32,
    volume: f32,
    bend: f32,
    wheel: f32,
    pressure: f32,
    lookahead: bool,
    /// Cents per scale degree from C: state that is not a parameter.
    tuning: [f32; 12],
}
impl BendSine {
    const RELEASE_SECONDS: f64 = 0.25;
    const LOOKAHEAD_FRAMES: u32 = 64;
    fn handle(&mut self, event: &Event) {
        match event.kind {
            event::NOTE_ON => {
                self.pitch = Some(event.key);
                self.target = event.value as f32 / 127.0;
            }
            event::NOTE_OFF if self.pitch == Some(event.key) => self.target = 0.0,
            event::PITCH_BEND => self.bend = event.bend_amount(),
            event::CONTROL if event.key == 1 => self.wheel = event.value as f32 / 127.0,
            event::CHANNEL_PRESSURE | event::POLY_PRESSURE => {
                self.pressure = event.value as f32 / 127.0
            }
            _ => {}
        }
    }
}
impl Plugin for BendSine {
    const INFO: Info = Info::instrument(
        "org.ondera.examples.bendsine",
        "Bend Sine",
        "Ondera Examples",
    )
    .describe("A sine voice that follows pitch bend, the mod wheel and pressure.");
    fn params() -> Vec<ParamSpec> {
        vec![
            param("Volume", -60.0, 0.0, -6.0, "dB"),
            switch("Lookahead", false),
        ]
    }
    fn new(rate: f64) -> Self {
        Self {
            rate,
            phase: 0.0,
            vibrato_phase: 0.0,
            pitch: None,
            level: 0.0,
            target: 0.0,
            volume: db_to_gain(-6.0),
            bend: 0.0,
            wheel: 0.0,
            pressure: 0.0,
            lookahead: false,
            tuning: [0.0; 12],
        }
    }
    fn set_param(&mut self, index: usize, value: f64) {
        match index {
            0 => self.volume = db_to_gain(value),
            1 => self.lookahead = value >= 0.5,
            _ => {}
        }
    }
    /// ABI 1 hosts call this: notes only, which is all they send.
    fn process(&mut self, audio: &mut [[f32; 2]], notes: &[NoteEvent], ctx: &ProcessContext) {
        let mut events = [Event::default(); 64];
        let count = notes.len().min(events.len());
        for (event, note) in events.iter_mut().zip(notes) {
            *event = (*note).into();
        }
        self.process_events(audio, &events[..count], &[], ctx);
    }
    fn process_events(
        &mut self,
        audio: &mut [[f32; 2]],
        events: &[Event],
        params: &[TimedParam],
        _: &ProcessContext,
    ) {
        let (mut next_event, mut next_param) = (0, 0);
        let step = (1.0 / (Self::RELEASE_SECONDS * self.rate)) as f32;
        for (index, frame) in audio.iter_mut().enumerate() {
            while let Some(change) = params.get(next_param).filter(|p| p.frame as usize <= index) {
                self.set_param(change.index as usize, change.value);
                next_param += 1;
            }
            while let Some(event) = events.get(next_event).filter(|e| e.frame as usize <= index) {
                self.handle(event);
                next_event += 1;
            }
            self.level += (self.target - self.level).clamp(-step, step);
            let Some(pitch) = self.pitch.filter(|_| self.level > 0.0) else {
                continue;
            };
            self.vibrato_phase = (self.vibrato_phase + 5.5 / self.rate).fract();
            let vibrato = (self.vibrato_phase * std::f64::consts::TAU).sin() as f32 * self.wheel;
            // Bend spans two semitones, the wheel half of one; tuning is in cents.
            let semitones = pitch as f32 - 69.0
                + self.bend * 2.0
                + vibrato * 0.5
                + self.tuning[pitch as usize % 12] / 100.0;
            let hz = 440.0 * 2f64.powf(semitones as f64 / 12.0);
            self.phase = (self.phase + hz / self.rate).fract();
            let value = (self.phase * std::f64::consts::TAU).sin() as f32
                * self.level
                * self.volume
                * (1.0 + self.pressure * 0.5);
            frame[0] += value;
            frame[1] += value;
        }
    }
    fn reset(&mut self) {
        self.level = 0.0;
        self.target = 0.0;
        self.pitch = None;
    }
    fn latency(&self) -> u32 {
        if self.lookahead {
            Self::LOOKAHEAD_FRAMES
        } else {
            0
        }
    }
    fn tail_seconds(&self) -> f64 {
        Self::RELEASE_SECONDS
    }
    /// Versioned text: a later release can add fields and still read this one.
    fn save(&self) -> Vec<u8> {
        if self.tuning == [0.0; 12] {
            return Vec::new();
        }
        let cents: Vec<String> = self.tuning.iter().map(|c| c.to_string()).collect();
        format!("bendsine 1\n{}", cents.join(" ")).into_bytes()
    }
    fn load(&mut self, state: &[u8]) -> Result<(), String> {
        let text = std::str::from_utf8(state).map_err(|e| e.to_string())?;
        let (header, body) = text
            .split_once('\n')
            .ok_or("Bend Sine state has no header")?;
        if header != "bendsine 1" {
            return Err(format!("Bend Sine cannot read `{header}` state"));
        }
        let cents: Vec<f32> = body
            .split_whitespace()
            .map(|c| c.parse::<f32>().map_err(|e| e.to_string()))
            .collect::<Result<_, _>>()?;
        self.tuning = cents
            .try_into()
            .map_err(|_| "Bend Sine tuning needs twelve values".to_string())?;
        Ok(())
    }
}

export_plugins!(Trim, TiltEq, BendSine);
