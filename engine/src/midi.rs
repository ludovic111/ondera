//! MIDI input. Notes, control changes, pitch bend and channel pressure go straight to the
//! audio thread for live playing and are also queued, timestamped in beats, for recording
//! and the interface.

use crate::{
    device::{Message, Sender, Telemetry},
    plugin::{event, Event},
    Result,
};
use midir::{MidiInput as Port, MidiInputConnection};
use rtrb::{Consumer, RingBuffer};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};

/// No track is routed.
pub const UNROUTED: usize = usize::MAX;

/// Stable identity across track selection, insertion and reordering. Calculated
/// off the MIDI callback and matched against the renderer's compiled channels.
pub fn route_id(track: &str) -> usize {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in track.bytes() {
        hash = (hash ^ byte as u64).wrapping_mul(0x100000001b3);
    }
    (hash as usize) & (usize::MAX >> 1)
}
pub struct NoteOwners {
    tracks: [usize; 16 * 128],
}
impl Default for NoteOwners {
    fn default() -> Self {
        Self {
            tracks: [UNROUTED; 16 * 128],
        }
    }
}
impl NoteOwners {
    /// Return a previous owner to release before a retrigger, then this event's
    /// owner. Releases always belong to the corresponding attack's track.
    pub fn event(
        &mut self,
        on: bool,
        pitch: u8,
        channel: u8,
        current: usize,
    ) -> (Option<usize>, Option<usize>) {
        if pitch > 127 || channel > 15 {
            return (None, None);
        }
        let owner = &mut self.tracks[channel as usize * 128 + pitch as usize];
        let previous = std::mem::replace(owner, if on { current } else { UNROUTED });
        if on {
            (
                (previous != UNROUTED).then_some(previous),
                (current != UNROUTED).then_some(current),
            )
        } else {
            (None, (previous != UNROUTED).then_some(previous))
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoutedEvent {
    pub route: usize,
    pub on: bool,
    pub pitch: u8,
    pub velocity: u8,
    pub channel: u8,
}
/// Fixed storage; controller messages emit at most 128 releases per channel.
pub struct MidiNotes {
    owners: NoteOwners,
    pedal: [bool; 16],
    deferred: [bool; 16 * 128],
}
impl Default for MidiNotes {
    fn default() -> Self {
        Self {
            owners: NoteOwners::default(),
            pedal: [false; 16],
            deferred: [false; 16 * 128],
        }
    }
}
impl MidiNotes {
    fn release(&mut self, pitch: u8, channel: u8, emit: &mut impl FnMut(RoutedEvent)) {
        self.deferred[channel as usize * 128 + pitch as usize] = false;
        if let (_, Some(route)) = self.owners.event(false, pitch, channel, UNROUTED) {
            emit(RoutedEvent {
                route,
                on: false,
                pitch,
                velocity: 0,
                channel,
            });
        }
    }
    pub fn reset(&mut self, mut emit: impl FnMut(RoutedEvent)) {
        for channel in 0..16 {
            for pitch in 0..128 {
                self.release(pitch, channel, &mut emit);
            }
        }
        self.pedal.fill(false);
    }
    pub fn receive(&mut self, bytes: &[u8], current: usize, mut emit: impl FnMut(RoutedEvent)) {
        if bytes.len() < 3 || bytes[1] > 127 || bytes[2] > 127 {
            return;
        }
        let channel = bytes[0] & 0x0f;
        let pitch = bytes[1];
        match bytes[0] & 0xf0 {
            0x90 if bytes[2] > 0 => {
                self.deferred[channel as usize * 128 + pitch as usize] = false;
                let (prior, owner) = self.owners.event(true, pitch, channel, current);
                if let Some(route) = prior {
                    emit(RoutedEvent {
                        route,
                        on: false,
                        pitch,
                        velocity: 0,
                        channel,
                    });
                }
                if let Some(route) = owner {
                    emit(RoutedEvent {
                        route,
                        on: true,
                        pitch,
                        velocity: bytes[2],
                        channel,
                    });
                }
            }
            0x80 | 0x90 => {
                if self.pedal[channel as usize] {
                    self.deferred[channel as usize * 128 + pitch as usize] = true;
                } else {
                    self.release(pitch, channel, &mut emit);
                }
            }
            0xb0 if pitch == 64 => {
                self.pedal[channel as usize] = bytes[2] >= 64;
                if !self.pedal[channel as usize] {
                    for note in 0..128 {
                        if self.deferred[channel as usize * 128 + note as usize] {
                            self.release(note, channel, &mut emit);
                        }
                    }
                }
            }
            0xb0 if pitch == 120 || pitch == 123 => {
                self.pedal[channel as usize] = false;
                for note in 0..128 {
                    self.release(note, channel, &mut emit);
                }
            }
            _ => {}
        }
    }
}

/// A controller message worth playing and recording: control changes 0-119 (120-127 are
/// channel mode messages, handled with the notes), pitch bend, channel pressure and
/// polyphonic key pressure.
pub fn controller(bytes: &[u8]) -> Option<Event> {
    let e = Event::from_midi(0, bytes)?;
    match e.kind {
        event::CONTROL if e.key < 120 => Some(e),
        event::PITCH_BEND | event::CHANNEL_PRESSURE | event::POLY_PRESSURE => Some(e),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MidiEvent {
    pub beats: f64,
    pub playing: bool,
    pub on: bool,
    pub pitch: u8,
    pub velocity: u8,
    pub channel: u8,
    /// A control change, bend or pressure instead of a note; `on`, `pitch` and `velocity`
    /// are then unused.
    pub control: Option<Event>,
}
pub struct MidiInput {
    _connection: MidiInputConnection<()>,
    pub port_name: String,
    events: Consumer<MidiEvent>,
    pub route: Arc<AtomicUsize>,
    pub failed: Arc<AtomicBool>,
    reset: Arc<AtomicBool>,
    checked_connection: std::time::Instant,
    pub disconnected: bool,
}
pub fn ports() -> Vec<String> {
    let Ok(input) = Port::new("Ondera") else {
        return vec![];
    };
    input
        .ports()
        .iter()
        .filter_map(|p| input.port_name(p).ok())
        .collect()
}
/// Connect the named port (or the first one). `route` selects a stable track ID
/// that receives live notes and may be changed at any time.
pub fn connect(
    port: Option<&str>,
    telemetry: Arc<Telemetry>,
    sender: Sender,
    route: Arc<AtomicUsize>,
) -> Result<MidiInput> {
    let mut input = Port::new("Ondera").map_err(|e| e.to_string())?;
    input.ignore(midir::Ignore::All);
    let ports = input.ports();
    let chosen = match port {
        Some(name) => ports
            .iter()
            .find(|p| input.port_name(p).is_ok_and(|n| n == name))
            .cloned()
            .ok_or_else(|| format!("MIDI port not found: {name}"))?,
        None => ports.first().cloned().ok_or("No MIDI input ports")?,
    };
    let port_name = input.port_name(&chosen).unwrap_or_default();
    let (mut producer, events) = RingBuffer::new(4096);
    let route_for_callback = route.clone();
    let failed = Arc::new(AtomicBool::new(false));
    let callback_failed = failed.clone();
    let mut notes = MidiNotes::default();
    let reset = Arc::new(AtomicBool::new(false));
    let callback_reset = reset.clone();
    let connection = input
        .connect(
            &chosen,
            "ondera-in",
            move |_, bytes, _| {
                // Channel pressure is the one two-byte message played and recorded.
                if bytes.len() < 2 || callback_failed.load(Ordering::Relaxed) {
                    return;
                }
                let mut overflow = false;
                let beats = telemetry.beats();
                let playing = telemetry.playing.load(Ordering::Relaxed);
                let mut emit = |event: RoutedEvent| {
                    overflow |= sender
                        .send(Message::RoutedNote {
                            route: event.route,
                            on: event.on,
                            pitch: event.pitch,
                            velocity: event.velocity,
                            channel: event.channel,
                        })
                        .is_err();
                    overflow |= producer
                        .push(MidiEvent {
                            beats,
                            playing,
                            on: event.on,
                            pitch: event.pitch,
                            velocity: event.velocity,
                            channel: event.channel,
                            control: None,
                        })
                        .is_err();
                };
                if callback_reset.swap(false, Ordering::AcqRel) {
                    notes.reset(&mut emit);
                }
                let route = route_for_callback.load(Ordering::Relaxed);
                notes.receive(bytes, route, &mut emit);
                if let Some(control) = controller(bytes) {
                    if route != UNROUTED {
                        overflow |= sender
                            .send(Message::RoutedControl {
                                route,
                                event: control,
                            })
                            .is_err();
                    }
                    overflow |= producer
                        .push(MidiEvent {
                            beats,
                            playing,
                            on: false,
                            pitch: 0,
                            velocity: 0,
                            channel: control.channel,
                            control: Some(control),
                        })
                        .is_err();
                }
                if overflow {
                    callback_failed.store(true, Ordering::Relaxed);
                    telemetry.input_overflow.store(true, Ordering::Release);
                }
            },
            (),
        )
        .map_err(|e| e.to_string())?;
    Ok(MidiInput {
        _connection: connection,
        port_name,
        events,
        route,
        failed,
        reset,
        checked_connection: std::time::Instant::now(),
        disconnected: false,
    })
}
impl MidiInput {
    pub fn reset_notes(&mut self) {
        self.reset.store(true, Ordering::Release);
    }
    pub fn check_connection(&mut self) {
        if self.checked_connection.elapsed() < std::time::Duration::from_secs(1) {
            return;
        }
        self.checked_connection = std::time::Instant::now();
        if !ports().iter().any(|port| port == &self.port_name) {
            self.disconnected = true;
            self.failed.store(true, Ordering::Relaxed);
        }
    }
    pub fn drain(&mut self) -> Vec<MidiEvent> {
        let mut out = vec![];
        while let Ok(e) = self.events.pop() {
            out.push(e);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn controllers_are_read_from_their_bytes_and_channel_mode_messages_are_not() {
        let cc = controller(&[0xb3, 1, 90]).unwrap();
        assert_eq!(
            (cc.kind, cc.channel, cc.key, cc.value),
            (event::CONTROL, 3, 1, 90)
        );
        let bend = controller(&[0xe0, 0x00, 0x60]).unwrap();
        assert_eq!(bend.kind, event::PITCH_BEND);
        assert_eq!(bend.bend, 0x3000 - 8192);
        assert_eq!(bend.to_midi(), Some([0xe0, 0x00, 0x60]));
        let pressure = controller(&[0xd0, 70]).unwrap();
        assert_eq!(
            (pressure.kind, pressure.value),
            (event::CHANNEL_PRESSURE, 70)
        );
        let poly = controller(&[0xa2, 61, 33]).unwrap();
        assert_eq!(
            (poly.kind, poly.channel, poly.key, poly.value),
            (event::POLY_PRESSURE, 2, 61, 33)
        );
        assert!(controller(&[0xb0, 123, 0]).is_none());
        assert!(controller(&[0x90, 60, 100]).is_none());
        assert!(
            controller(&[0xe0, 0x00]).is_none(),
            "A truncated bend is dropped"
        );
    }
    #[test]
    fn sustain_holds_releases_per_channel_and_preserves_the_attack_route() {
        let mut notes = MidiNotes::default();
        let mut events = vec![];
        notes.receive(&[0x90, 60, 100], 1, |event| events.push(event));
        notes.receive(&[0xb0, 64, 127], 1, |event| events.push(event));
        notes.receive(&[0x80, 60, 0], 2, |event| events.push(event));
        notes.receive(&[0x91, 64, 100], 2, |event| events.push(event));
        notes.receive(&[0x81, 64, 0], 2, |event| events.push(event));
        assert_eq!(
            events
                .iter()
                .map(|e| (e.channel, e.on, e.pitch))
                .collect::<Vec<_>>(),
            vec![(0, true, 60), (1, true, 64), (1, false, 64)]
        );
        notes.receive(&[0xb0, 64, 0], 2, |event| events.push(event));
        assert_eq!(
            events.last().unwrap(),
            &RoutedEvent {
                route: 1,
                on: false,
                pitch: 60,
                velocity: 0,
                channel: 0
            }
        );
        notes.receive(&[0xb0, 64, 0], 2, |event| events.push(event));
        assert_eq!(events.len(), 4);
    }
    #[test]
    fn sustained_retrigger_all_notes_off_and_reset_release_each_voice_once() {
        for ending in [Some(120), Some(123), None] {
            let mut notes = MidiNotes::default();
            let mut events = vec![];
            notes.receive(&[0xb0, 64, 127], 1, |e| events.push(e));
            notes.receive(&[0x90, 60, 100], 1, |e| events.push(e));
            notes.receive(&[0x80, 60, 0], 2, |e| events.push(e));
            notes.receive(&[0x90, 60, 90], 2, |e| events.push(e));
            assert_eq!(
                events.iter().map(|e| (e.route, e.on)).collect::<Vec<_>>(),
                vec![(1, true), (1, false), (2, true)]
            );
            notes.receive(&[0x80, 60, 0], 3, |e| events.push(e));
            if let Some(controller) = ending {
                notes.receive(&[0xb0, controller, 0], 3, |e| events.push(e));
            } else {
                notes.reset(|e| events.push(e));
            }
            assert_eq!(events.last().unwrap().route, 2);
            assert!(!events.last().unwrap().on);
            assert_eq!(events.len(), 4);
            notes.receive(&[0x90, 67, 100], 3, |e| events.push(e));
            notes.receive(&[0x80, 67, 0], 3, |e| events.push(e));
            assert_eq!(events.len(), 6, "Reset must clear the sustain pedal too");
        }
    }
    #[test]
    fn note_release_keeps_its_attack_owner_when_route_changes_and_retriggers_release_previous_owner(
    ) {
        let mut owners = NoteOwners::default();
        let first = route_id("first-track");
        let second = route_id("second-track");
        assert_eq!(owners.event(true, 60, 0, first), (None, Some(first)));
        assert_eq!(owners.event(false, 60, 0, second), (None, Some(first)));
        assert_eq!(owners.event(false, 60, 0, second), (None, None));
        owners.event(true, 60, 0, first);
        assert_eq!(
            owners.event(true, 60, 0, second),
            (Some(first), Some(second))
        );
        assert_eq!(owners.event(false, 60, 0, UNROUTED), (None, Some(second)));
        assert_eq!(owners.event(true, 255, 0, first), (None, None));
    }
}
