//! The Settings window (⌘,) over engine::settings::Settings.

use super::{daw::Daw, theme::Theme};
use gpui::{div, prelude::*, Context, Entity, Window};

pub struct SettingsWindow {
    daw: Entity<Daw>,
}

impl SettingsWindow {
    pub fn new(daw: Entity<Daw>, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self { daw }
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let _ = self.daw.read(cx);
        div().size_full().text_color(theme.text_3).child("SettingsWindow")
    }
}
