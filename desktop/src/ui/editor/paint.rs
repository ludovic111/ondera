//! Drawing in a canvas with local coordinates: the editor's painters place everything from
//! the canvas's top-left corner, as the geometry does, and `Pen` turns that into window
//! pixels.

use crate::ui::theme::{FONT_MONO, FONT_UI};
use gpui::{
    point, px, size, App, Background, Bounds, BoxShadow, ContentMask, Corners, FontWeight, Hsla,
    PathBuilder, Pixels, Point, TextRun, Window,
};

/// Which face a label is set in.
#[derive(Clone, Copy)]
pub enum Face {
    UiBold,
    Mono,
}

/// Paints relative to a canvas's origin.
#[derive(Clone, Copy)]
pub struct Pen {
    pub bounds: Bounds<Pixels>,
}

// A shape is a position, a size and its paint: the arguments add up.
#[allow(clippy::too_many_arguments)]
impl Pen {
    pub fn new(bounds: Bounds<Pixels>) -> Self {
        Self { bounds }
    }
    pub fn width(&self) -> f32 {
        f32::from(self.bounds.size.width)
    }
    pub fn height(&self) -> f32 {
        f32::from(self.bounds.size.height)
    }
    pub fn at(&self, x: f32, y: f32) -> Point<Pixels> {
        self.bounds.origin + point(px(x), px(y))
    }
    pub fn rect(&self, x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
        Bounds::new(self.at(x, y), size(px(w.max(0.0)), px(h.max(0.0))))
    }
    /// Keep everything painted inside the canvas.
    pub fn clip<R>(&self, window: &mut Window, f: impl FnOnce(&mut Window) -> R) -> R {
        window.with_content_mask(
            Some(ContentMask {
                bounds: self.bounds,
            }),
            f,
        )
    }
    pub fn fill(
        &self,
        window: &mut Window,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: impl Into<Background>,
    ) {
        window.paint_quad(gpui::fill(self.rect(x, y, w, h), color));
    }
    /// A 1 px horizontal line.
    pub fn hline(&self, window: &mut Window, x: f32, y: f32, w: f32, color: Hsla) {
        self.fill(window, x, y.round(), w, 1.0, color);
    }
    /// A 1 px vertical line.
    pub fn vline(&self, window: &mut Window, x: f32, y: f32, h: f32, color: Hsla) {
        self.fill(window, x.round(), y, 1.0, h, color);
    }
    /// A rounded rectangle; the radius never exceeds half the shorter side.
    pub fn round(
        &self,
        window: &mut Window,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        radius: impl Into<Corners<Pixels>>,
        background: impl Into<Background>,
    ) {
        let bounds = self.rect(x, y, w, h);
        let radii = radius.into().clamp_radii_for_quad_size(bounds.size);
        window.paint_quad(gpui::fill(bounds, background).corner_radii(radii));
    }
    /// A rounded outline of `width` pixels.
    pub fn outline(
        &self,
        window: &mut Window,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        radius: f32,
        width: f32,
        color: Hsla,
    ) {
        let bounds = self.rect(x, y, w, h);
        let radii = Corners::all(px(radius)).clamp_radii_for_quad_size(bounds.size);
        window.paint_quad(
            gpui::outline(bounds, color, gpui::BorderStyle::Solid)
                .corner_radii(radii)
                .border_widths(px(width)),
        );
    }
    /// A soft shadow under a rounded rectangle.
    pub fn shadow(
        &self,
        window: &mut Window,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        radius: f32,
        shadow: BoxShadow,
    ) {
        let bounds = self.rect(x, y, w, h);
        let radii = Corners::all(px(radius)).clamp_radii_for_quad_size(bounds.size);
        window.paint_shadows(bounds, radii, &[shadow]);
    }
    /// A filled polygon.
    pub fn polygon(&self, window: &mut Window, points: &[(f32, f32)], color: Hsla) {
        if points.len() < 3 {
            return;
        }
        let mut path = PathBuilder::fill();
        let pts: Vec<Point<Pixels>> = points.iter().map(|&(x, y)| self.at(x, y)).collect();
        path.add_polygon(&pts, true);
        if let Ok(path) = path.build() {
            window.paint_path(path, color);
        }
    }
    /// A filled ellipse, turned by `angle` radians.
    pub fn ellipse(
        &self,
        window: &mut Window,
        cx_: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        angle: f32,
        color: Hsla,
    ) {
        let (sin, cos) = angle.sin_cos();
        let points: Vec<(f32, f32)> = (0..24)
            .map(|i| {
                let t = i as f32 / 24.0 * std::f32::consts::TAU;
                let (x, y) = (rx * t.cos(), ry * t.sin());
                (cx_ + x * cos - y * sin, cy + x * sin + y * cos)
            })
            .collect();
        self.polygon(window, &points, color);
    }
    pub fn circle(&self, window: &mut Window, x: f32, y: f32, r: f32, color: Hsla) {
        self.round(window, x - r, y - r, r * 2.0, r * 2.0, px(r), color);
    }
    /// A label with its top-left at (x, y). Returns its width.
    pub fn text(
        &self,
        window: &mut Window,
        cx: &mut App,
        text: &str,
        x: f32,
        y: f32,
        size_px: f32,
        color: Hsla,
        face: Face,
    ) -> f32 {
        let line = shape(window, text, size_px, color, face);
        let width = f32::from(line.width);
        let _ = line.paint(self.at(x, y), px(size_px * 1.25), window, cx);
        width
    }
    /// A label whose right edge sits at `right`.
    pub fn text_right(
        &self,
        window: &mut Window,
        cx: &mut App,
        text: &str,
        right: f32,
        y: f32,
        size_px: f32,
        color: Hsla,
        face: Face,
    ) {
        let line = shape(window, text, size_px, color, face);
        let x = right - f32::from(line.width);
        let _ = line.paint(self.at(x, y), px(size_px * 1.25), window, cx);
    }
}

fn shape(
    window: &mut Window,
    text: &str,
    size_px: f32,
    color: Hsla,
    face: Face,
) -> gpui::ShapedLine {
    let mut font = window.text_style().font();
    font.family = match face {
        Face::Mono => FONT_MONO,
        Face::UiBold => FONT_UI,
    }
    .into();
    if let Face::UiBold = face {
        font.weight = FontWeight::SEMIBOLD;
    }
    let run = TextRun {
        len: text.len(),
        font,
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    window
        .text_system()
        .shape_line(text.to_string().into(), px(size_px), &[run], None)
}

/// A drop shadow description in the theme's colour.
pub fn drop(color: Hsla, y: f32, blur: f32) -> BoxShadow {
    BoxShadow {
        color,
        offset: point(px(0.0), px(y)),
        blur_radius: px(blur),
        spread_radius: px(0.0),
    }
}
