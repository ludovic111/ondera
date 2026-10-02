//! The Takes tab: creative A/B takes saved inside the project (`take.*`). "Create &
//! explore" keeps the original first (the first time), saves the variation, then hands the
//! person to the chat with a prompt to describe the new direction. Switching takes stops
//! playback and is one undo step; takes are locked while the agent works.

use super::{AgentPanel, Tab};
use crate::ui::{
    daw::Daw,
    theme::{radius, size, Theme},
    widgets::{field, Button, InputEvent, TextInput},
};
use gpui::{
    div, prelude::*, px, AnyElement, Context, Entity, FontWeight, SharedString, Subscription,
    Window,
};
use serde_json::{json, Value};

/// At most this many takes travel with a project.
pub const MAX_TAKES: usize = 8;

/// The message the chat gets after a new take: the person fills in the direction.
pub const VARIATION_PROMPT: &str = "Explore a different musical direction in this creative take: [describe your idea]. The original version is preserved in Takes. Inspect the current project and preserve its identity.";

#[derive(Clone, Debug, PartialEq)]
pub struct Take {
    pub id: String,
    pub name: String,
    pub tracks: u64,
    pub clips: u64,
}

pub fn takes_from(value: &Value) -> (Option<String>, Vec<Take>) {
    let takes = value["takes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|t| {
            Some(Take {
                id: t["id"].as_str()?.to_string(),
                name: t["name"].as_str().unwrap_or("").to_string(),
                tracks: t["tracks"].as_u64().unwrap_or(0),
                clips: t["clips"].as_u64().unwrap_or(0),
            })
        })
        .collect();
    (value["active"].as_str().map(str::to_string), takes)
}

/// The names "Create & explore" saves: the original first when there is no take yet.
pub fn names_to_create(existing: usize, typed: &str) -> Vec<String> {
    let mut names = vec![];
    if existing == 0 {
        names.push("Original".to_string());
    }
    let typed = typed.trim();
    names.push(if typed.is_empty() {
        format!("Variation {}", existing.max(1))
    } else {
        typed.chars().take(120).collect()
    });
    names
}

pub struct Takes {
    name: Entity<TextInput>,
    active: Option<String>,
    list: Vec<Take>,
    error: String,
    /// The document revision the list was read at.
    revision: Option<u64>,
    _name: Subscription,
}

impl Takes {
    pub fn new(window: &mut Window, cx: &mut Context<AgentPanel>) -> Self {
        let name = cx.new(|cx| TextInput::new(cx).placeholder("Warmer chorus, half-time groove…"));
        let subscription = cx.subscribe_in(&name, window, |this, _, event, _, cx| {
            if *event == InputEvent::Submit {
                this.create_take(cx);
            }
        });
        Self {
            name,
            active: None,
            list: vec![],
            error: String::new(),
            revision: None,
            _name: subscription,
        }
    }

    /// Read `take.list` again.
    pub fn refresh(&mut self, daw: &Entity<Daw>, cx: &mut Context<AgentPanel>) {
        let result = daw.update(cx, |daw, cx| {
            let revision = daw.app.store.revision;
            (revision, daw.request("take.list", json!({}), cx))
        });
        self.revision = Some(result.0);
        match result.1 {
            Ok(value) => (self.active, self.list) = takes_from(&value),
            Err(error) => self.error = error,
        }
    }
}

impl AgentPanel {
    fn take_action(&mut self, method: &str, params: Value, cx: &mut Context<Self>) -> bool {
        self.takes.error.clear();
        let result = self
            .daw
            .update(cx, |daw, cx| daw.request(method, params, cx));
        let ok = match result {
            Ok(_) => true,
            Err(error) => {
                self.takes.error = error;
                false
            }
        };
        let daw = self.daw.clone();
        self.takes.refresh(&daw, cx);
        cx.notify();
        ok
    }

    pub(super) fn create_take(&mut self, cx: &mut Context<Self>) {
        if self.busy(cx) || self.takes.list.len() >= MAX_TAKES {
            return;
        }
        let typed = self.takes.name.read(cx).text().to_string();
        for name in names_to_create(self.takes.list.len(), &typed) {
            if !self.take_action("take.create", json!({ "name": name }), cx) {
                return;
            }
        }
        self.takes
            .name
            .update(cx, |input, cx| input.set_text("", cx));
        self.select_tab(Tab::Chat, cx);
        self.set_draft(VARIATION_PROMPT, cx);
    }

    pub(super) fn render_takes(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let revision = self.daw.read(cx).app.store.revision;
        if self.takes.revision != Some(revision) {
            let daw = self.daw.clone();
            self.takes.refresh(&daw, cx);
        }
        let theme = Theme::get(cx).clone();
        let busy = self.busy(cx);
        let full = self.takes.list.len() >= MAX_TAKES;
        let focused = self.takes.name.read(cx).is_focused(window);
        let text = |s: &'static str| {
            div()
                .text_size(px(size::BASE))
                .line_height(px(21.0))
                .text_color(theme.text_2)
                .child(s)
        };
        div()
            .py(px(16.0))
            .px(px(2.0))
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(
                div()
                    .text_size(px(size::BASE + 1.0))
                    .font_weight(FontWeight::BOLD)
                    .child("Try another direction"),
            )
            .child(text("Keep the original. Explore a new take, then switch between versions to compare. Switching stops playback and can be undone."))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(div().text_size(px(size::SM)).text_color(theme.text_2).child("New take"))
                    .child(field(&self.takes.name, focused, cx))
                    .child(div().flex().flex_col().child(
                        Button::new("take-create", "Create & explore")
                            .primary()
                            .disabled(busy || full)
                            .tooltip(if busy {
                                "Stop the agent before saving creative takes"
                            } else if full {
                                "Eight takes at most: remove one first"
                            } else {
                                "Save this version and start a new take"
                            })
                            .on_click(cx.listener(|this, _, _, cx| this.create_take(cx))),
                    )),
            )
            .when(!self.takes.error.is_empty(), |d| {
                d.child(
                    div()
                        .text_size(px(size::SM))
                        .text_color(theme.danger)
                        .child(SharedString::from(self.takes.error.clone())),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .my(px(6.0))
                    .children(self.takes.list.iter().map(|take| {
                        let active = self.takes.active.as_deref() == Some(take.id.as_str());
                        let (select_id, remove_id) = (take.id.clone(), take.id.clone());
                        div()
                            .flex()
                            .gap(px(5.0))
                            .child(
                                div()
                                    .id(SharedString::from(format!("take-{}", take.id)))
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap(px(3.0))
                                    .px(px(10.0))
                                    .py(px(8.0))
                                    .rounded(px(radius::SM))
                                    .bg(if active { theme.accent_soft } else { theme.control })
                                    .border_1()
                                    .border_color(if active { theme.accent } else { theme.control_edge })
                                    .child(div().font_weight(FontWeight::SEMIBOLD).child(take.name.clone()))
                                    .child(
                                        div()
                                            .text_size(px(size::SM))
                                            .text_color(theme.text_2)
                                            .child(if active {
                                                "Current version".to_string()
                                            } else {
                                                format!("{} tracks · {} regions", take.tracks, take.clips)
                                            }),
                                    )
                                    .when(!active && !busy, |d| {
                                        d.cursor_pointer()
                                            .hover(|s| s.bg(theme.control_hover))
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.take_action("take.select", json!({"id": select_id}), cx);
                                            }))
                                    })
                                    .when(busy && !active, |d| d.opacity(0.5)),
                            )
                            .when(!active, |d| {
                                d.child(
                                    Button::icon(SharedString::from(format!("take-remove-{}", take.id)), "close")
                                        .ghost()
                                        .disabled(busy)
                                        .tooltip("Remove this saved take (undoable)")
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.take_action("take.remove", json!({"id": remove_id}), cx);
                                        })),
                                )
                            })
                    })),
            )
            .child(text("Up to eight takes. Save your project to keep them all, including plugin settings and audio. Edits stay with the active take."))
            .child(
                div().flex().child(
                    Button::new("take-listen", "Listen to current take")
                        .disabled(busy)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.daw.update(cx, |daw, cx| {
                                daw.fire("transport.play", cx);
                            })
                        })),
                ),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_a_protected_original_before_a_variation() {
        assert_eq!(names_to_create(0, ""), ["Original", "Variation 1"]);
        assert_eq!(names_to_create(2, "  Half-time "), ["Half-time"]);
        assert_eq!(names_to_create(3, ""), ["Variation 3"]);
        let (active, list) = takes_from(&json!({"active":"t2","takes":[
            {"id":"t1","name":"Original","tracks":3,"clips":5},
            {"id":"t2","name":"Variation 1","tracks":4,"clips":6}
        ]}));
        assert_eq!(active.as_deref(), Some("t2"));
        assert_eq!(list[1].clips, 6);
    }
}
