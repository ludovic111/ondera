# The window (GPUI)

ryolune's window is drawn with GPUI 0.2 (gpui.rs, Zed's GPU interface framework), straight
from Rust: no webview, no JavaScript. The 0.4-0.12 React renderer (`frontend/`) and the egui
painting code before it are gone; their behaviour is what these views reproduce.

## Ownership

- `daw.rs`: `Daw` is the one GPUI entity that owns the host (`crate::app::Ryolune`): document
  store, audio, plugins, the control bridge, the agent. It ticks the host (every frame while
  something moves, ten times a second otherwise) and wakes at once when the bridge or a
  worker calls `Ryolune::wake`. It notifies when anything shown changed (`fingerprint`).
- Every view holds an `Entity<Daw>`. **Read** with `self.daw.read(cx).app` (the session is
  `app.store.session()`, the playhead `app.position`, panels `app.show_*`, the agent
  `app.agents`, …). **Change** through the registry: `daw.run("track.setMute", json!({..}), cx)`
  inside `self.daw.update(cx, |daw, cx| ...)`. That is the same path the CLI, MCP and the
  built-in agent take, so whatever the window does a script can do. A window interaction that
  has no registry command gets one first (in `engine/src/control*.rs`, or a live one in
  `desktop/src/control.rs`), then the view calls it. Never write session fields directly.
- `daw.run` shows a failure in the error dialog; `daw.request` hands the error back for forms
  that show their own.
- Continuous edits (a fader, a knob, a drag in the arrangement) are one undo step: call
  `daw.gesture(true)` on press and `daw.gesture(false)` on release (`widgets::Phase`). Drag
  previews stay local to the view until release when the command would be expensive.
- View-local state (scroll offsets inside a list, hover, a drag in progress, a text field's
  draft) lives in the view's struct. Document state never does.

## Look

- `theme.rs` is the only place colours, radii and sizes live (lsuite design system, ryolune
  teal accent). `Theme::get(cx)` in render. Add a token there, in both modes, never a colour
  in a view. The accent means "yours or active" (selection, focus, playhead, lit keys, what
  the agent touched); states keep their colours (`record`, `danger`, `mute`, `solo`, meters).
  Text on an accent fill uses `accent_fill` + `text_on_accent`.
- Chrome (title bar, transport, browser, inspector, agent panel, toolbars) is glass tier 1:
  `.bg(theme.glass(1))`. Floating things (menus, palette, tooltips, toasts) tier 2, modals
  tier 3 over `theme.scrim` (`widgets::surface(tier, cx)`). The arrangement, editors and mixer
  strips are solid work surfaces (`theme.lane`, `theme.editor`, `theme.bg_raised`).
- Fonts: Manrope (`FONT_UI`, the default) and IBM Plex Mono (`FONT_MONO`) for numbers, time
  and caps labels. Sizes from `theme::size`, radii from `theme::radius`, panel widths from
  `theme::layout`.
- Icons are SVGs in `desktop/assets/icons` (`widgets::icon("play", 12.0, color)`); add one there
  when needed, drawn on a small grid with `#000` fills or strokes (GPUI tints them).

## Building blocks (`widgets/`)

- `Button` (text and/or icon; `ghost`, `primary`, `danger`, `lit`, `lit_color`, `tooltip`,
  `on_click`), `Key` (M/S/R/A channel keys), `Segmented` (tabs), `Switch`, `dot`, `caps`,
  `mono`, `row`, `section`, `rule`, `surface`, `glass`, `text_width`.
- `Knob`, `Fader`, `Slider` (0-1 values, `on_change(value, Phase, window, cx)`), `NumberDrag`
  (drag any element to change a number), `Meter` (LED meter, peaks or `.linear()`).
- `TextInput` (an entity: `cx.new(|cx| TextInput::new(cx).multiline())`, subscribe to
  `InputEvent::{Changed, Submit, Cancel, Blur}`; wrap with `widgets::field`). Its keys only
  bind while it has focus (`TextInput` key context), and single-key shortcuts are off then.
- `MenuHost` + `MenuItem` for popup and context menus (`MenuItem::action(id, &daw, cx)` for a
  table action with its shortcut and check mark); render `host.render(window, cx)` as the
  view's last child. `select_button` for selects, `on_context_menu` for right click.
- `actions.rs` is the one table behind menus, shortcuts, the palette and the shortcut sheet;
  `window.dispatch_action(Box::new(Do { id }), cx)` runs one. `docs/agent-parity.json` maps
  every id to registry commands (checked by `engine/tests/agent_parity.rs`).
- `format.rs`: bar/beat, SMPTE, decibels, fader taper, snapping, note names.
- Canvas drawing: `gpui::canvas(prepaint, paint)` with `window.paint_quad`, `paint_path`
  (`PathBuilder`), `paint_shadows`, and text through `window.text_system().shape_line`.
  The arrangement, piano roll, controller and tempo lanes, waveforms and the EQ curve are
  canvases; their hit testing is done in the view from the same geometry.

## Checking

- `cargo build -p ryolune`, `cargo test -p ryolune`, `cargo clippy -p ryolune --all-targets
  -- -D warnings`, `cargo fmt --all`. Toolchain 1.88 (`rustup` override on the repository).
- Look at the real window: run with a scratch profile so your settings and recovery files
  are untouched, and capture it:
  `RYOLUNE_SETTINGS=/tmp/x/settings.json RYOLUNE_DATA_DIR=/tmp/x/data RYOLUNE_NO_UPDATE=1
  ./target/debug/ryolune --screenshot /tmp/x/shot.png` (opens the demo song, captures after
  ~90 frames and quits). `ryolune-cli` drives a running window (`ui.showPanel`,
  `ui.screenshot`, any command) for states the startup capture does not show.
