//! The mixer, in place of the region editor: one strip per track, bus and the Stereo Out.

use super::{daw::Daw, theme::Theme};
use gpui::{div, prelude::*, Context, Entity, Window};

pub struct Mixer {
    daw: Entity<Daw>,
}

impl Mixer {
    pub fn new(daw: Entity<Daw>, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self { daw }
    }
}

impl Render for Mixer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let _ = self.daw.read(cx);
        div().size_full().text_color(theme.text_3).child("Mixer")
    }
}
