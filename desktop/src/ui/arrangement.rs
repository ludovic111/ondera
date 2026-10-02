//! The arrangement: tool bar, ruler with markers and cycle, tempo track, track headers and lanes with regions.

use super::{daw::Daw, theme::Theme};
use gpui::{div, prelude::*, Context, Entity, Window};

pub struct Arrangement {
    daw: Entity<Daw>,
}

impl Arrangement {
    pub fn new(daw: Entity<Daw>, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self { daw }
    }
}

impl Render for Arrangement {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let _ = self.daw.read(cx);
        div()
            .size_full()
            .text_color(theme.text_3)
            .child("Arrangement")
    }
}
