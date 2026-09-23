//! Standard MIDI File interchange. Quarter-note timing is preserved; instrument
//! patches and live controller data are reported rather than silently approximated.
use crate::{control::new_id, document, model::*, store::Command, Result};
use midly::{
    num::{u15, u24, u28, u4, u7},
    Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, VecDeque},
    path::Path,
};

const MAX_FILE: u64 = 32 * 1024 * 1024;
const PPQ: u16 = 960;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportOptions {
    pub start_bar: f64,
    pub import_tempo: bool,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub track_ids: Vec<String>,
    pub clip_ids: Vec<String>,
    pub notes: usize,
    pub file_tempo: f64,
    pub tempo_imported: bool,
    pub warnings: Vec<String>,
}

/// Decode completely and construct a single undoable batch before mutating a store.
pub fn import(
    path: &Path,
    session: &Session,
    options: &ImportOptions,
    agent: bool,
) -> Result<(Command, ImportReport)> {
    let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if metadata.len() > MAX_FILE {
        return Err("MIDI file exceeds 32 MiB".into());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    import_bytes(&bytes, session, options, agent)
}

pub fn import_bytes(
    bytes: &[u8],
    session: &Session,
    options: &ImportOptions,
    agent: bool,
) -> Result<(Command, ImportReport)> {
    if bytes.len() as u64 > MAX_FILE {
        return Err("MIDI file exceeds 32 MiB".into());
    }
    if !valid_time(options.start_bar) {
        return Err("MIDI startBar must be finite and nonnegative".into());
    }
    let smf = Smf::parse(bytes).map_err(|e| format!("Invalid MIDI file: {e}"))?;
    if smf.header.format == Format::Sequential {
        return Err(
            "SMF type 2 contains independent songs; export it as type 0 or type 1 before importing"
                .into(),
        );
    }
    let ppq = match smf.header.timing {
        Timing::Metrical(ppq) if ppq.as_int() > 0 => ppq.as_int() as f64,
        Timing::Metrical(_) => {
            return Err("MIDI ticks per quarter note must be greater than zero".into())
        }
        Timing::Timecode(_, _) => {
            return Err(
                "SMPTE MIDI timing is not supported; export with ticks per quarter note".into(),
            )
        }
    };
    if smf.tracks.len() > 1024 {
        return Err("MIDI file exceeds 1024 tracks".into());
    }
    let mut tempo_events = vec![];
    let mut meters = vec![];
    let mut lanes: Vec<(String, u8, Vec<Note>, f64)> = vec![];
    let mut ignored = 0usize;
    let mut unmatched = 0usize;
    let mut total = 0usize;
    for (track_index, events) in smf.tracks.iter().enumerate() {
        let mut tick = 0u64;
        let mut name = format!("MIDI {}", track_index + 1);
        let mut open: BTreeMap<(u8, u8), VecDeque<(u64, u8)>> = BTreeMap::new();
        let mut notes: BTreeMap<u8, Vec<Note>> = BTreeMap::new();
        for event in events {
            tick = tick
                .checked_add(event.delta.as_int() as u64)
                .ok_or("MIDI timing overflow")?;
            if tick as f64 / ppq > 1_000_000.0 {
                return Err("MIDI timing exceeds session limits".into());
            }
            match event.kind {
                TrackEventKind::Meta(MetaMessage::TrackName(value)) => {
                    name = String::from_utf8_lossy(value).chars().take(256).collect()
                }
                TrackEventKind::Meta(MetaMessage::Tempo(value)) => {
                    tempo_events.push((tick, value.as_int()))
                }
                TrackEventKind::Meta(MetaMessage::TimeSignature(n, d, _, _)) => {
                    meters.push((tick, n, d))
                }
                TrackEventKind::Midi { channel, message } => {
                    let channel = channel.as_int();
                    match message {
                        MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                            total += 1;
                            if total > 200_000 {
                                return Err("MIDI file exceeds 200,000 notes".into());
                            }
                            open.entry((channel, key.as_int()))
                                .or_default()
                                .push_back((tick, vel.as_int()));
                        }
                        MidiMessage::NoteOff { key, .. } | MidiMessage::NoteOn { key, .. } => {
                            if let Some((start, velocity)) = open
                                .get_mut(&(channel, key.as_int()))
                                .and_then(VecDeque::pop_front)
                            {
                                notes.entry(channel).or_default().push(Note {
                                    id: new_id("note"),
                                    start: start as f64 / ppq,
                                    length: (tick.saturating_sub(start).max(1)) as f64 / ppq,
                                    pitch: key.as_int(),
                                    velocity,
                                    agent,
                                });
                            } else {
                                unmatched += 1;
                            }
                        }
                        _ => ignored += 1,
                    }
                }
                TrackEventKind::SysEx(_) | TrackEventKind::Escape(_) => ignored += 1,
                _ => {}
            }
        }
        for ((channel, pitch), starts) in open {
            for (start, velocity) in starts {
                unmatched += 1;
                notes.entry(channel).or_default().push(Note {
                    id: new_id("note"),
                    start: start as f64 / ppq,
                    length: (tick.saturating_sub(start).max(1)) as f64 / ppq,
                    pitch,
                    velocity,
                    agent,
                });
            }
        }
        let split = notes.len() > 1;
        for (channel, mut notes) in notes {
            notes.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.pitch.cmp(&b.pitch)));
            let end = notes
                .iter()
                .map(|n| n.start + n.length)
                .fold(tick as f64 / ppq, f64::max);
            let label = if split {
                format!("{name} · Ch {}", channel + 1)
            } else {
                name.clone()
            };
            lanes.push((label, channel, notes, end));
        }
    }
    if lanes.is_empty() {
        return Err("The MIDI file contains no notes".into());
    }
    tempo_events.sort_by_key(|e| e.0);
    meters.sort_by_key(|e| e.0);
    // A tempo event after tick zero does not replace the MIDI default before it.
    let micros = tempo_events
        .iter()
        .filter(|(tick, _)| *tick == 0)
        .map(|(_, v)| *v)
        .next_back()
        .unwrap_or(500_000);
    if micros == 0 {
        return Err("MIDI tempo cannot be zero".into());
    }
    // A file stores whole microseconds per quarter, so 90 BPM comes back as 89.99995:
    // thousandths of a BPM are all a tempo shows, and a round trip lands where it started.
    let tempo = (60_000_000.0 / micros as f64 * 1000.0).round() / 1000.0;
    let mut warnings = vec![];
    if tempo_events
        .iter()
        .any(|(tick, v)| *tick > 0 && *v != micros)
    {
        warnings.push(
            "Later tempo changes are not imported; note positions remain in quarter-note beats."
                .into(),
        );
    }
    if meters.iter().any(|(tick, _, _)| *tick > 0) {
        warnings.push("Later time-signature changes are not imported.".into());
    }
    if ignored > 0 {
        warnings.push(format!("{ignored} controller, program, pitch-bend or SysEx events were not imported. Choose instruments in Ondera."));
    }
    if unmatched > 0 {
        warnings.push(format!("{unmatched} unmatched note events were repaired or ignored; open notes end at their source track's end."));
    }
    let mut transport = session.transport.clone();
    if options.import_tempo {
        if !(20.0..=400.0).contains(&tempo) {
            return Err("The file's initial tempo is outside Ondera's 20–400 BPM range; import with importTempo=false to retain session tempo".into());
        }
        transport.tempo = tempo;
        transport.time_signature = TimeSignature {
            numerator: 4,
            denominator: 4,
        };
        if let Some((_, numerator, exponent)) =
            meters.iter().filter(|(tick, _, _)| *tick == 0).next_back()
        {
            let denominator = 1u32
                .checked_shl(*exponent as u32)
                .ok_or("Invalid MIDI time signature")?;
            if !(1..=32).contains(numerator) || ![1, 2, 4, 8, 16, 32].contains(&denominator) {
                return Err("The file's initial time signature is unsupported".into());
            }
            transport.time_signature = TimeSignature {
                numerator: *numerator as u32,
                denominator,
            };
        }
        if !session.clips.is_empty() {
            warnings.push(
                "Initial tempo and time signature were applied to the entire existing session."
                    .into(),
            );
        }
    }
    let beats_per_bar = transport.time_signature.numerator as f64 * 4.0
        / transport.time_signature.denominator as f64;
    let mut commands = vec![];
    if options.import_tempo {
        commands.push(Command::SetTransport(transport));
    }
    let mut report = ImportReport {
        track_ids: vec![],
        clip_ids: vec![],
        notes: total,
        file_tempo: tempo,
        tempo_imported: options.import_tempo,
        warnings,
    };
    for (index, (name, channel, notes, end)) in lanes.into_iter().enumerate() {
        let track_id = new_id("track");
        let clip_id = new_id("clip");
        let mut extra = std::collections::HashMap::new();
        extra.insert("midiChannel".into(), serde_json::json!(channel));
        commands.push(Command::AddTrack(Track {
            id: track_id.clone(),
            name: name.clone(),
            kind: "midi".into(),
            color: crate::control::TRACK_PALETTE[(session.tracks.len() + index) % 8].into(),
            armed: false,
            monitor: Default::default(),
            volume: 0.75,
            pan: 0.0,
            mute: false,
            solo: false,
            extra,
        }));
        commands.push(Command::SetStrip {
            track: track_id.clone(),
            strip: Strip::default(),
        });
        commands.push(Command::PutClip(Clip {
            id: clip_id.clone(),
            name,
            agent,
            track_id: track_id.clone(),
            start_bar: options.start_bar,
            length_bars: (end / beats_per_bar).max(1.0 / PPQ as f64),
            data: ClipData::Midi { notes },
        }));
        report.track_ids.push(track_id);
        report.clip_ids.push(clip_id);
    }
    // Validate against all existing content and capacity limits before returning a batch.
    let command = Command::Batch(commands);
    let mut probe = crate::store::Store::new(session.clone())?;
    probe.dispatch(command.clone())?;
    Ok((command, report))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MidiExportReport {
    pub path: std::path::PathBuf,
    pub track_count: usize,
    pub note_count: usize,
    pub ticks_per_quarter: u16,
    pub warnings: Vec<String>,
}

/// Export arrangement MIDI as SMF type 1 at 960 PPQ, clipping notes to region
/// bounds. Every selected MIDI track is included regardless of mute/solo state.
pub fn export(
    session: &Session,
    path: &Path,
    track_ids: Option<&[String]>,
) -> Result<MidiExportReport> {
    session.validate()?;
    let selected = crate::export::select_tracks(session, track_ids)?;
    if track_ids.is_some() && selected.iter().any(|t| t.kind != "midi") {
        return Err("MIDI export accepts instrument tracks only".into());
    }
    let tracks: Vec<_> = selected.into_iter().filter(|t| t.kind == "midi").collect();
    if tracks.is_empty() {
        return Err("There are no MIDI tracks to export".into());
    }
    let tempo = (60_000_000.0 / session.transport.tempo).round() as u32;
    let meter = &session.transport.time_signature;
    let mut smf = Smf::new(Header::new(
        Format::Parallel,
        Timing::Metrical(u15::new(PPQ)),
    ));
    smf.tracks.push(vec![
        TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::new(tempo))),
        },
        TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Meta(MetaMessage::TimeSignature(
                meter.numerator as u8,
                meter.denominator.trailing_zeros() as u8,
                24,
                8,
            )),
        },
        TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
        },
    ]);
    let mut count = 0;
    for (index, track) in tracks.iter().enumerate() {
        let channel = track
            .extra
            .get("midiChannel")
            .and_then(serde_json::Value::as_u64)
            .filter(|v| *v < 16)
            .unwrap_or((index % 16) as u64) as u8;
        let mut events = vec![];
        for clip in session.clips.iter().filter(|c| c.track_id == track.id) {
            if let ClipData::Midi { notes } = &clip.data {
                let offset = clip.start_bar * session.beats_per_bar();
                let end = (clip.start_bar + clip.length_bars) * session.beats_per_bar();
                for note in notes {
                    let start = offset + note.start;
                    if start >= end {
                        continue;
                    }
                    let note_end = (start + note.length).min(end);
                    let on = (start * PPQ as f64).round() as u64;
                    let off = ((note_end * PPQ as f64).round() as u64).max(on + 1);
                    events.push((on, true, note.pitch, note.velocity));
                    events.push((off, false, note.pitch, 0));
                    count += 1;
                }
            }
        }
        events.sort_unstable_by_key(|(tick, on, pitch, _)| (*tick, *on, *pitch));
        let mut sequence = vec![TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Meta(MetaMessage::TrackName(track.name.as_bytes())),
        }];
        let mut previous = 0;
        for (tick, on, pitch, velocity) in events {
            let delta = tick - previous;
            if delta > 0x0fff_ffff {
                return Err("MIDI gap exceeds the Standard MIDI File delta-time limit".into());
            }
            sequence.push(TrackEvent {
                delta: u28::new(delta as u32),
                kind: TrackEventKind::Midi {
                    channel: u4::new(channel),
                    message: if on {
                        MidiMessage::NoteOn {
                            key: u7::new(pitch),
                            vel: u7::new(velocity),
                        }
                    } else {
                        MidiMessage::NoteOff {
                            key: u7::new(pitch),
                            vel: u7::new(0),
                        }
                    },
                },
            });
            previous = tick;
        }
        sequence.push(TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
        });
        smf.tracks.push(sequence);
    }
    document::atomic_write(path, |file| smf.write_std(file).map_err(|e| e.to_string()))?;
    Ok(MidiExportReport{path:path.into(),track_count:tracks.len(),note_count:count,ticks_per_quarter:PPQ,
        warnings:vec!["MIDI contains notes, track names and the session's initial tempo/meter. Audio, plugins, mixer settings and automation are not embedded.".into()]})
}
