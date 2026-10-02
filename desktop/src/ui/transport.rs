//! The transport: locate, play, stop, record and cycle keys, the time display, tempo, signature and key, click and snap, the master meter and CPU, and the agent key.

use super::{daw::Daw, theme::Theme};
use gpui::{div, prelude::*, Context, Entity, Window};

pub struct Transport {
    daw: Entity<Daw>,
}

impl Transport {
    pub fn new(daw: Entity<Daw>, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self { daw }
    }
}

impl Render for Transport {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let _ = self.daw.read(cx);
        div().size_full().text_color(theme.text_3).child("Transport")
    }
}
