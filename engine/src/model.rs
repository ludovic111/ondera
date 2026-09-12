use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Strip keys that are not tracks: the stereo output and the two aux buses.
pub const MASTER: &str = "master";
pub const BUS_A: &str = "bus-a";
pub const BUS_B: &str = "bus-b";
pub const MAX_INSERTS: usize = 8;
pub fn is_bus(id: &str) -> bool {
    id == MASTER || id == BUS_A || id == BUS_B
}
pub fn bus_name(id: &str) -> &'static str {
    match id {
        MASTER => "Stereo Out",
        BUS_A => "A · Reverb",
        BUS_B => "B · Delay",
        _ => "Bus",
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub name: String,
    pub tracks: Vec<Track>,
    pub clips: Vec<Clip>,
    pub sources: HashMap<String, Source>,
    pub strips: HashMap<String, Strip>,
    pub transport: Transport,
    pub view: View,
    #[serde(default = "default_master_volume")]
    pub master_volume: f32,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}
fn default_master_volume() -> f32 {
    0.75
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: String,
    pub name: String,
    pub color: String,
    pub armed: bool,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
    pub kind: String,
    pub volume: f32,
    pub pan: f32,
    pub mute: bool,
    pub solo: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub agent: bool,
    pub track_id: String,
    pub start_bar: f64,
    pub length_bars: f64,
    pub data: ClipData,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ClipData {
    Midi {
        notes: Vec<Note>,
    },
    Audio {
        #[serde(rename = "sourceId")]
        source_id: String,
        #[serde(rename = "offsetSeconds")]
        offset_seconds: f64,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Note {
    pub id: String,
    pub start: f64,
    pub length: f64,
    pub pitch: u8,
    pub velocity: u8,
    #[serde(default)]
    pub agent: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub id: String,
    pub name: String,
    pub sample_rate: u32,
    pub channels: u16,
    #[serde(default)]
    pub file_name: Option<String>,
    pub duration_seconds: f64,
    pub origin: String,
    pub seed: Option<u32>,
    pub wave_kind: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Strip {
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
    #[serde(default = "default_instrument")]
    pub instrument: String,
    #[serde(default)]
    pub inserts: Vec<Insert>,
    #[serde(default)]
    pub sends: Vec<Send>,
    /// An external instrument plugin. `None` plays the stock instrument named by `instrument`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub synth: Option<Insert>,
}
fn default_instrument() -> String {
    "Ondera Synth".into()
}
impl Default for Strip {
    fn default() -> Self {
        Self {
            extra: HashMap::new(),
            instrument: default_instrument(),
            inserts: vec![],
            sends: vec![],
            synth: None,
        }
    }
}
impl Strip {
    /// The rack key of this strip's instrument, unique per track and plugin.
    pub fn synth_key(&self, track_id: &str) -> String {
        match &self.synth {
            Some(insert) => insert.id.clone(),
            None => format!("{track_id}/synth/{}", self.instrument),
        }
    }
    pub fn instrument_name(&self) -> String {
        match &self.synth {
            Some(insert) => insert.name.clone(),
            None => self.instrument.clone(),
        }
    }
}

/// One slot of a channel strip. `plugin` is a descriptor id (`stock:Space`,
/// `clap:…`, `vst3:…`, `au:…`); when empty the stock plugin named by `name` is meant.
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct Insert {
    pub name: String,
    pub state: String,
    #[serde(default)]
    pub meta: String,
    #[serde(default)]
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub plugin: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<u32, f64>,
    /// Base64 plugin state captured on save.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub blob: String,
}
impl Insert {
    pub fn empty_slot() -> Self {
        Self {
            name: "Empty slot".into(),
            state: "empty".into(),
            ..Default::default()
        }
    }
    pub fn new(id: String, plugin_id: &str, name: &str) -> Self {
        Self {
            name: name.into(),
            state: "active".into(),
            meta: String::new(),
            id,
            plugin: plugin_id.into(),
            params: BTreeMap::new(),
            blob: String::new(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.state == "empty"
    }
    pub fn plugin_id(&self) -> String {
        if self.plugin.is_empty() {
            format!("stock:{}", self.name)
        } else {
            self.plugin.clone()
        }
    }
}

/// A plugin instance the session needs, keyed for the audio-thread rack.
#[derive(Clone, Debug, PartialEq)]
pub struct Need {
    pub key: String,
    pub plugin: String,
    pub name: String,
    pub params: BTreeMap<u32, f64>,
    pub blob: String,
    pub instrument: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Send {
    pub level_db: Option<f32>,
    #[serde(default)]
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transport {
    #[serde(default)]
    pub playing: bool,
    #[serde(default)]
    pub recording: bool,
    pub position_beats: f64,
    pub key: String,
    pub snap_division: u32,
    pub tempo: f64,
    pub time_signature: TimeSignature,
    pub cycle: bool,
    pub cycle_start_bar: f64,
    pub cycle_end_bar: f64,
    pub metronome: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TimeSignature {
    pub numerator: u32,
    pub denominator: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub selected_track_id: Option<String>,
    pub selected_clip_id: Option<String>,
    pub editor_clip_id: Option<String>,
    pub selected_note_id: Option<String>,
    pub pixels_per_bar: f32,
    pub scroll_bars: f64,
    pub follow_playhead: bool,
    pub editor_mode: String,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl Session {
    /// Fill in missing insert ids and default buses so every strip has stable rack keys.
    pub fn normalize(&mut self) {
        let mut used: HashSet<String> = HashSet::new();
        for strip in self.strips.values() {
            used.extend(strip.inserts.iter().map(|i| i.id.clone()));
            if let Some(s) = &strip.synth {
                used.insert(s.id.clone());
            }
        }
        let mut serial = 0u64;
        let mut fresh = |used: &mut HashSet<String>| loop {
            serial += 1;
            let id = format!("insert-{serial}");
            if used.insert(id.clone()) {
                return id;
            }
        };
        for strip in self.strips.values_mut() {
            for insert in &mut strip.inserts {
                if insert.id.is_empty() {
                    insert.id = fresh(&mut used);
                }
            }
            if let Some(s) = &mut strip.synth {
                if s.id.is_empty() {
                    s.id = fresh(&mut used);
                }
            }
        }
        for (bus, effect, mix_index) in [(BUS_A, "Space", 4), (BUS_B, "Echo", 5)] {
            if !self.strips.contains_key(bus) {
                let mut insert = Insert::new(fresh(&mut used), &format!("stock:{effect}"), effect);
                insert.params.insert(mix_index, 100.0);
                self.strips.insert(
                    bus.into(),
                    Strip {
                        inserts: vec![insert],
                        ..Default::default()
                    },
                );
            }
        }
        if !self.strips.contains_key(MASTER) {
            self.strips.insert(MASTER.into(), Strip::default());
        }
    }
    /// Every plugin instance the session needs, in a deterministic order.
    pub fn needs(&self) -> Vec<Need> {
        let mut needs = vec![];
        fn push_inserts(needs: &mut Vec<Need>, inserts: &[Insert]) {
            for insert in inserts.iter().filter(|i| !i.is_empty()) {
                needs.push(Need {
                    key: insert.id.clone(),
                    plugin: insert.plugin_id(),
                    name: insert.name.clone(),
                    params: insert.params.clone(),
                    blob: insert.blob.clone(),
                    instrument: false,
                });
            }
        }
        for track in &self.tracks {
            let Some(strip) = self.strips.get(&track.id) else {
                if track.kind == "midi" {
                    let strip = Strip::default();
                    needs.push(Need {
                        key: strip.synth_key(&track.id),
                        plugin: format!("stock:{}", strip.instrument),
                        name: strip.instrument.clone(),
                        params: BTreeMap::new(),
                        blob: String::new(),
                        instrument: true,
                    });
                }
                continue;
            };
            if track.kind == "midi" {
                needs.push(match &strip.synth {
                    Some(s) => Need {
                        key: s.id.clone(),
                        plugin: s.plugin_id(),
                        name: s.name.clone(),
                        params: s.params.clone(),
                        blob: s.blob.clone(),
                        instrument: true,
                    },
                    None => Need {
                        key: strip.synth_key(&track.id),
                        plugin: format!("stock:{}", strip.instrument),
                        name: strip.instrument.clone(),
                        params: BTreeMap::new(),
                        blob: String::new(),
                        instrument: true,
                    },
                });
            }
            push_inserts(&mut needs, &strip.inserts);
        }
        for bus in [BUS_A, BUS_B, MASTER] {
            if let Some(strip) = self.strips.get(bus) {
                push_inserts(&mut needs, &strip.inserts);
            }
        }
        needs
    }
    pub fn beats_per_bar(&self) -> f64 {
        self.transport.time_signature.numerator as f64 * 4.0
            / self.transport.time_signature.denominator as f64
    }
    pub fn end_bar(&self) -> f64 {
        self.clips
            .iter()
            .map(|c| c.start_bar + c.length_bars)
            .fold(1.0, f64::max)
    }
    pub fn validate(&self) -> Result<()> {
        let t = &self.transport;
        if !self.view.pixels_per_bar.is_finite()
            || !(12.0..=480.0).contains(&self.view.pixels_per_bar)
            || !valid_time(self.view.scroll_bars)
        {
            return Err("Invalid view scale or position".into());
        }
        if !valid_time(t.position_beats) || ![1, 2, 4, 8, 16, 32, 64].contains(&t.snap_division) {
            return Err("Invalid transport".into());
        }
        if !t.tempo.is_finite() || !(20.0..=400.0).contains(&t.tempo) {
            return Err("Tempo must be between 20 and 400 BPM".into());
        }
        if !(1..=32).contains(&t.time_signature.numerator)
            || ![1, 2, 4, 8, 16, 32].contains(&t.time_signature.denominator)
        {
            return Err("Invalid time signature".into());
        }
        if self.tracks.len() > 128 || self.clips.len() > 50_000 || self.sources.len() > 10_000 {
            return Err("Session exceeds engine capacity".into());
        }
        if !valid_time(t.cycle_start_bar)
            || !valid_time(t.cycle_end_bar)
            || t.cycle_end_bar <= t.cycle_start_bar
        {
            return Err("Invalid cycle range".into());
        }
        let mut ids = HashSet::new();
        for track in &self.tracks {
            if !ids.insert(&track.id)
                || !["audio", "midi"].contains(&track.kind.as_str())
                || !track.volume.is_finite()
                || !(0.0..=1.0).contains(&track.volume)
                || !track.pan.is_finite()
                || !(-100.0..=100.0).contains(&track.pan)
            {
                return Err("Invalid or duplicate track".into());
            }
        }
        let mut clips = HashSet::new();
        let mut notes = 0;
        for c in &self.clips {
            if !clips.insert(&c.id)
                || !ids.contains(&c.track_id)
                || !valid_time(c.start_bar)
                || !valid_time(c.length_bars)
                || c.length_bars <= 0.0
            {
                return Err("Invalid clip".into());
            }
            let track = self.tracks.iter().find(|t| t.id == c.track_id).unwrap();
            match &c.data {
                ClipData::Midi { notes: ns } => {
                    if track.kind != "midi" {
                        return Err("MIDI clip on audio track".into());
                    }
                    notes += ns.len();
                    if ns.iter().any(|n| {
                        !valid_time(n.start)
                            || !valid_time(n.length)
                            || n.length <= 0.0
                            || n.pitch > 127
                            || n.velocity == 0
                            || n.velocity > 127
                    }) {
                        return Err("Invalid MIDI note".into());
                    }
                }
                ClipData::Audio {
                    source_id,
                    offset_seconds,
                } => {
                    if track.kind != "audio"
                        || !self.sources.contains_key(source_id)
                        || !valid_time(*offset_seconds)
                    {
                        return Err("Invalid audio clip or missing source".into());
                    }
                }
            }
        }
        if notes > 200_000 {
            return Err("Too many notes".into());
        }
        let mut decoded_bytes = 0.0;
        for (id, src) in &self.sources {
            if !(8000..=384000).contains(&src.sample_rate) || !(1..=2).contains(&src.channels) {
                return Err("Invalid source sample rate or channels".into());
            }
            decoded_bytes += src.duration_seconds
                * if src.origin == "generated" {
                    48000.0
                } else {
                    src.sample_rate as f64
                }
                * 8.0;
            if id != &src.id
                || !valid_time(src.duration_seconds)
                || src.duration_seconds > 14_400.0
                || !["generated", "file", "recording"].contains(&src.origin.as_str())
            {
                return Err("Invalid audio source".into());
            }
        }
        if decoded_bytes > crate::audio::MAX_LIBRARY_BYTES as f64 {
            return Err("Session audio exceeds the 1 GiB decoded-audio limit".into());
        }
        if !self.master_volume.is_finite() || !(0.0..=1.0).contains(&self.master_volume) {
            return Err("Invalid master volume".into());
        }
        for strip in self.strips.values() {
            if strip.inserts.len() > MAX_INSERTS
                || strip.sends.len() > 2
                || strip.sends.iter().any(|s| {
                    s.level_db
                        .is_some_and(|v| !v.is_finite() || !(-100.0..=0.0).contains(&v))
                })
            {
                return Err("Invalid channel strip".into());
            }
            for insert in strip.inserts.iter().chain(strip.synth.iter()) {
                if insert.params.values().any(|v| !v.is_finite())
                    || insert.blob.len() > 64 * 1024 * 1024
                    || !["active", "bypassed", "empty"].contains(&insert.state.as_str())
                {
                    return Err("Invalid insert".into());
                }
            }
        }
        Ok(())
    }
}
pub fn valid_time(v: f64) -> bool {
    v.is_finite() && (0.0..=1_000_000.0).contains(&v)
}
pub fn fader_gain(v: f32) -> f32 {
    if v <= 0.0 {
        0.0
    } else if v < 0.75 {
        (v / 0.75).powf(1.6)
    } else {
        10.0_f32.powf(((v - 0.75) * 24.0) / 20.0)
    }
}
