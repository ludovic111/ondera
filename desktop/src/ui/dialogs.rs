//! Dialogs over the window: unsaved changes, errors, export, recovery, updates, help.

use super::{daw::Daw, theme::Theme};
use gpui::{div, prelude::*, Context, Entity, Window};

pub struct Dialogs {
    daw: Entity<Daw>,
}

impl Dialogs {
    pub fn new(daw: Entity<Daw>, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self { daw }
    }
}

impl Render for Dialogs {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let _ = self.daw.read(cx);
        div().size_full().text_color(theme.text_3).child("Dialogs")
    }
}
