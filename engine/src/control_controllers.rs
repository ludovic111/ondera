//! Controller commands: control changes, pitch bend, channel and polyphonic pressure inside
//! MIDI clips,
//! edited like notes. Each command is one undo step; points an agent creates are marked.
use crate::{
    control::{self, edit, find_clip, opt, query, req, Args, Host, Kind, Spec, CLIP_ID},
    controllers,
    model::{Clip, ClipData, Controller, ControllerKind},
    store::Command,
    Result,
};
use serde_json::{json, Value};

const KIND: crate::control::Param = req(
    "kind",
    Kind::String,
    "cc (control change), bend (pitch bend), pressure (channel pressure) or poly (polyphonic key pressure).",
);
const NUMBER: crate::control::Param = opt(
    "number",
    Kind::Integer,
    "Controller number 0-119 for kind cc: 1 mod wheel, 7 volume, 10 pan, 11 expression, 64 sustain pedal. The key 0-127 for kind poly. Omit for bend and pressure.",
);
const CHANNEL: crate::control::Param = opt(
    "channel",
    Kind::Integer,
    "MIDI channel 0-15 (channel 1-16 to a musician); default 0. A lane is one kind, number and channel.",
);
const VALUE_DOC: &str =
    "0-127 for cc, pressure and poly; -8192 (down) to 8191 (up) for bend, 0 centred.";
/// Most points one lane may hold.
pub const MAX_LANE_POINTS: usize = 20_000;

pub const SPECS: &[Spec] = &[
    query("controller.list", "List a MIDI clip's controller points (control changes, pitch bend, channel and polyphonic pressure) and a summary of its lanes. Each value holds until the next point of its lane.", &[
        CLIP_ID,
        opt("kind", Kind::String, "Only this kind: cc, bend, pressure or poly."),
        opt("number", Kind::Integer, "Only this controller number (kind cc) or key (kind poly)."),
        opt("channel", Kind::Integer, "Only this MIDI channel, 0-15."),
    ]),
    edit("controller.add", "Add a controller point to a MIDI clip. A point already at that time in the same lane takes the new value instead.", &[
        CLIP_ID,
        KIND,
        NUMBER,
        CHANNEL,
        req("time", Kind::Number, "Beats from the clip start, inside the clip."),
        req("value", Kind::Integer, VALUE_DOC),
    ]),
    edit("controller.update", "Move a controller point or change its value.", &[
        CLIP_ID,
        req("controllerId", Kind::String, "Point id from controller.list."),
        opt("time", Kind::Number, "New time in beats from the clip start."),
        opt("value", Kind::Integer, VALUE_DOC),
    ]),
    edit("controller.remove", "Delete one controller point (CC, pitch bend or pressure) from a MIDI clip. One undo step.", &[
        CLIP_ID,
        req("controllerId", Kind::String, "Point id from controller.list."),
    ]),
    edit("controller.setPoints", "Replace the points of one lane (kind and number) in one undo step: all of them, or only those from `from` up to `to` when given, which is how a drawn curve lands. An empty list clears the lane or the range.", &[
        CLIP_ID,
        KIND,
        NUMBER,
        CHANNEL,
        req("points", Kind::Array, "Array of {time, value, id?}: time in beats from the clip start, value as for controller.add."),
        opt("from", Kind::Number, "Start of the range to replace, in beats (default: the clip start)."),
        opt("to", Kind::Number, "End of the range to replace, in beats, exclusive (default: the clip end)."),
    ]),
];

fn lane_of(a: &Args<'_>) -> Result<controllers::Lane> {
    let kind = ControllerKind::parse(a.str("kind")?)?;
    let number = match (kind, a.opt_int("number")) {
        (ControllerKind::Cc, Some(n)) if (0..=119).contains(&n) => Some(n as u8),
        (ControllerKind::Cc, Some(_)) => {
            return Err(
                "Controller number must be 0-119 (120-127 are channel mode messages)".into(),
            )
        }
        (ControllerKind::Cc, None) => return Err("kind cc needs `number`".into()),
        (ControllerKind::PolyPressure, Some(n)) if (0..=127).contains(&n) => Some(n as u8),
        (ControllerKind::PolyPressure, _) => {
            return Err("kind poly needs `number`, the key 0-127".into())
        }
        (_, Some(_)) => return Err(format!("{} takes no `number`", kind.as_str())),
        (_, None) => None,
    };
    Ok((kind, number, channel_of(a.opt_int("channel"))?))
}
fn channel_of(channel: Option<i64>) -> Result<u8> {
    match channel {
        None => Ok(0),
        Some(c) if (0..16).contains(&c) => Ok(c as u8),
        Some(_) => Err("channel must be 0-15".into()),
    }
}
fn value_of(kind: ControllerKind, value: Option<i64>) -> Result<i16> {
    let value = value.ok_or("Missing `value`")?;
    let (low, high) = kind.range();
    if !(low as i64..=high as i64).contains(&value) {
        return Err(format!(
            "{} values run from {low} to {high}",
            controllers::lane_name(kind, None)
        ));
    }
    Ok(value as i16)
}
fn time_in(clip: &Clip, bpb: f64, time: f64) -> Result<f64> {
    let length = clip.length_bars * bpb;
    if !time.is_finite() || time < 0.0 || time >= length {
        return Err(format!(
            "time must be at least 0 and before the clip's end ({length} beats)"
        ));
    }
    Ok(time)
}
fn points_mut(clip: &mut Clip) -> Result<&mut Vec<Controller>> {
    match &mut clip.data {
        ClipData::Midi { controllers, .. } => Ok(controllers),
        ClipData::Audio { .. } => Err("Only MIDI clips hold controllers".into()),
    }
}
fn lane_json((kind, number, channel): controllers::Lane, count: usize) -> Value {
    let mut lane = json!({
        "kind": kind.as_str(),
        "name": controllers::lane_name(kind, number),
        "count": count,
    });
    if let Some(n) = number {
        lane["number"] = json!(n);
    }
    if channel != 0 {
        lane["channel"] = json!(channel);
    }
    lane
}

pub(crate) fn call(host: &mut dyn Host, name: &str, a: &Args<'_>, agent: bool) -> Result<Value> {
    let session = host.store().session();
    let bpb = session.beats_per_bar();
    let mut clip = find_clip(session, a.str("clipId")?)?.clone();
    let clip_id = clip.id.clone();
    if name == "controller.list" {
        let points = match &clip.data {
            ClipData::Midi { controllers, .. } => controllers,
            ClipData::Audio { .. } => return Err("Only MIDI clips hold controllers".into()),
        };
        let kind = a.opt_str("kind").map(ControllerKind::parse).transpose()?;
        let number = a.opt_int("number");
        let channel = a.opt_int("channel").map(Some).map(channel_of).transpose()?;
        let keep = |p: &&Controller| {
            kind.is_none_or(|k| p.kind == k)
                && number.is_none_or(|n| p.number == Some(n as u8))
                && channel.is_none_or(|c| p.channel == c)
        };
        let mut listed: Vec<Controller> = points.iter().filter(keep).cloned().collect();
        controllers::sort(&mut listed);
        let lanes: Vec<Value> = controllers::lanes(points)
            .into_iter()
            .map(|(lane, list)| lane_json(lane, list.len()))
            .collect();
        return Ok(json!({ "clipId": clip_id, "lanes": lanes, "controllers": listed }));
    }
    let touched: Value = match name {
        "controller.add" => {
            let lane = lane_of(a)?;
            let (kind, number, channel) = lane;
            let time = time_in(&clip, bpb, a.f64("time")?)?;
            let value = value_of(kind, a.opt_int("value"))?;
            let points = points_mut(&mut clip)?;
            let point = match points
                .iter_mut()
                .find(|p| p.lane() == lane && (p.time - time).abs() < 1e-9)
            {
                Some(existing) => {
                    existing.value = value;
                    existing.agent |= agent;
                    existing.clone()
                }
                None => {
                    if points.iter().filter(|p| p.lane() == lane).count() >= MAX_LANE_POINTS {
                        return Err(format!("A lane holds at most {MAX_LANE_POINTS} points"));
                    }
                    let point = Controller {
                        id: control::new_id("ctl"),
                        kind,
                        number,
                        time,
                        value,
                        agent,
                        channel,
                    };
                    points.push(point.clone());
                    point
                }
            };
            controllers::sort(points);
            json!(point)
        }
        "controller.update" => {
            let id = a.str("controllerId")?;
            let time = a
                .opt_f64("time")
                .map(|t| time_in(&clip, bpb, t))
                .transpose()?;
            let points = points_mut(&mut clip)?;
            let point = points
                .iter_mut()
                .find(|p| p.id == id)
                .ok_or_else(|| format!("Unknown controller point `{id}`. Use controller.list."))?;
            if let Some(time) = time {
                point.time = time;
            }
            if a.get("value").is_some() {
                point.value = value_of(point.kind, a.opt_int("value"))?;
            }
            let point = point.clone();
            // Two points of a lane at one time would be ambiguous; the moved one wins.
            points.retain(|p| {
                p.id == point.id || p.lane() != point.lane() || (p.time - point.time).abs() >= 1e-9
            });
            controllers::sort(points);
            json!(point)
        }
        "controller.remove" => {
            let id = a.str("controllerId")?;
            let points = points_mut(&mut clip)?;
            let before = points.len();
            points.retain(|p| p.id != id);
            if points.len() == before {
                return Err(format!(
                    "Unknown controller point `{id}`. Use controller.list."
                ));
            }
            json!({ "removed": id })
        }
        "controller.setPoints" => {
            let lane = lane_of(a)?;
            let (kind, number, channel) = lane;
            let length = clip.length_bars * bpb;
            let from = a.opt_f64("from").unwrap_or(0.0);
            let to = a.opt_f64("to").unwrap_or(length);
            if !(from.is_finite() && to.is_finite() && from >= 0.0 && to > from) {
                return Err("`from` and `to` must be beats with from < to".into());
            }
            let items = a
                .get("points")
                .and_then(Value::as_array)
                .ok_or("`points` must be an array")?;
            if items.len() > MAX_LANE_POINTS {
                return Err(format!("A lane holds at most {MAX_LANE_POINTS} points"));
            }
            let mut incoming = Vec::with_capacity(items.len());
            for (i, item) in items.iter().enumerate() {
                let time = item["time"]
                    .as_f64()
                    .ok_or_else(|| format!("Point {i} needs a numeric `time`"))?;
                let time = time_in(&clip, bpb, time).map_err(|e| format!("Point {i}: {e}"))?;
                if time < from || time >= to {
                    return Err(format!("Point {i} lies outside from..to"));
                }
                let value = value_of(kind, item["value"].as_i64())
                    .map_err(|e| format!("Point {i}: {e}"))?;
                incoming.push(Controller {
                    id: item["id"]
                        .as_str()
                        .filter(|id| !id.is_empty())
                        .map(str::to_string)
                        .unwrap_or_else(|| control::new_id("ctl")),
                    kind,
                    number,
                    time,
                    value,
                    agent,
                    channel,
                });
            }
            // One point per time: a later entry at the same time replaces an earlier one.
            incoming.sort_by(|a, b| a.time.total_cmp(&b.time));
            let mut deduped: Vec<Controller> = Vec::with_capacity(incoming.len());
            for point in incoming {
                match deduped.last_mut() {
                    Some(last) if (last.time - point.time).abs() < 1e-9 => *last = point,
                    _ => deduped.push(point),
                }
            }
            let count = deduped.len();
            let points = points_mut(&mut clip)?;
            points.retain(|p| p.lane() != lane || p.time < from || p.time >= to);
            if points.iter().filter(|p| p.lane() == lane).count() + count > MAX_LANE_POINTS {
                return Err(format!("A lane holds at most {MAX_LANE_POINTS} points"));
            }
            points.extend(deduped);
            controllers::sort(points);
            lane_json(lane, points.iter().filter(|p| p.lane() == lane).count())
        }
        _ => return Err(format!("Unknown command `{name}`")),
    };
    host.dispatch(Command::PutClip(clip))?;
    let mut summary = control::clip_summary(find_clip(host.store().session(), &clip_id)?);
    summary["controller"] = touched;
    Ok(summary)
}
