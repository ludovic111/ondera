//! The arrangement in a test window: real pointer events through GPUI, real registry
//! commands on a host with no audio device.

use super::{geometry::tests::song, Arrangement};
use crate::{
    app::Ryolune,
    ui::{
        daw::Daw,
        theme::{Mode, Theme},
    },
};
use gpui::{
    point, prelude::*, px, Entity, Modifiers, MouseButton, Pixels, Point, TestAppContext,
    VisualTestContext,
};

fn open(cx: &mut TestAppContext) -> (Entity<Arrangement>, Entity<Daw>, &mut VisualTestContext) {
    cx.update(|cx| {
        cx.set_global(Theme::new(Mode::Dark, true));
        crate::ui::actions::bind(cx);
    });
    let mut session = song();
    session.view.pixels_per_bar = 40.0;
    session.view.scroll_bars = 0.0;
    let app = Ryolune::from_session(session, None);
    let daw = cx.new(|_| Daw::new(app));
    let d = daw.clone();
    let (view, cx) = cx.add_window_view(move |window, cx| Arrangement::new(d, window, cx));
    cx.run_until_parked();
    (view, daw, cx)
}

/// A point of the lanes, by bar and track row (the middle of the row's clip body).
fn lane_point(
    view: &Entity<Arrangement>,
    cx: &mut VisualTestContext,
    bar: f64,
    row: usize,
) -> Point<Pixels> {
    view.update(cx, |v, cx| {
        let b = v.areas.lanes.get();
        let geo = v.geo(cx);
        point(
            b.origin.x + px(geo.x(bar) as f32),
            b.origin.y + px((row as f64 * geo.row + geo.row * 0.6 - v.scroll_y) as f32),
        )
    })
}

fn drag(cx: &mut VisualTestContext, from: Point<Pixels>, to: Point<Pixels>) {
    let none = Modifiers::none();
    cx.simulate_mouse_down(from, MouseButton::Left, none);
    cx.run_until_parked();
    let mid = point((from.x + to.x) / 2.0, (from.y + to.y) / 2.0);
    cx.simulate_mouse_move(mid, MouseButton::Left, none);
    cx.simulate_mouse_move(to, MouseButton::Left, none);
    cx.simulate_mouse_up(to, MouseButton::Left, none);
    cx.run_until_parked();
}

#[gpui::test]
fn dragging_a_region_moves_it_in_one_undo_step(cx: &mut TestAppContext) {
    let (view, daw, cx) = open(cx);
    let from = lane_point(&view, cx, 1.5, 0);
    let to = lane_point(&view, cx, 3.5, 0);
    drag(cx, from, to);
    let start = |cx: &mut VisualTestContext| {
        daw.read_with(cx, |d, _| {
            d.app
                .store
                .session()
                .clips
                .iter()
                .find(|c| c.id == "hook")
                .unwrap()
                .start_bar
        })
    };
    assert_eq!(start(cx), 2.0);
    daw.update(cx, |d, cx| {
        d.fire("history.undo", cx);
    });
    assert_eq!(start(cx), 0.0, "the drag was one step");
    view.read_with(cx, |v, _| {
        assert!(v.drag.is_none());
        assert!(v.lane_overlay.ghost.is_none(), "no ghost left behind");
    });
}

// A drag interrupted by the system (the button came up where the window could not see it)
// left its ghost and its state behind, and the next release committed a move nobody made.
#[gpui::test]
fn a_drag_whose_button_went_missing_is_dropped_not_committed(cx: &mut TestAppContext) {
    let (view, daw, cx) = open(cx);
    let from = lane_point(&view, cx, 1.5, 0);
    let to = lane_point(&view, cx, 3.5, 0);
    let none = Modifiers::none();
    cx.simulate_mouse_down(from, MouseButton::Left, none);
    cx.run_until_parked();
    cx.simulate_mouse_move(to, MouseButton::Left, none);
    cx.run_until_parked();
    assert!(view.read_with(cx, |v, _| v.lane_overlay.ghost.is_some()));
    cx.simulate_mouse_move(to, None, none);
    cx.run_until_parked();
    assert!(view.read_with(cx, |v, _| v.lane_overlay.ghost.is_none()
        && v.drag.is_none()));
    cx.simulate_mouse_up(to, MouseButton::Left, none);
    cx.run_until_parked();
    let start = daw.read_with(cx, |d, _| d.app.store.session().clips[0].start_bar);
    assert_eq!(start, 0.0);
}

#[gpui::test]
fn the_pencil_draws_a_region_where_it_is_dragged(cx: &mut TestAppContext) {
    let (view, daw, cx) = open(cx);
    daw.update(cx, |d, cx| {
        d.run("ui.setTool", serde_json::json!({"tool": "pencil"}), cx);
    });
    cx.run_until_parked();
    let from = lane_point(&view, cx, 6.0, 0);
    let to = lane_point(&view, cx, 9.0, 0);
    drag(cx, from, to);
    let clips = daw.read_with(cx, |d, _| {
        d.app
            .store
            .session()
            .clips
            .iter()
            .map(|c| (c.track_id.clone(), c.start_bar, c.length_bars))
            .collect::<Vec<_>>()
    });
    assert!(clips.contains(&("keys".into(), 6.0, 3.0)), "{clips:?}");
}

#[gpui::test]
fn a_ruler_click_locates_and_a_drag_sets_the_cycle(cx: &mut TestAppContext) {
    let (view, daw, cx) = open(cx);
    let at = |view: &Entity<Arrangement>, cx: &mut VisualTestContext, bar: f64| {
        view.update(cx, |v, cx| {
            let b = v.areas.ruler.get();
            point(
                b.origin.x + px(v.geo(cx).x(bar) as f32),
                b.origin.y + px(6.0),
            )
        })
    };
    let p = at(&view, cx, 2.0);
    cx.simulate_click(p, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(daw.read_with(cx, |d, _| d.app.position), 8.0);
    let (from, to) = (at(&view, cx, 1.0), at(&view, cx, 3.0));
    drag(cx, from, to);
    let t = daw.read_with(cx, |d, _| d.app.store.session().transport.clone());
    assert!(t.cycle);
    assert_eq!((t.cycle_start_bar, t.cycle_end_bar), (1.0, 3.0));
}

#[gpui::test]
fn dragging_a_header_reorders_the_tracks(cx: &mut TestAppContext) {
    let (view, daw, cx) = open(cx);
    let (from, to) = view.update(cx, |v, cx| {
        let b = v.areas.headers.get();
        let row = v.geo(cx).row as f32;
        // Grab the row's empty right end, away from its controls and name.
        let x = b.origin.x + b.size.width - px(4.0);
        (
            point(x, b.origin.y + px(row * 0.3)),
            point(x, b.origin.y + px(row * 2.0)),
        )
    });
    drag(cx, from, to);
    let order = daw.read_with(cx, |d, _| {
        d.app
            .store
            .session()
            .tracks
            .iter()
            .map(|t| t.id.clone())
            .collect::<Vec<_>>()
    });
    assert_eq!(order, ["vox", "keys"]);
}

// The menu selected the clicked track, but the selection reached the store only after the
// rows were judged, so Mute showed the previously selected track's tick.
#[gpui::test]
fn a_track_menu_judges_the_track_it_was_opened_on(cx: &mut TestAppContext) {
    let (view, daw, cx) = open(cx);
    daw.update(cx, |d, cx| {
        d.run("track.select", serde_json::json!({"trackId": "vox"}), cx);
    });
    let p = view.update(cx, |v, _| {
        let b = v.areas.headers.get();
        point(b.origin.x + b.size.width - px(4.0), b.origin.y + px(10.0))
    });
    cx.simulate_mouse_down(p, MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
    let selected = daw.read_with(cx, |d, _| {
        d.app.store.session().view.selected_track_id.clone()
    });
    assert_eq!(selected.as_deref(), Some("keys"));
    assert!(view.read_with(cx, |v, _| v.menu.is_open()));
}

#[gpui::test]
fn the_lane_width_is_reported_to_the_host(cx: &mut TestAppContext) {
    let (view, daw, cx) = open(cx);
    let width = view.read_with(cx, |v, _| {
        f32::from(v.areas.lanes.get().size.width).round() as f64
    });
    assert!(width >= 50.0);
    assert_eq!(daw.read_with(cx, |d, _| d.app.lane_width), width);
}

#[gpui::test]
fn a_region_renamed_in_place_commits_on_enter(cx: &mut TestAppContext) {
    let (view, daw, cx) = open(cx);
    cx.update(|window, cx| {
        view.update(cx, |v, cx| {
            v.edit(
                super::Editing::Clip("hook".into()),
                "Hook".into(),
                window,
                cx,
            );
            v.name_input
                .update(cx, |input, cx| input.set_text("Chorus hook", cx));
        })
    });
    cx.run_until_parked();
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    let name = daw.read_with(cx, |d, _| d.app.store.session().clips[0].name.clone());
    assert_eq!(name, "Chorus hook");
    assert!(view.read_with(cx, |v, _| v.editing.is_none()));
}

#[gpui::test]
fn the_wheel_scrolls_the_song_and_zooms_with_ctrl(cx: &mut TestAppContext) {
    let (view, daw, cx) = open(cx);
    let at = lane_point(&view, cx, 4.0, 0);
    let wheel = |cx: &mut VisualTestContext, dx: f32, dy: f32, modifiers: Modifiers| {
        cx.simulate_event(gpui::ScrollWheelEvent {
            position: at,
            delta: gpui::ScrollDelta::Pixels(point(px(dx), px(dy))),
            modifiers,
            touch_phase: gpui::TouchPhase::Moved,
        });
        cx.run_until_parked();
    };
    // Content moving left: later bars come into view.
    wheel(cx, -80.0, 0.0, Modifiers::none());
    assert_eq!(daw.read_with(cx, |d, _| d.app.scroll), 2.0);
    let before = daw.read_with(cx, |d, _| (d.app.zoom, d.app.scroll));
    wheel(cx, 0.0, 50.0, Modifiers::control());
    let (zoom, scroll) = daw.read_with(cx, |d, _| (d.app.zoom, d.app.scroll));
    assert!(zoom > before.0, "zoomed in");
    // The bar under the pointer (160 px in: bar 6 after scrolling two bars) stays under it.
    assert!(
        (scroll + 160.0 / zoom as f64 - 6.0).abs() < 1e-3,
        "{scroll} {zoom}"
    );
}
