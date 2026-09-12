//! Block renderer. Sequenced notes become `NoteEvent`s for the instruments in
//! the rack; audio regions are sampled directly. Effects, buses and the master
//! chain are rack processors too, so their state survives graph rebuilds.

use crate::{
    audio::{AudioBuffer, Library},
    model::{fader_gain, is_bus, ClipData, Session, BUS_A, BUS_B, MASTER},
    plugin::{NoteEvent, ProcessContext, Rack, MAX_BLOCK},
    Result,
};
use std::{collections::HashMap, sync::Arc};

const MAX_VOICES: usize = 256;
const NOTE_CAPACITY: usize = 512;
const QUEUE_CAPACITY: usize = 2048;
const PREVIEW_SECONDS: f64 = 0.3;

enum Sound {
    Midi {
        pitch: u8,
        velocity: u8,
    },
    Audio {
        buffer: Arc<AudioBuffer>,
        offset: f64,
    },
}
struct Event {
    start: f64,
    end: f64,
    track: usize,
    sound: Sound,
}
struct Channel {
    gain: f32,
    pan: f32,
    midi: bool,
    synth: Option<u32>,
    inserts: Vec<u32>,
    sends: [f32; 2],
}
#[derive(Clone, Copy)]
struct Preview {
    track: usize,
    pitch: u8,
    remaining: u32,
}
type Held = [u8; 128];

/// All allocations and source resolution happen in `new`, off the audio thread.
pub struct Renderer {
    rate: u32,
    session: Session,
    events: Vec<Event>,
    channels: Vec<Channel>,
    buses: [Vec<u32>; 2],
    master: Vec<u32>,
    master_gain: f32,
    active: [Option<usize>; MAX_VOICES],
    held: Vec<Held>,
    live: Vec<Held>,
    queued: Vec<(usize, NoteEvent)>,
    notes: Vec<Vec<NoteEvent>>,
    buffers: Vec<Vec<[f32; 2]>>,
    sends: [Vec<[f32; 2]>; 2],
    mix: Vec<[f32; 2]>,
    previews: [Option<Preview>; 32],
    live_total: u32,
    next: usize,
    position: f64,
    sample_time: i64,
    pub playing: bool,
    pub recording: bool,
    pub peak: [f32; 2],
    pub channel_peak: [f32; 2],
    pub voice_overflows: u64,
    pub note_overflows: u64,
    idle_frames: u32,
    selected: Option<usize>,
}
impl Renderer {
    /// `slots` maps rack keys (see `Session::needs`) to mounted rack slots.
    pub fn new(
        session: Session,
        library: &Library,
        rate: u32,
        slots: &HashMap<String, u32>,
    ) -> Result<Self> {
        session.validate()?;
        if !(8000..=192000).contains(&rate) {
            return Err("Unsupported output sample rate".into());
        }
        let mut events = Vec::new();
        let mut channels = Vec::new();
        let bpb = session.beats_per_bar();
        let solo = session.tracks.iter().any(|t| t.solo);
        let chain = |inserts: &[crate::model::Insert]| -> Vec<u32> {
            inserts
                .iter()
                .filter(|s| s.state == "active")
                .filter_map(|s| slots.get(&s.id).copied())
                .collect()
        };
        for (index, track) in session.tracks.iter().enumerate() {
            let strip = session.strips.get(&track.id).cloned().unwrap_or_default();
            let muted = track.mute || (solo && !track.solo);
            channels.push(Channel {
                gain: if muted { 0.0 } else { fader_gain(track.volume) },
                pan: track.pan / 100.0,
                midi: track.kind == "midi",
                synth: if track.kind == "midi" {
                    slots.get(&strip.synth_key(&track.id)).copied()
                } else {
                    None
                },
                inserts: chain(&strip.inserts),
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
                            events.push(Event {
                                start: start + note.start,
                                end: (start + note.start + note.length).min(end),
                                track: index,
                                sound: Sound::Midi {
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
        let bus = |id: &str| {
            session
                .strips
                .get(id)
                .map(|s| chain(&s.inserts))
                .unwrap_or_default()
        };
        let selected = session
            .tracks
            .iter()
            .position(|t| Some(&t.id) == session.view.selected_track_id.as_ref() && !is_bus(&t.id));
        let count = channels.len();
        Ok(Self {
            rate,
            master_gain: fader_gain(session.master_volume),
            buses: [bus(BUS_A), bus(BUS_B)],
            master: bus(MASTER),
            session,
            events,
            channels,
            active: [None; MAX_VOICES],
            held: vec![[0; 128]; count],
            live: vec![[0; 128]; count],
            queued: Vec::with_capacity(QUEUE_CAPACITY),
            notes: (0..count)
                .map(|_| Vec::with_capacity(NOTE_CAPACITY))
                .collect(),
            buffers: (0..count).map(|_| vec![[0.0; 2]; MAX_BLOCK]).collect(),
            sends: [vec![[0.0; 2]; MAX_BLOCK], vec![[0.0; 2]; MAX_BLOCK]],
            mix: vec![[0.0; 2]; MAX_BLOCK],
            previews: [None; 32],
            live_total: 0,
            next: 0,
            position: 0.0,
            sample_time: 0,
            playing: false,
            recording: false,
            peak: [0.0; 2],
            channel_peak: [0.0; 2],
            voice_overflows: 0,
            note_overflows: 0,
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
    /// Take over transport and held notes from the renderer being replaced.
    /// Notes that no longer exist are released; new ones are chased.
    pub fn adopt(&mut self, old: &Renderer) {
        self.position = old.position;
        self.playing = old.playing;
        self.recording = old.recording;
        self.sample_time = old.sample_time;
        self.idle_frames = old.idle_frames;
        for (index, track) in self.session.tracks.iter().enumerate() {
            if let Some(prev) = old.session.tracks.iter().position(|t| t.id == track.id) {
                self.held[index] = old.held[prev];
                self.live[index] = old.live[prev];
                self.live_total += old.live[prev].iter().map(|&c| c as u32).sum::<u32>();
                for preview in old.previews.iter().flatten() {
                    if preview.track == prev {
                        if let Some(slot) = self.previews.iter_mut().find(|p| p.is_none()) {
                            *slot = Some(Preview {
                                track: index,
                                ..*preview
                            });
                        }
                    }
                }
            }
        }
        self.resync();
    }
    fn queue(&mut self, track: usize, event: NoteEvent) {
        if self.queued.len() < self.queued.capacity() {
            self.queued.push((track, event));
        } else {
            self.note_overflows += 1;
        }
    }
    /// Re-derive active events for the current position; send note-ons for
    /// notes that should sound and note-offs for held notes that should not.
    fn resync(&mut self) {
        self.active.fill(None);
        self.next = self.events.partition_point(|e| e.start < self.position);
        // Expected counts live on the stack per track to stay allocation-free.
        for track in 0..self.channels.len() {
            let mut expected: Held = [0; 128];
            for i in 0..self.next {
                let e = &self.events[i];
                if e.track != track || e.end <= self.position {
                    continue;
                }
                if let Sound::Midi { pitch, velocity } = e.sound {
                    let p = pitch as usize;
                    expected[p] = expected[p].saturating_add(1);
                    if expected[p] > self.held[track][p] {
                        self.held[track][p] += 1;
                        self.queue(
                            track,
                            NoteEvent {
                                frame: 0,
                                on: true,
                                pitch,
                                velocity,
                                channel: 0,
                            },
                        );
                    }
                }
            }
            for (p, want) in expected.iter().enumerate() {
                while self.held[track][p] > *want {
                    self.held[track][p] -= 1;
                    self.queue(
                        track,
                        NoteEvent {
                            frame: 0,
                            on: false,
                            pitch: p as u8,
                            velocity: 0,
                            channel: 0,
                        },
                    );
                }
            }
        }
        for i in 0..self.next {
            if self.events[i].end > self.position {
                self.activate_slot(i);
            }
        }
    }
    fn activate_slot(&mut self, i: usize) -> bool {
        let Some(slot) = self.active.iter_mut().find(|v| v.is_none()) else {
            self.voice_overflows += 1;
            return false;
        };
        *slot = Some(i);
        true
    }
    pub fn locate(&mut self, beats: f64) {
        self.position = beats.max(0.0);
        self.resync();
    }
    fn push_note(&mut self, track: usize, event: NoteEvent) {
        let Some(list) = self.notes.get_mut(track) else {
            return;
        };
        if list.len() < list.capacity() {
            list.push(event);
        } else {
            self.note_overflows += 1;
        }
    }
    /// A short audition note on a track's instrument.
    pub fn preview(&mut self, track: usize, pitch: u8, velocity: u8) {
        self.idle_frames = 0;
        if !self.channels.get(track).is_some_and(|c| c.midi) {
            return;
        }
        let pitch = pitch.min(127);
        if let Some(slot) = self.previews.iter_mut().find(|v| v.is_none()) {
            *slot = Some(Preview {
                track,
                pitch,
                remaining: (PREVIEW_SECONDS * self.rate as f64) as u32,
            });
            self.live[track][pitch as usize] = self.live[track][pitch as usize].saturating_add(1);
            self.live_total += 1;
            self.queue(
                track,
                NoteEvent {
                    frame: 0,
                    on: true,
                    pitch,
                    velocity: velocity.clamp(1, 127),
                    channel: 0,
                },
            );
        }
    }
    /// Live note input (keyboard or MIDI port) routed to a track's instrument.
    pub fn note(&mut self, track: usize, on: bool, pitch: u8, velocity: u8) {
        self.idle_frames = 0;
        if !self.channels.get(track).is_some_and(|c| c.midi) {
            return;
        }
        let pitch = pitch.min(127);
        let count = &mut self.live[track][pitch as usize];
        if on {
            *count = count.saturating_add(1);
            self.live_total += 1;
        } else {
            if *count == 0 {
                return;
            }
            *count -= 1;
            self.live_total = self.live_total.saturating_sub(1);
        }
        self.queue(
            track,
            NoteEvent {
                frame: 0,
                on,
                pitch,
                velocity: if on { velocity.clamp(1, 127) } else { 0 },
                channel: 0,
            },
        );
    }
    fn all_notes_off(&mut self) {
        for track in 0..self.channels.len() {
            for p in 0..128 {
                let total = self.held[track][p] as u32 + self.live[track][p] as u32;
                for _ in 0..total {
                    self.queue(
                        track,
                        NoteEvent {
                            frame: 0,
                            on: false,
                            pitch: p as u8,
                            velocity: 0,
                            channel: 0,
                        },
                    );
                }
                self.held[track][p] = 0;
                self.live[track][p] = 0;
            }
        }
        self.live_total = 0;
        self.previews.fill(None);
    }
    pub fn stop(&mut self) {
        self.idle_frames = 0;
        self.playing = false;
        self.recording = false;
        self.active.fill(None);
        self.all_notes_off();
    }
    pub fn set_selected(&mut self, index: Option<usize>) {
        self.selected = index;
    }
    pub fn begin_block(&mut self) {
        self.peak = [0.0; 2];
        self.channel_peak = [0.0; 2];
    }
    /// True while anything may still produce sound; lets the device idle.
    fn busy(&self) -> bool {
        self.playing
            || self.idle_frames < self.rate * 3
            || self.live_total > 0
            || !self.queued.is_empty()
            || self.previews.iter().any(|p| p.is_some())
    }
    /// One stereo frame, for tests and tools. Real-time callers use `render`.
    pub fn next_frame(&mut self, rack: &mut Rack) -> [f32; 2] {
        let mut out = [[0.0; 2]];
        self.render(rack, &mut out);
        out[0]
    }
    /// Render any number of frames. No locks, allocation, filesystem calls or logging.
    pub fn render(&mut self, rack: &mut Rack, out: &mut [[f32; 2]]) {
        let mut done = 0;
        while done < out.len() {
            let mut n = (out.len() - done).min(MAX_BLOCK);
            if self.playing && self.session.transport.cycle {
                let bpb = self.session.beats_per_bar();
                let end = self.session.transport.cycle_end_bar * bpb;
                let start = self.session.transport.cycle_start_bar * bpb;
                if self.position >= end {
                    self.locate(start + (self.position - end).rem_euclid(end - start));
                }
                let dpb = 1.0 / (self.rate as f64 * 60.0 / self.session.transport.tempo);
                let frames_left = ((end - self.position) / dpb).ceil().max(1.0) as usize;
                n = n.min(frames_left);
            }
            self.block(rack, &mut out[done..done + n]);
            done += n;
        }
    }
    fn block(&mut self, rack: &mut Rack, out: &mut [[f32; 2]]) {
        let n = out.len();
        if !self.busy() {
            out.fill([0.0; 2]);
            return;
        }
        if !self.playing && self.live_total == 0 {
            self.idle_frames = self.idle_frames.saturating_add(n as u32);
        } else {
            self.idle_frames = 0;
        }
        let spb = 60.0 / self.session.transport.tempo;
        let dpb = 1.0 / (self.rate as f64 * spb);
        let bpb = self.session.beats_per_bar();
        let block_start = self.position;
        for list in &mut self.notes {
            list.clear();
        }
        for buffer in &mut self.buffers {
            buffer[..n].fill([0.0; 2]);
        }
        self.sends[0][..n].fill([0.0; 2]);
        self.sends[1][..n].fill([0.0; 2]);
        self.mix[..n].fill([0.0; 2]);
        // Queued notes (chase, live input, previews, all-notes-off) land at frame 0.
        for i in 0..self.queued.len() {
            let (track, event) = self.queued[i];
            self.push_note(track, event);
        }
        self.queued.clear();
        for i in 0..self.previews.len() {
            let Some(p) = self.previews[i] else {
                continue;
            };
            if (p.remaining as usize) < n {
                self.previews[i] = None;
                if self.live[p.track][p.pitch as usize] > 0 {
                    self.live[p.track][p.pitch as usize] -= 1;
                    self.live_total = self.live_total.saturating_sub(1);
                }
                self.push_note(
                    p.track,
                    NoteEvent {
                        frame: p.remaining,
                        on: false,
                        pitch: p.pitch,
                        velocity: 0,
                        channel: 0,
                    },
                );
            } else {
                self.previews[i] = Some(Preview {
                    remaining: p.remaining - n as u32,
                    ..p
                });
            }
        }
        if self.playing {
            for i in 0..n {
                while self.next < self.events.len()
                    && self.events[self.next].start <= self.position + 1e-9
                {
                    let index = self.next;
                    self.next += 1;
                    if self.activate_slot(index) {
                        if let Sound::Midi { pitch, velocity } = self.events[index].sound {
                            let track = self.events[index].track;
                            self.held[track][pitch as usize] =
                                self.held[track][pitch as usize].saturating_add(1);
                            self.push_note(
                                track,
                                NoteEvent {
                                    frame: i as u32,
                                    on: true,
                                    pitch,
                                    velocity,
                                    channel: 0,
                                },
                            );
                        }
                    }
                }
                for slot in 0..MAX_VOICES {
                    let Some(index) = self.active[slot] else {
                        continue;
                    };
                    let e = &self.events[index];
                    if self.position >= e.end {
                        self.active[slot] = None;
                        if let Sound::Midi { pitch, .. } = e.sound {
                            let track = e.track;
                            if self.held[track][pitch as usize] > 0 {
                                self.held[track][pitch as usize] -= 1;
                            }
                            self.push_note(
                                track,
                                NoteEvent {
                                    frame: i as u32,
                                    on: false,
                                    pitch,
                                    velocity: 0,
                                    channel: 0,
                                },
                            );
                        }
                        continue;
                    }
                    if let Sound::Audio { buffer, offset } = &e.sound {
                        let age = (self.position - e.start).max(0.0) * spb;
                        let mut v = buffer.sample(age + offset);
                        // 3 ms boundary ramps avoid discontinuities when trimming or looping clips.
                        let ramp = (age / 0.003)
                            .min(1.0)
                            .min(((e.end - self.position) * spb / 0.003).clamp(0.0, 1.0))
                            as f32;
                        v[0] *= ramp;
                        v[1] *= ramp;
                        let frame = &mut self.buffers[e.track][i];
                        frame[0] += v[0];
                        frame[1] += v[1];
                    }
                }
                self.position += dpb;
            }
        }
        let ctx = ProcessContext {
            playing: self.playing,
            recording: self.recording,
            tempo: self.session.transport.tempo,
            position_beats: block_start,
            position_seconds: block_start * spb,
            sample_time: self.sample_time,
            numerator: self.session.transport.time_signature.numerator,
            denominator: self.session.transport.time_signature.denominator,
            cycle: if self.session.transport.cycle {
                Some((
                    self.session.transport.cycle_start_bar * bpb,
                    self.session.transport.cycle_end_bar * bpb,
                ))
            } else {
                None
            },
            bar_start_beats: (block_start / bpb).floor() * bpb,
        };
        for (index, channel) in self.channels.iter().enumerate() {
            let buffer = &mut self.buffers[index][..n];
            if channel.midi {
                if let Some(slot) = channel.synth {
                    rack.process(slot, buffer, &self.notes[index], &ctx);
                }
            }
            for &slot in &channel.inserts {
                rack.process(slot, buffer, &[], &ctx);
            }
            // Stereo balance preserves channels at centre; mono source duplicates are -3 dB.
            let left = channel.gain * (1.0 - channel.pan.max(0.0)).sqrt();
            let right = channel.gain * (1.0 + channel.pan.min(0.0)).sqrt();
            let selected = self.selected == Some(index);
            for (i, frame) in buffer.iter().enumerate() {
                let v = [frame[0] * left, frame[1] * right];
                for (c, value) in v.iter().enumerate() {
                    self.mix[i][c] += value;
                    self.sends[0][i][c] += value * channel.sends[0];
                    self.sends[1][i][c] += value * channel.sends[1];
                    if selected {
                        self.channel_peak[c] = self.channel_peak[c].max(value.abs());
                    }
                }
            }
        }
        for bus in 0..2 {
            let send = &mut self.sends[bus][..n];
            for &slot in &self.buses[bus] {
                rack.process(slot, send, &[], &ctx);
            }
            for (i, frame) in send.iter().enumerate() {
                self.mix[i][0] += frame[0];
                self.mix[i][1] += frame[1];
            }
        }
        let mix = &mut self.mix[..n];
        for &slot in &self.master {
            rack.process(slot, mix, &[], &ctx);
        }
        let metronome = self.playing && self.session.transport.metronome;
        let tick_unit = 4.0 / self.session.transport.time_signature.denominator as f64;
        for (i, frame) in mix.iter_mut().enumerate() {
            frame[0] *= self.master_gain;
            frame[1] *= self.master_gain;
            if metronome {
                let position = block_start + i as f64 * dpb;
                let time = position.rem_euclid(tick_unit) * spb;
                if time < 0.045 {
                    let accent = (position / tick_unit).floor() as u64
                        % self.session.transport.time_signature.numerator as u64
                        == 0;
                    let click =
                        ((std::f64::consts::TAU * if accent { 1600.0 } else { 1100.0 } * time)
                            .sin()
                            * (-time * 140.0).exp()
                            * 0.25) as f32;
                    frame[0] += click;
                    frame[1] += click;
                }
            }
            for (c, sample) in frame.iter_mut().enumerate() {
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
            out[i] = *frame;
        }
        self.sample_time += n as i64;
    }
}

/// Instantiate every plugin a session needs into a fresh rack, for offline
/// rendering and tests. External plugins are loaded on the calling thread.
pub fn offline(session: &Session, library: &Library, rate: u32) -> Result<(Renderer, Rack)> {
    let needs = session.needs();
    let mut rack = Rack::new(needs.len().max(1));
    let mut slots = HashMap::new();
    for (slot, need) in needs.iter().enumerate() {
        let mut instance = crate::host::instantiate(&need.plugin, &need.name, rate)?;
        if !need.blob.is_empty() {
            if let Ok(bytes) = crate::host::decode_blob(&need.blob) {
                instance.editor.load(&bytes)?;
            }
        }
        if let Some(processor) = instance.processor.take() {
            rack.mount(slot as u32, processor);
            for (&id, &value) in &need.params {
                rack.set_param(slot as u32, id, value);
            }
        }
        slots.insert(need.key.clone(), slot as u32);
        // Editors are dropped here; offline rendering keeps only processors.
        drop(instance);
    }
    let renderer = Renderer::new(session.clone(), library, rate, &slots)?;
    Ok((renderer, rack))
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
    let (mut renderer, mut rack) = offline(&s, library, rate)?;
    renderer.playing = true;
    renderer.locate(0.0);
    crate::document::atomic_write(path, |file| {
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: rate,
            bits_per_sample: 24,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::new(std::io::BufWriter::new(file), spec)
            .map_err(|e| e.to_string())?;
        let total = (seconds * rate as f64).ceil() as usize;
        let mut block = [[0.0f32; 2]; MAX_BLOCK];
        let mut written = 0;
        while written < total {
            let n = (total - written).min(MAX_BLOCK);
            renderer.render(&mut rack, &mut block[..n]);
            for frame in &block[..n] {
                for sample in frame {
                    writer
                        .write_sample((sample * 8_388_607.0).round() as i32)
                        .map_err(|e| e.to_string())?;
                }
            }
            written += n;
        }
        writer.finalize().map_err(|e| e.to_string())
    })
}
