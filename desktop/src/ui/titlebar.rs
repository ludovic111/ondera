//! The title bar: menus at the left (after the traffic lights on macOS), the song name in the
//! middle, the status line and a quiet Sponsor key at the right. Glass tier 1. Dragging the
//! empty bar moves the window; a double click zooms it.

use super::{
    actions::MENUS,
    daw::Daw,
    platform,
    theme::{radius, size, Theme},
    widgets::{icon, MenuHost, MenuItem},
};
use gpui::{div, prelude::*, px, Context, Entity, MouseButton, Window};
use serde_json::json;

pub struct TitleBar {
    daw: Entity<Daw>,
    menu: MenuHost,
    open_title: Option<&'static str>,
}

impl TitleBar {
    pub fn new(daw: Entity<Daw>, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            daw,
            menu: MenuHost::default(),
            open_title: None,
        }
    }

    fn open_menu(
        &mut self,
        title: &'static str,
        x: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let entries = MENUS
            .iter()
            .find(|(t, _)| *t == title)
            .map(|(_, e)| *e)
            .unwrap_or(&[]);
        let mut items: Vec<MenuItem> = entries
            .iter()
            .map(|e| match e {
                Some(id) => MenuItem::action(id, &self.daw, cx),
                None => MenuItem::Separator,
            })
            .collect();
        if title == "Help" {
            items.push(MenuItem::Separator);
            items.push(
                MenuItem::new(format!("ryolune {}", env!("CARGO_PKG_VERSION")), |_, _| {})
                    .disabled(true),
            );
        }
        self.open_title = Some(title);
        self.menu
            .open(items, gpui::point(px(x), px(32.0)), window, cx);
    }
}

impl Render for TitleBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        let app = &self.daw.read(cx).app;
        let name = format!(
            "{}{}",
            app.store.session().name,
            if app.store.dirty() { " *" } else { "" }
        );
        let status = app.status.clone();
        let update = match (&app.updates.available, &app.updates.installed) {
            (_, Some(_)) => Some("Relaunch to update".to_string()),
            (Some(r), None) => Some(format!("Update to {}", r.version)),
            _ => None,
        };
        if !self.menu.is_open() {
            self.open_title = None;
        }
        let open_title = self.open_title;
        let left_inset = if cfg!(target_os = "macos") {
            82.0
        } else {
            12.0
        };
        let mut x = left_inset;
        let menus: Vec<_> = MENUS
            .iter()
            .map(|(title, _)| {
                let title: &'static str = title;
                let at = x;
                x += super::widgets::text_width(title, size::BASE, window) + 16.0 + 2.0;
                let open = open_title == Some(title);
                div()
                    .id(title)
                    .px(px(8.0))
                    .h(px(24.0))
                    .flex()
                    .items_center()
                    .rounded(px(radius::SM))
                    .text_size(px(size::BASE))
                    .text_color(if open { theme.text } else { theme.text_2 })
                    .when(open, |d| d.bg(theme.hover))
                    .hover(|s| s.bg(theme.hover).text_color(theme.text))
                    .cursor_pointer()
                    .child(title)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            cx.stop_propagation();
                            if this.open_title == Some(title) {
                                this.menu.close();
                                this.open_title = None;
                                cx.notify();
                            } else {
                                this.open_menu(title, at, window, cx);
                            }
                        }),
                    )
                    // Sliding across titles while a menu is open switches menus, like a menu bar.
                    .on_hover(cx.listener(move |this, over: &bool, window, cx| {
                        if *over && this.open_title.is_some() && this.open_title != Some(title) {
                            this.open_menu(title, at, window, cx);
                        }
                    }))
            })
            .collect();
        let menu = self.menu.render(window, cx);
        div()
            .id("title-bar")
            .size_full()
            .flex()
            .items_center()
            .pl(px(left_inset))
            .pr(px(10.0))
            .gap(px(2.0))
            .bg(theme.glass(1))
            .border_b_1()
            .border_color(theme.line)
            .on_mouse_down(MouseButton::Left, |e, window, _| {
                if e.click_count == 2 {
                    window.titlebar_double_click();
                } else {
                    platform::start_window_drag(window);
                }
            })
            .children(menus)
            .child(
                div()
                    .flex_1()
                    .flex()
                    .justify_center()
                    .text_size(px(size::BASE))
                    .text_color(theme.text)
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child(name),
            )
            .child(
                div()
                    .max_w(px(320.0))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_size(px(size::SM))
                    .text_color(theme.text_3)
                    .mr(px(10.0))
                    .child(status),
            )
            .when_some(update, |d, label| {
                d.child(
                    div()
                        .id("update")
                        .px(px(10.0))
                        .h(px(22.0))
                        .flex()
                        .items_center()
                        .mr(px(8.0))
                        .rounded_full()
                        .bg(theme.accent_fill)
                        .text_color(theme.text_on_accent)
                        .text_size(px(size::SM))
                        .cursor_pointer()
                        .child(label)
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.daw.update(cx, |daw, cx| {
                                daw.app.updates.show = true;
                                cx.notify();
                            })
                        })),
                )
            })
            // ryolune is free; donations through GitHub Sponsors are the only money it takes.
            .child(
                div()
                    .id("sponsor")
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .px(px(10.0))
                    .h(px(22.0))
                    .rounded_full()
                    .border_1()
                    .border_color(theme.line)
                    .text_size(px(size::SM))
                    .text_color(theme.text_2)
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.hover).text_color(theme.text))
                    .child(icon("heart", 10.0, theme.danger))
                    .child("Sponsor")
                    .tooltip(|_, cx| {
                        super::widgets::tip(
                            "Sponsor ryolune on GitHub: donate once or monthly, nothing is locked"
                                .into(),
                            cx,
                        )
                    })
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.daw.update(cx, |daw, cx| {
                            daw.run("app.openGuide", json!({"guide": "support"}), cx);
                        })
                    })),
            )
            .children(menu)
    }
}
