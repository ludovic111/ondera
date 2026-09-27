//! Buses and routing. A bus is a track of kind `bus` (`track.add kind=bus`): it holds no
//! clips, sums whatever tracks route to it (`track.setOutput`, a group) or send to it
//! (`strip.setSend`, an aux return), and runs that through its own inserts, fader and pan to
//! the Stereo Out. Tracks feed bus tracks, bus tracks feed A · Reverb, B · Delay and the
//! Stereo Out, so the mix never loops. `track.group` makes a bus and routes tracks to it in
//! one undo step.
use crate::{
    control::{
        edit, find_track, full_strip, new_track, opt, req, strip_json, track_json, Args, Host,
        Kind, Spec, TRACK_ID, TRACK_PALETTE,
    },
    model::{Session, Track, BUS_A, BUS_B, MAX_SENDS},
    store::Command,
    Result,
};
use serde_json::{json, Value};

pub const SPECS: &[Spec] = &[
    edit("track.setOutput", "Route a track's fader to a bus track (a group: drums into a Drums bus) or back to the Stereo Out, like the inspector's Output menu. Bus tracks always feed the Stereo Out. One undo step.", &[
        TRACK_ID,
        req("output", Kind::String, "A bus track's id or name, or \"Stereo Out\" (also master or none)."),
    ]),
    edit("strip.setSend", "Point one of a track's sends at a bus and/or set its level, like a send knob and its menu. Sends 0 and 1 feed A · Reverb and B · Delay unless pointed elsewhere; sends 2 and 3 exist once pointed at a bus. A track sends to bus tracks, A or B; a bus track to A or B. One undo step.", &[
        TRACK_ID,
        req("send", Kind::Integer, "Send 0-3."),
        opt("bus", Kind::String, "Where it goes: a bus track's id or name, A (A · Reverb) or B (B · Delay). \"none\" removes send 2 or 3, or gives send 0 or 1 back to A or B, off."),
        opt("levelDb", Kind::Number, "Level in dB, -100 to 0. Omit to keep it; null for off."),
    ]),
    edit("track.group", "Make a bus and route tracks to it, like selecting tracks and choosing Group into Bus: a drum group, a vocal group. The bus comes right after the last of them. One undo step.", &[
        req("trackIds", Kind::Array, "Tracks to route, by id or name (not bus tracks)."),
        opt("name", Kind::String, "The bus's name. Defaults to \"Group N\"."),
    ]),
];

/// A bus track by id or name (ignoring case), or an error that lists the buses.
pub(crate) fn find_bus<'a>(s: &'a Session, text: &str) -> Result<&'a Track> {
    let wanted = text.trim().to_lowercase();
    s.tracks
        .iter()
        .filter(|t| t.is_bus())
        .find(|t| t.id == text || t.name.to_lowercase() == wanted)
        .ok_or_else(|| {
            let buses: Vec<&str> = s
                .tracks
                .iter()
                .filter(|t| t.is_bus())
                .map(|t| t.name.as_str())
                .collect();
            if buses.is_empty() {
                format!("No bus named `{text}`: the song has no bus tracks yet (track.add kind=bus, or track.group)")
            } else {
                format!("No bus named `{text}`. Buses: {}", buses.join(", "))
            }
        })
}

/// What a send is pointed at, by name, for strips and the overview.
pub(crate) fn send_name(s: &Session, index: usize, target: Option<&str>) -> String {
    match target {
        Some(BUS_A) => "A · Reverb".into(),
        Some(BUS_B) => "B · Delay".into(),
        Some(id) => s
            .tracks
            .iter()
            .find(|t| t.id == id)
            .map_or_else(|| id.to_string(), |t| t.name.clone()),
        None => format!("Send {}", index + 1),
    }
}

/// Where a track's fader goes, by name.
pub(crate) fn output_name(s: &Session, t: &Track) -> String {
    t.output
        .as_deref()
        .and_then(|id| s.tracks.iter().find(|b| b.id == id))
        .map_or_else(|| "Stereo Out".to_string(), |b| b.name.clone())
}

fn is_stereo_out(text: &str) -> bool {
    matches!(
        text.trim().to_lowercase().as_str(),
        "stereo out" | "master" | "none" | "" | "main" | "stereo"
    )
}

pub(crate) fn call(host: &mut dyn Host, name: &str, a: &Args) -> Result<Value> {
    match name {
        "track.setOutput" => {
            let s = host.store().session();
            let mut track = find_track(s, a.str("trackId")?)?.clone();
            let output = a.str("output")?;
            track.output = if is_stereo_out(output) {
                None
            } else {
                if track.is_bus() {
                    return Err(format!(
                        "\"{}\" is a bus: buses feed the Stereo Out, not other buses",
                        track.name
                    ));
                }
                Some(find_bus(s, output)?.id.clone())
            };
            let id = track.id.clone();
            host.dispatch(Command::UpdateTrack(track))?;
            let s = host.store().session();
            Ok(track_json(s, find_track(s, &id)?))
        }
        "strip.setSend" => {
            let s = host.store().session();
            let id = a.str("trackId")?.to_string();
            let track = find_track(s, &id)?;
            let index = a.int("send")?;
            if !(0..MAX_SENDS as i64).contains(&index) {
                return Err(format!("Send must be 0 to {}", MAX_SENDS - 1));
            }
            let index = index as usize;
            let mut strip = full_strip(s, &id);
            if index > strip.sends.len() {
                return Err(format!(
                    "Point send {} at a bus first; this strip has {} sends",
                    strip.sends.len(),
                    strip.sends.len()
                ));
            }
            let bus = a.opt_str("bus");
            let remove = bus.is_some_and(|b| b.trim().eq_ignore_ascii_case("none"));
            if index == strip.sends.len() {
                if bus.is_none() || remove {
                    return Err(format!("Send {index} needs a bus to point at"));
                }
                strip.sends.push(crate::model::Send {
                    level_db: None,
                    name: String::new(),
                    bus: None,
                });
            }
            if remove {
                if index >= 2 {
                    strip.sends.remove(index);
                } else {
                    strip.sends[index].bus = None;
                    strip.sends[index].level_db = None;
                }
            } else {
                if let Some(bus) = bus {
                    let target = match bus.trim().to_lowercase().as_str() {
                        "a" | "bus-a" | "a · reverb" | "reverb" => BUS_A.to_string(),
                        "b" | "bus-b" | "b · delay" | "delay" => BUS_B.to_string(),
                        _ => {
                            if track.is_bus() {
                                return Err(
                                    "A bus sends to A or B; buses do not feed other buses".into()
                                );
                            }
                            find_bus(s, bus)?.id.clone()
                        }
                    };
                    // The first two keep their default out of the file.
                    let default = [BUS_A, BUS_B].get(index).copied();
                    strip.sends[index].bus = (default != Some(target.as_str())).then_some(target);
                }
                match a.get("levelDb") {
                    Some(Value::Null) => strip.sends[index].level_db = None,
                    Some(_) => {
                        let db = a.f64("levelDb")?;
                        if !(-100.0..=0.0).contains(&db) {
                            return Err("Send level must be between -100 and 0 dB".into());
                        }
                        strip.sends[index].level_db = Some(db as f32);
                    }
                    None => {}
                }
            }
            let s = host.store().session();
            strip.sends[..]
                .iter_mut()
                .enumerate()
                .for_each(|(i, send)| {
                    send.name = send_name(s, i, send.target(i));
                });
            host.dispatch(Command::SetStrip {
                track: id.clone(),
                strip,
            })?;
            Ok(strip_json(host.store().session(), &id))
        }
        "track.group" => {
            let s = host.store().session();
            let ids: Vec<String> = a
                .get("trackIds")
                .and_then(Value::as_array)
                .ok_or("trackIds must be an array of track ids or names")?
                .iter()
                .map(|v| {
                    let text = v.as_str().ok_or("trackIds holds strings")?;
                    let wanted = text.to_lowercase();
                    s.tracks
                        .iter()
                        .find(|t| t.id == text)
                        .or_else(|| s.tracks.iter().find(|t| t.name.to_lowercase() == wanted))
                        .map(|t| t.id.clone())
                        .ok_or_else(|| find_track(s, text).err().unwrap_or_default())
                })
                .collect::<Result<_>>()?;
            if ids.is_empty() {
                return Err("trackIds needs at least one track".into());
            }
            if let Some(bus) = ids
                .iter()
                .filter_map(|id| s.tracks.iter().find(|t| &t.id == id))
                .find(|t| t.is_bus())
            {
                return Err(format!(
                    "\"{}\" is a bus; group tracks, not buses",
                    bus.name
                ));
            }
            let count = s.tracks.iter().filter(|t| t.is_bus()).count();
            let name = match a.opt_str("name") {
                Some(n) if !n.trim().is_empty() => n.trim().to_string(),
                _ => (count + 1..)
                    .map(|n| format!("Group {n}"))
                    .find(|n| !s.tracks.iter().any(|t| &t.name == n))
                    .unwrap_or_else(|| "Group".into()),
            };
            let color = s
                .tracks
                .iter()
                .find(|t| t.id == ids[0])
                .map_or(TRACK_PALETTE[0].to_string(), |t| t.color.clone());
            let bus = new_track(s, "bus", Some(name), color);
            let bus_id = bus.id.clone();
            let last = ids
                .iter()
                .filter_map(|id| s.tracks.iter().position(|t| &t.id == id))
                .max()
                .unwrap_or(s.tracks.len());
            let mut commands = vec![Command::AddTrack(bus)];
            commands.push(Command::MoveTrack {
                id: bus_id.clone(),
                index: last + 1,
            });
            for id in &ids {
                let mut track = find_track(s, id)?.clone();
                track.output = Some(bus_id.clone());
                commands.push(Command::UpdateTrack(track));
            }
            host.dispatch(Command::Batch(commands))?;
            let s = host.store().session();
            let mut out = track_json(s, find_track(s, &bus_id)?);
            out["routed"] = json!(ids);
            Ok(out)
        }
        _ => Err(format!("Unknown command `{name}`")),
    }
}
