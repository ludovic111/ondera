//! The arrangement's tool bar: the pointer, pencil and scissors, the grid and cycle range,
//! follow, and the zoom slider. Glass tier 1, like the rest of the chrome.

use super::{geometry, Arrangement};
use crate::ui::{
    actions::Do,
    theme::{arrange, layout, radius, size, Theme},
    widgets::{Button, Phase, Slider},
};
use gpui::{div, prelude::*, px, AnyElement, App, Context, Window};
use serde_json::json;

const TOOLS: [(&str, &str, &str); 3] = [
    (
        "pointer",
        "toolPointer",
        "Pointer (1): select, move and trim regions",
    ),
    ("pencil", "toolPencil", "Pencil (2): draw MIDI regions"),
    ("scissors", "toolScissors", "Scissors (3): split regions"),
];

fn act(id: &'static str) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut App) {
    move |_, window, cx| window.dispatch_action(Box::new(Do { id }), cx)
}

/// A bar number as the ruler shows it: 1-based, whole when it is.
fn bar_label(bar: f64) -> String {
    let n = bar + 1.0;
    if (n - n.round()).abs() < 1e-6 {
        format!("{}", n.round() as i64)
    } else {
        format!("{n:.2}")
    }
}

impl Arrangement {
    pub(super) fn toolbar(&mut self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::get(cx).clone();
        let app = &self.daw.read(cx).app;
        let s = app.store.session();
        let tool = app.tool;
        let snap = s.transport.snap_division;
        let (start, end) = (s.transport.cycle_start_bar, s.transport.cycle_end_bar);
        let follow = s.view.follow_playhead;
        let zoom = app.zoom as f64;

        let tools = div()
            .flex()
            .gap(px(2.0))
            .p(px(2.0))
            .rounded(px(radius::SM + 1.0))
            .bg(theme.well)
            .border_1()
            .border_color(theme.hairline)
            .children(TOOLS.iter().enumerate().map(|(i, (icon, action, tip))| {
                Button::icon(*icon, icon)
                    .ghost()
                    .compact()
                    .icon_size(12.0)
                    .lit(tool == i)
                    .tooltip(*tip)
                    .on_click(act(action))
            }));
        let info = |label: &'static str, value: String| {
            div()
                .flex()
                .gap(px(5.0))
                .child(div().text_color(theme.text_3).child(label))
                .child(div().text_color(theme.text_2).child(value))
        };
        let daw = self.daw.clone();
        let zoom_slider = Slider::new("zoom", geometry::zoom_to_t(zoom) as f32)
            .default_value(geometry::zoom_to_t(geometry::ZOOM_DEFAULT) as f32)
            .on_change(move |t, phase, _, cx| {
                if phase == Phase::Start && (t as f64 - geometry::zoom_to_t(zoom)).abs() < 1e-6 {
                    return;
                }
                let ppb = geometry::t_to_zoom(t as f64);
                daw.update(cx, |daw, cx| {
                    daw.run("view.set", json!({ "pixelsPerBar": ppb }), cx);
                });
            });

        div()
            .h(px(layout::TOOLBAR))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(10.0))
            .px(px(10.0))
            .bg(theme.glass(1))
            .border_b_1()
            .border_color(theme.line)
            .text_size(px(size::SM))
            .child(tools)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(14.0))
                    .ml(px(6.0))
                    .child(info(
                        "Grid",
                        if snap == 1 {
                            "Bar".into()
                        } else {
                            format!("1/{snap}")
                        },
                    ))
                    .child(info(
                        "Cycle",
                        format!("{} – {}", bar_label(start), bar_label(end)),
                    ))
                    .child(
                        Button::new("follow", if follow { "Follow" } else { "Follow off" })
                            .ghost()
                            .compact()
                            .tooltip("Scroll with the playhead while playing (F)")
                            .on_click(act("followPlayhead")),
                    ),
            )
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .text_color(theme.text_3)
                    .child("Zoom")
                    .child(div().w(px(arrange::ZOOM_RAIL)).flex().child(zoom_slider)),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn bar_labels_are_one_based() {
        assert_eq!(super::bar_label(4.0), "5");
        assert_eq!(super::bar_label(0.5), "1.50");
    }
}
