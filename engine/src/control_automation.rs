//! Automation commands shared by the native editor, CLI and MCP registry.
use crate::{
    automation::{AutomationLane, AutomationPoint, AutomationTarget, Interpolation},
    control::{edit, opt, query, req, Host, Kind, Spec},
    model::{Insert, Strip},
    store::Command,
    Result,
};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
pub const SPECS:&[Spec]=&[
    query("automation.list","List automation lanes and absolute beat points. Track/master gain and pan are sample-accurate; plugin changes occur at blocks of at most 256 frames.",&[]),
    edit("automation.create","Create read automation for trackVolume, trackPan, masterVolume or pluginParameter. One lane per target; values use the parameter's plain units.",&[
        req("target",Kind::String,"trackVolume, trackPan, masterVolume or pluginParameter"),opt("trackId",Kind::String,"Track ID; master/bus-a/bus-b also accept plugin parameters"),opt("slot",Kind::Integer,"Plugin insert 0-7; omit for the track instrument"),opt("parameterId",Kind::Integer,"Plugin parameter ID from strip.parameters"),opt("parameter",Kind::String,"Plugin parameter name instead of parameterId, matched like strip.parameters query"),opt("name",Kind::String,"Lane name"),opt("interpolation",Kind::String,"linear (default) or step"),opt("points",Kind::Array,"Optional {beat,value,id?} points in absolute quarter-note beats")]),
    edit("automation.setPoints","Replace all points in a lane atomically, with one undo step.",&[req("laneId",Kind::String,"Automation lane ID"),req("points",Kind::Array,"Array of {beat,value,id?}; beats must be distinct")]),
    edit("automation.setPoint","Add or move one automation point. Beats are absolute quarter notes from the song start (bar × beats per bar); values are the target's plain units: volume 0-1 (0.75 = 0 dB), pan -100 to 100, plugin parameters as strip.parameters lists them.",&[req("laneId",Kind::String,"Automation lane ID"),opt("pointId",Kind::String,"Existing point ID to move; omitted creates a point"),req("beat",Kind::Number,"Absolute quarter-note beat"),req("value",Kind::Number,"Plain parameter value")]),
    edit("automation.removePoint","Delete an automation point.",&[req("laneId",Kind::String,"Automation lane ID"),req("pointId",Kind::String,"Point ID")]),
    edit("automation.setEnabled","Enable read automation or leave the manual control active.",&[req("laneId",Kind::String,"Automation lane ID"),req("enabled",Kind::Boolean,"Whether automation controls its target")]),
    edit("automation.setInterpolation","Choose linear ramps or steps between points.",&[req("laneId",Kind::String,"Automation lane ID"),req("interpolation",Kind::String,"linear or step")]),
    edit("automation.remove","Delete an automation lane; undo restores it.",&[req("laneId",Kind::String,"Automation lane ID")]),
];
fn id(prefix: &str) -> String {
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    format!(
        "{prefix}-{}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    )
}
fn string<'a>(params: &'a Value, key: &str) -> Result<&'a str> {
    params[key].as_str().ok_or_else(|| format!("Missing {key}"))
}
fn number(params: &Value, key: &str) -> Result<f64> {
    params[key]
        .as_f64()
        .filter(|value| value.is_finite())
        .ok_or_else(|| format!("Invalid {key}"))
}
fn interpolation(value: Option<&str>) -> Result<Interpolation> {
    match value.unwrap_or("linear") {
        "linear" => Ok(Interpolation::Linear),
        "step" => Ok(Interpolation::Step),
        _ => Err("Interpolation must be linear or step".into()),
    }
}
fn points(value: &Value) -> Result<Vec<AutomationPoint>> {
    let values = value.as_array().ok_or("Points must be an array")?;
    if values.len() > crate::automation::MAX_POINTS {
        return Err("Too many automation points".into());
    }
    values
        .iter()
        .map(|value| {
            Ok(AutomationPoint {
                id: value["id"]
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| id("point")),
                beat: number(value, "beat")?,
                value: number(value, "value")?,
            })
        })
        .collect()
}
pub fn call(host: &mut dyn Host, name: &str, params: &Value, _agent: bool) -> Result<Value> {
    if name == "automation.list" {
        return Ok(json!({"lanes":host.store().session().automation}));
    }
    if name == "automation.create" {
        let target_name = string(params, "target")?;
        let track_id = params["trackId"].as_str().unwrap_or("");
        let mut manual_value = 0.0;
        let (target, min, max) = match target_name {
            "trackVolume" => (
                AutomationTarget::TrackVolume {
                    track_id: track_id.into(),
                },
                0.0,
                1.0,
            ),
            "trackPan" => (
                AutomationTarget::TrackPan {
                    track_id: track_id.into(),
                },
                -100.0,
                100.0,
            ),
            "masterVolume" => (AutomationTarget::MasterVolume, 0.0, 1.0),
            "pluginParameter" => {
                let slot = params["slot"].as_u64().map(|slot| slot as usize);
                if params.get("slot").is_some_and(|value| !value.is_null())
                    && (slot.is_none()
                        || slot.is_some_and(|slot| slot >= crate::model::MAX_INSERTS))
                {
                    return Err("Plugin slot must be 0-7".into());
                }
                let metadata = host.plugin_parameters(track_id, slot)?;
                let parameter_id =
                    match (params["parameterId"].as_u64(), params["parameter"].as_str()) {
                        (Some(id), _) => u32::try_from(id).map_err(|_| "Invalid parameterId")?,
                        // A name, found the way strip.setParameter finds it.
                        (None, Some(name)) => {
                            let list: Vec<crate::plugin::ParamInfo> = metadata["parameters"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .filter_map(|p| {
                                    Some(crate::plugin::ParamInfo {
                                        id: u32::try_from(p["id"].as_u64()?).ok()?,
                                        name: p["name"].as_str()?.to_string(),
                                        min: 0.0,
                                        max: 1.0,
                                        default: 0.0,
                                        unit: String::new(),
                                        steps: 0,
                                        log: false,
                                        labels: vec![],
                                    })
                                })
                                .collect();
                            crate::control_params::find_parameter(&list, name, "The plugin")?.id
                        }
                        (None, None) => {
                            return Err(
                                "pluginParameter needs parameterId or parameter (a name)".into()
                            )
                        }
                    };
                let parameter = metadata["parameters"]
                    .as_array()
                    .and_then(|list| {
                        list.iter()
                            .find(|parameter| parameter["id"].as_u64() == Some(parameter_id as u64))
                    })
                    .ok_or("Plugin parameter not found")?;
                manual_value = parameter["value"]
                    .as_f64()
                    .unwrap_or(number(parameter, "default")?);
                let strip = host
                    .store()
                    .session()
                    .strips
                    .get(track_id)
                    .cloned()
                    .unwrap_or_else(Strip::default);
                let insert = match slot {
                    Some(slot) => strip
                        .inserts
                        .get(slot)
                        .cloned()
                        .ok_or("Plugin insert not found")?,
                    None => strip.synth.clone().unwrap_or_else(|| {
                        Insert::new(
                            strip.synth_key(track_id),
                            &format!("stock:{}", strip.instrument),
                            &strip.instrument,
                        )
                    }),
                };
                (
                    AutomationTarget::PluginParameter {
                        track_id: track_id.into(),
                        insert_id: insert.id.clone(),
                        plugin_id: insert.plugin_id(),
                        parameter_id,
                    },
                    number(parameter, "min")?,
                    number(parameter, "max")?,
                )
            }
            _ => return Err("Unknown automation target".into()),
        };
        match &target {
            AutomationTarget::TrackVolume { track_id } => {
                manual_value = host
                    .store()
                    .session()
                    .tracks
                    .iter()
                    .find(|track| &track.id == track_id)
                    .map_or(0.75, |track| track.volume as f64)
            }
            AutomationTarget::TrackPan { track_id } => {
                manual_value = host
                    .store()
                    .session()
                    .tracks
                    .iter()
                    .find(|track| &track.id == track_id)
                    .map_or(0.0, |track| track.pan as f64)
            }
            AutomationTarget::MasterVolume => {
                manual_value = host.store().session().master_volume as f64
            }
            _ => {}
        }
        let lane = AutomationLane {
            id: id("automation"),
            name: params["name"]
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| crate::automation::target_label(&target)),
            target,
            min,
            max,
            manual_value,
            interpolation: interpolation(params["interpolation"].as_str())?,
            enabled: true,
            points: if let Some(value) = params.get("points") {
                points(value)?
            } else {
                vec![]
            },
        };
        host.dispatch(Command::PutAutomation(lane.clone()))?;
        return Ok(
            json!({"lane":host.store().session().automation.iter().find(|current|current.id==lane.id)}),
        );
    }
    let lane_id = string(params, "laneId")?;
    let mut lane = host
        .store()
        .session()
        .automation
        .iter()
        .find(|lane| lane.id == lane_id)
        .cloned()
        .ok_or("Automation lane not found")?;
    match name {
        "automation.remove" => {
            host.dispatch(Command::RemoveAutomation(lane_id.into()))?;
            return Ok(json!({"removed":lane_id}));
        }
        "automation.setPoints" => lane.points = points(&params["points"])?,
        "automation.setPoint" => {
            let point_id = params["pointId"].as_str();
            if let Some(point_id) = point_id {
                let point = lane
                    .points
                    .iter_mut()
                    .find(|point| point.id == point_id)
                    .ok_or("Automation point not found")?;
                point.beat = number(params, "beat")?;
                point.value = number(params, "value")?;
            } else {
                lane.points.push(AutomationPoint {
                    id: id("point"),
                    beat: number(params, "beat")?,
                    value: number(params, "value")?,
                });
            }
        }
        "automation.removePoint" => {
            let point_id = string(params, "pointId")?;
            if !lane.points.iter().any(|point| point.id == point_id) {
                return Err("Automation point not found".into());
            }
            lane.points.retain(|point| point.id != point_id);
        }
        "automation.setEnabled" => {
            lane.enabled = params["enabled"]
                .as_bool()
                .ok_or("enabled must be boolean")?
        }
        "automation.setInterpolation" => {
            lane.interpolation = interpolation(Some(string(params, "interpolation")?))?
        }
        _ => return Err(format!("Unknown automation command: {name}")),
    }
    host.dispatch(Command::PutAutomation(lane.clone()))?;
    Ok(json!({"lane":host.store().session().automation.iter().find(|current|current.id==lane.id)}))
}
