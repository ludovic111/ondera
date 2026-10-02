//! The arrangement's context menus: a region, an empty lane, a track header, the ruler and a
//! marker, the tempo track and the add-track button. A menu first selects what it was opened
//! on, so its rows (Mute's tick, whether Delete can run) judge that, not the old selection.

use super::{geometry, Arrangement, Editing};
use crate::ui::{
    actions,
    theme::{css_color, palette_css, Theme, TRACK_PALETTE},
    widgets::MenuItem,
};
use gpui::{Context, MouseDownEvent, Pixels, Point, SharedString, Window};
use serde_json::{json, Value};

impl Arrangement {
    /// A table action's row, under another label.
    fn action_as(&self, id: &'static str, label: &'static str, cx: &gpui::App) -> MenuItem {
        let mut item = MenuItem::action(id, &self.daw, cx);
        if let MenuItem::Item { label: l, .. } = &mut item {
            *l = SharedString::from(label);
        }
        item
    }
    fn action(&self, id: &'static str, cx: &gpui::App) -> MenuItem {
        MenuItem::action(id, &self.daw, cx)
    }
    /// A row that runs one registry command.
    fn command(
        &self,
        label: impl Into<SharedString>,
        method: &'static str,
        params: Value,
    ) -> MenuItem {
        let daw = self.daw.clone();
        MenuItem::new(label, move |_, cx| {
            let params = params.clone();
            daw.update(cx, |daw, cx| {
                daw.run(method, params, cx);
            })
        })
    }
    /// A row that starts typing a name or tempo in place.
    fn rename(
        &self,
        label: &'static str,
        editing: Editing,
        text: String,
        cx: &mut Context<Self>,
    ) -> MenuItem {
        let this = cx.entity().downgrade();
        MenuItem::new(label, move |window, cx| {
            let (editing, text, this) = (editing.clone(), text.clone(), this.clone());
            // After the menu has closed and handed focus back, or the field would lose it.
            window.defer(cx, move |window, cx| {
                let _ = this.update(cx, |this, cx| this.edit(editing, text, window, cx));
            });
        })
    }
    fn open(
        &mut self,
        items: Vec<MenuItem>,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.drag.is_some() {
            return;
        }
        self.menu.open(items, at, window, cx);
    }

    pub(super) fn lane_menu(
        &mut self,
        e: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let s = self.session(cx);
        let geo = self.geo(cx);
        let (x, y) = super::local(self.areas.lanes.get(), e.position);
        let y = y + self.scroll_y;
        if let Some(clip) = geometry::clip_at(&s, &geo, x, y) {
            self.run(vec![("clip.select", json!({ "clipId": clip.id }))], cx);
            let items = vec![
                self.action("openInEditor", cx),
                self.rename(
                    "Rename…",
                    Editing::Clip(clip.id.clone()),
                    clip.name.clone(),
                    cx,
                ),
                MenuItem::Separator,
                self.action("cut", cx),
                self.action("copy", cx),
                self.action("duplicateClip", cx),
                self.action("splitAtPlayhead", cx),
                MenuItem::Separator,
                self.action_as("deleteSelection", "Delete Clip", cx),
                MenuItem::Separator,
                self.action_as("askAgent", "Ask Agent About This Region…", cx),
            ];
            self.open(items, e.position, window, cx);
            return;
        }
        if let Some(track) = geo.row_at(y, s.tracks.len()).map(|i| &s.tracks[i]) {
            if s.view.selected_track_id.as_deref() != Some(track.id.as_str()) {
                self.run(vec![("track.select", json!({ "trackId": track.id }))], cx);
            }
        }
        let items = vec![
            self.action("paste", cx),
            MenuItem::Separator,
            self.action("addAudioTrack", cx),
            self.action("addMidiTrack", cx),
            MenuItem::Separator,
            self.action("importAudio", cx),
            MenuItem::Separator,
            self.action_as("askAgent", "Ask Agent…", cx),
        ];
        self.open(items, e.position, window, cx);
    }

    pub(super) fn track_menu(
        &mut self,
        id: &str,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run(vec![("track.select", json!({ "trackId": id }))], cx);
        let s = self.session(cx);
        let Some(index) = s.tracks.iter().position(|t| t.id == id) else {
            return;
        };
        let track = &s.tracks[index];
        let mut items = vec![
            self.rename(
                "Rename…",
                Editing::Track(track.id.clone()),
                track.name.clone(),
                cx,
            ),
            self.action("muteSelectedTrack", cx),
            self.action("soloSelectedTrack", cx),
        ];
        if !track.is_bus() {
            items.push(self.action("armSelectedTrack", cx));
        }
        if track.kind == "audio" {
            items.push(self.action("cycleMonitorSelectedTrack", cx));
        }
        if !track.is_bus() {
            items.push(self.action("groupSelectedTrack", cx));
        }
        items.push(self.action("duplicateTrack", cx));
        items.push(MenuItem::Separator);
        items.push(
            self.command(
                "Move Up",
                "track.move",
                json!({"trackId": id, "index": index.saturating_sub(1)}),
            )
            .disabled(index == 0),
        );
        items.push(
            self.command(
                "Move Down",
                "track.move",
                json!({"trackId": id, "index": index + 1}),
            )
            .disabled(index + 1 >= s.tracks.len()),
        );
        items.push(MenuItem::Separator);
        let theme = Theme::get(cx);
        for (i, (name, _)) in TRACK_PALETTE.iter().enumerate() {
            let color = palette_css(i);
            items.push(
                self.command(
                    format!("Colour · {name}"),
                    "track.setColor",
                    json!({"trackId": id, "color": color}),
                )
                .checked(track.color == color)
                .swatch(theme.track(&color, i)),
            );
        }
        items.extend([
            MenuItem::Separator,
            self.action("addAudioTrack", cx),
            self.action("addMidiTrack", cx),
            self.action("addBusTrack", cx),
            self.action("removeSelectedTrack", cx),
            MenuItem::Separator,
            self.action_as("askAgent", "Ask Agent About This Track…", cx),
        ]);
        self.open(items, at, window, cx);
    }

    pub(super) fn add_track_menu(
        &mut self,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let items = vec![
            self.action("addAudioTrack", cx),
            self.action("addMidiTrack", cx),
            self.action("addBusTrack", cx),
        ];
        self.open(items, at, window, cx);
    }

    pub(super) fn ruler_menu(
        &mut self,
        e: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let s = self.session(cx);
        let geo = self.geo(cx);
        let (x, y) = super::local(self.areas.ruler.get(), e.position);
        let marker = geometry::marker_at(&s, &geo, x, y, &self.flag_widths.borrow());
        if let Some(m) = marker {
            let here = actions::playhead_bar(self.daw.read(cx), true);
            let theme = Theme::get(cx).clone();
            let taken = s
                .markers
                .iter()
                .any(|o| o.id != m.id && (o.bar - here).abs() < geometry::SAME_BAR);
            let mut items = vec![
                self.command("Go to Marker", "marker.goto", json!({"markerId": m.id})),
                self.rename(
                    "Rename Marker…",
                    Editing::Marker(m.id.clone()),
                    m.name.clone(),
                    cx,
                ),
                self.command(
                    "Move to Playhead",
                    "marker.move",
                    json!({"markerId": m.id, "bar": here}),
                )
                .disabled(taken || (m.bar - here).abs() < geometry::SAME_BAR),
                self.command(
                    "Cycle This Section",
                    "marker.cycleSection",
                    json!({"markerId": m.id}),
                ),
                MenuItem::Separator,
                self.command(
                    "Colour · Default",
                    "marker.setColor",
                    json!({"markerId": m.id}),
                )
                .checked(m.color.is_none())
                .swatch(theme.marker),
            ];
            for (i, (name, _)) in TRACK_PALETTE.iter().enumerate() {
                let color = palette_css(i);
                let swatch = css_color(&color).unwrap_or(theme.marker);
                items.push(
                    self.command(
                        format!("Colour · {name}"),
                        "marker.setColor",
                        json!({"markerId": m.id, "color": color}),
                    )
                    .checked(m.color.as_deref() == Some(color.as_str()))
                    .swatch(swatch),
                );
            }
            items.extend([
                MenuItem::Separator,
                self.command("Delete Marker", "marker.remove", json!({"markerId": m.id})),
            ]);
            self.open(items, e.position, window, cx);
            return;
        }
        let bar = geometry::snap(&s, geo.bar(x), false).max(0.0);
        let taken = s
            .markers
            .iter()
            .any(|m| (m.bar - bar).abs() < geometry::SAME_BAR);
        let items = vec![
            self.command("Add Marker Here", "marker.add", json!({ "bar": bar }))
                .disabled(taken),
            self.action("addMarker", cx),
            MenuItem::Separator,
            self.action("previousMarker", cx),
            self.action("nextMarker", cx),
            self.action("cycleSection", cx),
        ];
        self.open(items, e.position, window, cx);
    }

    pub(super) fn tempo_menu(
        &mut self,
        e: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let s = self.session(cx);
        let geo = self.geo(cx);
        let b = self.areas.tempo.get();
        let (x, y) = super::local(b, e.position);
        let h = f32::from(b.size.height) as f64;
        let hit = geometry::tempo_at(&s, &geo, x, y, h);
        let clear = self
            .command("Clear Tempo Changes", "tempo.clear", json!({}))
            .disabled(s.tempo_changes.is_empty());
        let hide = self.action_as("toggleTempoTrack", "Hide Tempo Track", cx);
        let point = hit.and_then(|bar| {
            s.tempo_changes
                .iter()
                .find(|p| (p.bar - bar).abs() < geometry::SAME_BAR)
                .copied()
        });
        let items = if let Some(p) = point {
            vec![
                self.command(
                    "Ramp from the Previous Tempo",
                    "tempo.set",
                    json!({"bar": p.bar, "bpm": p.bpm, "ramp": !p.ramp}),
                )
                .checked(p.ramp),
                self.rename(
                    "Set Tempo…",
                    Editing::Tempo(p.bar),
                    geometry::bpm_label(p.bpm),
                    cx,
                ),
                MenuItem::Separator,
                self.command("Delete Tempo Change", "tempo.remove", json!({"bar": p.bar})),
                MenuItem::Separator,
                clear,
                hide,
            ]
        } else {
            let bar = geometry::snap(&s, geo.bar(x), false);
            let bpb = s.beats_per_bar();
            let bpm = s.tempo_map().bpm(bar * bpb).round();
            let taken = s
                .tempo_changes
                .iter()
                .any(|p| (p.bar - bar).abs() < geometry::SAME_BAR);
            let (label, at) = if hit.is_some() {
                ("Set Starting Tempo…", 0.0)
            } else {
                (
                    "Set Tempo…",
                    crate::ui::format::tempo_source_bar(&s, bar * bpb),
                )
            };
            let current = geometry::tempo_points(&s, None)
                .iter()
                .find(|p| (p.bar - at).abs() < geometry::SAME_BAR)
                .map_or(s.transport.tempo, |p| p.bpm);
            vec![
                self.command(
                    "Add Tempo Change Here",
                    "tempo.set",
                    json!({"bar": bar, "bpm": bpm}),
                )
                .disabled(bar <= geometry::SAME_BAR || taken),
                self.rename(label, Editing::Tempo(at), geometry::bpm_label(current), cx),
                MenuItem::Separator,
                clear,
                hide,
            ]
        };
        self.open(items, e.position, window, cx);
    }
}
