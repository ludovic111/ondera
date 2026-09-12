use eframe::egui::{self, Color32, FontId, Stroke};

// Port of packages/app/src/theme/tokens.ts: warm graphite, restrained cyan,
// top-edge highlights and coloured tracks. Native widgets share these tokens.
pub const DESK: Color32 = Color32::from_rgb(20, 20, 19);
pub const WELL: Color32 = Color32::from_rgb(22, 22, 21);
pub const PANEL: Color32 = Color32::from_rgb(44, 44, 43);
pub const RAISED: Color32 = Color32::from_rgb(53, 53, 52);
pub const FACE: Color32 = Color32::from_rgb(66, 66, 63);
pub const LANE: Color32 = Color32::from_rgb(34, 34, 33);
pub const LANE_SELECTED: Color32 = Color32::from_rgb(43, 43, 42);
pub const INK: Color32 = Color32::from_rgb(232, 231, 228);
pub const DIM: Color32 = Color32::from_rgb(169, 168, 164);
pub const FAINT: Color32 = Color32::from_rgb(127, 126, 122);
pub const ACCENT: Color32 = Color32::from_rgb(92, 210, 200);
pub const RED: Color32 = Color32::from_rgb(226, 113, 107);
pub const GOLD: Color32 = Color32::from_rgb(215, 195, 127);
pub const GRID: Color32 = Color32::from_rgb(55, 55, 53);
pub const WHITE_KEY: Color32 = Color32::from_rgb(217, 215, 209);
pub const BLACK_KEY: Color32 = Color32::from_rgb(35, 35, 34);
pub const TRACKS: [Color32; 8] = [
    Color32::from_rgb(211, 166, 115),
    Color32::from_rgb(171, 143, 212),
    Color32::from_rgb(122, 181, 207),
    Color32::from_rgb(144, 183, 146),
    Color32::from_rgb(215, 143, 154),
    Color32::from_rgb(194, 158, 181),
    Color32::from_rgb(205, 184, 127),
    Color32::from_rgb(115, 190, 184),
];
pub const ROW: f32 = 70.0;
pub const HEADER: f32 = 184.0;
pub const RULER: f32 = 28.0;
pub const KEY_ROW: f32 = 12.0;
pub const KEY_WIDTH: f32 = 56.0;
pub const BODY: f32 = 12.0;
pub const SMALL: f32 = 10.0;
pub const TITLE: f32 = 15.0;
pub const DIGITS: f32 = 21.0;
pub const GAP: f32 = 8.0;

pub fn install(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = PANEL;
    style.visuals.window_fill = PANEL;
    style.visuals.extreme_bg_color = WELL;
    style.visuals.faint_bg_color = LANE;
    style.visuals.override_text_color = Some(INK);
    style.visuals.selection.bg_fill = ACCENT.gamma_multiply(0.3);
    style.visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    style.visuals.widgets.inactive.bg_fill = RAISED;
    style.visuals.widgets.inactive.weak_bg_fill = RAISED;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, FACE);
    style.visuals.widgets.hovered.bg_fill = FACE;
    style.visuals.widgets.hovered.weak_bg_fill = FACE;
    style.spacing.item_spacing = egui::vec2(GAP, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    style.spacing.interact_size = egui::vec2(28.0, 26.0);
    style
        .text_styles
        .insert(egui::TextStyle::Body, FontId::proportional(BODY));
    style
        .text_styles
        .insert(egui::TextStyle::Button, FontId::proportional(BODY));
    style
        .text_styles
        .insert(egui::TextStyle::Small, FontId::proportional(SMALL));
    style
        .text_styles
        .insert(egui::TextStyle::Heading, FontId::proportional(TITLE));
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
pub fn bevel(painter: &egui::Painter, rect: egui::Rect, color: Color32, selected: bool) {
    painter.rect_filled(rect.translate(egui::vec2(0.0, 2.0)), 4, DESK);
    painter.rect_filled(rect, 4, color.gamma_multiply(0.58));
    painter.rect_filled(
        egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), rect.center().y)),
        4,
        color.gamma_multiply(0.68),
    );
    painter.line_segment(
        [
            rect.left_top() + egui::vec2(3.0, 1.0),
            rect.right_top() + egui::vec2(-3.0, 1.0),
        ],
        Stroke::new(1.0, color.gamma_multiply(0.85)),
    );
    if selected {
        painter.rect_stroke(rect, 4, Stroke::new(1.5, color), egui::StrokeKind::Inside);
    }
}
