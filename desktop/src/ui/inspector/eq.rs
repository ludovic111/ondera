//! The inspector's Channel EQ display: the summed response of the stock Channel EQ's three
//! bands (low shelf, peaking mid, high shelf) drawn from the insert's parameters. The maths
//! is the RBJ biquad design of `sdk/src/dsp.rs`, the one the stock plugin runs, so the
//! picture is the sound (a test feeds sines through the real filters to keep them agreed).

use crate::ui::theme::{radius, with_alpha, Theme};
use gpui::{canvas, div, point, prelude::*, px, App, Bounds, PathBuilder, Pixels};
use ryolune_engine::model::Insert;
use std::f64::consts::{PI, SQRT_2};

/// The stock plugin the display reads.
pub const EQ_NAME: &str = "Channel EQ";
const RATE: f64 = 48_000.0;
pub const F_MIN: f64 = 20.0;
pub const F_MAX: f64 = 20_000.0;
/// The gain the display spans either side of 0 dB.
const RANGE_DB: f64 = 18.0;
/// Display height, as the design frame draws it.
pub const HEIGHT: f32 = 74.0;

/// The Channel EQ's parameters, by meaning.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EqBands {
    pub low_gain: f64,
    pub low_freq: f64,
    pub mid_gain: f64,
    pub mid_freq: f64,
    pub mid_q: f64,
    pub high_gain: f64,
    pub high_freq: f64,
}

impl EqBands {
    /// The bands an insert holds: its explicit values over the plugin's defaults.
    pub fn of(insert: &Insert) -> Self {
        let infos = ryolune_engine::stock::params(EQ_NAME);
        let value = |name: &str, fallback: f64| {
            infos.iter().find(|p| p.name == name).map_or(fallback, |p| {
                insert.params.get(&p.id).copied().unwrap_or(p.default)
            })
        };
        Self {
            low_gain: value("Low Gain", 0.0),
            low_freq: value("Low Freq", 120.0),
            mid_gain: value("Mid Gain", 0.0),
            mid_freq: value("Mid Freq", 1000.0),
            mid_q: value("Mid Q", 0.8),
            high_gain: value("High Gain", 0.0),
            high_freq: value("High Freq", 6000.0),
        }
    }

    /// Summed response of the three bands at `hz`, in dB.
    pub fn db(&self, hz: f64) -> f64 {
        magnitude_db(shelf(self.low_freq, self.low_gain, false), hz)
            + magnitude_db(peaking(self.mid_freq, self.mid_gain, self.mid_q), hz)
            + magnitude_db(shelf(self.high_freq, self.high_gain, true), hz)
    }
}

/// How the strip runs its Channel EQ.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EqState {
    Missing,
    Bypassed(EqBands),
    Active(EqBands),
}

impl EqState {
    /// The first Channel EQ on a strip, running or bypassed, with the slot it is in.
    pub fn of(slots: &[Insert]) -> (Self, Option<usize>) {
        let found = slots
            .iter()
            .enumerate()
            .find(|(_, i)| !i.is_empty() && i.plugin_id() == format!("stock:{EQ_NAME}"));
        match found {
            Some((slot, insert)) if insert.state == "bypassed" => {
                (Self::Bypassed(EqBands::of(insert)), Some(slot))
            }
            Some((slot, insert)) => (Self::Active(EqBands::of(insert)), Some(slot)),
            None => (Self::Missing, None),
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Self::Missing => "not inserted",
            Self::Bypassed(_) => "bypassed",
            Self::Active(_) => "active",
        }
    }
}

/// x in 0..1 to Hz on a log axis, and back.
pub fn x_to_hz(x: f64) -> f64 {
    F_MIN * (F_MAX / F_MIN).powf(x)
}
pub fn hz_to_x(hz: f64) -> f64 {
    (hz.clamp(F_MIN, F_MAX) / F_MIN).ln() / (F_MAX / F_MIN).ln()
}

type Coefficients = [f64; 6];

fn shelf(hz: f64, gain_db: f64, high: bool) -> Coefficients {
    let a = 10f64.powf(gain_db / 40.0);
    let w = 2.0 * PI * hz.clamp(10.0, RATE * 0.45) / RATE;
    let (s, c) = w.sin_cos();
    let sq = 2.0 * a.sqrt() * (s / 2.0 * SQRT_2);
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

fn peaking(hz: f64, gain_db: f64, q: f64) -> Coefficients {
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

fn magnitude_db([b0, b1, b2, a0, a1, a2]: Coefficients, hz: f64) -> f64 {
    let w = 2.0 * PI * hz / RATE;
    let (s1, c1) = w.sin_cos();
    let (s2, c2) = (2.0 * w).sin_cos();
    let num = (b0 + b1 * c1 + b2 * c2).powi(2) + (b1 * s1 + b2 * s2).powi(2);
    let den = (a0 + a1 * c1 + a2 * c2).powi(2) + (a1 * s1 + a2 * s2).powi(2);
    10.0 * (num / den).log10()
}

/// The display: a deep well with a frequency and gain grid, and the curve. A running EQ is
/// drawn in the accent with a soft fill, a bypassed one dimmed, a missing one as a flat line.
pub fn display(state: EqState, cx: &App) -> gpui::Div {
    let theme = Theme::get(cx).clone();
    div()
        .h(px(HEIGHT))
        .w_full()
        .rounded(px(radius::SM))
        .overflow_hidden()
        .bg(theme.display)
        .border_1()
        .border_color(theme.hairline)
        .child(
            canvas(
                |_, _, _| (),
                move |b: Bounds<Pixels>, _, window, _| {
                    let (w, h) = (f32::from(b.size.width), f32::from(b.size.height));
                    let at = |x: f32, y: f32| point(b.origin.x + px(x), b.origin.y + px(y));
                    let y_of = |db: f64| h / 2.0 - (db / RANGE_DB) as f32 * (h / 2.0 - 4.0);
                    // Decades of frequency, then ±12 dB and the 0 dB line.
                    for hz in [100.0, 1_000.0, 10_000.0] {
                        let x = (hz_to_x(hz) as f32 * w).round();
                        window.paint_quad(gpui::fill(
                            Bounds::new(at(x, 0.0), gpui::size(px(1.0), px(h))),
                            theme.hairline,
                        ));
                    }
                    for db in [12.0, -12.0] {
                        let y = y_of(db).round();
                        window.paint_quad(gpui::fill(
                            Bounds::new(at(0.0, y), gpui::size(px(w), px(1.0))),
                            theme.hairline,
                        ));
                    }
                    window.paint_quad(gpui::fill(
                        Bounds::new(at(0.0, y_of(0.0).round()), gpui::size(px(w), px(1.0))),
                        theme.line,
                    ));
                    let (bands, color) = match state {
                        EqState::Missing => (None, theme.text_3.opacity(0.5)),
                        EqState::Bypassed(bands) => (Some(bands), theme.text_3),
                        EqState::Active(bands) => (Some(bands), theme.accent),
                    };
                    let steps = (w / 2.0).max(8.0) as usize;
                    let points: Vec<_> = (0..=steps)
                        .map(|i| {
                            let x = i as f32 / steps as f32;
                            let db = bands.map_or(0.0, |b| b.db(x_to_hz(x as f64)));
                            at(x * w, y_of(db.clamp(-RANGE_DB, RANGE_DB)))
                        })
                        .collect();
                    if let (EqState::Active(_), Some(first), Some(last)) =
                        (state, points.first(), points.last())
                    {
                        // The area between the curve and 0 dB, softly lit.
                        let zero = b.origin.y + px(y_of(0.0));
                        let mut fill = PathBuilder::fill();
                        fill.move_to(point(first.x, zero));
                        for p in &points {
                            fill.line_to(*p);
                        }
                        fill.line_to(point(last.x, zero));
                        fill.close();
                        if let Ok(path) = fill.build() {
                            window.paint_path(path, with_alpha(theme.accent, 0.14));
                        }
                    }
                    let mut stroke = PathBuilder::stroke(px(1.5));
                    stroke.move_to(points[0]);
                    for p in &points[1..] {
                        stroke.line_to(*p);
                    }
                    if let Ok(path) = stroke.build() {
                        window.paint_path(path, color);
                    }
                },
            )
            .size_full(),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ryolune_plugin::dsp::Biquad;

    fn flat() -> EqBands {
        EqBands {
            low_gain: 0.0,
            low_freq: 120.0,
            mid_gain: 0.0,
            mid_freq: 1000.0,
            mid_q: 0.8,
            high_gain: 0.0,
            high_freq: 6000.0,
        }
    }

    /// Steady-state gain of the plugin's own three biquads for a sine at `hz`.
    fn measured_db(b: &EqBands, hz: f64) -> f64 {
        let mut bands = [Biquad::default(); 3];
        bands[0].low_shelf(RATE, b.low_freq, b.low_gain);
        bands[1].peaking(RATE, b.mid_freq, b.mid_gain, b.mid_q);
        bands[2].high_shelf(RATE, b.high_freq, b.high_gain);
        let n = RATE as usize / 2;
        // Power over the second half, once the filters have settled.
        let (mut power_in, mut power_out) = (0f64, 0f64);
        for i in 0..n {
            let x = (2.0 * PI * hz * i as f64 / RATE).sin() as f32;
            let y = bands.iter_mut().fold(x, |v, band| band.process(0, v));
            if i > n / 2 {
                power_in += (x as f64).powi(2);
                power_out += (y as f64).powi(2);
            }
        }
        10.0 * (power_out / power_in).log10()
    }

    #[test]
    fn a_flat_eq_is_flat() {
        for hz in [30.0, 200.0, 1000.0, 9000.0, 18000.0] {
            assert!(flat().db(hz).abs() < 1e-6, "{hz}");
        }
    }

    #[test]
    fn the_curve_is_what_the_plugin_does() {
        let b = EqBands {
            low_gain: 6.0,
            mid_gain: -9.0,
            mid_q: 1.4,
            high_gain: 4.0,
            ..flat()
        };
        for hz in [50.0, 120.0, 400.0, 1000.0, 3000.0, 8000.0, 15000.0] {
            let drawn = b.db(hz);
            let heard = measured_db(&b, hz);
            assert!(
                (drawn - heard).abs() < 0.25,
                "{hz} Hz: drawn {drawn:.2}, heard {heard:.2}"
            );
        }
        assert!((b.db(1000.0) + 9.0).abs() < 1.0);
        assert!(b.db(30.0) > 5.0);
    }

    #[test]
    fn inserts_read_their_values_over_the_defaults() {
        let mut insert = Insert::new("eq".into(), "stock:Channel EQ", EQ_NAME);
        let defaults = EqBands::of(&insert);
        assert_eq!(defaults.low_freq, 120.0);
        assert_eq!(defaults.mid_gain, -2.0);
        let mid_gain = ryolune_engine::stock::params(EQ_NAME)
            .into_iter()
            .find(|p| p.name == "Mid Gain")
            .unwrap()
            .id;
        insert.params.insert(mid_gain, 5.0);
        assert_eq!(EqBands::of(&insert).mid_gain, 5.0);

        let mut slots = vec![Insert::empty_slot(); 8];
        assert_eq!(EqState::of(&slots), (EqState::Missing, None));
        slots[3] = insert.clone();
        assert!(matches!(EqState::of(&slots), (EqState::Active(_), Some(3))));
        slots[3].state = "bypassed".into();
        assert_eq!(EqState::of(&slots).0.label(), "bypassed");
    }

    #[test]
    fn the_axis_is_logarithmic() {
        assert!((x_to_hz(0.0) - F_MIN).abs() < 1e-9);
        assert!((x_to_hz(1.0) - F_MAX).abs() < 1e-6);
        assert!((hz_to_x(x_to_hz(0.37)) - 0.37).abs() < 1e-9);
        assert!((hz_to_x(632.455_532) - 0.5).abs() < 1e-6);
    }
}
