//! Settings › Generation: the service `generate.audio` calls and its key. A key is saved
//! only when the person saves, never shown back (the settings mask it), and an agent may not
//! change any of this (`settings.set` refuses `generation.*` from agents).

use crate::ui::{
    daw::Daw,
    dialogs::modal::{self, request_async, Dismiss},
    theme::{radius, size, Theme},
    widgets::{
        field,
        secret_input::{secret_field, SecretInput},
        Button, InputEvent, TextInput,
    },
};
use gpui::{div, prelude::*, px, Context, Entity, Subscription, Window};
use ryolune_engine::settings::Service;
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// A service as the cards show it: (service, name, what it makes, key field).
pub(crate) const SERVICES: [(Service, &str, &str, &str); 4] = [
    (
        Service::ElevenLabs,
        "ElevenLabs",
        "Songs and loops with Eleven Music; one-shots and effects up to 30 s.",
        "elevenlabsApiKey",
    ),
    (
        Service::Stability,
        "Stable Audio",
        "Stability AI's model: loops, songs up to 3 min and sound design.",
        "stabilityApiKey",
    ),
    (
        Service::Fal,
        "fal.ai",
        "Any of fal's audio models: Stable Audio, Lyria, ACE-Step…",
        "falApiKey",
    ),
    (
        Service::Custom,
        "Your endpoint",
        "Any HTTP service that follows ryolune's simple contract.",
        "customApiKey",
    ),
];

pub struct GenerationForm {
    daw: Entity<Daw>,
    service: Service,
    key: Entity<SecretInput>,
    fal_model: Entity<TextInput>,
    custom_url: Entity<TextInput>,
    /// Which services have what they need, from `generate.services`.
    ready: BTreeMap<String, bool>,
    notice: Option<String>,
    error: Option<String>,
    _subscriptions: Vec<Subscription>,
}

impl GenerationForm {
    pub fn new(daw: Entity<Daw>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let key = cx.new(SecretInput::new);
        let fal_model = cx.new(|cx| TextInput::new(cx).placeholder("fal-ai/stable-audio"));
        let custom_url =
            cx.new(|cx| TextInput::new(cx).placeholder("https://my-server.example/generate"));
        let mut subscriptions =
            vec![cx.subscribe_in(&key, window, |this, _, e, w, cx| this.event(e, w, cx))];
        for input in [&fal_model, &custom_url] {
            subscriptions
                .push(cx.subscribe_in(input, window, |this, _, e, w, cx| this.event(e, w, cx)));
        }
        let service = daw.read(cx).app.settings.generation.service;
        let mut form = Self {
            daw,
            service,
            key,
            fal_model,
            custom_url,
            ready: BTreeMap::new(),
            notice: None,
            error: None,
            _subscriptions: subscriptions,
        };
        form.reset(cx);
        form
    }

    fn event(&mut self, event: &InputEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            InputEvent::Changed => cx.notify(),
            InputEvent::Submit => self.use_service(cx),
            InputEvent::Cancel => window.dispatch_action(Box::new(Dismiss), cx),
            InputEvent::Blur => {}
        }
    }

    fn saved(&self, cx: &gpui::App) -> Value {
        self.daw.read(cx).app.settings.redacted()["generation"].clone()
    }

    fn key_path(&self) -> &'static str {
        SERVICES
            .iter()
            .find(|s| s.0 == self.service)
            .map_or("elevenlabsApiKey", |s| s.3)
    }

    /// Fill the fields from the saved settings, and say whether a key is saved.
    fn reset(&mut self, cx: &mut Context<Self>) {
        let saved = self.saved(cx);
        let text = |key: &str| saved[key].as_str().unwrap_or("").to_string();
        let has_key = !text(self.key_path()).is_empty();
        self.key.update(cx, |key, cx| {
            key.clear(cx);
            key.set_placeholder(
                if has_key {
                    "Key saved · enter a replacement"
                } else {
                    "Paste your API key"
                },
                cx,
            );
        });
        let (fal, url) = (text("falModel"), text("customUrl"));
        self.fal_model.update(cx, |i, cx| i.set_text(fal, cx));
        self.custom_url.update(cx, |i, cx| i.set_text(url, cx));
    }

    /// Ask which services are ready (have a key, or an address).
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        let daw = self.daw.clone();
        request_async(
            self,
            &daw,
            "generate.services",
            json!({}),
            cx,
            |this, result, cx| {
                if let Ok(value) = result {
                    this.ready = value["services"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|s| {
                            (
                                s["id"].as_str().unwrap_or("").to_string(),
                                s["ready"].as_bool().unwrap_or(false),
                            )
                        })
                        .collect();
                }
                cx.notify();
            },
        );
    }

    fn save(&mut self, entries: Vec<(&str, Value)>, cx: &mut Context<Self>) -> bool {
        self.notice = None;
        self.error = None;
        let result = self.daw.update(cx, |daw, cx| {
            for (path, value) in entries {
                daw.request(
                    "settings.set",
                    json!({ "path": format!("generation.{path}"), "value": value }),
                    cx,
                )?;
            }
            Ok::<_, String>(())
        });
        match result {
            Ok(()) => {
                self.reset(cx);
                self.refresh(cx);
                self.notice = Some("Saved.".into());
                cx.notify();
                true
            }
            Err(error) => {
                self.error = Some(error);
                cx.notify();
                false
            }
        }
    }

    /// Use this service: the choice, a typed key, and the service's own field.
    fn use_service(&mut self, cx: &mut Context<Self>) {
        let mut entries = vec![("service", json!(self.service.key()))];
        let key = self.key.read(cx).text().trim().to_string();
        if !key.is_empty() {
            entries.push((self.key_path(), json!(key)));
        }
        match self.service {
            Service::Fal => {
                entries.push(("falModel", json!(self.fal_model.read(cx).text().trim())))
            }
            Service::Custom => {
                let url = self.custom_url.read(cx).text().trim().to_string();
                if url.is_empty() {
                    self.error = Some("Enter the endpoint address.".into());
                    cx.notify();
                    return;
                }
                entries.push(("customUrl", json!(url)))
            }
            _ => {}
        }
        self.save(entries, cx);
    }
}

impl Render for GenerationForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        let saved = self.saved(cx);
        let has_key = !saved[self.key_path()].as_str().unwrap_or("").is_empty();
        let service = self.service;
        let cards =
            div()
                .flex()
                .flex_wrap()
                .gap(px(8.0))
                .children(SERVICES.iter().enumerate().map(|(i, (s, name, blurb, _))| {
                    let s = *s;
                    let on = s == service;
                    let ready = self.ready.get(s.key()).copied().unwrap_or(false);
                    div()
                        .id(("gen-service", i))
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .flex_grow()
                        .flex_basis(gpui::relative(0.4))
                        .p(px(12.0))
                        .rounded(px(radius::MD))
                        .bg(if on {
                            theme.accent_soft
                        } else {
                            theme.bg_raised
                        })
                        .border_1()
                        .border_color(if on { theme.accent_ring } else { theme.line })
                        .cursor_pointer()
                        .hover(|d| d.border_color(theme.line_strong))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .text_size(px(size::BASE))
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(theme.text)
                                        .child(*name),
                                )
                                .when(ready, |d| {
                                    d.child(
                                        div()
                                            .text_size(px(size::XS))
                                            .text_color(theme.accent_text)
                                            .child("Connected"),
                                    )
                                }),
                        )
                        .child(modal::note(*blurb, cx))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.service = s;
                            this.notice = None;
                            this.error = None;
                            this.reset(cx);
                            cx.notify();
                        }))
                }));
        let key_label = if service == Service::Custom {
            "API key (optional)"
        } else {
            "API key"
        };
        let focused = |input: &Entity<TextInput>| input.read(cx).is_focused(window);
        let mut form = div().flex().flex_col().gap(px(12.0));
        if service == Service::Custom {
            form = form.child(modal::stacked(
                "Endpoint address",
                None,
                field(&self.custom_url, focused(&self.custom_url), cx),
                cx,
            ));
        }
        form = form.child(modal::stacked(
            key_label,
            None,
            secret_field(&self.key, self.key.read(cx).is_focused(window), cx),
            cx,
        ));
        if service == Service::Fal {
            form = form.child(modal::stacked(
                "Model",
                None,
                field(&self.fal_model, focused(&self.fal_model), cx),
                cx,
            ));
        }
        let key_path = self.key_path();
        form =
            form.child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        Button::new("gen-use", "Use this service")
                            .primary()
                            .on_click(cx.listener(|this, _, _, cx| this.use_service(cx))),
                    )
                    .when(has_key, |d| {
                        d.child(Button::new("gen-remove", "Remove saved key").on_click(
                            cx.listener(move |this, _, _, cx| {
                                this.save(vec![(key_path, json!(""))], cx);
                            }),
                        ))
                    })
                    .child(
                        Button::new(
                            "gen-guide",
                            if service == Service::Custom {
                                "How an endpoint answers"
                            } else {
                                "Get a key"
                            },
                        )
                        .ghost()
                        .with_icon("external")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.daw.update(cx, |daw, cx| {
                                daw.run("app.openGuide", json!({ "guide": service.key() }), cx);
                            })
                        })),
                    ),
            );
        div()
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(14.0))
            .child(modal::heading("Make sounds from a description", cx))
            .child(modal::text(
                "Loops, songs, one-shots and playable instruments, from the Generate tab of the agent panel or by asking the agent. Connect a service once; the service bills your use directly and ryolune takes nothing.",
                cx,
            ))
            .child(cards)
            .child(form)
            .when_some(self.notice.clone(), |d, n| d.child(modal::note(n, cx)))
            .when_some(self.error.clone(), |d, e| d.child(modal::error_line(e, cx)))
            .child(modal::note(
                "Your description, and for loops the song's tempo and key, go to the service you choose. Sounds are kept on this computer until you delete them. The agent can generate only while “Generate sounds” is allowed in Settings › Agent.",
                cx,
            ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_service_has_a_card_and_a_masked_key() {
        let defaults = ryolune_engine::settings::Settings::default().redacted();
        for service in Service::ALL {
            let (_, _, _, key) = SERVICES.iter().find(|s| s.0 == service).unwrap();
            assert!(defaults["generation"].get(*key).is_some());
            let path = format!("generation.{key}");
            assert!(ryolune_engine::settings::SECRET_PATHS.contains(&path.as_str()));
        }
    }
}
