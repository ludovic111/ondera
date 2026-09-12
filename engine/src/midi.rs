//! MIDI input. Notes go straight to the audio thread for live playing and are
//! also queued, timestamped in beats, for recording and the interface.

use crate::{
    device::{Message, Sender, Telemetry},
    Result,
};
use midir::{MidiInput as Port, MidiInputConnection};
use rtrb::{Consumer, RingBuffer};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

/// No track is routed.
pub const UNROUTED: usize = usize::MAX;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MidiEvent {
    pub beats: f64,
    pub playing: bool,
    pub on: bool,
    pub pitch: u8,
    pub velocity: u8,
    pub channel: u8,
}
pub struct MidiInput {
    _connection: MidiInputConnection<()>,
    pub port_name: String,
    events: Consumer<MidiEvent>,
    pub route: Arc<AtomicUsize>,
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
/// Connect the named port (or the first one). `route` selects the track index
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
    let connection = input
        .connect(
            &chosen,
            "ondera-in",
            move |_, bytes, _| {
                if bytes.len() < 3 {
                    return;
                }
                let status = bytes[0] & 0xf0;
                let channel = bytes[0] & 0x0f;
                let (on, pitch, velocity) = match status {
                    0x90 if bytes[2] > 0 => (true, bytes[1], bytes[2]),
                    0x90 | 0x80 => (false, bytes[1], 0),
                    _ => return,
                };
                let track = route_for_callback.load(Ordering::Relaxed);
                if track != UNROUTED {
                    let _ = sender.send(Message::Note {
                        track,
                        on,
                        pitch,
                        velocity,
                    });
                }
                let _ = producer.push(MidiEvent {
                    beats: telemetry.beats(),
                    playing: telemetry.playing.load(Ordering::Relaxed),
                    on,
                    pitch,
                    velocity,
                    channel,
                });
            },
            (),
        )
        .map_err(|e| e.to_string())?;
    Ok(MidiInput {
        _connection: connection,
        port_name,
        events,
        route,
    })
}
impl MidiInput {
    pub fn drain(&mut self) -> Vec<MidiEvent> {
        let mut out = vec![];
        while let Ok(e) = self.events.pop() {
            out.push(e);
        }
        out
    }
}
