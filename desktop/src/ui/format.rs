//! Musical time, gain and labels as the window writes them.

use ryolune_engine::model::Session;

/// Ticks per quarter note, Logic-style: 240 per sixteenth.
pub const PPQ: f64 = 960.0;

/// A position as 1-based bar, beat and sixteenth, and 0-239 ticks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BarBeat {
    pub bar: u64,
    pub beat: u64,
    pub division: u64,
    pub tick: u64,
}

pub fn bar_beat(beats: f64, beats_per_bar: f64) -> BarBeat {
    let safe = beats.max(0.0);
    let bpb = beats_per_bar.max(1e-9);
    let bar = (safe / bpb).floor();
    let in_bar = safe - bar * bpb;
    let beat = in_bar.floor();
    let sixteenth = (in_bar - beat) * 4.0;
    let division = sixteenth.floor();
    let tick = ((sixteenth - division) * (PPQ / 4.0)).floor();
    BarBeat {
        bar: bar as u64 + 1,
        beat: beat as u64 + 1,
        division: division as u64 + 1,
        tick: tick as u64,
    }
}

/// "5.2.3", the playhead flag's short form.
pub fn bar_beat_short(beats: f64, beats_per_bar: f64) -> String {
    let p = bar_beat(beats, beats_per_bar);
    format!("{}.{}.{}", p.bar, p.beat, p.division)
}

/// HH:MM:SS:FF at 30 frames per second.
pub fn smpte(seconds: f64) -> String {
    let safe = seconds.max(0.0);
    let whole = safe.floor() as u64;
    let frames = ((safe - whole as f64) * 30.0).floor() as u64;
    format!(
        "{:02}:{:02}:{:02}:{:02}",
        whole / 3600,
        (whole % 3600) / 60,
        whole % 60,
        frames
    )
}

/// The song position under the playhead, in seconds, through the tempo map.
pub fn seconds_at(session: &Session, beats: f64) -> f64 {
    session.tempo_map().seconds(beats)
}

/// The bar whose tempo is in force at a beat: 0 is the starting tempo.
pub fn tempo_source_bar(session: &Session, beat: f64) -> f64 {
    let bar = beat / session.beats_per_bar();
    session
        .tempo_changes
        .iter()
        .filter(|p| p.bar <= bar + 1e-9)
        .map(|p| p.bar)
        .fold(0.0, f64::max)
}

/// Length of one snap step in bars for a note division (16 = sixteenth).
pub fn snap_step_bars(division: u32, beats_per_bar: f64) -> f64 {
    4.0 / division.max(1) as f64 / beats_per_bar.max(1e-9)
}
pub fn snap_bars(bar: f64, division: u32, beats_per_bar: f64) -> f64 {
    let step = snap_step_bars(division, beats_per_bar);
    (bar / step).round() * step
}
pub fn snap_beats(beats: f64, division: u32) -> f64 {
    let step = 4.0 / division.max(1) as f64;
    (beats / step).round() * step
}

/// Fader taper, Logic-style: unity at 0.75, +6 dB at the top, silence at the bottom.
pub const FADER_UNITY: f32 = 0.75;
pub const FADER_MAX_DB: f32 = 6.0;
pub const FADER_MIN_DB: f32 = -60.0;

pub fn fader_to_db(position: f32) -> f32 {
    let p = position.clamp(0.0, 1.0);
    if p <= 0.0 {
        return f32::NEG_INFINITY;
    }
    if p >= FADER_UNITY {
        return (p - FADER_UNITY) / (1.0 - FADER_UNITY) * FADER_MAX_DB;
    }
    20.0 * (p / FADER_UNITY).log10() * 1.6
}
pub fn db_to_fader(db: f32) -> f32 {
    if db == f32::NEG_INFINITY || db <= FADER_MIN_DB {
        return 0.0;
    }
    if db >= 0.0 {
        return FADER_UNITY + db.min(FADER_MAX_DB) / FADER_MAX_DB * (1.0 - FADER_UNITY);
    }
    10f32.powf(db / 1.6 / 20.0) * FADER_UNITY
}
/// "−3.9", "+1.2", "0.0", "−∞".
pub fn db(db: f32, digits: usize) -> String {
    if db == f32::NEG_INFINITY || db <= FADER_MIN_DB {
        return "−∞".into();
    }
    let sign = if db < 0.0 && (db * 10f32.powi(digits as i32)).round() != 0.0 {
        "−"
    } else if db > 0.0 && (db * 10f32.powi(digits as i32)).round() != 0.0 {
        "+"
    } else {
        ""
    };
    format!("{sign}{:.*}", digits, db.abs())
}
/// A linear peak as decibels.
pub fn peak_db(peak: f32) -> f32 {
    if peak <= 1e-6 {
        f32::NEG_INFINITY
    } else {
        20.0 * peak.log10()
    }
}
/// Pan −1..1 as "L 15", "C", "R 40".
pub fn pan(pan: f32) -> String {
    let v = (pan * 100.0).round() as i32;
    match v {
        0 => "C".into(),
        v if v < 0 => format!("L {}", -v),
        v => format!("R {v}"),
    }
}

/// Where the beat lines of one bar fall, in pixels from the bar line: none when they would
/// crowd closer than six pixels.
pub fn beat_line_offsets(pixels_per_bar: f32, numerator: u32) -> Vec<f32> {
    let beats = numerator.max(1);
    let step = pixels_per_bar / beats as f32;
    if pixels_per_bar < 24.0 || step < 6.0 {
        return vec![];
    }
    (1..beats).map(|i| i as f32 * step).collect()
}

/// MIDI pitch as a note name: 60 → C4.
pub fn note_name(pitch: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B",
    ];
    format!("{}{}", NAMES[pitch as usize % 12], pitch as i32 / 12 - 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn positions_and_gain_read_as_in_the_design() {
        assert_eq!(
            bar_beat(13.0, 4.0),
            BarBeat {
                bar: 4,
                beat: 2,
                division: 1,
                tick: 0
            }
        );
        assert_eq!(bar_beat_short(0.0, 4.0), "1.1.1");
        assert_eq!(smpte(6.5), "00:00:06:15");
        assert!((fader_to_db(0.75)).abs() < 1e-6);
        assert!((db_to_fader(fader_to_db(0.4)) - 0.4).abs() < 1e-4);
        assert_eq!(db(-3.94, 1), "−3.9");
        assert_eq!(db(f32::NEG_INFINITY, 1), "−∞");
        assert_eq!(pan(-0.15), "L 15");
        assert_eq!(note_name(60), "C4");
        assert!((snap_bars(1.13, 16, 4.0) - 1.125).abs() < 1e-9);
    }
}
