//! Persistent read automation in absolute quarter-note beats. Track and master
//! gain/pan are evaluated per sample; plugin parameters at process boundaries.
use crate::{
    model::{Session, MASTER},
    Result,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const MAX_LANES: usize = 256;
pub const MAX_POINTS: usize = 65536;
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AutomationTarget {
    TrackVolume {
        track_id: String,
    },
    TrackPan {
        track_id: String,
    },
    MasterVolume,
    PluginParameter {
        track_id: String,
        insert_id: String,
        plugin_id: String,
        parameter_id: u32,
    },
}
impl AutomationTarget {
    pub fn track_id(&self) -> Option<&str> {
        match self {
            Self::TrackVolume { track_id }
            | Self::TrackPan { track_id }
            | Self::PluginParameter { track_id, .. } => Some(track_id),
            Self::MasterVolume => None,
        }
    }
    pub fn exists(&self, session: &Session) -> bool {
        match self {
            Self::TrackVolume { track_id } | Self::TrackPan { track_id } => {
                session.tracks.iter().any(|track| &track.id == track_id)
            }
            Self::MasterVolume => true,
            Self::PluginParameter {
                track_id,
                insert_id,
                plugin_id,
                ..
            } => {
                let strip = session.strips.get(track_id).cloned().unwrap_or_default();
                session
                    .needs()
                    .iter()
                    .any(|need| &need.key == insert_id && &need.plugin == plugin_id)
                    && (strip
                        .inserts
                        .iter()
                        .chain(strip.synth.iter())
                        .any(|insert| &insert.id == insert_id && insert.plugin_id() == *plugin_id)
                        || (session
                            .tracks
                            .iter()
                            .any(|track| &track.id == track_id && track.kind == "midi")
                            && strip.synth.is_none()
                            && strip.synth_key(track_id) == *insert_id
                            && format!("stock:{}", strip.instrument) == *plugin_id))
            }
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Interpolation {
    #[default]
    Linear,
    Step,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationPoint {
    pub id: String,
    pub beat: f64,
    pub value: f64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationLane {
    pub id: String,
    pub name: String,
    pub target: AutomationTarget,
    pub min: f64,
    pub max: f64,
    #[serde(default)]
    pub manual_value: f64,
    #[serde(default)]
    pub interpolation: Interpolation,
    #[serde(default = "enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub points: Vec<AutomationPoint>,
}
fn enabled() -> bool {
    true
}
impl AutomationLane {
    pub fn value_at(&self, beat: f64) -> Option<f64> {
        self.cursor(beat).value(beat)
    }
    pub fn cursor(&self, beat: f64) -> Cursor<'_> {
        Cursor {
            lane: self,
            last_beat: beat,
            index: self
                .points
                .partition_point(|point| point.beat <= beat + 1e-9)
                .saturating_sub(1),
        }
    }
}
pub struct Cursor<'a> {
    lane: &'a AutomationLane,
    index: usize,
    last_beat: f64,
}
impl Cursor<'_> {
    /// The point the cursor stands on; it changes when a breakpoint is crossed (or musical
    /// time wraps round a cycle).
    pub fn segment(&self) -> usize {
        self.index
    }
    pub fn value(&mut self, beat: f64) -> Option<f64> {
        let lane = self.lane;
        if !lane.enabled || lane.points.is_empty() {
            return None;
        }
        // A latency-compensated cursor can cross the cycle boundary inside a
        // block; restart its search once when musical time wraps backwards.
        if beat < self.last_beat {
            self.index = lane
                .points
                .partition_point(|point| point.beat <= beat + 1e-9)
                .saturating_sub(1);
        }
        self.last_beat = beat;
        while self.index + 1 < lane.points.len() && lane.points[self.index + 1].beat <= beat + 1e-9
        {
            self.index += 1;
        }
        let left = &lane.points[self.index];
        let Some(right) = lane.points.get(self.index + 1) else {
            return Some(left.value);
        };
        if lane.interpolation == Interpolation::Step || beat <= left.beat {
            return Some(left.value);
        }
        let fraction = ((beat - left.beat) / (right.beat - left.beat)).clamp(0.0, 1.0);
        Some(left.value + (right.value - left.value) * fraction)
    }
}
pub fn validate(session: &Session) -> Result<()> {
    if session.automation.len() > MAX_LANES {
        return Err("Automation exceeds 256 lanes".into());
    }
    let mut ids = HashSet::new();
    let mut targets = HashSet::new();
    let mut count = 0;
    for lane in &session.automation {
        if lane.id.is_empty()
            || lane.id.len() > 256
            || !ids.insert(lane.id.as_str())
            || lane.name.len() > 512
        {
            return Err("Automation lane identifiers must be unique and nonempty".into());
        }
        if !targets.insert(&lane.target) {
            return Err("Only one automation lane per target is allowed".into());
        }
        if !lane.target.exists(session) {
            return Err(format!("Automation target no longer exists: {}", lane.name));
        }
        let expected = match lane.target {
            AutomationTarget::TrackVolume { .. } | AutomationTarget::MasterVolume => {
                Some((0.0, 1.0))
            }
            AutomationTarget::TrackPan { .. } => Some((-100.0, 100.0)),
            _ => None,
        };
        if !lane.min.is_finite()
            || !lane.max.is_finite()
            || lane.min >= lane.max
            || !lane.manual_value.is_finite()
            || !(lane.min..=lane.max).contains(&lane.manual_value)
            || lane.min.abs() > 1e9
            || lane.max.abs() > 1e9
            || expected.is_some_and(|bounds| bounds != (lane.min, lane.max))
        {
            return Err("Invalid automation parameter bounds".into());
        }
        let mut previous = None;
        let mut point_ids = HashSet::new();
        for point in &lane.points {
            count += 1;
            if count > MAX_POINTS {
                return Err("Automation exceeds 65536 points".into());
            }
            if point.id.is_empty()
                || point.id.len() > 256
                || !point_ids.insert(point.id.as_str())
                || !crate::model::valid_time(point.beat)
                || !point.value.is_finite()
                || !(lane.min..=lane.max).contains(&point.value)
                || previous.is_some_and(|beat| point.beat <= beat + 1e-9)
            {
                return Err(format!(
                    "Invalid, duplicate or unordered automation point in {}",
                    lane.name
                ));
            }
            previous = Some(point.beat);
        }
    }
    Ok(())
}
/// Keep the meaningful lanes when removing tracks/plugins or isolating stems.
pub fn retain_targets(session: &mut Session) {
    let lanes = std::mem::take(&mut session.automation);
    session.automation = lanes
        .into_iter()
        .filter(|lane| lane.target.exists(session))
        .collect();
}
/// Range exports hold the endpoint while effect tails decay.
pub fn hold_after(session: &mut Session, beat: f64) {
    for lane in &mut session.automation {
        let value = lane.value_at(beat);
        lane.points.retain(|point| point.beat < beat);
        if let Some(value) = value {
            let mut id = format!("{}-range-end", lane.id);
            while lane.points.iter().any(|point| point.id == id) {
                id.push('_');
            }
            lane.points.push(AutomationPoint { id, beat, value });
        }
    }
}
pub fn target_label(target: &AutomationTarget) -> String {
    match target {
        AutomationTarget::TrackVolume { track_id } => format!("{track_id} · Volume"),
        AutomationTarget::TrackPan { track_id } => format!("{track_id} · Pan"),
        AutomationTarget::MasterVolume => format!("{MASTER} · Volume"),
        AutomationTarget::PluginParameter {
            track_id,
            parameter_id,
            ..
        } => format!("{track_id} · Parameter {parameter_id}"),
    }
}
