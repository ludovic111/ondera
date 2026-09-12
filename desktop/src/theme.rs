//! Visual tokens and the skeuomorphic material system.
//!
//! Source: `design/Ondera Arrangement.dc.html`, spec sheet 02 (light from 90°
//! above). Every colour, size, gradient and shadow used by the native interface
//! lives here, as it did in the former `tokens.ts`. Nothing else in `desktop/`
//! may introduce a visual constant.
//!
//! Materials follow one recipe: a 1 px white specular on the top edge, a face
//! gradient darkening downward, a 1 px dark contact line on the bottom edge and
//! a soft drop shadow at two to three times the contact distance. Pressed
//! states swap the drop shadow for an inner shadow and darken the face.
//!
//! egui has no CSS. Gradients are painted by tessellating a rounded rectangle
//! or a disc and recolouring its vertices; drop shadows use `epaint::Shadow`;
//! inner shadows are gradients that fade to transparent.

use eframe::egui::{
    self, epaint, pos2, vec2, Align2, Color32, CornerRadius, FontFamily, FontId, Painter, Pos2,
    Rect, Response, RichText, Sense, Stroke, StrokeKind, Ui, Vec2,
};
use epaint::{CircleShape, Mesh, RectShape, Shadow, Shape, TessellationOptions, Tessellator};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Colour. Graphite is oklch hue 90 at chroma 0.003: warm-neutral, never blue.
// ---------------------------------------------------------------------------

pub const WELL_DEEP: Color32 = Color32::from_rgb(0x16, 0x16, 0x15);
pub const GROOVE: Color32 = Color32::from_rgb(0x1a, 0x1a, 0x19);
pub const GROOVE_ALT: Color32 = Color32::from_rgb(0x1c, 0x1c, 0x1b);
pub const TIMELINE_EMPTY: Color32 = Color32::from_rgb(0x1f, 0x1f, 0x1e);
pub const TIMELINE: Color32 = Color32::from_rgb(0x22, 0x22, 0x21);
pub const TIMELINE_SELECTED: Color32 = Color32::from_rgb(0x26, 0x26, 0x26);
pub const EDITOR: Color32 = Color32::from_rgb(0x26, 0x26, 0x25);
pub const RULER_BG: Color32 = Color32::from_rgb(0x27, 0x27, 0x26);
pub const PANEL: Color32 = Color32::from_rgb(0x2c, 0x2c, 0x2b);
pub const MENU: Color32 = Color32::from_rgb(0x33, 0x33, 0x32);
pub const MENU_HOVER: Color32 = Color32::from_rgb(0x3f, 0x3f, 0x3d);

pub const CONTROL_TOP: Color32 = Color32::from_rgb(0x42, 0x42, 0x3f);
pub const CONTROL_BOTTOM: Color32 = Color32::from_rgb(0x31, 0x31, 0x30);
pub const PRESSED_TOP: Color32 = Color32::from_rgb(0x26, 0x26, 0x25);
pub const PRESSED_BOTTOM: Color32 = Color32::from_rgb(0x2c, 0x2c, 0x2b);
pub const TRANSPORT_TOP: Color32 = Color32::from_rgb(0x33, 0x33, 0x32);
pub const TRANSPORT_BOTTOM: Color32 = Color32::from_rgb(0x2c, 0x2c, 0x2b);
pub const SEGMENT_TOP: Color32 = Color32::from_rgb(0x45, 0x45, 0x3f);
pub const SEGMENT_BOTTOM: Color32 = Color32::from_rgb(0x36, 0x36, 0x34);
pub const THUMB_TOP: Color32 = Color32::from_rgb(0x5a, 0x5a, 0x57);
pub const THUMB_BOTTOM: Color32 = Color32::from_rgb(0x3a, 0x3a, 0x38);
pub const KNOB_HI: Color32 = Color32::from_rgb(0x4c, 0x4c, 0x49);
pub const KNOB_LO: Color32 = Color32::from_rgb(0x2a, 0x2a, 0x29);
pub const KNOB_BIG_HI: Color32 = Color32::from_rgb(0x55, 0x55, 0x52);
pub const KNOB_BIG_LO: Color32 = Color32::from_rgb(0x2c, 0x2c, 0x2b);
pub const KNOB_INNER_HI: Color32 = Color32::from_rgb(0x3a, 0x3a, 0x38);
pub const KNOB_INNER_LO: Color32 = Color32::from_rgb(0x25, 0x25, 0x24);
pub const CAP_TOP: Color32 = Color32::from_rgb(0x60, 0x5f, 0x5c);
pub const CAP_MID: Color32 = Color32::from_rgb(0x3c, 0x3c, 0x3a);
pub const CAP_BOTTOM: Color32 = Color32::from_rgb(0x33, 0x33, 0x2f);
pub const HEADER_TOP: Color32 = Color32::from_rgb(0x2e, 0x2e, 0x2d);
pub const HEADER_BOTTOM: Color32 = Color32::from_rgb(0x2a, 0x2a, 0x29);
pub const HEADER_SELECTED_TOP: Color32 = Color32::from_rgb(0x3a, 0x3a, 0x39);
pub const HEADER_SELECTED_BOTTOM: Color32 = Color32::from_rgb(0x33, 0x33, 0x32);
pub const INSERT_TOP: Color32 = Color32::from_rgb(0x3a, 0x3a, 0x39);
pub const INSERT_BOTTOM: Color32 = Color32::from_rgb(0x31, 0x31, 0x30);

pub const INK_BRIGHT: Color32 = Color32::from_rgb(0xf2, 0xf1, 0xee);
pub const INK: Color32 = Color32::from_rgb(0xe8, 0xe7, 0xe4);
pub const INK_CONTROL: Color32 = Color32::from_rgb(0xd6, 0xd5, 0xd1);
pub const INK_DIM: Color32 = Color32::from_rgb(0xc8, 0xc7, 0xc3);
pub const DIM: Color32 = Color32::from_rgb(0xa9, 0xa8, 0xa4);
pub const FAINT: Color32 = Color32::from_rgb(0x7f, 0x7e, 0x7a);
pub const SEPARATOR: Color32 = Color32::from_rgb(0x5a, 0x59, 0x57);
pub const KEY_INK: Color32 = Color32::from_rgb(0x4a, 0x4a, 0x47);
pub const WHITE_KEY: Color32 = Color32::from_rgb(0xd9, 0xd7, 0xd1);
pub const BLACK_KEY: Color32 = Color32::from_rgb(0x23, 0x23, 0x22);

pub const LED: Color32 = Color32::from_rgb(0xf0, 0xe8, 0xd8);
pub const LED_OFF: Color32 = Color32::from_rgb(0x25, 0x25, 0x23);
pub const LED_GLOW: Color32 = Color32::from_rgb(0xff, 0xec, 0xcd);
/// oklch(0.80 0.12 190): playhead, record state and agent activity only.
pub const ACCENT: Color32 = Color32::from_rgb(71, 214, 207);
pub const ACCENT_HI: Color32 = Color32::from_rgb(118, 239, 231);
pub const ACCENT_LO: Color32 = Color32::from_rgb(0, 169, 162);
pub const ACCENT_INK: Color32 = Color32::from_rgb(0x0f, 0x2a, 0x29);
pub const NEUTRAL_DOT: Color32 = Color32::from_rgb(0x5a, 0x5a, 0x57);

/// Track palette: L 0.72–0.78, C 0.12–0.14 so no track outshouts another.
pub const TRACKS: [Color32; 8] = [
    Color32::from_rgb(237, 131, 94),
    Color32::from_rgb(177, 145, 234),
    Color32::from_rgb(106, 179, 253),
    Color32::from_rgb(217, 145, 210),
    Color32::from_rgb(224, 175, 59),
    Color32::from_rgb(149, 189, 105),
    Color32::from_rgb(235, 129, 130),
    Color32::from_rgb(238, 151, 72),
];

pub fn white(alpha: f32) -> Color32 {
    let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgba_premultiplied(a, a, a, a)
}
pub fn black(alpha: f32) -> Color32 {
    Color32::from_rgba_premultiplied(0, 0, 0, (alpha.clamp(0.0, 1.0) * 255.0).round() as u8)
}
pub fn accent(alpha: f32) -> Color32 {
    ACCENT.gamma_multiply(alpha)
}

// ---------------------------------------------------------------------------
// Grid and dimensions.
// ---------------------------------------------------------------------------

pub const TITLE_BAR: f32 = 28.0;
pub const TRANSPORT: f32 = 52.0;
pub const TOOLBAR: f32 = 32.0;
pub const RULER: f32 = 28.0;
pub const ROW: f32 = 70.0;
pub const HEADER: f32 = 184.0;
pub const EDITOR_HEADER: f32 = 32.0;
pub const EDITOR_RULER: f32 = 18.0;
pub const KEY_WIDTH: f32 = 56.0;
pub const KEY_ROW: f32 = 10.0;
pub const BROWSER: f32 = 220.0;
pub const INSPECTOR: f32 = 240.0;
pub const BUTTON: Vec2 = vec2(34.0, 26.0);
pub const PLAY_BUTTON: Vec2 = vec2(44.0, 26.0);
pub const SMALL_BUTTON: Vec2 = vec2(20.0, 17.0);
pub const TIME_DISPLAY: f32 = 34.0;
pub const KNOB_SM: f32 = 20.0;
pub const KNOB_MD: f32 = 26.0;
pub const KNOB_LG: f32 = 36.0;
pub const SLIDER_THUMB: f32 = 13.0;
pub const FADER_H: f32 = 150.0;
pub const FADER_RAIL_W: f32 = 8.0;
pub const FADER_CAP: Vec2 = vec2(28.0, 18.0);
pub const LED_SEG: Vec2 = vec2(5.0, 6.0);
pub const LED_CPU_H: f32 = 14.0;
pub const ZOOM_RAIL_W: f32 = 90.0;
pub const COLOR_STRIP: f32 = 5.0;
pub const CLIP_INSET: f32 = 5.0;
pub const CLIP_TITLE: f32 = 14.0;
pub const CLIP_EDGE_GRIP: f32 = 7.0;
pub const NOTE_EDGE_GRIP: f32 = 5.0;
pub const GAP: f32 = 8.0;

pub const R_CLIP: f32 = 4.0;
pub const R_BUTTON: f32 = 4.0;
pub const R_MD: f32 = 5.0;
pub const R_CONTROL: f32 = 6.0;
pub const R_LG: f32 = 7.0;

// ---------------------------------------------------------------------------
// Type. Manrope for everything a human reads, IBM Plex Mono for anything that
// counts. Type sits flat on the material: no engraving anywhere.
// ---------------------------------------------------------------------------

pub const FS_TRANSPORT: f32 = 19.0;
pub const FS_PANEL_TITLE: f32 = 14.0;
pub const FS_BODY: f32 = 12.0;
pub const FS_LIST: f32 = 11.5;
pub const FS_SECONDARY: f32 = 11.0;
pub const FS_VALUE: f32 = 10.5;
pub const FS_SMALL: f32 = 10.0;
pub const FS_CAPS: f32 = 9.5;
pub const FS_KIND: f32 = 9.0;
pub const FS_MICRO: f32 = 8.5;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Weight {
    Medium,
    SemiBold,
    Bold,
}
impl Weight {
    fn family(self) -> &'static str {
        match self {
            Weight::Medium => "manrope-medium",
            Weight::SemiBold => "manrope-semibold",
            Weight::Bold => "manrope-bold",
        }
    }
}
pub fn font(size: f32, weight: Weight) -> FontId {
    FontId::new(size, FontFamily::Name(weight.family().into()))
}
pub fn mono_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}
pub fn text(s: impl Into<String>, size: f32, weight: Weight, color: Color32) -> RichText {
    RichText::new(s).font(font(size, weight)).color(color)
}
pub fn mono(s: impl Into<String>, size: f32, color: Color32) -> RichText {
    RichText::new(s).font(mono_font(size)).color(color)
}
/// Caps label: Manrope 9.5/700, +9% tracking, uppercase, ink-500.
pub fn caps(s: &str) -> RichText {
    RichText::new(s.to_uppercase())
        .font(font(FS_CAPS, Weight::Bold))
        .color(FAINT)
        .extra_letter_spacing(FS_CAPS * 0.09)
}
pub fn caps_at(painter: &Painter, pos: Pos2, anchor: Align2, s: &str, color: Color32) -> Rect {
    // The painter has no letter spacing; lay glyphs out one by one.
    let galley_width: f32 = s
        .to_uppercase()
        .chars()
        .map(|c| {
            painter
                .layout_no_wrap(c.to_string(), font(FS_CAPS, Weight::Bold), color)
                .size()
                .x
                + FS_CAPS * 0.09
        })
        .sum();
    let height = FS_CAPS * 1.3;
    let rect = anchor.anchor_size(pos, vec2(galley_width, height));
    let mut x = rect.left();
    for c in s.to_uppercase().chars() {
        let galley = painter.layout_no_wrap(c.to_string(), font(FS_CAPS, Weight::Bold), color);
        painter.galley(
            pos2(x, rect.center().y - galley.size().y / 2.0),
            galley.clone(),
            color,
        );
        x += galley.size().x + FS_CAPS * 0.09;
    }
    rect
}

pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let faces: [(&str, &'static [u8]); 6] = [
        (
            "manrope-regular",
            include_bytes!("../assets/fonts/Manrope-Regular.ttf"),
        ),
        (
            "manrope-medium",
            include_bytes!("../assets/fonts/Manrope-Medium.ttf"),
        ),
        (
            "manrope-semibold",
            include_bytes!("../assets/fonts/Manrope-SemiBold.ttf"),
        ),
        (
            "manrope-bold",
            include_bytes!("../assets/fonts/Manrope-Bold.ttf"),
        ),
        (
            "plex-mono",
            include_bytes!("../assets/fonts/IBMPlexMono-Regular.ttf"),
        ),
        (
            "plex-mono-medium",
            include_bytes!("../assets/fonts/IBMPlexMono-Medium.ttf"),
        ),
    ];
    for (name, bytes) in faces {
        fonts
            .font_data
            .insert(name.into(), Arc::new(egui::FontData::from_static(bytes)));
    }
    let fallback = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let mono_fallback = fonts
        .families
        .get(&FontFamily::Monospace)
        .cloned()
        .unwrap_or_default();
    let with = |primary: &str, rest: &[String]| {
        let mut list = vec![primary.to_string()];
        list.extend(rest.iter().cloned());
        list
    };
    fonts
        .families
        .insert(FontFamily::Proportional, with("manrope-medium", &fallback));
    fonts
        .families
        .insert(FontFamily::Monospace, with("plex-mono", &mono_fallback));
    for name in [
        "manrope-regular",
        "manrope-medium",
        "manrope-semibold",
        "manrope-bold",
    ] {
        fonts
            .families
            .insert(FontFamily::Name(name.into()), with(name, &fallback));
    }
    fonts.families.insert(
        FontFamily::Name("plex-mono-medium".into()),
        with("plex-mono-medium", &mono_fallback),
    );
    ctx.set_fonts(fonts);

    let mut style = (*ctx.style()).clone();
    let mut v = egui::Visuals::dark();
    v.panel_fill = PANEL;
    v.window_fill = MENU;
    v.window_stroke = Stroke::new(1.0, black(0.6));
    v.window_corner_radius = CornerRadius::same(R_LG as u8);
    v.window_shadow = Shadow {
        offset: [0, 8],
        blur: 24,
        spread: 0,
        color: black(0.55),
    };
    v.popup_shadow = v.window_shadow;
    v.menu_corner_radius = CornerRadius::same(R_CONTROL as u8);
    v.extreme_bg_color = WELL_DEEP;
    v.faint_bg_color = TIMELINE;
    v.override_text_color = Some(INK);
    v.selection.bg_fill = accent(0.3);
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.text_cursor.stroke = Stroke::new(1.0, ACCENT);
    v.widgets.noninteractive.bg_fill = PANEL;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, black(0.5));
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, INK);
    v.widgets.inactive.bg_fill = Color32::TRANSPARENT;
    v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    v.widgets.inactive.bg_stroke = Stroke::NONE;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, INK_CONTROL);
    v.widgets.inactive.corner_radius = CornerRadius::same(R_MD as u8);
    v.widgets.hovered.bg_fill = MENU_HOVER;
    v.widgets.hovered.weak_bg_fill = white(0.06);
    v.widgets.hovered.bg_stroke = Stroke::NONE;
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, INK);
    v.widgets.hovered.corner_radius = CornerRadius::same(R_MD as u8);
    v.widgets.active.bg_fill = MENU_HOVER;
    v.widgets.active.weak_bg_fill = white(0.09);
    v.widgets.active.bg_stroke = Stroke::NONE;
    v.widgets.active.fg_stroke = Stroke::new(1.0, INK);
    v.widgets.active.corner_radius = CornerRadius::same(R_MD as u8);
    v.widgets.open = v.widgets.hovered;
    style.visuals = v;
    style.spacing.item_spacing = vec2(GAP, 6.0);
    style.spacing.button_padding = vec2(10.0, 4.0);
    style.spacing.interact_size = vec2(28.0, 24.0);
    style.spacing.menu_margin = egui::Margin::same(4);
    style.spacing.menu_width = 200.0;
    style.spacing.window_margin = egui::Margin::same(14);
    style
        .text_styles
        .insert(egui::TextStyle::Body, font(FS_BODY, Weight::Medium));
    style
        .text_styles
        .insert(egui::TextStyle::Button, font(FS_LIST, Weight::Medium));
    style
        .text_styles
        .insert(egui::TextStyle::Small, font(FS_VALUE, Weight::Medium));
    style
        .text_styles
        .insert(egui::TextStyle::Heading, font(FS_PANEL_TITLE, Weight::Bold));
    style
        .text_styles
        .insert(egui::TextStyle::Monospace, mono_font(FS_VALUE));
    ctx.set_style(style);
}

pub fn track_color(css: &str, index: usize) -> Color32 {
    if let Some(hex) = css.strip_prefix('#') {
        if hex.len() == 6 {
            if let Ok(n) = u32::from_str_radix(hex, 16) {
                return Color32::from_rgb((n >> 16) as u8, (n >> 8) as u8, n as u8);
            }
        }
    }
    // The old palette uses oklch. Convert it rather than depending on a browser.
    if let Some(raw) = css.strip_prefix("oklch(").and_then(|s| s.strip_suffix(')')) {
        let values: Vec<f64> = raw
            .split_whitespace()
            .filter_map(|s| s.parse().ok())
            .collect();
        if values.len() == 3 {
            let [l, c, h] = [values[0], values[1], values[2].to_radians()];
            let a = c * h.cos();
            let b = c * h.sin();
            let ll = (l + 0.3963377774 * a + 0.2158037573 * b).powi(3);
            let mm = (l - 0.1055613458 * a - 0.0638541728 * b).powi(3);
            let ss = (l - 0.0894841775 * a - 1.291485548 * b).powi(3);
            let rgb = [
                4.0767416621 * ll - 3.3077115913 * mm + 0.2309699292 * ss,
                -1.2684380046 * ll + 2.6097574011 * mm - 0.3413193965 * ss,
                -0.0041960863 * ll - 0.7034186147 * mm + 1.707614701 * ss,
            ]
            .map(|v| {
                let v = if v <= 0.0031308 {
                    v * 12.92
                } else {
                    1.055 * v.powf(1.0 / 2.4) - 0.055
                };
                (v.clamp(0.0, 1.0) * 255.0).round() as u8
            });
            return Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
        }
    }
    TRACKS[index % TRACKS.len()]
}

// ---------------------------------------------------------------------------
// Painting primitives.
// ---------------------------------------------------------------------------

fn tessellator(painter: &Painter) -> Tessellator {
    let options = TessellationOptions {
        prerasterized_discs: false,
        ..Default::default()
    };
    Tessellator::new(painter.pixels_per_point(), options, [1, 1], Vec::new())
}
fn recolour(mesh: &mut Mesh, color_at: impl Fn(Pos2) -> Color32) {
    for v in &mut mesh.vertices {
        let a = v.color.a();
        v.color = match a {
            255 => color_at(v.pos),
            0 => Color32::TRANSPARENT,
            _ => color_at(v.pos).gamma_multiply(a as f32 / 255.0),
        };
    }
}
/// Fill a rounded rectangle, choosing the colour per vertex.
pub fn shade_rect(
    painter: &Painter,
    rect: Rect,
    radius: impl Into<CornerRadius>,
    color_at: impl Fn(Pos2) -> Color32,
) {
    if !rect.is_positive() {
        return;
    }
    let mut mesh = Mesh::default();
    tessellator(painter)
        .tessellate_rect(&RectShape::filled(rect, radius, Color32::WHITE), &mut mesh);
    recolour(&mut mesh, color_at);
    painter.add(Shape::mesh(mesh));
}
pub fn shade_circle(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    color_at: impl Fn(Pos2) -> Color32,
) {
    if radius <= 0.0 {
        return;
    }
    let mut mesh = Mesh::default();
    tessellator(painter).tessellate_circle(
        CircleShape {
            center,
            radius,
            fill: Color32::WHITE,
            stroke: Stroke::NONE,
        },
        &mut mesh,
    );
    recolour(&mut mesh, color_at);
    painter.add(Shape::mesh(mesh));
}
pub fn vertical(rect: Rect, top: Color32, bottom: Color32) -> impl Fn(Pos2) -> Color32 {
    move |p| {
        let t = ((p.y - rect.top()) / rect.height().max(1.0)).clamp(0.0, 1.0);
        top.lerp_to_gamma(bottom, t)
    }
}
/// CSS `radial-gradient(circle at 50% <focus>, hi, lo <extent>)` on a disc.
pub fn radial(
    center: Pos2,
    radius: f32,
    focus: f32,
    extent: f32,
    hi: Color32,
    lo: Color32,
) -> impl Fn(Pos2) -> Color32 {
    let focus = pos2(center.x, center.y - radius + 2.0 * radius * focus);
    let reach = (radius * 1.75 * extent).max(1.0);
    move |p| hi.lerp_to_gamma(lo, (p.distance(focus) / reach).clamp(0.0, 1.0))
}
pub fn drop_shadow(painter: &Painter, rect: Rect, radius: f32, dy: f32, blur: f32, color: Color32) {
    painter.add(
        Shadow {
            offset: [0, dy.round() as i8],
            blur: blur.round() as u8,
            spread: 0,
            color,
        }
        .as_shape(rect, radius),
    );
}
#[derive(Clone, Copy)]
pub enum Side {
    Top,
    Bottom,
    Left,
    Right,
}
/// Inner shadow: darkens `depth` points from one edge and fades to nothing.
pub fn inset(painter: &Painter, rect: Rect, radius: f32, side: Side, depth: f32, color: Color32) {
    let depth = depth.max(0.5);
    shade_rect(painter, rect, radius, move |p| {
        let d = match side {
            Side::Top => p.y - rect.top(),
            Side::Bottom => rect.bottom() - p.y,
            Side::Left => p.x - rect.left(),
            Side::Right => rect.right() - p.x,
        };
        color.gamma_multiply((1.0 - d / depth).clamp(0.0, 1.0))
    });
}
pub fn hline(painter: &Painter, x0: f32, x1: f32, y: f32, color: Color32) {
    if x1 > x0 {
        painter.line_segment(
            [pos2(x0, y.floor() + 0.5), pos2(x1, y.floor() + 0.5)],
            Stroke::new(1.0, color),
        );
    }
}
pub fn vline(painter: &Painter, x: f32, y0: f32, y1: f32, color: Color32) {
    if y1 > y0 {
        painter.line_segment(
            [pos2(x.floor() + 0.5, y0), pos2(x.floor() + 0.5, y1)],
            Stroke::new(1.0, color),
        );
    }
}
/// The specular: 1 px of light along the top edge, inside the corners.
pub fn edge_top(painter: &Painter, rect: Rect, radius: f32, color: Color32) {
    hline(
        painter,
        rect.left() + radius,
        rect.right() - radius,
        rect.top(),
        color,
    );
}
/// The contact line: 1 px of dark along the bottom edge.
pub fn edge_bottom(painter: &Painter, rect: Rect, radius: f32, color: Color32) {
    hline(
        painter,
        rect.left() + radius,
        rect.right() - radius,
        rect.bottom() - 1.0,
        color,
    );
}
/// The lip below a well that catches the light.
pub fn lip(painter: &Painter, rect: Rect, radius: f32, color: Color32) {
    hline(
        painter,
        rect.left() + radius,
        rect.right() - radius,
        rect.bottom(),
        color,
    );
}
pub fn inner_border(painter: &Painter, rect: Rect, radius: f32, color: Color32) {
    painter.rect_stroke(rect, radius, Stroke::new(1.0, color), StrokeKind::Inside);
}

// ---------------------------------------------------------------------------
// Materials.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Face {
    Raised,
    Pressed,
    Lit,
}
impl Face {
    pub fn ink(self) -> Color32 {
        match self {
            Face::Raised => INK_CONTROL,
            Face::Pressed => INK,
            Face::Lit => ACCENT_INK,
        }
    }
    pub fn from_flag(on: bool) -> Self {
        if on {
            Face::Pressed
        } else {
            Face::Raised
        }
    }
    pub fn lit_flag(on: bool) -> Self {
        if on {
            Face::Lit
        } else {
            Face::Raised
        }
    }
}
pub fn raised(p: &Painter, r: Rect, radius: f32) {
    drop_shadow(p, r, radius, 3.0, 5.0, black(0.25));
    drop_shadow(p, r, radius, 1.0, 2.0, black(0.55));
    shade_rect(p, r, radius, vertical(r, CONTROL_TOP, CONTROL_BOTTOM));
    edge_top(p, r, radius, white(0.09));
    edge_bottom(p, r, radius, black(0.35));
}
pub fn pressed(p: &Painter, r: Rect, radius: f32) {
    lip(p, r, radius, white(0.05));
    shade_rect(p, r, radius, vertical(r, PRESSED_TOP, PRESSED_BOTTOM));
    inset(p, r, radius, Side::Top, 4.0, black(0.65));
    inset(p, r, radius, Side::Top, 1.5, black(0.5));
}
pub fn lit(p: &Painter, r: Rect, radius: f32) {
    drop_shadow(p, r, radius, 0.0, 22.0, accent(0.25));
    drop_shadow(p, r, radius, 0.0, 12.0, accent(0.55));
    drop_shadow(p, r, radius, 1.0, 2.0, black(0.6));
    let reach = r.width().max(r.height());
    shade_rect(
        p,
        r,
        radius,
        radial(r.center(), reach / 2.0, 0.3, 1.0, ACCENT_HI, ACCENT_LO),
    );
    edge_top(p, r, radius, white(0.45));
    edge_bottom(p, r, radius, black(0.3));
}
pub fn face(p: &Painter, r: Rect, radius: f32, face: Face) {
    match face {
        Face::Raised => raised(p, r, radius),
        Face::Pressed => pressed(p, r, radius),
        Face::Lit => lit(p, r, radius),
    }
}
pub fn groove(p: &Painter, r: Rect, radius: f32) {
    lip(p, r, radius, white(0.05));
    p.rect_filled(r, radius, GROOVE);
    inset(p, r, radius, Side::Top, 3.0, black(0.9));
    inner_border(p, r, radius, black(0.5));
}
pub fn groove_shallow(p: &Painter, r: Rect, radius: f32) {
    lip(p, r, radius, white(0.05));
    p.rect_filled(r, radius, GROOVE_ALT);
    inset(p, r, radius, Side::Top, 3.0, black(0.8));
}
pub fn groove_send(p: &Painter, r: Rect, radius: f32) {
    lip(p, r, radius, white(0.04));
    p.rect_filled(r, radius, EDITOR);
    inset(p, r, radius, Side::Top, 2.0, black(0.6));
}
pub fn well_deep(p: &Painter, r: Rect, radius: f32) {
    lip(p, r, radius, white(0.06));
    p.rect_filled(r, radius, WELL_DEEP);
    inset(p, r, radius, Side::Top, 5.0, black(0.85));
    inner_border(p, r, radius, black(0.6));
}
pub fn well_input(p: &Painter, r: Rect, radius: f32) {
    lip(p, r, radius, white(0.05));
    p.rect_filled(r, radius, GROOVE_ALT);
    inset(p, r, radius, Side::Top, 4.0, black(0.8));
    inner_border(p, r, radius, black(0.6));
}
pub fn well_meter(p: &Painter, r: Rect, radius: f32) {
    lip(p, r, radius, white(0.05));
    p.rect_filled(r, radius, WELL_DEEP);
    inset(p, r, radius, Side::Top, 3.0, black(0.9));
    inner_border(p, r, radius, black(0.5));
}
pub fn well_value(p: &Painter, r: Rect, radius: f32) {
    p.rect_filled(r, radius, WELL_DEEP);
    inset(p, r, radius, Side::Top, 3.0, black(0.85));
}
pub fn segment_active(p: &Painter, r: Rect, radius: f32) {
    drop_shadow(p, r, radius, 1.0, 2.0, black(0.5));
    shade_rect(p, r, radius, vertical(r, SEGMENT_TOP, SEGMENT_BOTTOM));
    edge_top(p, r, radius, white(0.1));
}
pub fn insert_face(p: &Painter, r: Rect, radius: f32) {
    drop_shadow(p, r, radius, 1.0, 2.0, black(0.5));
    shade_rect(p, r, radius, vertical(r, INSERT_TOP, INSERT_BOTTOM));
    edge_top(p, r, radius, white(0.08));
}
pub fn glass(p: &Painter, r: Rect, radius: f32) {
    drop_shadow(p, r, radius, 3.0, 8.0, black(0.4));
    p.rect_stroke(r, radius, Stroke::new(1.0, black(0.4)), StrokeKind::Outside);
    p.rect_filled(r, radius, Color32::from_rgba_unmultiplied(60, 60, 58, 140));
    edge_top(p, r, radius, white(0.15));
}
pub fn swatch(p: &Painter, r: Rect, color: Color32) {
    drop_shadow(p, r, 2.0, 1.0, 1.0, black(0.5));
    p.rect_filled(r, 2.0, color);
    edge_top(p, r, 1.0, white(0.25));
}
pub fn led(p: &Painter, r: Rect, on: bool, hot: bool) {
    if on {
        let (color, glow) = if hot {
            (ACCENT, accent(0.4))
        } else {
            (LED, LED_GLOW.gamma_multiply(0.35))
        };
        p.rect_filled(r.expand(2.0), 2.0, glow.gamma_multiply(0.5));
        p.rect_filled(r.expand(1.0), 1.5, glow);
        p.rect_filled(r, 1.0, color);
    } else {
        p.rect_filled(r, 1.0, LED_OFF);
        hline(p, r.left(), r.right(), r.top(), black(0.6));
    }
}
/// Meter level in 0..=1 from a linear peak: 54 dB of range, like the DSP scale.
pub fn level(peak: f32) -> f32 {
    if peak > 0.0 {
        ((20.0 * peak.log10() + 54.0) / 54.0).clamp(0.0, 1.0)
    } else {
        0.0
    }
}
/// A row (or column) of LED segments. `level` is 0..=1; the last tenth is hot.
pub fn led_strip(
    p: &Painter,
    origin: Pos2,
    count: usize,
    seg: Vec2,
    vertical_up: bool,
    level: f32,
) {
    let lit_count = (level * count as f32).round() as usize;
    for i in 0..count {
        let r = if vertical_up {
            Rect::from_min_size(
                pos2(origin.x, origin.y - (i as f32 + 1.0) * (seg.y + 1.0) + 1.0),
                seg,
            )
        } else {
            Rect::from_min_size(pos2(origin.x + i as f32 * (seg.x + 1.0), origin.y), seg)
        };
        let on = i < lit_count;
        led(p, r, on, on && i as f32 >= count as f32 * 0.9);
    }
}
pub fn thumb(p: &Painter, r: Rect) {
    drop_shadow(p, r, 3.0, 3.0, 4.0, black(0.3));
    drop_shadow(p, r, 3.0, 1.0, 2.0, black(0.6));
    shade_rect(p, r, 3.0, vertical(r, THUMB_TOP, THUMB_BOTTOM));
    edge_top(p, r, 3.0, white(0.22));
    edge_bottom(p, r, 3.0, black(0.4));
    // Milled slot.
    let x = r.center().x;
    vline(p, x, r.top() + 2.0, r.bottom() - 2.0, black(0.6));
    vline(p, x + 1.0, r.top() + 2.0, r.bottom() - 2.0, white(0.12));
}
pub fn fader_cap(p: &Painter, r: Rect) {
    drop_shadow(p, r, 4.0, 5.0, 8.0, black(0.35));
    drop_shadow(p, r, 4.0, 2.0, 3.0, black(0.7));
    shade_rect(p, r, 4.0, move |q| {
        let t = ((q.y - r.top()) / r.height().max(1.0)).clamp(0.0, 1.0);
        if t < 0.55 {
            CAP_TOP.lerp_to_gamma(CAP_MID, t / 0.55)
        } else {
            CAP_MID.lerp_to_gamma(CAP_BOTTOM, (t - 0.55) / 0.45)
        }
    });
    edge_top(p, r, 4.0, white(0.25));
    edge_bottom(p, r, 4.0, black(0.5));
    let y = r.top() + 8.0;
    hline(p, r.left() + 3.0, r.right() - 3.0, y, black(0.7));
    hline(p, r.left() + 3.0, r.right() - 3.0, y + 1.0, white(0.14));
}
/// A knob face with its indicator at `angle` degrees clockwise from twelve o'clock.
pub fn knob(p: &Painter, center: Pos2, radius: f32, angle: f32, big: bool) {
    let r = Rect::from_center_size(center, Vec2::splat(radius * 2.0));
    if big {
        drop_shadow(p, r, radius, 8.0, 12.0, black(0.35));
        drop_shadow(p, r, radius, 3.0, 4.0, black(0.65));
        shade_circle(
            p,
            center,
            radius,
            radial(center, radius, 0.25, 0.7, KNOB_BIG_HI, KNOB_BIG_LO),
        );
    } else {
        drop_shadow(p, r, radius, 4.0, 7.0, black(0.3));
        drop_shadow(p, r, radius, 2.0, 3.0, black(0.6));
        shade_circle(
            p,
            center,
            radius,
            radial(center, radius, 0.28, 0.72, KNOB_HI, KNOB_LO),
        );
    }
    let top = Rect::from_min_max(r.min, pos2(r.right(), center.y));
    let bottom = Rect::from_min_max(pos2(r.left(), center.y), r.max);
    p.with_clip_rect(top.intersect(p.clip_rect()))
        .circle_stroke(
            center,
            radius - 0.5,
            Stroke::new(1.0, white(if big { 0.18 } else { 0.16 })),
        );
    p.with_clip_rect(bottom.intersect(p.clip_rect()))
        .circle_stroke(center, radius - 0.75, Stroke::new(1.5, black(0.6)));
    if big {
        let inner = radius - 6.0;
        shade_circle(
            p,
            center,
            inner,
            radial(center, inner, 0.35, 1.0, KNOB_INNER_HI, KNOB_INNER_LO),
        );
        p.with_clip_rect(top.intersect(p.clip_rect()))
            .circle_stroke(center, inner - 0.5, Stroke::new(1.0, black(0.7)));
        p.with_clip_rect(bottom.intersect(p.clip_rect()))
            .circle_stroke(center, inner + 0.5, Stroke::new(1.0, white(0.08)));
    }
    let len = if big {
        9.0
    } else if radius >= 13.0 {
        7.0
    } else {
        6.0
    };
    let a = (angle - 90.0).to_radians();
    let dir = vec2(a.cos(), a.sin());
    let from = center + dir * (radius - 2.0 - len);
    let to = center + dir * (radius - 2.0);
    p.line_segment([from, to], Stroke::new(4.0, black(0.35)));
    p.line_segment([from, to], Stroke::new(2.0, INK));
}
/// The clip slab: a gradient of the track colour mixed into the panel, with a
/// darker title strip. Returns the title strip.
pub fn clip_slab(p: &Painter, r: Rect, color: Color32, selected: bool, agent: bool) -> Rect {
    drop_shadow(p, r, R_CLIP, 5.0, 10.0, black(0.3));
    drop_shadow(p, r, R_CLIP, 2.0, 3.0, black(0.55));
    if agent {
        drop_shadow(p, r, R_CLIP, 0.0, 14.0, accent(0.45));
    }
    let top = PANEL.lerp_to_gamma(color, 0.66);
    let bottom = PANEL.lerp_to_gamma(color, 0.50);
    shade_rect(p, r, R_CLIP, vertical(r, top, bottom));
    let strip = Rect::from_min_size(r.min, vec2(r.width(), CLIP_TITLE.min(r.height())));
    p.rect_filled(
        strip,
        CornerRadius {
            nw: R_CLIP as u8,
            ne: R_CLIP as u8,
            sw: 0,
            se: 0,
        },
        black(0.22),
    );
    if r.height() > CLIP_TITLE + 2.0 {
        hline(p, r.left(), r.right(), strip.bottom() - 1.0, black(0.2));
    }
    edge_top(p, r, R_CLIP, white(0.22));
    edge_bottom(p, r, R_CLIP, black(0.4));
    if agent {
        p.rect_stroke(r, R_CLIP, Stroke::new(1.0, ACCENT), StrokeKind::Outside);
    } else if selected {
        p.rect_stroke(
            r,
            R_CLIP,
            Stroke::new(1.5, white(0.85)),
            StrokeKind::Outside,
        );
    }
    strip
}
/// A piano-roll note: the track colour at two lightnesses with a velocity bar.
pub fn note_slab(p: &Painter, r: Rect, color: Color32, velocity: u8, selected: bool, agent: bool) {
    drop_shadow(p, r, 2.0, 2.0, 4.0, black(0.3));
    drop_shadow(p, r, 2.0, 1.0, 2.0, black(0.6));
    let top = color.lerp_to_gamma(Color32::WHITE, 0.18);
    let bottom = color.lerp_to_gamma(PANEL, 0.3);
    shade_rect(p, r, 2.0, vertical(r, top, bottom));
    let vel = Rect::from_min_size(
        r.min,
        vec2(
            (r.width() * velocity as f32 / 127.0).min(r.width()),
            r.height(),
        ),
    );
    p.rect_filled(vel, 2.0, white(0.18));
    edge_top(p, r, 2.0, white(0.35));
    edge_bottom(p, r, 2.0, black(0.3));
    if agent {
        p.rect_stroke(r, 2.0, Stroke::new(1.0, ACCENT), StrokeKind::Outside);
        drop_shadow(p, r, 2.0, 0.0, 6.0, accent(0.5));
    } else if selected {
        p.rect_stroke(r, 2.0, Stroke::new(1.0, white(0.9)), StrokeKind::Outside);
    }
}
pub fn playhead(p: &Painter, x: f32, y0: f32, y1: f32) {
    let x = x.floor() + 0.5;
    p.line_segment([pos2(x, y0), pos2(x, y1)], Stroke::new(5.0, accent(0.12)));
    p.line_segment([pos2(x, y0), pos2(x, y1)], Stroke::new(3.0, accent(0.25)));
    p.line_segment([pos2(x, y0), pos2(x, y1)], Stroke::new(1.0, ACCENT));
}
pub fn playhead_flag(p: &Painter, x: f32, top: f32) {
    let x = x.floor() + 0.5;
    let pts = vec![pos2(x - 7.0, top), pos2(x + 7.0, top), pos2(x, top + 8.0)];
    p.add(Shape::convex_polygon(
        pts.iter().map(|q| *q + vec2(0.0, 1.0)).collect(),
        accent(0.35),
        Stroke::NONE,
    ));
    p.add(Shape::convex_polygon(pts, ACCENT, Stroke::NONE));
}

// ---------------------------------------------------------------------------
// Icons, drawn with the painter so they scale with the display.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Return,
    Rewind,
    Forward,
    Play,
    Stop,
    Record,
    Cycle,
    Plus,
    Pointer,
    Pencil,
    Scissors,
    Search,
}
fn tri(p: &Painter, a: Pos2, b: Pos2, c: Pos2, color: Color32) {
    p.add(Shape::convex_polygon(vec![a, b, c], color, Stroke::NONE));
}
pub fn icon(p: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let c = rect.center();
    let at = |x: f32, y: f32, w: f32, h: f32| pos2(c.x - w / 2.0 + x, c.y - h / 2.0 + y);
    let stroke = Stroke::new(1.5, color);
    match icon {
        Icon::Return => {
            let (w, h) = (12.0, 10.0);
            p.rect_filled(
                Rect::from_min_size(at(0.0, 0.0, w, h), vec2(2.0, h)),
                0.0,
                color,
            );
            tri(
                p,
                at(11.0, 0.0, w, h),
                at(3.0, 5.0, w, h),
                at(11.0, 10.0, w, h),
                color,
            );
        }
        Icon::Rewind => {
            let (w, h) = (14.0, 10.0);
            tri(
                p,
                at(7.0, 0.0, w, h),
                at(0.0, 5.0, w, h),
                at(7.0, 10.0, w, h),
                color,
            );
            tri(
                p,
                at(14.0, 0.0, w, h),
                at(7.0, 5.0, w, h),
                at(14.0, 10.0, w, h),
                color,
            );
        }
        Icon::Forward => {
            let (w, h) = (14.0, 10.0);
            tri(
                p,
                at(0.0, 0.0, w, h),
                at(7.0, 5.0, w, h),
                at(0.0, 10.0, w, h),
                color,
            );
            tri(
                p,
                at(7.0, 0.0, w, h),
                at(14.0, 5.0, w, h),
                at(7.0, 10.0, w, h),
                color,
            );
        }
        Icon::Play => {
            let (w, h) = (11.0, 12.0);
            tri(
                p,
                at(0.0, 0.0, w, h),
                at(11.0, 6.0, w, h),
                at(0.0, 12.0, w, h),
                color,
            );
        }
        Icon::Stop => {
            p.rect_filled(Rect::from_center_size(c, Vec2::splat(10.0)), 1.0, color);
        }
        Icon::Record => {
            p.circle_filled(c, 5.0, color);
        }
        Icon::Cycle => {
            let r = 5.0;
            let arc = |from: f32, to: f32| {
                let n = 14;
                (0..=n)
                    .map(|i| {
                        let a = (from + (to - from) * i as f32 / n as f32).to_radians();
                        pos2(c.x + r * a.cos(), c.y + r * a.sin())
                    })
                    .collect::<Vec<_>>()
            };
            p.add(Shape::line(arc(200.0, 340.0), Stroke::new(1.6, color)));
            p.add(Shape::line(arc(20.0, 160.0), Stroke::new(1.6, color)));
            tri(
                p,
                pos2(c.x + r + 2.0, c.y - 4.5),
                pos2(c.x + r + 2.0, c.y + 0.5),
                pos2(c.x + r - 2.5, c.y - 1.5),
                color,
            );
            tri(
                p,
                pos2(c.x - r - 2.0, c.y + 4.5),
                pos2(c.x - r - 2.0, c.y - 0.5),
                pos2(c.x - r + 2.5, c.y + 1.5),
                color,
            );
        }
        Icon::Plus => {
            p.line_segment([pos2(c.x - 4.0, c.y), pos2(c.x + 4.0, c.y)], stroke);
            p.line_segment([pos2(c.x, c.y - 4.0), pos2(c.x, c.y + 4.0)], stroke);
        }
        Icon::Pointer => {
            let (w, h) = (9.0, 12.0);
            tri(
                p,
                at(0.0, 0.0, w, h),
                at(9.0, 8.0, w, h),
                at(0.0, 11.0, w, h),
                color,
            );
            p.add(Shape::convex_polygon(
                vec![
                    at(3.0, 8.5, w, h),
                    at(5.0, 8.0, w, h),
                    at(7.0, 12.0, w, h),
                    at(5.0, 12.0, w, h),
                ],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Pencil => {
            let (w, h) = (11.0, 11.0);
            p.add(Shape::closed_line(
                vec![
                    at(1.0, 10.0, w, h),
                    at(2.0, 7.0, w, h),
                    at(8.0, 1.0, w, h),
                    at(10.0, 3.0, w, h),
                    at(4.0, 9.0, w, h),
                ],
                stroke,
            ));
        }
        Icon::Scissors => {
            let (w, h) = (11.0, 11.0);
            p.circle_stroke(at(3.0, 8.0, w, h), 2.0, stroke);
            p.circle_stroke(at(8.0, 8.0, w, h), 2.0, stroke);
            p.line_segment([at(4.5, 6.5, w, h), at(9.0, 0.0, w, h)], stroke);
            p.line_segment([at(6.5, 6.5, w, h), at(2.0, 0.0, w, h)], stroke);
        }
        Icon::Search => {
            let (w, h) = (11.0, 11.0);
            p.circle_stroke(at(4.5, 4.5, w, h), 3.5, stroke);
            p.line_segment([at(7.0, 7.0, w, h), at(10.0, 10.0, w, h)], stroke);
        }
    }
}

// ---------------------------------------------------------------------------
// Widgets. Each one allocates space, paints its material and returns the
// egui response so callers keep egui's interaction model.
// ---------------------------------------------------------------------------

pub fn button(
    ui: &mut Ui,
    size: Vec2,
    face_kind: Face,
    radius: f32,
    draw: impl FnOnce(&Painter, Rect, Color32),
) -> Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let shown = if response.is_pointer_button_down_on() && face_kind == Face::Raised {
            Face::Pressed
        } else {
            face_kind
        };
        let p = ui.painter();
        face(p, rect, radius, shown);
        let ink = if shown == Face::Raised && response.hovered() {
            INK
        } else {
            shown.ink()
        };
        draw(p, rect, ink);
    }
    response
}
pub fn icon_button(ui: &mut Ui, size: Vec2, face_kind: Face, icon_kind: Icon) -> Response {
    button(ui, size, face_kind, R_CONTROL, |p, r, ink| {
        icon(p, r, icon_kind, ink)
    })
}
pub fn text_button(ui: &mut Ui, label: &str, face_kind: Face) -> Response {
    let galley =
        ui.painter()
            .layout_no_wrap(label.into(), font(FS_SECONDARY, Weight::SemiBold), INK);
    let size = vec2(galley.size().x + 20.0, 26.0);
    button(ui, size, face_kind, R_CONTROL, |p, r, ink| {
        p.galley(r.center() - galley.size() / 2.0, galley.clone(), ink);
    })
}
/// M / S / R: 20 × 17, bold 9.5.
pub fn toggle_small(ui: &mut Ui, label: &str, on: bool, lit_when_on: bool) -> Response {
    let face_kind = if lit_when_on {
        Face::lit_flag(on)
    } else {
        Face::from_flag(on)
    };
    let galley = ui
        .painter()
        .layout_no_wrap(label.into(), font(FS_CAPS, Weight::Bold), INK);
    button(ui, SMALL_BUTTON, face_kind, R_BUTTON, |p, r, ink| {
        if label == "R" {
            p.circle_filled(r.center(), 3.5, ink);
        } else {
            p.galley(r.center() - galley.size() / 2.0, galley.clone(), ink);
        }
    })
}
/// A segmented control in a shallow groove. Returns the newly chosen index.
pub fn segmented(ui: &mut Ui, labels: &[&str], selected: usize, min_seg: f32) -> Option<usize> {
    let widths: Vec<f32> = labels
        .iter()
        .map(|l| {
            ui.painter()
                .layout_no_wrap(l.to_string(), font(FS_VALUE, Weight::SemiBold), INK)
                .size()
                .x
                + 20.0
        })
        .map(|w| w.max(min_seg))
        .collect();
    let total = widths.iter().sum::<f32>() + 4.0;
    let (rect, _) = ui.allocate_exact_size(vec2(total, 24.0), Sense::hover());
    let p = ui.painter();
    groove_shallow(p, rect, R_CONTROL);
    let mut x = rect.left() + 2.0;
    let mut chosen = None;
    for (i, (label, w)) in labels.iter().zip(widths).enumerate() {
        let seg = Rect::from_min_size(pos2(x, rect.top() + 2.0), vec2(w, 20.0));
        let response = ui.interact(seg, ui.id().with(("segment", i, label)), Sense::click());
        if i == selected {
            segment_active(ui.painter(), seg, R_BUTTON);
        }
        let color = if i == selected {
            INK
        } else if response.hovered() {
            DIM
        } else {
            FAINT
        };
        ui.painter().text(
            seg.center(),
            Align2::CENTER_CENTER,
            *label,
            font(FS_VALUE, Weight::SemiBold),
            color,
        );
        if response.clicked() {
            chosen = Some(i);
        }
        x += w;
    }
    chosen
}
pub fn segmented_icons(ui: &mut Ui, icons: &[(Icon, &str)], selected: usize) -> Option<usize> {
    let (rect, _) =
        ui.allocate_exact_size(vec2(icons.len() as f32 * 27.0 + 3.0, 24.0), Sense::hover());
    groove_shallow(ui.painter(), rect, R_CONTROL);
    let mut chosen = None;
    for (i, (kind, tip)) in icons.iter().enumerate() {
        let seg = Rect::from_min_size(
            pos2(rect.left() + 2.0 + i as f32 * 27.0, rect.top() + 2.0),
            vec2(26.0, 20.0),
        );
        let response = ui
            .interact(seg, ui.id().with(("segment-icon", i)), Sense::click())
            .on_hover_text(*tip);
        if i == selected {
            segment_active(ui.painter(), seg, R_BUTTON);
        }
        let color = if i == selected {
            INK
        } else if response.hovered() {
            DIM
        } else {
            FAINT
        };
        icon(ui.painter(), seg, *kind, color);
        if response.clicked() {
            chosen = Some(i);
        }
    }
    chosen
}
/// Horizontal slider: 4 px groove rail and a 13 px milled thumb.
pub fn hslider(
    ui: &mut Ui,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    width: f32,
) -> Response {
    let (rect, response) =
        ui.allocate_exact_size(vec2(width, SLIDER_THUMB + 2.0), Sense::click_and_drag());
    let rail = Rect::from_center_size(rect.center(), vec2(width, 4.0));
    let travel = (width - SLIDER_THUMB).max(1.0);
    let span = range.end() - range.start();
    if response.dragged() || response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let t = ((pos.x - rect.left() - SLIDER_THUMB / 2.0) / travel).clamp(0.0, 1.0);
            let next = range.start() + t * span;
            if (next - *value).abs() > f32::EPSILON {
                *value = next;
                response.clone().mark_changed();
            }
        }
    }
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        groove(p, rail, 2.0);
        let t = if span.abs() > f32::EPSILON {
            ((*value - range.start()) / span).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let t_rect = Rect::from_min_size(
            pos2(
                rect.left() + t * travel,
                rect.center().y - SLIDER_THUMB / 2.0,
            ),
            Vec2::splat(SLIDER_THUMB),
        );
        thumb(p, t_rect);
    }
    response
}
/// Rotary control. Vertical drag changes the value; double-click resets it.
pub fn knob_widget(
    ui: &mut Ui,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    reset: f32,
    diameter: f32,
) -> Response {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::splat(diameter + 4.0), Sense::click_and_drag());
    let span = range.end() - range.start();
    if response.double_clicked() {
        *value = reset;
        response.clone().mark_changed();
    } else if response.dragged() {
        let delta = -response.drag_delta().y / 150.0 * span;
        if delta != 0.0 {
            *value = (*value + delta).clamp(*range.start(), *range.end());
            response.clone().mark_changed();
        }
    }
    if ui.is_rect_visible(rect) {
        let t = if span.abs() > f32::EPSILON {
            ((*value - range.start()) / span).clamp(0.0, 1.0)
        } else {
            0.0
        };
        knob(
            ui.painter(),
            rect.center(),
            diameter / 2.0,
            -135.0 + 270.0 * t,
            diameter >= KNOB_LG,
        );
    }
    response
}
/// Vertical fader: 8 px rail, 28 × 18 cap. `value` is 0..=1.
pub fn fader(ui: &mut Ui, value: &mut f32, height: f32) -> Response {
    let (rect, response) =
        ui.allocate_exact_size(vec2(FADER_CAP.x, height), Sense::click_and_drag());
    let travel = (height - FADER_CAP.y).max(1.0);
    if response.dragged() || response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let t = 1.0 - ((pos.y - rect.top() - FADER_CAP.y / 2.0) / travel).clamp(0.0, 1.0);
            if (t - *value).abs() > f32::EPSILON {
                *value = t;
                response.clone().mark_changed();
            }
        }
    }
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        let rail = Rect::from_center_size(rect.center(), vec2(FADER_RAIL_W, height));
        groove(p, rail, 4.0);
        let cap = Rect::from_min_size(
            pos2(
                rect.left(),
                rect.top() + (1.0 - value.clamp(0.0, 1.0)) * travel,
            ),
            FADER_CAP,
        );
        fader_cap(p, cap);
    }
    response
}
/// Frameless single-line text field that sits inside a well the caller painted.
pub fn inline_edit(
    ui: &mut Ui,
    id: impl std::hash::Hash,
    value: &mut String,
    font_id: FontId,
    color: Color32,
    width: f32,
) -> Response {
    ui.add_sized(
        [width, font_id.size * 1.5],
        egui::TextEdit::singleline(value)
            .id_salt(id)
            .frame(false)
            .font(font_id)
            .text_color(color)
            .margin(egui::Margin::symmetric(4, 2)),
    )
}
/// Fills the bar behind a toolbar with the transport gradient and its edges.
pub fn transport_bar(p: &Painter, r: Rect) {
    shade_rect(p, r, 0.0, vertical(r, TRANSPORT_TOP, TRANSPORT_BOTTOM));
    hline(p, r.left(), r.right(), r.top(), white(0.06));
    inset(
        p,
        Rect::from_min_size(r.left_bottom(), vec2(r.width(), 6.0)),
        0.0,
        Side::Top,
        6.0,
        black(0.45),
    );
}
pub fn toolbar_bar(p: &Painter, r: Rect) {
    p.rect_filled(r, 0.0, PANEL);
    hline(p, r.left(), r.right(), r.bottom() - 1.0, black(0.6));
}
