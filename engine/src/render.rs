use crate::{
    audio::Library,
    dsp::{Effect, Preset, Voice},
    model::{fader_gain, ClipData, Session},
    Result,
};
use std::sync::Arc;

const MAX_VOICES: usize = 256;
enum Sound {
    Midi {
        preset: Preset,
        pitch: u8,
        velocity: u8,
    },
    Audio {
        buffer: Arc<crate::audio::AudioBuffer>,
        offset: f64,
    },
}
struct Event {
    start: f64,
    end: f64,
    tail: f64,
    track: usize,
    sound: Sound,
}
struct Channel {
    gain: f32,
    pan: f32,
    effects: Vec<Effect>,
    sends: [f32; 2],
}
#[derive(Clone, Copy)]
struct Active {
    event: usize,
    voice: Option<Voice>,
}
#[derive(Clone, Copy)]
struct Preview {
    track: usize,
    voice: Voice,
    age: f64,
}

/// All allocations and source resolution happen in `new`, off the audio thread.
pub struct Renderer {
    rate: u32,
    session: Session,
    events: Vec<Event>,
    channels: Vec<Channel>,
    buses: [Effect; 2],
    active: [Option<Active>; MAX_VOICES],
    previews: [Option<Preview>; 32],
    scratch: Vec<[f32; 2]>,
    next: usize,
    position: f64,
    pub playing: bool,
    pub peak: [f32; 2],
    pub channel_peak: [f32; 2],
    pub voice_overflows: u64,
    idle_frames: u32,
    selected: Option<usize>,
}
impl Renderer {
    pub fn new(session: Session, library: &Library, rate: u32) -> Result<Self> {
        session.validate()?;
        if !(8000..=192000).contains(&rate) {
            return Err("Unsupported output sample rate".into());
        }
        let mut events = Vec::new();
        let mut channels = Vec::new();
        let bpb = session.beats_per_bar();
        let spb = 60.0 / session.transport.tempo;
        let solo = session.tracks.iter().any(|t| t.solo);
        for (index, track) in session.tracks.iter().enumerate() {
            let strip = session.strips.get(&track.id).cloned().unwrap_or_default();
            let preset = Preset::named(&strip.instrument);
            let muted = track.mute || (solo && !track.solo);
            channels.push(Channel {
                gain: if muted { 0.0 } else { fader_gain(track.volume) },
                pan: track.pan / 100.0,
                effects: strip
                    .inserts
                    .iter()
                    .filter(|s| s.state == "active")
                    .filter_map(|s| Effect::new(&s.name, rate))
                    .collect(),
                sends: std::array::from_fn(|i| {
                    if muted {
                        0.0
                    } else {
                        strip
                            .sends
                            .get(i)
                            .and_then(|s| s.level_db)
                            .map_or(0.0, |db| 10.0_f32.powf(db / 20.0))
                    }
                }),
            });
            for clip in session.clips.iter().filter(|c| c.track_id == track.id) {
                let start = clip.start_bar * bpb;
                let end = (clip.start_bar + clip.length_bars) * bpb;
                match &clip.data {
                    ClipData::Midi { notes } => {
                        for note in notes {
                            if start + note.start >= end {
                                continue;
                            }
                            let note_end = (start + note.start + note.length).min(end);
                            events.push(Event {
                                start: start + note.start,
                                end: note_end,
                                tail: preset.release() / spb,
                                track: index,
                                sound: Sound::Midi {
                                    preset,
                                    pitch: note.pitch,
                                    velocity: note.velocity,
                                },
                            });
                        }
                    }
                    ClipData::Audio {
                        source_id,
                        offset_seconds,
                    } => {
                        let buffer = library
                            .get(source_id)
                            .ok_or_else(|| format!("Missing decoded source: {source_id}"))?;
                        events.push(Event {
                            start,
                            end,
                            tail: 0.0,
                            track: index,
                            sound: Sound::Audio {
                                buffer: Arc::clone(buffer),
                                offset: *offset_seconds,
                            },
                        });
                    }
                }
            }
        }
        events.sort_by(|a, b| a.start.total_cmp(&b.start));
        let selected = session
            .tracks
            .iter()
            .position(|t| Some(&t.id) == session.view.selected_track_id.as_ref());
        let scratch = vec![[0.0; 2]; channels.len()];
        Ok(Self {
            rate,
            session,
            events,
            channels,
            buses: [
                Effect::new("Space", rate).unwrap(),
                Effect::new("Echo", rate).unwrap(),
            ],
            active: [None; MAX_VOICES],
            previews: [None; 32],
            scratch,
            next: 0,
            position: 0.0,
            playing: false,
            peak: [0.0; 2],
            channel_peak: [0.0; 2],
            voice_overflows: 0,
            idle_frames: rate * 3,
            selected,
        })
    }
    pub fn position(&self) -> f64 {
        self.position
    }
    pub fn rate(&self) -> u32 {
        self.rate
    }
    pub fn session(&self) -> &Session {
        &self.session
    }
    pub fn locate(&mut self, beats: f64) {
        self.position = beats.max(0.0);
        self.active.fill(None);
        self.next = self.events.partition_point(|e| e.start < self.position);
        for i in 0..self.next {
            if self.events[i].end + self.events[i].tail > self.position {
                self.activate(i);
            }
        }
    }
    fn activate(&mut self, i: usize) {
        let Some(slot) = self.active.iter_mut().find(|v| v.is_none()) else {
            self.voice_overflows += 1;
            return;
        };
        let e = &self.events[i];
        let spb = 60.0 / self.session.transport.tempo;
        let voice = match e.sound {
            Sound::Midi {
                preset,
                pitch,
                velocity,
            } => Some(Voice::new(
                preset,
                pitch,
                velocity,
                (e.end - e.start) * spb,
                (self.position - e.start).max(0.0) * spb,
            )),
            _ => None,
        };
        *slot = Some(Active { event: i, voice });
    }
    pub fn preview(&mut self, track: usize, pitch: u8, velocity: u8) {
        self.idle_frames = 0;
        let Some(t) = self.session.tracks.get(track) else {
            return;
        };
        if t.kind != "midi" {
            return;
        }
        let preset = self
            .session
            .strips
            .get(&t.id)
            .map_or(Preset::Synth, |s| Preset::named(&s.instrument));
        if let Some(slot) = self.previews.iter_mut().find(|v| v.is_none()) {
            *slot = Some(Preview {
                track,
                voice: Voice::new(preset, pitch.min(127), velocity.min(127), 0.3, 0.0),
                age: 0.0,
            });
        }
    }
    pub fn stop(&mut self) {
        self.idle_frames = 0;
        self.playing = false;
        self.active.fill(None);
        self.previews.fill(None);
    }
    pub fn set_selected(&mut self, index: Option<usize>) {
        self.selected = index;
    }
    pub fn begin_block(&mut self) {
        self.peak = [0.0; 2];
        self.channel_peak = [0.0; 2];
    }
    /// One stereo frame. No locks, allocation, filesystem calls or logging.
    pub fn next_frame(&mut self) -> [f32; 2] {
        if self.playing {
            self.idle_frames = 0;
        } else {
            if self.idle_frames >= self.rate * 3 {
                return [0.0; 2];
            }
            self.idle_frames += 1;
        }
        let spb = 60.0 / self.session.transport.tempo;
        let bpb = self.session.beats_per_bar();
        if self.playing && self.session.transport.cycle {
            let end = self.session.transport.cycle_end_bar * bpb;
            let start = self.session.transport.cycle_start_bar * bpb;
            if self.position >= end {
                self.locate(start + (self.position - end).rem_euclid(end - start));
            }
        }
        self.scratch.fill([0.0; 2]);
        if self.playing {
            while self.next < self.events.len()
                && self.events[self.next].start <= self.position + 1e-9
            {
                self.activate(self.next);
                self.next += 1;
            }
            for slot in &mut self.active {
                let Some(active) = slot else {
                    continue;
                };
                let e = &self.events[active.event];
                if self.position >= e.end + e.tail {
                    *slot = None;
                    continue;
                }
                let age = (self.position - e.start).max(0.0) * spb;
                let frame = match &e.sound {
                    Sound::Midi { .. } => {
                        let v = active.voice.as_mut().unwrap().sample(age, self.rate as f64);
                        [v, v]
                    }
                    Sound::Audio { buffer, offset } => {
                        let mut v = buffer.sample(age + offset);
                        // 3 ms boundary ramps avoid discontinuities when trimming or looping clips.
                        let ramp = (age / 0.003)
                            .min(1.0)
                            .min(((e.end - self.position) * spb / 0.003).clamp(0.0, 1.0))
                            as f32;
                        v[0] *= ramp;
                        v[1] *= ramp;
                        v
                    }
                };
                self.scratch[e.track][0] += frame[0];
                self.scratch[e.track][1] += frame[1];
            }
        }
        for slot in &mut self.previews {
            if let Some(p) = slot {
                let sample = p.voice.sample(p.age, self.rate as f64);
                self.scratch[p.track][0] += sample;
                self.scratch[p.track][1] += sample;
                p.age += 1.0 / self.rate as f64;
                if p.age > 0.3 + p.voice.preset.release() {
                    *slot = None;
                }
            }
        }
        let mut master = [0.0_f32; 2];
        let mut sends = [[0.0; 2]; 2];
        for (i, (channel, frame)) in self.channels.iter_mut().zip(&self.scratch).enumerate() {
            let mut v = *frame;
            for effect in &mut channel.effects {
                v = effect.process(v, self.rate);
            }
            // Stereo balance preserves channels at centre; mono source duplicates are -3 dB.
            let left = (1.0 - channel.pan.max(0.0)).sqrt();
            let right = (1.0 + channel.pan.min(0.0)).sqrt();
            v[0] *= channel.gain * left;
            v[1] *= channel.gain * right;
            for c in 0..2 {
                master[c] += v[c];
                for (bus, send) in sends.iter_mut().enumerate() {
                    send[c] += v[c] * channel.sends[bus];
                }
                if self.selected == Some(i) {
                    self.channel_peak[c] = self.channel_peak[c].max(v[c].abs());
                }
            }
        }
        for (bus, input) in self.buses.iter_mut().zip(sends) {
            let output = bus.process(input, self.rate);
            for c in 0..2 {
                master[c] += output[c] - input[c];
            }
        }
        if self.playing && self.session.transport.metronome {
            let tick_unit = 4.0 / self.session.transport.time_signature.denominator as f64;
            let time = self.position.rem_euclid(tick_unit) * spb;
            if time < 0.045 {
                let accent = (self.position / tick_unit).floor() as u64
                    % self.session.transport.time_signature.numerator as u64
                    == 0;
                let click = ((std::f64::consts::TAU * if accent { 1600.0 } else { 1100.0 } * time)
                    .sin()
                    * (-time * 140.0).exp()
                    * 0.25) as f32;
                master[0] += click;
                master[1] += click;
            }
        }
        if self.playing {
            self.position += 1.0 / (self.rate as f64 * spb);
        }
        for (c, sample) in master.iter_mut().enumerate() {
            *sample = if sample.is_finite() {
                let x = *sample * 0.7;
                if x.abs() <= 0.7 {
                    x
                } else {
                    x.signum() * (0.7 + 0.28 * ((x.abs() - 0.7) / 0.28).tanh())
                }
            } else {
                0.0
            };
            self.peak[c] = self.peak[c].max(sample.abs());
        }
        master
    }
}

/// Streaming offline bounce shares exactly the same renderer as device playback.
pub fn bounce(
    session: &Session,
    library: &Library,
    path: &std::path::Path,
    rate: u32,
) -> Result<()> {
    let mut s = session.clone();
    s.transport.cycle = false;
    s.transport.metronome = false;
    let seconds = s.end_bar() * s.beats_per_bar() * 60.0 / s.transport.tempo + 3.0;
    if seconds > 14_400.0 {
        return Err("Bounce is limited to four hours".into());
    }
    let mut renderer = Renderer::new(s, library, rate)?;
    renderer.playing = true;
    crate::document::atomic_write(path, |file| {
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: rate,
            bits_per_sample: 24,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::new(std::io::BufWriter::new(file), spec)
            .map_err(|e| e.to_string())?;
        for _ in 0..(seconds * rate as f64).ceil() as u64 {
            for sample in renderer.next_frame() {
                writer
                    .write_sample((sample * 8_388_607.0).round() as i32)
                    .map_err(|e| e.to_string())?;
            }
        }
        writer.finalize().map_err(|e| e.to_string())
    })
}
