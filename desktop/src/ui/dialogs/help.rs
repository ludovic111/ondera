//! The shortcut sheet (Help > Shortcuts and Help, ⌘/). It is built from the action table, so
//! it cannot drift from what the keyboard, the menus and the palette do, then adds the keys
//! musical typing plays and what the mouse does in the arrangement.

use super::modal;
use crate::ui::{
    actions::{self, ACTIONS},
    theme::{radius, size, Theme, FONT_MONO},
};
use gpui::{div, prelude::*, px, relative, AnyElement, App, SharedString};

/// A section of the sheet: its title and (what, keys) rows.
pub(crate) struct Group {
    pub title: &'static str,
    pub rows: Vec<(SharedString, SharedString)>,
}

/// Which section an action belongs to, in the order the sheet shows them; the last one
/// takes what the others leave.
type Belongs = fn(&str) -> bool;

const GROUPS: [(&str, Belongs); 6] = [
    ("Transport", |id| {
        matches!(
            id,
            "togglePlay"
                | "stop"
                | "record"
                | "cycle"
                | "returnToStart"
                | "rewind"
                | "forward"
                | "metronome"
        )
    }),
    ("Editing", |id| {
        matches!(
            id,
            "undo"
                | "redo"
                | "copy"
                | "cut"
                | "paste"
                | "deleteSelection"
                | "duplicateClip"
                | "splitAtPlayhead"
                | "openInEditor"
        ) || id.starts_with("transpose")
    }),
    ("Tracks", |id| {
        id.ends_with("Track") && !id.starts_with("toggle")
    }),
    ("Markers", |id| {
        id.ends_with("Marker") || id == "cycleSection"
    }),
    ("Session", |id| {
        matches!(
            id,
            "newSession"
                | "openSession"
                | "save"
                | "saveAs"
                | "importAudio"
                | "exportAudio"
                | "settings"
                | "quit"
                | "musicalTyping"
        )
    }),
    ("View and tools", |_| true),
];

/// Musical typing: the home row and the row above it are a keyboard from C.
const TYPING: [(&str, &str); 4] = [
    ("White keys, C to E", "A S D F G H J K L ;"),
    ("Black keys", "W E T Y U O P"),
    ("Octave down / up", "Z / X"),
    ("Turn musical typing on or off", "⌘K"),
];

/// What the mouse does where it is not obvious.
const MOUSE: [(&str, &str); 9] = [
    ("Draw a region", "Pencil tool, drag in a lane"),
    ("Open a MIDI region in the editor", "Double-click it"),
    ("Resize a region", "Drag its edges"),
    ("Set the cycle", "Drag in the ruler"),
    ("Fade an audio region", "Drag its top corners"),
    ("Move a marker", "Drag it"),
    ("Rename a marker", "Double-click it"),
    ("Import audio", "Drop files from your file manager"),
    (
        "Fine adjustment, default value",
        "Shift-drag, double-click a control",
    ),
];

/// Every action with a shortcut, in its section.
pub(crate) fn groups() -> Vec<Group> {
    let mut taken = vec![false; ACTIONS.len()];
    let mut out: Vec<Group> = GROUPS
        .iter()
        .map(|(title, belongs)| {
            let mut rows = vec![];
            for (i, def) in ACTIONS.iter().enumerate() {
                if taken[i] || def.keys.is_empty() || !belongs(def.id) {
                    continue;
                }
                if let Some(keys) = actions::shortcut_label(def.id) {
                    taken[i] = true;
                    rows.push((def.label.into(), keys.into()));
                }
            }
            Group { title, rows }
        })
        .collect();
    let typing = TYPING
        .iter()
        .map(|(what, keys)| {
            let keys = if keys.starts_with('⌘') {
                actions::shortcut_label("musicalTyping").unwrap_or_else(|| keys.to_string())
            } else {
                keys.to_string()
            };
            (SharedString::from(*what), SharedString::from(keys))
        })
        .collect();
    out.push(Group {
        title: "Musical typing",
        rows: typing,
    });
    out.push(Group {
        title: "Mouse",
        rows: MOUSE
            .iter()
            .map(|(what, how)| ((*what).into(), (*how).into()))
            .collect(),
    });
    out.retain(|g| !g.rows.is_empty());
    out
}

/// The sheet's content: an introduction, the sections in two columns and the version.
pub(crate) fn content(cx: &App) -> Vec<AnyElement> {
    let theme = Theme::get(cx).clone();
    let palette = actions::shortcut_label("commandPalette").unwrap_or_default();
    let mixer = actions::shortcut_label("toggleMixer").unwrap_or_default();
    let intro = format!(
        "Press {palette} for the command palette: every menu command, by name. {mixer} swaps the region editor for the mixer. {} is Ctrl on Windows and Linux.",
        if cfg!(target_os = "macos") { "⌘" } else { "Ctrl" }
    );
    let section = |group: Group| {
        div()
            .flex()
            .flex_col()
            .w(relative(0.5))
            .pr(px(20.0))
            .pb(px(14.0))
            .child(
                div()
                    .pb(px(4.0))
                    .font_family(FONT_MONO)
                    .text_size(px(10.5))
                    .text_color(theme.text_3)
                    .child(group.title.to_uppercase()),
            )
            .children(group.rows.into_iter().map(|(what, keys)| {
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.0))
                    .min_h(px(24.0))
                    .border_b_1()
                    .border_color(theme.hairline)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(size::BASE))
                            .text_color(theme.text_2)
                            .child(what),
                    )
                    .child(
                        div()
                            .flex_none()
                            .px(px(6.0))
                            .py(px(1.0))
                            .rounded(px(radius::XS))
                            .bg(theme.well)
                            .border_1()
                            .border_color(theme.hairline)
                            .font_family(FONT_MONO)
                            .text_size(px(size::SM))
                            .text_color(theme.text)
                            .child(keys),
                    )
            }))
    };
    vec![
        modal::text(intro, cx).into_any_element(),
        div()
            .flex()
            .flex_wrap()
            .items_start()
            .children(groups().into_iter().map(section))
            .into_any_element(),
        modal::note(format!("ryolune {}", crate::update::current_version()), cx).into_any_element(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shortcut_appears_once_in_its_section() {
        let groups = groups();
        let shown: Vec<&str> = groups
            .iter()
            .flat_map(|g| g.rows.iter().map(|(what, _)| what.as_ref()))
            .collect();
        for def in ACTIONS.iter().filter(|d| !d.keys.is_empty()) {
            let count = shown.iter().filter(|label| **label == def.label).count();
            assert_eq!(count, 1, "{} should be on the sheet once", def.id);
        }
        let titles: Vec<&str> = groups.iter().map(|g| g.title).collect();
        assert_eq!(
            &titles[..5],
            ["Transport", "Editing", "Tracks", "Markers", "Session"]
        );
        let transport = &groups[0];
        assert!(transport.rows.iter().any(|(what, _)| what == "Play / Stop"));
        assert!(titles.contains(&"Musical typing") && titles.contains(&"Mouse"));
    }
}
