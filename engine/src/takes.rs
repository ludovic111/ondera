//! Named creative alternatives travel with the project. Switching is one undo step.
use crate::{
    control::{Args, Host},
    model::Session,
    store::Command,
    Result,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
const KEY: &str = "onderaCreativeTakes";
const MAX_BYTES: usize = 32 * 1024 * 1024;
#[derive(Default, Serialize, Deserialize)]
struct Takes {
    active: Option<String>,
    entries: Vec<Take>,
}
#[derive(Serialize, Deserialize)]
struct Take {
    id: String,
    name: String,
    session: Session,
}
fn read(session: &Session) -> Result<Takes> {
    let Some(value) = session.extra.get(KEY) else {
        return Ok(Takes::default());
    };
    if value["entries"]
        .as_array()
        .is_none_or(|entries| entries.len() > 8)
        || serde_json::to_vec(value).map_err(|e| e.to_string())?.len() > MAX_BYTES
    {
        return Err("Invalid or oversized creative takes".into());
    }
    let takes: Takes = serde_json::from_value(value.clone())
        .map_err(|e| format!("Could not read creative takes: {e}"))?;
    let mut ids = std::collections::HashSet::new();
    for take in &takes.entries {
        if take.id.is_empty()
            || !ids.insert(&take.id)
            || take.name.is_empty()
            || take.name.len() > 120
            || take.session.extra.contains_key(KEY)
        {
            return Err("Invalid creative take metadata".into());
        }
    }
    if takes.active.as_ref().is_some_and(|id| !ids.contains(id)) {
        return Err("The active creative take is missing".into());
    }
    Ok(takes)
}
fn snapshot(session: &Session) -> Session {
    let mut result = session.clone();
    result.extra.remove(KEY);
    result
}
fn summary(takes: &Takes) -> Value {
    json!({"active":takes.active,"takes":takes.entries.iter().map(|t| json!({"id":t.id,"name":t.name,"tracks":t.session.tracks.len(),"clips":t.session.clips.len()})).collect::<Vec<_>>()})
}
pub(crate) fn call(host: &mut dyn Host, method: &str, args: &Args<'_>) -> Result<Value> {
    let mut takes = read(host.store().session())?;
    if method == "take.list" {
        return Ok(summary(&takes));
    }
    if host.recording() {
        return Err("Stop recording before changing creative takes".into());
    }
    host.stop()?;
    host.capture_states()?;
    let current = snapshot(host.store().session());
    if let Some(active) = &takes.active {
        if let Some(take) = takes.entries.iter_mut().find(|t| &t.id == active) {
            take.session = current.clone();
        }
    }
    let mut next = current.clone();
    match method {
        "take.create" => {
            if takes.entries.len() >= 8 {
                return Err(
                    "This project already has eight takes. Keep or remove a take first.".into(),
                );
            }
            let name = args.str("name")?.trim();
            if name.is_empty() || name.len() > 120 {
                return Err("Take names must be 1–120 characters".into());
            }
            let id = crate::control::new_id("take");
            takes.entries.push(Take {
                id: id.clone(),
                name: name.into(),
                session: current,
            });
            takes.active = Some(id);
        }
        "take.select" => {
            let id = args.str("id")?;
            next = takes
                .entries
                .iter()
                .find(|t| t.id == id)
                .ok_or("Take not found")?
                .session
                .clone();
            next.validate()?;
            // Keep every audio source so inactive alternatives survive save/reopen.
            next.sources.extend(current.sources);
            next.extra.extend(current.extra);
            takes.active = Some(id.into());
        }
        "take.remove" => {
            let id = args.str("id")?;
            if takes.active.as_deref() == Some(id) {
                return Err("Select another take before removing this one".into());
            }
            let index = takes
                .entries
                .iter()
                .position(|t| t.id == id)
                .ok_or("Take not found")?;
            takes.entries.remove(index);
        }
        _ => return Err("Unknown creative take command".into()),
    }
    let output = summary(&takes);
    let data = serde_json::to_value(takes).map_err(|e| e.to_string())?;
    if serde_json::to_vec(&data).map_err(|e| e.to_string())?.len() > MAX_BYTES {
        return Err(
            "Creative takes exceed 32 MiB. Remove an unused take before continuing.".into(),
        );
    }
    next.extra.insert(KEY.into(), data);
    host.dispatch(Command::RestoreTake(Box::new(next)))?;
    host.view_changed();
    Ok(output)
}
