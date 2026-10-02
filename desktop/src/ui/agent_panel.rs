//! The agent panel at the right edge: conversation, generate, changes and takes.

use super::{daw::Daw, theme::Theme};
use gpui::{div, prelude::*, Context, Entity, Window};

pub struct AgentPanel {
    daw: Entity<Daw>,
}

impl AgentPanel {
    pub fn new(daw: Entity<Daw>, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self { daw }
    }
}

impl AgentPanel {
    /// Open the panel with the current selection as the subject of the next message.
    pub fn ask_about_selection(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.daw.update(cx, |daw, cx| {
            daw.run(
                "ui.showPanel",
                serde_json::json!({"panel": "agent", "visible": true}),
                cx,
            );
        });
    }
}

impl Render for AgentPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let _ = self.daw.read(cx);
        div().size_full().text_color(theme.text_3).child("AgentPanel")
    }
}
