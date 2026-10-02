//! Controls shared by every panel, drawn from the theme. Panels compose these; a colour,
//! radius or size that is not in `theme.rs` does not belong in a view.

pub mod controls;
pub mod dial;
pub mod menu;
pub mod secret_input;
pub mod text_input;

pub use controls::{caps, dot, icon, tip, Button, Key, Segmented, Switch};
pub use dial::{Fader, Knob, Meter, NumberDrag, Phase, Slider};
pub use menu::{select_button, MenuHost, MenuItem};
pub use text_input::{field, InputEvent, TextInput};

use super::theme::{radius, Theme};
use gpui::{div, prelude::*, px, App};

/// A floating or modal surface: glass with its edge, top highlight and drop.
pub fn surface(tier: u8, cx: &App) -> gpui::Div {
    let theme = Theme::get(cx);
    div()
        .bg(theme.glass(tier))
        .rounded(px(if tier >= 3 { radius::LG } else { radius::MD }))
        .border_1()
        .border_color(theme.glass_edge)
        .shadow(vec![gpui::BoxShadow {
            color: theme.glass_shadow,
            offset: gpui::point(px(0.0), px(if tier >= 3 { 28.0 } else { 12.0 })),
            blur_radius: px(if tier >= 3 { 80.0 } else { 32.0 }),
            spread_radius: px(0.0),
        }])
}

pub fn bind(cx: &mut App) {
    text_input::bind(cx);
    menu::bind(cx);
}

/// The width of a label in the interface face, for laying out things placed by hand.
pub fn text_width(text: &str, size_px: f32, window: &mut gpui::Window) -> f32 {
    let style = window.text_style();
    let mut font = style.font();
    font.family = super::theme::FONT_UI.into();
    let run = gpui::TextRun {
        len: text.len(),
        font,
        color: gpui::black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line = window
        .text_system()
        .shape_line(text.to_string().into(), px(size_px), &[run], None);
    f32::from(line.width)
}
