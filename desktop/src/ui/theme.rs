//! The one place visual values live. ryolune wears the lsuite design system
//! (`lsuite/design/tokens.json`, lsuite.xyz/design): frosted glass for the chrome, solid
//! surfaces for the work, and ryolune's signature teal (hue 185) as the accent. The accent
//! means "yours or active": the playhead, selection, focus, lit keys and what the agent
//! touched. States keep their own colours: record and errors red, meters mint and amber,
//! mute blue, solo yellow.
//!
//! Add a token here, in both modes, never a colour in a view. `contrast` tests below keep
//! text readable on every surface, glass tiers included, over the brightest and darkest
//! desktop behind the window.

use gpui::{px, App, Global, Hsla, Pixels, Rgba, SharedString};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Dark,
    Light,
}

/// The lsuite signature scale of an app (11 steps, 50 to 950), here ryolune's teal.
pub const TEAL: [u32; 11] = [
    0xe0fcf8, 0xbbf6ed, 0x81eadd, 0x37d8c9, 0x00c5b4, 0x00b09f, 0x009586, 0x00766a, 0x00584f,
    0x003b35, 0x002420,
];
/// Index of a step in [`TEAL`]: 50 → 0, 100 → 1 … 950 → 10.
const fn step(n: u32) -> usize {
    if n == 50 {
        0
    } else if n == 950 {
        10
    } else {
        (n / 100) as usize
    }
}

/// A glass tier: translucent fill, its opaque fallback, and the edge light.
#[derive(Clone, Copy, Debug)]
pub struct Glass {
    pub fill: Hsla,
    pub opaque: Hsla,
}

#[derive(Clone, Debug)]
pub struct Theme {
    pub mode: Mode,
    /// Opaque fills instead of glass: the system asks for reduced transparency, or the
    /// platform cannot blur the window.
    pub opaque: bool,

    // Window backdrop: what the glass sits on. Translucent over the blurred desktop.
    pub backdrop: Hsla,
    /// The two soft glows of the app colour in the backdrop (`.ls-backdrop`).
    pub aurora: Hsla,

    // Work surfaces: never glass.
    pub bg: Hsla,
    pub bg_raised: Hsla,
    pub bg_sunken: Hsla,
    pub lane: Hsla,
    pub lane_alt: Hsla,
    pub lane_selected: Hsla,
    pub lane_agent: Hsla,
    pub lane_empty: Hsla,
    pub ruler: Hsla,
    pub editor: Hsla,
    pub display: Hsla,
    pub well: Hsla,

    // Controls.
    pub control: Hsla,
    pub control_hover: Hsla,
    pub control_pressed: Hsla,
    pub control_edge: Hsla,
    pub control_highlight: Hsla,
    pub thumb: Hsla,
    pub knob: Hsla,
    pub knob_edge: Hsla,

    // Text.
    pub text: Hsla,
    pub text_2: Hsla,
    pub text_3: Hsla,
    pub text_on_accent: Hsla,
    pub text_display: Hsla,

    // Lines.
    pub line: Hsla,
    pub line_strong: Hsla,
    pub hairline: Hsla,
    pub bar_line: Hsla,
    pub beat_line: Hsla,

    // Glass tiers (DESIGN.md: chrome, floating, modal) and their trim.
    pub glass_1: Glass,
    pub glass_2: Glass,
    pub glass_3: Glass,
    pub glass_edge: Hsla,
    pub glass_highlight: Hsla,
    pub glass_shadow: Hsla,
    pub scrim: Hsla,

    // Accent: ryolune teal.
    pub accent: Hsla,
    pub accent_hover: Hsla,
    /// The accent as a fill that carries text (primary buttons, lit keys): one step darker
    /// by day so white text keeps 4.5:1.
    pub accent_fill: Hsla,
    pub accent_text: Hsla,
    pub accent_soft: Hsla,
    pub accent_ring: Hsla,
    pub accent_glow: Hsla,

    // States.
    pub danger: Hsla,
    pub warning: Hsla,
    pub success: Hsla,
    pub record: Hsla,
    pub mute: Hsla,
    pub solo: Hsla,
    pub meter: Hsla,
    pub meter_hot: Hsla,
    pub meter_clip: Hsla,
    pub meter_off: Hsla,

    // Musical surfaces.
    pub key_white: Hsla,
    pub key_black: Hsla,
    pub key_label: Hsla,
    pub black_key_row: Hsla,
    pub note: Hsla,
    pub note_edge: Hsla,
    pub velocity: Hsla,
    pub waveform: Hsla,
    pub clip_text: Hsla,
    pub clip_title: Hsla,
    pub cycle: Hsla,
    pub cycle_lane: Hsla,
    pub marker: Hsla,
    pub hover: Hsla,
    pub selection_text: Hsla,
}

impl Global for Theme {}

impl Theme {
    pub fn new(mode: Mode, opaque: bool) -> Self {
        match mode {
            Mode::Dark => dark(opaque),
            Mode::Light => light(opaque),
        }
    }
    pub fn get(cx: &App) -> &Theme {
        cx.global::<Theme>()
    }
    /// The fill of a glass tier, or its opaque fallback.
    pub fn glass(&self, tier: u8) -> Hsla {
        let g = match tier {
            1 => self.glass_1,
            2 => self.glass_2,
            _ => self.glass_3,
        };
        if self.opaque {
            g.opaque
        } else {
            g.fill
        }
    }
    /// A sound family's colour (browser folders, plugin faces, insert slots), by the
    /// folder name the engine files a plugin under.
    pub fn family(&self, folder: &str) -> Hsla {
        let hue = family_hue(folder);
        match self.mode {
            Mode::Dark => oklch(0.78, 0.115, hue, 1.0),
            Mode::Light => oklch(0.55, 0.13, hue, 1.0),
        }
    }
    /// A track's colour: the document keeps a CSS colour; an unreadable one falls back to
    /// the palette by position.
    pub fn track(&self, color: &str, index: usize) -> Hsla {
        css_color(color).unwrap_or_else(|| {
            let (l, c, h) = TRACK_PALETTE[index % TRACK_PALETTE.len()].1;
            oklch(l, c, h, 1.0)
        })
    }
}

/// Track palette from the design spec sheet: L 0.72–0.78, C 0.12–0.14. Names are what the
/// track menu offers; the value written to the document is the CSS `oklch()` text.
pub const TRACK_PALETTE: [(&str, (f32, f32, f32)); 8] = [
    ("Drums", (0.72, 0.14, 40.0)),
    ("Bass", (0.72, 0.13, 300.0)),
    ("Keys", (0.75, 0.13, 250.0)),
    ("Pad", (0.75, 0.12, 330.0)),
    ("Vox", (0.78, 0.14, 85.0)),
    ("Backing vocals", (0.75, 0.12, 130.0)),
    ("Guitar", (0.72, 0.13, 20.0)),
    ("Riser", (0.75, 0.14, 60.0)),
];
pub fn palette_css(index: usize) -> String {
    let (_, (l, c, h)) = TRACK_PALETTE[index % TRACK_PALETTE.len()];
    format!("oklch({l} {c} {h})")
}

fn family_hue(folder: &str) -> f32 {
    const HUES: [(&str, f32); 14] = [
        ("famSynth", 285.0),
        ("famKeys", 85.0),
        ("famBass", 15.0),
        ("famDrums", 50.0),
        ("famPad", 235.0),
        ("famSampler", 160.0),
        ("famTexture", 335.0),
        ("famDynamics", 60.0),
        ("famEq", 305.0),
        ("famDrive", 28.0),
        ("famMod", 195.0),
        ("famSpace", 255.0),
        ("famPitch", 350.0),
        ("famUtility", 130.0),
    ];
    let key = match folder {
        "Synths" | "Other Instruments" => "famSynth",
        "Keys" | "Channel Strips" => "famKeys",
        "Bass" => "famBass",
        "Drums" => "famDrums",
        "Pads" => "famPad",
        "Samplers" | "Mastering" => "famSampler",
        "Textures" | "Restoration" => "famTexture",
        "Dynamics" => "famDynamics",
        "EQ & Filter" => "famEq",
        "Distortion" => "famDrive",
        "Modulation" => "famMod",
        "Space & Time" => "famSpace",
        "Pitch" => "famPitch",
        "Utility" | "Other Effects" => "famUtility",
        other => {
            // A folder the user made up gets a stable family from its name.
            let hash = other
                .chars()
                .fold(0u32, |h, c| h.wrapping_mul(31).wrapping_add(c as u32));
            return HUES[hash as usize % HUES.len()].1;
        }
    };
    HUES.iter().find(|(k, _)| *k == key).map_or(0.0, |(_, h)| *h)
}

pub fn hex(rgb: u32) -> Hsla {
    gpui::rgb(rgb).into()
}
pub fn hexa(rgb: u32, alpha: f32) -> Hsla {
    let mut c: Hsla = gpui::rgb(rgb).into();
    c.a = alpha;
    c
}
pub fn white(alpha: f32) -> Hsla {
    hexa(0xffffff, alpha)
}
pub fn black(alpha: f32) -> Hsla {
    hexa(0x000000, alpha)
}
pub fn with_alpha(mut c: Hsla, alpha: f32) -> Hsla {
    c.a = alpha;
    c
}
fn teal(n: u32) -> Hsla {
    hex(TEAL[step(n)])
}

/// OKLCH to sRGB (CSS Color 4), gamut-clipped.
pub fn oklch(l: f32, c: f32, h: f32, alpha: f32) -> Hsla {
    let (a, b) = (c * h.to_radians().cos(), c * h.to_radians().sin());
    let l_ = l + 0.396_337_78 * a + 0.215_803_76 * b;
    let m_ = l - 0.105_561_346 * a - 0.063_854_17 * b;
    let s_ = l - 0.089_484_18 * a - 1.291_485_5 * b;
    let (l3, m3, s3) = (l_.powi(3), m_.powi(3), s_.powi(3));
    let r = 4.076_741_7 * l3 - 3.307_711_6 * m3 + 0.230_969_94 * s3;
    let g = -1.268_438 * l3 + 2.609_757_4 * m3 - 0.341_319_38 * s3;
    let bl = -0.004_196_086_3 * l3 - 0.703_418_6 * m3 + 1.707_614_7 * s3;
    let encode = |x: f32| {
        let x = x.clamp(0.0, 1.0);
        if x <= 0.003_130_8 {
            12.92 * x
        } else {
            1.055 * x.powf(1.0 / 2.4) - 0.055
        }
    };
    Rgba {
        r: encode(r),
        g: encode(g),
        b: encode(bl),
        a: alpha,
    }
    .into()
}

/// The CSS colours a document can hold: `#rgb`, `#rrggbb`, `rgb()/rgba()` and `oklch()`.
pub fn css_color(text: &str) -> Option<Hsla> {
    let t = text.trim();
    if let Some(hex_text) = t.strip_prefix('#') {
        let expanded: String = if hex_text.len() == 3 {
            hex_text.chars().flat_map(|c| [c, c]).collect()
        } else {
            hex_text.to_string()
        };
        return u32::from_str_radix(&expanded, 16)
            .ok()
            .filter(|_| expanded.len() == 6)
            .map(hex);
    }
    let (name, args) = t.split_once('(')?;
    let args = args.strip_suffix(')')?;
    let parts: Vec<&str> = args
        .split(|c: char| c == ',' || c == '/' || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .collect();
    let num = |p: &str| -> Option<f32> {
        if let Some(pct) = p.strip_suffix('%') {
            pct.parse::<f32>().ok().map(|v| v / 100.0)
        } else {
            p.trim_end_matches("deg").parse::<f32>().ok()
        }
    };
    match name.trim() {
        "oklch" => {
            let l = num(parts.first()?)?;
            let c = num(parts.get(1)?)?;
            let h = num(parts.get(2)?)?;
            let a = parts.get(3).and_then(|p| num(p)).unwrap_or(1.0);
            Some(oklch(l, c, h, a))
        }
        "rgb" | "rgba" => {
            let v = |i: usize| parts.get(i).and_then(|p| p.parse::<f32>().ok());
            Some(
                Rgba {
                    r: v(0)? / 255.0,
                    g: v(1)? / 255.0,
                    b: v(2)? / 255.0,
                    a: parts.get(3).and_then(|p| num(p)).unwrap_or(1.0),
                }
                .into(),
            )
        }
        _ => None,
    }
}

fn dark(opaque: bool) -> Theme {
    let accent = teal(400);
    let n = |l: f32| oklch(l, 0.008, 268.0, 1.0);
    Theme {
        mode: Mode::Dark,
        opaque,
        backdrop: hexa(0x0b0c0f, if opaque { 1.0 } else { 0.86 }),
        aurora: with_alpha(accent, 0.16),
        bg: hex(0x0b0c0f),
        bg_raised: hex(0x121318),
        bg_sunken: hex(0x07080a),
        lane: n(0.19),
        lane_alt: n(0.175),
        lane_selected: n(0.215),
        lane_agent: oklch(0.21, 0.02, 185.0, 1.0),
        lane_empty: n(0.165),
        ruler: n(0.2),
        editor: n(0.175),
        display: hex(0x07080a),
        well: n(0.15),
        control: n(0.285),
        control_hover: n(0.315),
        control_pressed: n(0.2),
        control_edge: black(0.5),
        control_highlight: white(0.075),
        thumb: n(0.93),
        knob: n(0.3),
        knob_edge: black(0.55),
        text: hex(0xecedf1),
        text_2: hex(0xa9acb6),
        text_3: hex(0x8a8e99),
        text_on_accent: hex(0x06120f),
        text_display: hex(0xf4f5f8),
        line: white(0.08),
        line_strong: white(0.14),
        hairline: white(0.05),
        bar_line: white(0.07),
        beat_line: white(0.028),
        glass_1: Glass {
            fill: hexa(0x1a1c22, 0.55),
            opaque: hex(0x1a1c22),
        },
        glass_2: Glass {
            fill: hexa(0x1e2027, 0.92),
            opaque: hex(0x1e2027),
        },
        glass_3: Glass {
            fill: hexa(0x22242c, 0.96),
            opaque: hex(0x22242c),
        },
        glass_edge: white(0.09),
        glass_highlight: white(0.07),
        glass_shadow: black(0.45),
        scrim: hexa(0x050608, 0.55),
        accent,
        accent_hover: teal(300),
        accent_fill: accent,
        accent_text: teal(300),
        accent_soft: with_alpha(accent, 0.16),
        accent_ring: with_alpha(accent, 0.55),
        accent_glow: with_alpha(accent, 0.45),
        danger: hex(0xff6b6b),
        warning: hex(0xf2c14e),
        success: hex(0x5fd38a),
        record: oklch(0.68, 0.2, 24.0, 1.0),
        mute: oklch(0.76, 0.1, 245.0, 1.0),
        solo: oklch(0.86, 0.14, 95.0, 1.0),
        meter: oklch(0.84, 0.15, 158.0, 1.0),
        meter_hot: oklch(0.8, 0.15, 62.0, 1.0),
        meter_clip: hex(0xff6b6b),
        meter_off: n(0.235),
        key_white: n(0.92),
        key_black: n(0.2),
        key_label: n(0.45),
        black_key_row: black(0.18),
        note: oklch(0.76, 0.12, 185.0, 1.0),
        note_edge: black(0.3),
        velocity: white(0.24),
        waveform: white(0.82),
        clip_text: white(0.95),
        clip_title: black(0.22),
        cycle: with_alpha(accent, 0.24),
        cycle_lane: with_alpha(accent, 0.045),
        marker: oklch(0.8, 0.1, 222.0, 1.0),
        hover: white(0.055),
        selection_text: with_alpha(accent, 0.32),
    }
}

fn light(opaque: bool) -> Theme {
    let accent = teal(600);
    let n = |l: f32| oklch(l, 0.004, 268.0, 1.0);
    let ink = |a: f32| hexa(0x14161b, a);
    Theme {
        mode: Mode::Light,
        opaque,
        backdrop: hexa(0xeef0f4, if opaque { 1.0 } else { 0.88 }),
        aurora: with_alpha(accent, 0.2),
        bg: hex(0xeef0f4),
        bg_raised: hex(0xf7f8fb),
        bg_sunken: hex(0xe3e6ec),
        lane: hex(0xfbfcfd),
        lane_alt: hex(0xf3f5f8),
        lane_selected: hex(0xe9edf2),
        lane_agent: oklch(0.97, 0.02, 185.0, 1.0),
        lane_empty: hex(0xeef0f4),
        ruler: hex(0xf1f3f7),
        editor: hex(0xfbfcfd),
        display: hex(0xe3e6ec),
        well: hex(0xe8ebf0),
        control: hex(0xffffff),
        control_hover: hex(0xf4f5f8),
        control_pressed: hex(0xe3e6ec),
        control_edge: ink(0.13),
        control_highlight: white(0.9),
        thumb: hex(0xffffff),
        knob: hex(0xffffff),
        knob_edge: ink(0.18),
        text: hex(0x14161b),
        text_2: hex(0x4b505b),
        text_3: hex(0x646a76),
        text_on_accent: hex(0xffffff),
        text_display: hex(0x14161b),
        line: hexa(0x0f141e, 0.10),
        line_strong: hexa(0x0f141e, 0.16),
        hairline: hexa(0x0f141e, 0.06),
        bar_line: ink(0.095),
        beat_line: ink(0.038),
        glass_1: Glass {
            fill: white(0.58),
            opaque: hex(0xf4f5f8),
        },
        glass_2: Glass {
            fill: white(0.94),
            opaque: hex(0xf4f5f8),
        },
        glass_3: Glass {
            fill: white(0.97),
            opaque: hex(0xf4f5f8),
        },
        glass_edge: white(0.75),
        glass_highlight: white(0.9),
        glass_shadow: hexa(0x141e32, 0.14),
        scrim: hexa(0xf0f2f6, 0.6),
        accent,
        accent_hover: teal(700),
        accent_fill: teal(700),
        accent_text: teal(700),
        accent_soft: with_alpha(accent, 0.12),
        accent_ring: with_alpha(accent, 0.55),
        accent_glow: with_alpha(accent, 0.25),
        danger: hex(0xd43d3d),
        warning: hex(0xa86b00),
        success: hex(0x1e8a4c),
        record: oklch(0.57, 0.2, 26.0, 1.0),
        mute: oklch(0.6, 0.12, 248.0, 1.0),
        solo: oklch(0.8, 0.15, 92.0, 1.0),
        meter: oklch(0.66, 0.15, 156.0, 1.0),
        meter_hot: oklch(0.7, 0.16, 58.0, 1.0),
        meter_clip: hex(0xd43d3d),
        meter_off: n(0.87),
        key_white: hex(0xffffff),
        key_black: n(0.24),
        key_label: n(0.55),
        black_key_row: ink(0.035),
        note: oklch(0.6, 0.13, 185.0, 1.0),
        note_edge: ink(0.18),
        velocity: ink(0.3),
        waveform: ink(0.7),
        clip_text: ink(0.9),
        clip_title: white(0.38),
        cycle: with_alpha(accent, 0.2),
        cycle_lane: with_alpha(accent, 0.05),
        marker: oklch(0.55, 0.13, 222.0, 1.0),
        hover: ink(0.042),
        selection_text: with_alpha(accent, 0.26),
    }
}

/// Type sizes (lsuite: 11 · 12 · 13 controls · 15 body · 17 · 22 · 28 · 40).
pub mod size {
    pub const XS: f32 = 11.0;
    pub const SM: f32 = 12.0;
    pub const BASE: f32 = 13.0;
    pub const MD: f32 = 15.0;
    pub const LG: f32 = 17.0;
    pub const XL: f32 = 22.0;
    pub const XXL: f32 = 28.0;
}
/// Radii (lsuite: 4 · 6 controls · 10 popovers · 14 panels and windows · 20 cards).
pub mod radius {
    pub const XS: f32 = 4.0;
    pub const SM: f32 = 6.0;
    pub const MD: f32 = 10.0;
    pub const LG: f32 = 14.0;
    pub const XL: f32 = 20.0;
}
/// Fixed layout: the panel widths of the design frame.
pub mod layout {
    pub const TITLE_BAR: f32 = 36.0;
    pub const TRANSPORT: f32 = 64.0;
    pub const BROWSER: f32 = 260.0;
    pub const BROWSER_MIN: f32 = 220.0;
    pub const INSPECTOR: f32 = 300.0;
    pub const INSPECTOR_MIN: f32 = 260.0;
    pub const AGENT: f32 = 380.0;
    pub const AGENT_MIN: f32 = 320.0;
    pub const AGENT_RAIL: f32 = 32.0;
    pub const TOOLBAR: f32 = 40.0;
    pub const RULER: f32 = 34.0;
    pub const TRACK_HEADER: f32 = 228.0;
    pub const TRACK_HEIGHT: f32 = 88.0;
    pub const EDITOR: f32 = 440.0;
    pub const ARRANGEMENT_MIN: f32 = 420.0;
    pub const WINDOW_MIN_W: f32 = 1120.0;
    pub const WINDOW_MIN_H: f32 = 760.0;
}

pub const FONT_UI: &str = "Manrope";
pub const FONT_MONO: &str = "IBM Plex Mono";

pub fn font_ui() -> SharedString {
    FONT_UI.into()
}
pub fn font_mono() -> SharedString {
    FONT_MONO.into()
}
pub fn px_(v: f32) -> Pixels {
    px(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn linear(c: f32) -> f32 {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }
    fn luminance(c: Rgba) -> f32 {
        0.2126 * linear(c.r) + 0.7152 * linear(c.g) + 0.0722 * linear(c.b)
    }
    fn over(top: Hsla, bottom: Rgba) -> Rgba {
        let t: Rgba = top.into();
        let a = t.a;
        Rgba {
            r: t.r * a + bottom.r * (1.0 - a),
            g: t.g * a + bottom.g * (1.0 - a),
            b: t.b * a + bottom.b * (1.0 - a),
            a: 1.0,
        }
    }
    fn contrast(a: Rgba, b: Rgba) -> f32 {
        let (x, y) = (luminance(a), luminance(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }
    /// What a surface looks like over the brightest and the darkest desktop.
    fn composites(theme: &Theme, surface: Hsla) -> [Rgba; 2] {
        let desktops = [
            Rgba {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
        ];
        desktops.map(|desk| over(surface, over(theme.backdrop, desk)))
    }

    #[test]
    fn text_is_readable_on_every_surface_and_glass_tier_in_both_modes() {
        for mode in [Mode::Dark, Mode::Light] {
            for opaque in [false, true] {
                let t = Theme::new(mode, opaque);
                let surfaces = [
                    ("glass 1", t.glass(1)),
                    ("glass 2", t.glass(2)),
                    ("glass 3", t.glass(3)),
                    ("lane", t.lane),
                    ("editor", t.editor),
                    ("control", t.control),
                    ("raised", t.bg_raised),
                ];
                for (name, surface) in surfaces {
                    for base in composites(&t, surface) {
                        for (ink_name, ink, min) in [
                            ("text", t.text, 4.5),
                            ("text 2", t.text_2, 4.5),
                            ("text 3", t.text_3, 3.0),
                        ] {
                            let ratio = contrast(over(ink, base), base);
                            assert!(
                                ratio >= min,
                                "{mode:?} {ink_name} on {name}: {ratio:.2} < {min}"
                            );
                        }
                    }
                }
                // Text on the accent fill and the accent as text on the window.
                let fill: Rgba = t.accent_fill.into();
                assert!(contrast(t.text_on_accent.into(), fill) >= 4.5, "{mode:?}");
                for base in composites(&t, t.glass(1)) {
                    assert!(contrast(over(t.accent_text, base), base) >= 3.0, "{mode:?}");
                }
            }
        }
    }

    #[test]
    fn css_colors_from_documents_parse() {
        assert!(css_color("#ff0000").is_some());
        assert!(css_color("#f00").is_some());
        assert!(css_color("oklch(0.72 0.14 40)").is_some());
        assert!(css_color("rgba(20, 22, 32, .5)").is_some());
        assert!(css_color("nonsense").is_none());
        let red: Rgba = css_color("#ff0000").unwrap().into();
        assert!((red.r - 1.0).abs() < 1e-3 && red.g < 1e-3);
        // OKLCH white is white.
        let w: Rgba = oklch(1.0, 0.0, 0.0, 1.0).into();
        assert!(w.r > 0.99 && w.g > 0.99 && w.b > 0.99);
    }

    #[test]
    fn the_accent_is_ryolune_teal() {
        let dark: Rgba = Theme::new(Mode::Dark, false).accent.into();
        let light: Rgba = Theme::new(Mode::Light, false).accent.into();
        let to_hex = |c: Rgba| {
            ((c.r * 255.0).round() as u32) << 16
                | ((c.g * 255.0).round() as u32) << 8
                | (c.b * 255.0).round() as u32
        };
        assert_eq!(to_hex(dark), 0x00c5b4);
        assert_eq!(to_hex(light), 0x009586);
    }
}

/// Arrangement sizes: clips, fades, the ruler's flags and the tempo track, on the same scale
/// as [`layout`].
pub mod arrange {
    /// Space between a lane's edge and the clips on it.
    pub const CLIP_INSET: f32 = 6.0;
    /// The title strip across the top of a clip.
    pub const CLIP_TITLE: f32 = 17.0;
    pub const CLIP_RADIUS: f32 = 5.0;
    /// Height of a note in a clip's MIDI preview.
    pub const CLIP_NOTE_H: f32 = 3.0;
    /// Grab zone at each clip edge for trimming.
    pub const CLIP_EDGE_GRIP: f32 = 8.0;
    /// Fade handle in an audio clip's title strip, and its grab zone.
    pub const FADE_HANDLE: f32 = 8.0;
    pub const FADE_GRIP: f32 = 7.0;
    /// Grab zone at each cycle-range edge in the ruler.
    pub const CYCLE_GRIP: f32 = 7.0;
    /// Marker flags sit in the ruler's lower half: their top and height.
    pub const MARKER_TOP: f32 = 18.0;
    pub const MARKER_H: f32 = 14.0;
    /// Beat ticks at the ruler's foot.
    pub const RULER_TICK: f32 = 7.0;
    /// The playhead's triangle in the ruler.
    pub const PLAYHEAD_FLAG_W: f32 = 14.0;
    pub const PLAYHEAD_FLAG_H: f32 = 9.0;
    /// The tempo track: its height, a point's radius and its grab zone.
    pub const TEMPO_LANE: f32 = 64.0;
    pub const TEMPO_POINT: f32 = 4.0;
    pub const TEMPO_GRIP: f32 = 8.0;
    /// The track colour down a header's left edge.
    pub const COLOR_STRIP: f32 = 5.0;
    pub const ZOOM_RAIL: f32 = 110.0;
    /// A name or tempo typed in place.
    pub const INLINE_INPUT_H: f32 = 24.0;
}

/// The arrangement's own inks: lanes, clip faces, fades, the ruler and its flags. Derived
/// from the theme so they follow the mode.
#[derive(Clone, Debug)]
pub struct Timeline {
    pub lane_top: Hsla,
    pub lane_bottom: Hsla,
    pub ruler_bar: Hsla,
    pub ruler_tick: Hsla,
    pub ruler_bottom: Hsla,
    pub cycle_edge: Hsla,
    pub cycle_handle: Hsla,
    pub drag_ghost: Hsla,
    pub drag_ghost_edge: Hsla,
    pub pencil_preview: Hsla,
    pub drop_target: Hsla,
    pub split_guide: Hsla,
    /// What a clip face mixes the track colour with, and how much of the colour it keeps at
    /// the top and the bottom.
    pub face_base: Hsla,
    pub face_top: f32,
    pub face_bottom: f32,
    pub clip_shadow: Hsla,
    pub clip_title_bottom: Hsla,
    pub clip_highlight: Hsla,
    pub clip_contact: Hsla,
    pub clip_selected: Hsla,
    pub midi_note: Hsla,
    pub waveform_mid: Hsla,
    pub fade_shade: Hsla,
    pub fade_curve: Hsla,
    pub fade_handle: Hsla,
    pub marker_flag: Hsla,
    pub marker_lane: Hsla,
    pub tempo_fill: Hsla,
    pub header: Hsla,
}

impl Theme {
    pub fn timeline(&self) -> Timeline {
        let dark = self.mode == Mode::Dark;
        let ink = |a: f32| if dark { white(a) } else { hexa(0x14161b, a) };
        Timeline {
            lane_top: if dark { white(0.018) } else { white(0.6) },
            lane_bottom: if dark {
                black(0.32)
            } else {
                hexa(0x141620, 0.07)
            },
            ruler_bar: ink(if dark { 0.24 } else { 0.3 }),
            ruler_tick: ink(if dark { 0.1 } else { 0.13 }),
            ruler_bottom: if dark {
                black(0.55)
            } else {
                hexa(0x141628, 0.1)
            },
            cycle_edge: with_alpha(self.accent, 0.85),
            cycle_handle: with_alpha(self.accent, 0.95),
            drag_ghost: ink(if dark { 0.09 } else { 0.08 }),
            drag_ghost_edge: ink(0.42),
            pencil_preview: with_alpha(self.accent, 0.24),
            drop_target: with_alpha(self.accent, if dark { 0.09 } else { 0.1 }),
            split_guide: with_alpha(self.accent, 0.95),
            face_base: self.bg_raised,
            face_top: if dark { 0.64 } else { 0.6 },
            face_bottom: if dark { 0.52 } else { 0.7 },
            clip_shadow: if dark {
                black(0.45)
            } else {
                hexa(0x141620, 0.18)
            },
            clip_title_bottom: if dark {
                black(0.12)
            } else {
                hexa(0x141620, 0.05)
            },
            clip_highlight: if dark { white(0.16) } else { white(0.75) },
            clip_contact: if dark {
                black(0.32)
            } else {
                hexa(0x141620, 0.1)
            },
            clip_selected: self.text_display,
            midi_note: if dark {
                white(0.84)
            } else {
                hexa(0x141620, 0.72)
            },
            waveform_mid: if dark {
                white(0.28)
            } else {
                hexa(0x141620, 0.24)
            },
            fade_shade: if dark {
                black(0.38)
            } else {
                hexa(0x141620, 0.14)
            },
            fade_curve: if dark {
                white(0.86)
            } else {
                hexa(0x141620, 0.72)
            },
            fade_handle: if dark {
                white(0.94)
            } else {
                hexa(0x141620, 0.82)
            },
            marker_flag: if dark {
                hexa(0x16171c, 0.9)
            } else {
                white(0.94)
            },
            marker_lane: with_alpha(self.marker, if dark { 0.28 } else { 0.3 }),
            tempo_fill: with_alpha(self.accent, 0.1),
            header: if dark {
                oklch(0.205, 0.008, 268.0, 1.0)
            } else {
                hex(0xf7f8fb)
            },
        }
    }
}
