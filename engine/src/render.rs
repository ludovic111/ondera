//! Block renderer. Sequenced notes and controller changes become `Event`s for the
//! instruments in the rack; audio regions are sampled directly. Effects, buses and the master
//! chain are rack processors too, so their state survives graph rebuilds.

use crate::{
    audio::{AudioBuffer, Library},
    automation::AutomationTarget,
    model::{
        fader_gain, is_bus, ClipData, ControllerKind, FadeCurve, Monitor, Session, BUS_A, BUS_B,
        MASTER,
    },
    plugin::{event, Event, ProcessContext, Rack, MAX_BLOCK},
    Result,
};
use std::{collections::HashMap, sync::Arc};

/// Channels with a published meter; later tracks still play but read as silent in the mixer.
pub const METER_TRACKS: usize = 32;
const MAX_VOICES: usize = 256;
const NOTE_CAPACITY: usize = 1024;
const QUEUE_CAPACITY: usize = 4096;
/// Controller slots per track: 128 control changes, then pitch bend, then channel pressure.
const CONTROLS: usize = 130;
const BEND: usize = 128;
const PRESSURE: usize = 129;
const SUSTAIN: usize = 64;
/// No value sent or known.
const UNSET: i16 = i16::MIN;
type Controls = [i16; CONTROLS];

/// The controller slot an event sets, if it is a controller event.
fn control_slot(e: &Event) -> Option<(usize, i16)> {
    match e.kind {
        event::CONTROL if e.key < 128 => Some((e.key as usize, e.value.min(127) as i16)),
        event::PITCH_BEND => Some((BEND, e.bend.clamp(-8192, 8191))),
        event::CHANNEL_PRESSURE => Some((PRESSURE, e.value.min(127) as i16)),
        _ => None,
    }
}
/// The event that sets `slot` to `value`.
fn control_event(slot: usize, value: i16, frame: u32) -> Event {
    match slot {
        BEND => Event::new(frame, event::PITCH_BEND, 0, 0, 0, value.clamp(-8192, 8191)),
        PRESSURE => Event::channel_pressure(frame, value.clamp(0, 127) as u8),
        cc => Event::control(frame, cc as u8, value.clamp(0, 127) as u8),
    }
}
/// Order within one frame: releases, then controllers, then attacks, so a bend or a pedal
/// is in place before the note it shapes and a repeated pitch can start again.
fn event_rank(e: &Event) -> u8 {
    match e.kind {
        event::NOTE_OFF => 0,
        event::NOTE_ON => 2,
        _ => 1,
    }
}
/// A stable insertion sort by frame and rank: events are nearly sorted already, and a
/// stable library sort may allocate on the audio thread.
fn sort_events(events: &mut [Event]) {
    for i in 1..events.len() {
        let mut j = i;
        let key = (events[i].frame, event_rank(&events[i]));
        while j > 0 && (events[j - 1].frame, event_rank(&events[j - 1])) > key {
            events.swap(j - 1, j);
            j -= 1;
        }
    }
}
const PREVIEW_SECONDS: f64 = 0.3;

/// Plugin parameter automation sends a value every this many frames while it moves, plus one
/// on the exact frame of each breakpoint.
pub const AUTOMATION_GRAIN: usize = 32;

fn automation_beat(beat: f64, cycle: Option<(f64, f64)>) -> f64 {
    if let Some((start, end)) = cycle {
        if beat < start {
            return start + (beat - start).rem_euclid(end - start);
        }
    }
    beat
}

enum Sound {
    Midi {
        pitch: u8,
        velocity: u8,
    },
    Audio {
        buffer: Arc<AudioBuffer>,
        offset: f64,
        /// Linear clip gain.
        gain: f32,
        /// Fade lengths in seconds; zero for none.
        fade_in: f64,
        fade_out: f64,
        curve: FadeCurve,
    },
}
/// Edge ramp every audio clip gets, fade or not, so a cut never clicks.
const EDGE_RAMP_SECONDS: f64 = 0.003;
/// The clip's gain at `age` seconds after its start with `left` seconds before its end.
#[inline]
fn clip_envelope(
    age: f64,
    left: f64,
    gain: f32,
    fade_in: f64,
    fade_out: f64,
    curve: FadeCurve,
) -> f32 {
    let mut g = (age / EDGE_RAMP_SECONDS)
        .min(1.0)
        .min((left / EDGE_RAMP_SECONDS).clamp(0.0, 1.0));
    if fade_in > 0.0 && age < fade_in {
        g *= curve.gain(age / fade_in);
    }
    if fade_out > 0.0 && left < fade_out {
        g *= curve.gain(left / fade_out);
    }
    g as f32 * gain
}
struct Scheduled {
    start: f64,
    end: f64,
    track: usize,
    sound: Sound,
}
/// A sequenced controller change.
#[derive(Clone, Copy)]
struct Control {
    beat: f64,
    track: usize,
    slot: usize,
    value: i16,
}
#[derive(Default)]
struct DelayLine {
    frames: Vec<[f32; 2]>,
    position: usize,
}
impl DelayLine {
    fn new(samples: usize) -> Self {
        Self {
            frames: vec![[0.0; 2]; samples],
            position: 0,
        }
    }
    fn process(&mut self, audio: &mut [[f32; 2]]) {
        if self.frames.is_empty() {
            return;
        }
        for frame in audio {
            std::mem::swap(frame, &mut self.frames[self.position]);
            self.position = (self.position + 1) % self.frames.len();
        }
    }
    fn adopt(&mut self, old: &Self) {
        if self.frames.len() == old.frames.len() {
            self.frames.copy_from_slice(&old.frames);
            self.position = old.position;
        }
    }
}
struct Channel {
    route: usize,
    delay: DelayLine,
    automation_offset: u32,
    volume_lane: Option<usize>,
    pan_lane: Option<usize>,
    muted: bool,
    gain: f32,
    pan: f32,
    midi: bool,
    synth: Option<u32>,
    inserts: Vec<u32>,
    sends: [f32; 2],
    monitor: Monitor,
    armed: bool,
    /// One of this track's audio clips sounded during the current block.
    clip_sounding: bool,
    /// 0-1, eased over 5 ms so the input never cuts in or out with a click.
    monitor_gain: f32,
}
#[derive(Clone, Copy)]
struct Preview {
    track: usize,
    pitch: u8,
    remaining: u32,
}
type Held = [u16; 128];
struct PluginAutomation {
    lane: usize,
    slot: u32,
    parameter: u32,
    offset: u32,
    manual_value: f64,
}

/// All allocations and source resolution happen in `new`, off the audio thread.
pub struct Renderer {
    rate: u32,
    session: Session,
    events: Vec<Scheduled>,
    controls: Vec<Control>,
    next_control: usize,
    /// What each track's instrument was last told, per controller slot.
    applied: Vec<Controls>,
    /// Scratch for the chase on locate; reserved with the graph.
    chased: Vec<Controls>,
    /// Slots a track's clips drive, which a locate may return to rest.
    sequenced: Vec<[bool; CONTROLS]>,
    channels: Vec<Channel>,
    buses: [Vec<u32>; 2],
    master: Vec<u32>,
    master_gain: f32,
    master_volume_lane: Option<usize>,
    plugin_automation: Vec<PluginAutomation>,
    plugin_instances: HashMap<String, (String, u32)>,
    manual_parameters: HashMap<(u32, u32), f64>,
    automation_resets: Vec<(u32, u32, f64)>,
    dry_delay: DelayLine,
    bus_delays: [DelayLine; 2],
    latency_samples: u32,
    automation_looped: bool,
    active: [Option<usize>; MAX_VOICES],
    held: Vec<Held>,
    expected: Vec<Held>,
    live: Vec<Held>,
    queued: Vec<(usize, Event)>,
    notes: Vec<Vec<Event>>,
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
    /// Frames of count-in click still to play before the transport starts on its own.
    count_in_left: u64,
    count_in_done: u64,
    pub peak: [f32; 2],
    pub channel_peak: [f32; 2],
    /// Post-fader peak of the first `METER_TRACKS` channels, for the mixer's meters.
    pub track_peaks: [f32; METER_TRACKS],
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
        let mut controls: Vec<(Control, bool)> = Vec::new();
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
                route: crate::midi::route_id(&track.id),
                delay: DelayLine::default(),
                automation_offset: 0,
                volume_lane: session.automation.iter().position(|lane| {
                    lane.target
                        == AutomationTarget::TrackVolume {
                            track_id: track.id.clone(),
                        }
                }),
                pan_lane: session.automation.iter().position(|lane| {
                    lane.target
                        == AutomationTarget::TrackPan {
                            track_id: track.id.clone(),
                        }
                }),
                muted,
                gain: if muted { 0.0 } else { fader_gain(track.volume) },
                pan: track.pan / 100.0,
                midi: track.kind == "midi",
                synth: if track.kind == "midi" {
                    slots.get(&strip.synth_key(&track.id)).copied()
                } else {
                    None
                },
                inserts: chain(&strip.inserts),
                monitor: if track.kind == "audio" {
                    track.monitor
                } else {
                    Monitor::Off
                },
                armed: track.armed,
                clip_sounding: false,
                monitor_gain: 0.0,
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
                    ClipData::Midi { notes, controllers } => {
                        for played in crate::controllers::playback(controllers, end - start) {
                            let slot = match played.kind {
                                ControllerKind::Cc => played.number.unwrap_or(0).min(127) as usize,
                                ControllerKind::Bend => BEND,
                                ControllerKind::Pressure => PRESSURE,
                            };
                            controls.push((
                                Control {
                                    beat: start + played.time,
                                    track: index,
                                    slot,
                                    value: played.value,
                                },
                                played.reset,
                            ));
                        }
                        for note in notes {
                            if start + note.start >= end {
                                continue;
                            }
                            events.push(Scheduled {
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
                        fade_in,
                        fade_out,
                        fade_curve,
                        gain_db,
                    } => {
                        let buffer = library
                            .get(source_id)
                            .ok_or_else(|| format!("Missing decoded source: {source_id}"))?;
                        events.push(Scheduled {
                            start,
                            end,
                            track: index,
                            sound: Sound::Audio {
                                buffer: Arc::clone(buffer),
                                offset: *offset_seconds,
                                gain: 10f32.powf(gain_db / 20.0),
                                fade_in: *fade_in,
                                fade_out: *fade_out,
                                curve: *fade_curve,
                            },
                        });
                    }
                }
            }
        }
        events.sort_by(|a, b| a.start.total_cmp(&b.start));
        // A clip's closing reset comes before the next clip's first value at the same beat.
        controls.sort_by(|a, b| a.0.beat.total_cmp(&b.0.beat).then(b.1.cmp(&a.1)));
        let controls: Vec<Control> = controls.into_iter().map(|(c, _)| c).collect();
        let mut sequenced = vec![[false; CONTROLS]; channels.len()];
        for control in &controls {
            sequenced[control.track][control.slot] = true;
        }
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
            master_volume_lane: session
                .automation
                .iter()
                .position(|lane| lane.target == AutomationTarget::MasterVolume),
            plugin_automation: session
                .automation
                .iter()
                .enumerate()
                .filter_map(|(index, lane)| {
                    if let AutomationTarget::PluginParameter {
                        insert_id,
                        parameter_id,
                        ..
                    } = &lane.target
                    {
                        slots.get(insert_id).map(|slot| PluginAutomation {
                            lane: index,
                            slot: *slot,
                            parameter: *parameter_id,
                            offset: 0,
                            manual_value: lane.manual_value,
                        })
                    } else {
                        None
                    }
                })
                .collect(),
            manual_parameters: session
                .needs()
                .iter()
                .filter_map(|need| slots.get(&need.key).map(|slot| (slot, need)))
                .flat_map(|(slot, need)| {
                    need.params
                        .iter()
                        .map(move |(param, value)| ((*slot, *param), *value))
                })
                .collect(),
            plugin_instances: session
                .needs()
                .into_iter()
                .filter_map(|need| {
                    slots
                        .get(&need.key)
                        .map(|slot| (need.key, (need.plugin, *slot)))
                })
                .collect(),
            automation_resets: Vec::with_capacity(crate::automation::MAX_LANES * 2),
            dry_delay: DelayLine::default(),
            bus_delays: std::array::from_fn(|_| DelayLine::default()),
            latency_samples: 0,
            automation_looped: false,
            buses: [bus(BUS_A), bus(BUS_B)],
            master: bus(MASTER),
            session,
            events,
            controls,
            next_control: 0,
            applied: vec![[UNSET; CONTROLS]; count],
            chased: vec![[UNSET; CONTROLS]; count],
            sequenced,
            channels,
            active: [None; MAX_VOICES],
            held: vec![[0; 128]; count],
            expected: vec![[0; 128]; count],
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
            count_in_left: 0,
            count_in_done: 0,
            peak: [0.0; 2],
            channel_peak: [0.0; 2],
            track_peaks: [0.0; METER_TRACKS],
            voice_overflows: 0,
            note_overflows: 0,
            idle_frames: rate * 3,
            selected,
        })
    }
    /// Prepare static plugin delay compensation off the audio thread. Input
    /// paths meet at the same sample before buses and again before the master.
    /// Rebuild this plan when a plugin reports changed latency.
    pub fn set_latencies(&mut self, latencies: &HashMap<u32, u32>) -> Result<()> {
        let sum = |slots: &[u32]| -> Result<u32> {
            slots.iter().try_fold(0u32, |total, slot| {
                total
                    .checked_add(*latencies.get(slot).unwrap_or(&0))
                    .ok_or_else(|| "Plugin latency overflow".to_string())
            })
        };
        let tracks: Vec<u32> = self
            .channels
            .iter()
            .map(|channel| {
                sum(&channel.inserts)?
                    .checked_add(
                        channel
                            .synth
                            .and_then(|slot| latencies.get(&slot).copied())
                            .unwrap_or(0),
                    )
                    .ok_or_else(|| "Plugin latency overflow".to_string())
            })
            .collect::<Result<_>>()?;
        let track_max = tracks.iter().copied().max().unwrap_or(0);
        let buses = [sum(&self.buses[0])?, sum(&self.buses[1])?];
        let bus_max = buses.into_iter().max().unwrap_or(0);
        let total = track_max
            .checked_add(bus_max)
            .and_then(|n| n.checked_add(sum(&self.master).ok()?))
            .ok_or("Plugin latency overflow")?;
        let allocations = tracks.iter().map(|n| (track_max - n) as u64).sum::<u64>()
            + bus_max as u64
            + buses.iter().map(|n| (bus_max - n) as u64).sum::<u64>();
        if total > self.rate * 10 || allocations > 8_388_608 {
            return Err("Plugin delay compensation exceeds ten seconds or 64 MiB".into());
        }
        let mut offsets = HashMap::new();
        for channel in &self.channels {
            let mut offset = 0;
            if let Some(slot) = channel.synth {
                offsets.insert(slot, offset);
                offset += latencies.get(&slot).copied().unwrap_or(0);
            }
            for slot in &channel.inserts {
                offsets.insert(*slot, offset);
                offset += latencies.get(slot).copied().unwrap_or(0);
            }
        }
        for bus in &self.buses {
            let mut offset = track_max;
            for slot in bus {
                offsets.insert(*slot, offset);
                offset += latencies.get(slot).copied().unwrap_or(0);
            }
        }
        let mut offset = track_max + bus_max;
        for slot in &self.master {
            offsets.insert(*slot, offset);
            offset += latencies.get(slot).copied().unwrap_or(0);
        }
        for automation in &mut self.plugin_automation {
            automation.offset = offsets.get(&automation.slot).copied().unwrap_or(0);
        }
        for (channel, latency) in self.channels.iter_mut().zip(tracks) {
            channel.automation_offset = latency;
            channel.delay = DelayLine::new((track_max - latency) as usize);
        }
        self.dry_delay = DelayLine::new(bus_max as usize);
        self.bus_delays = std::array::from_fn(|i| DelayLine::new((bus_max - buses[i]) as usize));
        self.latency_samples = total;
        Ok(())
    }
    pub fn latency_samples(&self) -> u32 {
        self.latency_samples
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
        for previous in &old.plugin_automation {
            let lane = &old.session.automation[previous.lane];
            if !lane.enabled || lane.points.is_empty() {
                continue;
            }
            let remains = self.session.automation.iter().any(|current| {
                current.target == lane.target && current.enabled && !current.points.is_empty()
            });
            if !remains {
                let AutomationTarget::PluginParameter {
                    insert_id,
                    plugin_id,
                    ..
                } = &lane.target
                else {
                    continue;
                };
                let Some((current_plugin, slot)) = self.plugin_instances.get(insert_id) else {
                    continue;
                };
                if current_plugin != plugin_id {
                    continue;
                }
                let value = self
                    .manual_parameters
                    .get(&(*slot, previous.parameter))
                    .copied()
                    .unwrap_or(previous.manual_value);
                if self.automation_resets.len() < self.automation_resets.capacity() {
                    self.automation_resets
                        .push((*slot, previous.parameter, value));
                }
            }
        }
        self.position = old.position;
        self.automation_looped = old.automation_looped
            && self.session.transport.cycle == old.session.transport.cycle
            && self.session.transport.cycle_start_bar == old.session.transport.cycle_start_bar
            && self.session.transport.cycle_end_bar == old.session.transport.cycle_end_bar;
        self.playing = old.playing;
        self.recording = old.recording;
        self.count_in_left = old.count_in_left;
        self.count_in_done = old.count_in_done;
        self.sample_time = old.sample_time;
        self.idle_frames = old.idle_frames;
        self.dry_delay.adopt(&old.dry_delay);
        for (delay, old_delay) in self.bus_delays.iter_mut().zip(&old.bus_delays) {
            delay.adopt(old_delay);
        }
        for index in 0..self.session.tracks.len() {
            if let Some(prev) = old
                .session
                .tracks
                .iter()
                .position(|t| t.id == self.session.tracks[index].id)
            {
                self.channels[index].delay.adopt(&old.channels[prev].delay);
                self.channels[index].monitor_gain = old.channels[prev].monitor_gain;
                let same_instrument = self.channels[index].synth == old.channels[prev].synth;
                if same_instrument {
                    self.held[index] = old.held[prev];
                    self.applied[index] = old.applied[prev];
                    for &(track, event) in &old.queued {
                        if track == prev {
                            self.queue(index, event);
                        }
                    }
                }
                self.live[index] = old.live[prev];
                if !same_instrument {
                    for pitch in 0..128 {
                        for _ in 0..self.live[index][pitch] {
                            self.queue(index, Event::note_on(0, pitch as u8, 96));
                        }
                    }
                }
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
    fn queue(&mut self, track: usize, mut event: Event) {
        // UI taps and MIDI callbacks can enqueue a complete note before one
        // audio block. Preserve its order and at least one audible sample;
        // sorting a zero-frame release before its attack would leave it stuck.
        if event.is_note() {
            if let Some((_, previous)) =
                self.queued.iter().rev().find(|(previous_track, previous)| {
                    *previous_track == track
                        && previous.is_note()
                        && previous.key == event.key
                        && previous.channel == event.channel
                })
            {
                let attack = previous.kind == event::NOTE_ON;
                let release = event.kind == event::NOTE_OFF;
                event.frame = event
                    .frame
                    .max(previous.frame + u32::from(attack && release));
            }
        }
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
        // Each historical event is examined once, independent of track count.
        // Storage is reserved with the graph, not allocated during a seek.
        self.expected.fill([0; 128]);
        let mut active = 0;
        for i in 0..self.next {
            let e = &self.events[i];
            if e.end <= self.position {
                continue;
            }
            if active == MAX_VOICES {
                self.voice_overflows += 1;
                continue;
            }
            self.active[active] = Some(i);
            active += 1;
            if let Sound::Midi { pitch, velocity } = e.sound {
                let track = e.track;
                let p = pitch as usize;
                self.expected[track][p] += 1;
                if self.expected[track][p] > self.held[track][p] {
                    self.held[track][p] += 1;
                    self.queue(track, Event::note_on(0, pitch, velocity));
                }
            }
        }
        for track in 0..self.channels.len() {
            for p in 0..128 {
                while self.held[track][p] > self.expected[track][p] {
                    self.held[track][p] -= 1;
                    self.queue(track, Event::note_off(0, p as u8));
                }
            }
        }
        self.chase();
    }
    /// Send each track the controller values in force at the position: the latest one its
    /// clips set before it. A lane the clips drive but have not reached yet returns a bend,
    /// pedal or pressure to rest. Values the instrument already has are not sent again.
    fn chase(&mut self) {
        self.next_control = self.controls.partition_point(|c| c.beat < self.position);
        for chased in &mut self.chased {
            chased.fill(UNSET);
        }
        for control in &self.controls[..self.next_control] {
            self.chased[control.track][control.slot] = control.value;
        }
        for track in 0..self.channels.len() {
            for slot in 0..CONTROLS {
                let current = self.applied[track][slot];
                let target = match self.chased[track][slot] {
                    UNSET
                        if self.sequenced[track][slot]
                            && matches!(slot, SUSTAIN | BEND | PRESSURE)
                            && current != UNSET
                            && current != 0 =>
                    {
                        0
                    }
                    UNSET => continue,
                    value => value,
                };
                if target != current {
                    self.applied[track][slot] = target;
                    self.queue(track, control_event(slot, target, 0));
                }
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
        self.automation_looped = false;
        self.position = beats.max(0.0);
        self.resync();
    }
    fn push_note(&mut self, track: usize, event: Event) {
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
            self.queue(track, Event::note_on(0, pitch, velocity.clamp(1, 127)));
        }
    }
    /// Live note input (keyboard or MIDI port) routed to a track's instrument.
    pub fn routed_note(&mut self, route: usize, on: bool, pitch: u8, velocity: u8) {
        if let Some(track) = self
            .channels
            .iter()
            .position(|channel| channel.route == route)
        {
            self.note(track, on, pitch, velocity);
        }
    }
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
            if on {
                Event::note_on(0, pitch, velocity.clamp(1, 127))
            } else {
                Event::note_off(0, pitch)
            },
        );
    }
    /// A live controller change (MIDI port) routed to a track's instrument.
    pub fn routed_control(&mut self, route: usize, event: Event) {
        if let Some(track) = self
            .channels
            .iter()
            .position(|channel| channel.route == route)
        {
            self.control(track, event);
        }
    }
    /// A controller change for a track's instrument now: control change, bend or pressure.
    pub fn control(&mut self, track: usize, event: Event) {
        self.idle_frames = 0;
        if !self.channels.get(track).is_some_and(|c| c.midi) {
            return;
        }
        let Some((slot, value)) = control_slot(&event) else {
            return;
        };
        self.applied[track][slot] = value;
        self.queue(track, control_event(slot, value, 0));
    }
    fn all_notes_off(&mut self) {
        for track in 0..self.channels.len() {
            for p in 0..128 {
                let total = self.held[track][p] as u32 + self.live[track][p] as u32;
                for _ in 0..total {
                    self.queue(track, Event::note_off(0, p as u8));
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
        self.count_in_left = 0;
        self.active.fill(None);
        self.all_notes_off();
        // Nothing stays bent or held after stop.
        for track in 0..self.channels.len() {
            for slot in [SUSTAIN, BEND, PRESSURE] {
                let current = self.applied[track][slot];
                if current != UNSET && current != 0 {
                    self.applied[track][slot] = 0;
                    self.queue(track, control_event(slot, 0, 0));
                }
            }
        }
    }
    pub fn set_selected(&mut self, index: Option<usize>) {
        self.selected = index;
    }
    /// Park at `beats`, click for `count_beats`, then start playing exactly on the next beat.
    pub fn count_in(&mut self, beats: f64, count_beats: f64) {
        self.locate(beats);
        self.playing = false;
        let frames = count_beats.max(0.0) * 60.0 / self.session.transport.tempo * self.rate as f64;
        self.count_in_left = frames.round() as u64;
        self.count_in_done = 0;
        if self.count_in_left == 0 {
            self.playing = true;
        }
    }
    pub fn counting_in(&self) -> bool {
        self.count_in_left > 0
    }
    pub fn begin_block(&mut self) {
        self.peak = [0.0; 2];
        self.channel_peak = [0.0; 2];
        self.track_peaks = [0.0; METER_TRACKS];
    }
    /// Some track may route the live input, so the caller should supply it.
    pub fn wants_input(&self) -> bool {
        self.channels.iter().any(|c| match c.monitor {
            Monitor::Off => false,
            Monitor::Auto => c.armed,
            Monitor::On => true,
        })
    }
    /// True while anything may still produce sound; lets the device idle.
    fn busy(&self) -> bool {
        self.playing
            || self.count_in_left > 0
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
        self.render_monitored(rack, out, &[]);
    }
    /// `render`, with the live input (one frame per output frame, or none) mixed into every
    /// monitoring track ahead of its inserts, so the strip's effects and sends apply to it.
    pub fn render_monitored(&mut self, rack: &mut Rack, out: &mut [[f32; 2]], input: &[[f32; 2]]) {
        let input = if input.len() == out.len() { input } else { &[] };
        let mut done = 0;
        while done < out.len() {
            let mut n = (out.len() - done).min(MAX_BLOCK);
            if self.count_in_left > 0 {
                // End the block on the last count-in frame so playback starts on the beat.
                n = n.min(self.count_in_left.min(MAX_BLOCK as u64) as usize);
            }
            if self.playing && self.session.transport.cycle {
                let bpb = self.session.beats_per_bar();
                let end = self.session.transport.cycle_end_bar * bpb;
                let start = self.session.transport.cycle_start_bar * bpb;
                if self.position >= end {
                    self.locate(start + (self.position - end).rem_euclid(end - start));
                    self.automation_looped = true;
                }
                let dpb = 1.0 / (self.rate as f64 * 60.0 / self.session.transport.tempo);
                let frames_left = ((end - self.position) / dpb).ceil().max(1.0) as usize;
                n = n.min(frames_left);
            }
            let live = if input.is_empty() {
                input
            } else {
                &input[done..done + n]
            };
            self.block(rack, &mut out[done..done + n], live);
            done += n;
        }
    }
    fn block(&mut self, rack: &mut Rack, out: &mut [[f32; 2]], input: &[[f32; 2]]) {
        let n = out.len();
        let monitoring = !input.is_empty() && self.wants_input();
        if !self.busy() && !monitoring {
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
        for channel in &mut self.channels {
            channel.clip_sounding = false;
        }
        self.sends[0][..n].fill([0.0; 2]);
        self.sends[1][..n].fill([0.0; 2]);
        self.mix[..n].fill([0.0; 2]);
        // Most queued notes land at frame zero. A same-callback tap's release
        // follows its attack; carry it into the next block when necessary.
        let mut remaining = 0;
        for i in 0..self.queued.len() {
            let (track, mut event) = self.queued[i];
            if event.frame < n as u32 {
                self.push_note(track, event);
            } else {
                event.frame -= n as u32;
                self.queued[remaining] = (track, event);
                remaining += 1;
            }
        }
        self.queued.truncate(remaining);
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
                self.push_note(p.track, Event::note_off(p.remaining, p.pitch));
            } else {
                self.previews[i] = Some(Preview {
                    remaining: p.remaining - n as u32,
                    ..p
                });
            }
        }
        if self.playing {
            for i in 0..n {
                while self.next_control < self.controls.len()
                    && self.controls[self.next_control].beat <= self.position + 1e-9
                {
                    let control = self.controls[self.next_control];
                    self.next_control += 1;
                    self.applied[control.track][control.slot] = control.value;
                    self.push_note(
                        control.track,
                        control_event(control.slot, control.value, i as u32),
                    );
                }
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
                            self.push_note(track, Event::note_on(i as u32, pitch, velocity));
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
                            self.push_note(track, Event::note_off(i as u32, pitch));
                        }
                        continue;
                    }
                    if let Sound::Audio {
                        buffer,
                        offset,
                        gain,
                        fade_in,
                        fade_out,
                        curve,
                    } = &e.sound
                    {
                        let age = (self.position - e.start).max(0.0) * spb;
                        let mut v = buffer.sample(age + offset);
                        // Fades and gain, on the sample; 3 ms boundary ramps keep trims and
                        // loops free of clicks even without a fade.
                        let left = (e.end - self.position) * spb;
                        let ramp = clip_envelope(age, left, *gain, *fade_in, *fade_out, *curve);
                        v[0] *= ramp;
                        v[1] *= ramp;
                        let frame = &mut self.buffers[e.track][i];
                        frame[0] += v[0];
                        frame[1] += v[1];
                        self.channels[e.track].clip_sounding = true;
                    }
                }
                self.position += dpb;
            }
        }
        // Previews may finish later in this block than sequenced note starts.
        // Every host expects chronological events; note-offs win ties so a
        // repeated pitch can start again at the same sample.
        for notes in &mut self.notes {
            sort_events(notes);
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
        let automation_step = if self.playing { dpb } else { 0.0 };
        let automation_cycle =
            (self.automation_looped && self.session.transport.cycle).then_some((
                self.session.transport.cycle_start_bar * bpb,
                self.session.transport.cycle_end_bar * bpb,
            ));
        for &(slot, parameter, value) in &self.automation_resets {
            rack.set_param(slot, parameter, value);
        }
        self.automation_resets.clear();
        // Each lane's value on the block's first frame, then at its frame wherever it crosses a
        // breakpoint and every AUTOMATION_GRAIN frames while it moves.
        for automation in &self.plugin_automation {
            let start = block_start - automation.offset as f64 * dpb;
            let first = automation_beat(start, automation_cycle);
            let mut cursor = self.session.automation[automation.lane].cursor(first);
            let Some(mut sent) = cursor.value(first) else {
                continue;
            };
            rack.set_param(automation.slot, automation.parameter, sent);
            if automation_step == 0.0 {
                continue;
            }
            let mut segment = cursor.segment();
            for frame in 1..n {
                let beat =
                    automation_beat(start + frame as f64 * automation_step, automation_cycle);
                let Some(value) = cursor.value(beat) else {
                    break;
                };
                let crossed = cursor.segment() != segment;
                segment = cursor.segment();
                if value != sent && (crossed || frame % AUTOMATION_GRAIN == 0) {
                    rack.set_param_at(automation.slot, frame as u32, automation.parameter, value);
                    sent = value;
                }
            }
        }
        for (index, channel) in self.channels.iter_mut().enumerate() {
            let buffer = &mut self.buffers[index][..n];
            let listen = monitoring
                && match channel.monitor {
                    Monitor::Off => false,
                    // Hear the input until the track has something of its own to play back;
                    // while a take is being recorded over it, hear the input again.
                    Monitor::Auto => channel.armed && (self.recording || !channel.clip_sounding),
                    Monitor::On => true,
                };
            if monitoring && (listen || channel.monitor_gain > 0.0) {
                let step = 1.0 / (0.005 * self.rate as f32);
                for (frame, live) in buffer.iter_mut().zip(input) {
                    channel.monitor_gain = if listen {
                        (channel.monitor_gain + step).min(1.0)
                    } else {
                        (channel.monitor_gain - step).max(0.0)
                    };
                    frame[0] += live[0] * channel.monitor_gain;
                    frame[1] += live[1] * channel.monitor_gain;
                }
            }
            if channel.midi {
                if let Some(slot) = channel.synth {
                    rack.process(slot, buffer, &self.notes[index], &ctx);
                }
            }
            for &slot in &channel.inserts {
                rack.process(slot, buffer, &[], &ctx);
            }
            let start_beat = block_start - channel.automation_offset as f64 * dpb;
            let mut volume = channel.volume_lane.map(|lane| {
                self.session.automation[lane].cursor(automation_beat(start_beat, automation_cycle))
            });
            let mut pan = channel.pan_lane.map(|lane| {
                self.session.automation[lane].cursor(automation_beat(start_beat, automation_cycle))
            });
            for (i, frame) in buffer.iter_mut().enumerate() {
                let beat =
                    automation_beat(start_beat + i as f64 * automation_step, automation_cycle);
                let gain = if channel.muted {
                    0.0
                } else {
                    volume
                        .as_mut()
                        .and_then(|lane| lane.value(beat))
                        .map_or(channel.gain, |value| fader_gain(value as f32))
                };
                let pan = pan
                    .as_mut()
                    .and_then(|lane| lane.value(beat))
                    .map_or(channel.pan, |value| value as f32 / 100.0);
                frame[0] *= gain * (1.0 - pan.max(0.0)).sqrt();
                frame[1] *= gain * (1.0 + pan.min(0.0)).sqrt();
            }
            channel.delay.process(buffer);
            let selected = self.selected == Some(index);
            let mut track_peak = 0.0f32;
            for (i, v) in buffer.iter().enumerate() {
                for (c, value) in v.iter().enumerate() {
                    track_peak = track_peak.max(value.abs());
                    self.mix[i][c] += value;
                    self.sends[0][i][c] += value * channel.sends[0];
                    self.sends[1][i][c] += value * channel.sends[1];
                    if selected {
                        self.channel_peak[c] = self.channel_peak[c].max(value.abs());
                    }
                }
            }
            if let Some(slot) = self.track_peaks.get_mut(index) {
                *slot = track_peak;
            }
        }
        self.dry_delay.process(&mut self.mix[..n]);
        for bus in 0..2 {
            let send = &mut self.sends[bus][..n];
            for &slot in &self.buses[bus] {
                rack.process(slot, send, &[], &ctx);
            }
            self.bus_delays[bus].process(send);
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
        let counting = !self.playing && self.count_in_left > 0;
        let tick_unit = 4.0 / self.session.transport.time_signature.denominator as f64;
        let master_beat = block_start - self.latency_samples as f64 * dpb;
        let mut master_volume = self.master_volume_lane.map(|lane| {
            self.session.automation[lane].cursor(automation_beat(master_beat, automation_cycle))
        });
        for (i, frame) in mix.iter_mut().enumerate() {
            let gain = master_volume
                .as_mut()
                .and_then(|lane| {
                    lane.value(automation_beat(
                        master_beat + i as f64 * automation_step,
                        automation_cycle,
                    ))
                })
                .map_or(self.master_gain, |value| fader_gain(value as f32));
            frame[0] *= gain;
            frame[1] *= gain;
            if metronome || counting {
                // The count-in runs on its own clock from zero; the song position stays parked.
                let position = if counting {
                    (self.count_in_done + i as u64) as f64 * dpb
                } else {
                    block_start + i as f64 * dpb
                };
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
                *sample = if sample.is_finite() { *sample } else { 0.0 };
                self.peak[c] = self.peak[c].max(sample.abs());
            }
            out[i] = *frame;
        }
        if counting {
            self.count_in_done += n as u64;
            self.count_in_left = self.count_in_left.saturating_sub(n as u64);
            if self.count_in_left == 0 {
                self.playing = true;
            }
        }
        self.sample_time += n as i64;
    }
}

/// Offline processing stays on its calling thread, retaining editors for
/// state ownership and host callbacks until after the processors stop.
pub struct OfflineRack {
    rack: Rack,
    editors: Vec<Box<dyn crate::plugin::Editor>>,
}
impl std::ops::Deref for OfflineRack {
    type Target = Rack;
    fn deref(&self) -> &Rack {
        &self.rack
    }
}
impl std::ops::DerefMut for OfflineRack {
    fn deref_mut(&mut self) -> &mut Rack {
        &mut self.rack
    }
}
impl OfflineRack {
    /// The longest tail any plugin in the session reports, and which one reports it.
    pub fn longest_tail(&self) -> Option<(f64, &str)> {
        self.editors
            .iter()
            .map(|editor| (editor.tail_seconds(), editor.descriptor().name.as_str()))
            .filter(|(tail, _)| *tail > 0.0)
            .max_by(|a, b| a.0.total_cmp(&b.0))
    }
    pub fn idle(&mut self) {
        for editor in &mut self.editors {
            editor.idle();
        }
    }
}

/// Instantiate every plugin a session needs into a fresh rack, for offline
/// rendering and tests. External plugins are loaded on the calling thread.
pub fn offline(session: &Session, library: &Library, rate: u32) -> Result<(Renderer, OfflineRack)> {
    let needs = session.needs();
    let parameter_capacity = needs
        .iter()
        .map(|need| need.params.len())
        .max()
        .unwrap_or(0);
    if parameter_capacity > 8192 {
        return Err("A plugin has more than 8192 saved parameters".into());
    }
    let mut rack = Rack::with_parameter_capacity(needs.len().max(1), parameter_capacity.max(512));
    let mut editors = Vec::with_capacity(needs.len());
    let mut slots = HashMap::new();
    let mut latencies = HashMap::new();
    for (slot, need) in needs.iter().enumerate() {
        let mut instance = crate::host::instantiate(&need.plugin, &need.name, rate)?;
        if !need.blob.is_empty() {
            let bytes = crate::host::decode_blob(&need.blob)?;
            instance.editor.load(&bytes)?;
        }
        if let Some(processor) = instance.processor.take() {
            rack.mount(slot as u32, processor);
            for (&id, &value) in &need.params {
                rack.set_param(slot as u32, id, value);
            }
        }
        latencies.insert(slot as u32, instance.editor.latency());
        slots.insert(need.key.clone(), slot as u32);
        editors.push(instance.editor);
    }
    let mut renderer = Renderer::new(session.clone(), library, rate, &slots)?;
    renderer.set_latencies(&latencies)?;
    Ok((renderer, OfflineRack { rack, editors }))
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
        let mut warmup = renderer.latency_samples() as usize;
        while warmup > 0 {
            let n = warmup.min(MAX_BLOCK);
            renderer.render(&mut rack, &mut block[..n]);
            rack.idle();
            warmup -= n;
        }
        let mut written = 0;
        while written < total {
            let n = (total - written).min(MAX_BLOCK);
            renderer.render(&mut rack, &mut block[..n]);
            rack.idle();
            for frame in &block[..n] {
                for sample in frame {
                    writer
                        .write_sample((sample.clamp(-1.0, 1.0) * 8_388_607.0).round() as i32)
                        .map_err(|e| e.to_string())?;
                }
            }
            written += n;
        }
        writer.finalize().map_err(|e| e.to_string())
    })
}
