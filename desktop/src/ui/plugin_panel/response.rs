//! What each stock plugin does to a signal, as curves for its display. The maths mirrors
//! `sdk/src/dsp.rs` and `engine/src/stock.rs` so the picture is the sound: the same RBJ
//! biquads for the EQ, the same SVF damping for the filter, the same soft knee for the
//! dynamics. Ported from the React face (`response.ts`).

use std::f64::consts::{PI, SQRT_2};

const RATE: f64 = 48_000.0;
pub const F_MIN: f64 = 20.0;
pub const F_MAX: f64 = 20_000.0;

/// x in 0..1 to Hz on a log axis.
pub fn x_to_hz(x: f64) -> f64 {
    F_MIN * (F_MAX / F_MIN).powf(x)
}
/// Hz to x in 0..1 on a log axis.
pub fn hz_to_x(hz: f64) -> f64 {
    (hz.clamp(F_MIN, F_MAX) / F_MIN).ln() / (F_MAX / F_MIN).ln()
}

/// b0, b1, b2, a0, a1, a2.
type Biquad = [f64; 6];

fn shelf(hz: f64, gain_db: f64, high: bool) -> Biquad {
    let a = 10f64.powf(gain_db / 40.0);
    let w = 2.0 * PI * hz.clamp(10.0, RATE * 0.45) / RATE;
    let (s, c) = (w.sin(), w.cos());
    let sq = 2.0 * a.sqrt() * ((s / 2.0) * SQRT_2);
    let k = if high { -1.0 } else { 1.0 };
    [
        a * (a + 1.0 - k * (a - 1.0) * c + sq),
        k * 2.0 * a * (a - 1.0 - k * (a + 1.0) * c),
        a * (a + 1.0 - k * (a - 1.0) * c - sq),
        a + 1.0 + k * (a - 1.0) * c + sq,
        k * -2.0 * (a - 1.0 + k * (a + 1.0) * c),
        a + 1.0 + k * (a - 1.0) * c - sq,
    ]
}

fn peaking(hz: f64, gain_db: f64, q: f64) -> Biquad {
    let a = 10f64.powf(gain_db / 40.0);
    let w = 2.0 * PI * hz.clamp(10.0, RATE * 0.45) / RATE;
    let alpha = w.sin() / (2.0 * q.max(0.05));
    let c = w.cos();
    [
        1.0 + alpha * a,
        -2.0 * c,
        1.0 - alpha * a,
        1.0 + alpha / a,
        -2.0 * c,
        1.0 - alpha / a,
    ]
}

fn magnitude_db([b0, b1, b2, a0, a1, a2]: Biquad, hz: f64) -> f64 {
    let w = 2.0 * PI * hz / RATE;
    let (c1, c2, s1, s2) = (w.cos(), (2.0 * w).cos(), w.sin(), (2.0 * w).sin());
    let num = (b0 + b1 * c1 + b2 * c2).powi(2) + (b1 * s1 + b2 * s2).powi(2);
    let den = (a0 + a1 * c1 + a2 * c2).powi(2) + (a1 * s1 + a2 * s2).powi(2);
    10.0 * (num / den).log10()
}

/// The three bands of the Channel EQ.
#[derive(Clone, Copy, Debug)]
pub struct EqBands {
    pub low_gain: f64,
    pub low_freq: f64,
    pub mid_gain: f64,
    pub mid_freq: f64,
    pub mid_q: f64,
    pub high_gain: f64,
    pub high_freq: f64,
}

/// Summed response of the three Channel EQ bands at `hz`, in dB.
pub fn eq_db(b: &EqBands, hz: f64) -> f64 {
    magnitude_db(shelf(b.low_freq, b.low_gain, false), hz)
        + magnitude_db(peaking(b.mid_freq, b.mid_gain, b.mid_q), hz)
        + magnitude_db(shelf(b.high_freq, b.high_gain, true), hz)
}

/// State-variable filter response; `kind` 0 low-pass, 1 high-pass, 2 band-pass.
pub fn filter_db(kind: f64, cutoff: f64, resonance_percent: f64, hz: f64) -> f64 {
    let k = 2.0 - 1.9 * (resonance_percent / 100.0).clamp(0.0, 1.0);
    let f = hz / cutoff;
    // |1 / (1 - f² + jkf)| with the numerator of each tap.
    let den = ((1.0 - f * f).powi(2) + (k * f).powi(2)).sqrt();
    let num = match kind.round() as i64 {
        1 => f * f,
        2 => k * f,
        _ => 1.0,
    };
    20.0 * (num / den).max(1e-6).log10()
}

/// One-pole tone control used by the saturators and the echo.
pub fn tone_db(cutoff: f64, hz: f64) -> f64 {
    -10.0 * (1.0 + (hz / cutoff).powi(2)).log10()
}

/// Compressor output level for an input level, both dBFS. Soft 6 dB knee.
pub fn compress_db(input: f64, threshold: f64, ratio: f64, makeup: f64) -> f64 {
    let knee = 6.0;
    let over = input - threshold;
    let slope = 1.0 - 1.0 / ratio.max(1.0);
    let reduction = if over > knee / 2.0 {
        over * slope
    } else if over > -knee / 2.0 {
        slope * (over + knee / 2.0).powi(2) / (2.0 * knee)
    } else {
        0.0
    };
    input - reduction + makeup
}

/// Gate output level: below threshold the signal drops by `range` dB.
pub fn gate_db(input: f64, threshold: f64, range: f64) -> f64 {
    if input >= threshold {
        input
    } else {
        (input + range).max(-96.0)
    }
}

/// Limiter output level: the input gain, held under the ceiling.
pub fn limit_db(input: f64, gain: f64, ceiling: f64) -> f64 {
    (input + gain).min(ceiling)
}

/// Waveshaper transfer, x and result in -1..1.
pub fn saturate(x: f64, drive_db: f64, hard: bool) -> f64 {
    let g = 10f64.powf(drive_db / 20.0);
    if hard {
        ((x * g) / (1.0 + (x * g).abs() * 0.35)).clamp(-1.0, 1.0)
    } else {
        (x * g).tanh() / g.tanh().max(1e-6)
    }
}

/// A sine after bit reduction and sample-and-hold, t in 0..1.
pub fn crush(t: f64, bits: f64, downsample: f64) -> f64 {
    let steps = (96.0 / downsample).round().max(4.0);
    let held = (t * steps).floor() / steps;
    let levels = 2f64.powf(bits - 1.0);
    ((held * 2.0 * PI * 2.0).sin() * levels).round() / levels
}

/// LFO value in -1..1 at phase 0..1; shape 0 sine, 1 triangle, 2 square.
pub fn lfo(shape: f64, phase: f64) -> f64 {
    let p = phase.rem_euclid(1.0);
    match shape.round() as i64 {
        1 => 1.0 - 4.0 * (p - 0.5).abs(),
        2 => {
            if p < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        _ => (p * 2.0 * PI).sin(),
    }
}

/// Envelope stages in milliseconds, sustain 0-1.
#[derive(Clone, Copy, Debug)]
pub struct Adsr {
    pub attack: f64,
    pub decay: f64,
    pub sustain: f64,
    pub release: f64,
}

/// Envelope as polyline points in 0..1 × 0..1. Stage widths are compressed with a square
/// root so a 5 ms attack is still visible beside a 3 s release.
pub fn envelope_points(e: Adsr, hold: f64) -> [(f64, f64); 5] {
    let w = |ms: f64| ms.max(1.0).sqrt();
    let (a, d, r) = (w(e.attack), w(e.decay), w(e.release));
    let total = (a + d + r) / (1.0 - hold);
    let s = e.sustain.clamp(0.0, 1.0);
    let x1 = a / total;
    let x2 = x1 + d / total;
    let x3 = x2 + hold;
    [(0.0, 0.0), (x1, 1.0), (x2, s), (x3, s), (1.0, 0.0)]
}

/// Sample `f` over 0..1 into points in 0..1 × 0..1 (y up), clamped.
pub fn plot(f: impl Fn(f64) -> f64, samples: usize) -> Vec<(f64, f64)> {
    (0..=samples)
        .map(|i| {
            let x = i as f64 / samples as f64;
            (x, f(x).clamp(0.0, 1.0))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLAT: EqBands = EqBands {
        low_gain: 0.0,
        low_freq: 120.0,
        mid_gain: 0.0,
        mid_freq: 1000.0,
        mid_q: 0.8,
        high_gain: 0.0,
        high_freq: 6000.0,
    };
    fn close(a: f64, b: f64, digits: i32) -> bool {
        (a - b).abs() < 0.5 * 10f64.powi(-digits)
    }

    #[test]
    fn a_flat_eq_is_0_db_and_a_boosted_band_reaches_its_gain() {
        for hz in [30.0, 440.0, 12000.0] {
            assert!(eq_db(&FLAT, hz).abs() < 1e-6, "{hz}");
        }
        assert!(close(
            eq_db(
                &EqBands {
                    mid_gain: 6.0,
                    ..FLAT
                },
                1000.0
            ),
            6.0,
            1
        ));
        // A shelf reaches its full gain well past the corner, half of it at the corner.
        assert!(close(
            eq_db(
                &EqBands {
                    low_gain: 12.0,
                    ..FLAT
                },
                20.0
            ),
            12.0,
            0
        ));
        assert!(close(
            eq_db(
                &EqBands {
                    low_gain: 12.0,
                    ..FLAT
                },
                120.0
            ),
            6.0,
            0
        ));
        assert!(close(
            eq_db(
                &EqBands {
                    high_gain: -9.0,
                    ..FLAT
                },
                19000.0
            ),
            -9.0,
            0
        ));
    }

    #[test]
    fn the_filter_matches_the_state_variable_filter() {
        // -3 dB at the cutoff when k = √2, 12 dB per octave above it.
        let resonance = (2.0 - SQRT_2) / 1.9 * 100.0;
        assert!(close(filter_db(0.0, 1000.0, resonance, 1000.0), -3.01, 1));
        let slope =
            filter_db(0.0, 1000.0, resonance, 8000.0) - filter_db(0.0, 1000.0, resonance, 4000.0);
        assert!(close(slope, -12.0, 0));
        assert!(filter_db(1.0, 1000.0, resonance, 20.0) < -60.0);
        assert!(close(filter_db(0.0, 1000.0, 100.0, 1000.0), 20.0, 0));
    }

    #[test]
    fn the_compressor_reduces_above_the_knee_by_its_ratio() {
        assert!(close(compress_db(-40.0, -18.0, 4.0, 0.0), -40.0, 6));
        assert!(close(
            compress_db(-6.0, -18.0, 4.0, 0.0),
            -18.0 + 12.0 / 4.0,
            6
        ));
        assert!(close(compress_db(-6.0, -18.0, 4.0, 3.0), -12.0, 6));
        assert_eq!(gate_db(-50.0, -40.0, -80.0), -96.0);
        assert_eq!(limit_db(-1.0, 3.0, -0.3), -0.3);
    }

    #[test]
    fn a_short_attack_stays_visible_beside_a_long_release() {
        let points = envelope_points(
            Adsr {
                attack: 5.0,
                decay: 200.0,
                sustain: 0.6,
                release: 3000.0,
            },
            0.22,
        );
        assert!(points[1].0 > 0.02);
        assert_eq!(points[4], (1.0, 0.0));
        assert!(points.windows(2).all(|w| w[1].0 >= w[0].0));
    }

    #[test]
    fn the_log_axis_round_trips_and_shapes_stay_in_range() {
        assert!(close(x_to_hz(hz_to_x(1000.0)), 1000.0, 6));
        assert_eq!(hz_to_x(5.0), 0.0);
        assert_eq!(hz_to_x(40_000.0), 1.0);
        for shape in [0.0, 1.0, 2.0] {
            for i in 0..=20 {
                let v = lfo(shape, i as f64 / 7.0 - 1.0);
                assert!((-1.0..=1.0).contains(&v));
            }
        }
        assert!(close(saturate(1.0, 12.0, false), 1.0, 6));
        assert!(plot(|x| x * 2.0 - 0.5, 8)
            .iter()
            .all(|(_, y)| (0.0..=1.0).contains(y)));
        assert!(crush(0.3, 4.0, 4.0).abs() <= 1.0);
    }
}
