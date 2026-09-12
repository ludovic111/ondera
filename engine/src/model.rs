use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

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
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
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

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Strip {
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
    #[serde(default = "default_instrument")]
    pub instrument: String,
    #[serde(default)]
    pub inserts: Vec<Insert>,
    #[serde(default)]
    pub sends: Vec<Send>,
}
fn default_instrument() -> String {
    "Ondera Synth".into()
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct Insert {
    pub name: String,
    pub state: String,
    #[serde(default)]
    pub meta: String,
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
        for strip in self.strips.values() {
            if strip.inserts.len() > 4
                || strip.sends.len() > 2
                || strip.sends.iter().any(|s| {
                    s.level_db
                        .is_some_and(|v| !v.is_finite() || !(-100.0..=0.0).contains(&v))
                })
            {
                return Err("Invalid channel strip".into());
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
