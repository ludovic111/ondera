# Ondera native Rust branch

Since 0.4 the window is Tauri 2 with the React renderer in `frontend/` (`docs/TAURI_MIGRATION.md`);
`desktop/src/web.rs` hosts it and the egui painting code below is kept as reference only. UI work
happens in `frontend/src`: `state/actions.ts` is the one table behind menus, shortcuts, the command
palette and the shortcut sheet; window panels (mixer, help, settings…) are toggled through
`ui.showPanel` so the CLI, MCP and agent can drive them. Check with `npm --prefix frontend test`,
`npm --prefix frontend run build`, then the Rust checks. The marketing site is `site/`.

Themes (0.6): `frontend/src/theme` is the only place visual values live. `schema.ts` types a theme,
`materials.ts` holds the physical recipes against a light model, and `modern.ts`, `skeuo.ts`,
`aero.ts` each return a full `ThemeSpec` for `dark` and `light`. `tokens.ts` keeps live groups that
`setTheme()` refills (canvas code reads them at paint time); `applyAppearance(theme, mode)` emits the
CSS properties and sets `data-theme` / `data-mode`. Skeuomorphic dark is the design source, value
for value. Add a token to `schema.ts` and to all three themes, never a colour in a component;
structure that only one theme needs goes in `theme/<theme>.css` and takes its colours from that
theme's `vars`. `appearance.test.ts` enforces contrast on all six variants: fix the palette, not
the threshold. Stock plugin panels are `components/plugin` (`response.ts` mirrors the engine DSP).
`npm --prefix frontend run dev` in a plain browser serves a fixture song through `src/dev/mockHost.ts`
(`?theme=&mode=&panel=`); run `node scripts/gen-site-tokens.mjs` after changing a theme so the
site follows.

The owner requested a complete Rust rewrite on 2026-09-12, including the interface.
This supersedes the former Electron / TypeScript architecture in `legacy/CLAUDE.md`.

- `desktop/`: native egui interface with wgpu, no webview. `desktop/src/theme.rs` holds every visual
  token and the skeuomorphic material recipes from `design/Ondera Arrangement.dc.html` (spec sheet 02:
  raised, pressed, lit, groove, well, knob, fader cap, clip slab, glass, plus faceplate, screw,
  brushed, switch, LED button, plate). Paint with those helpers; never introduce colours, gradients
  or shadows elsewhere. Floating windows use `window_frame()` and wrap their content in `plate()`;
  modals use `dialog_frame()`. `chrome.rs` is the title bar, transport, browser and inspector;
  `timeline.rs` the arrangement; `editor.rs` the region editor; `agents.rs` the agent panel at the
  right edge (380 px open, 32 px rail closed): a conversation with streamed replies and one card
  per tool call, a Changes tab with Revert/Redo, and one prompt. `agent/` is the runtime (providers
  `anthropic`, `openai`, `cli` for Codex and Claude Code; tool calls execute on the interface thread
  through `run_control_command`). `settings.rs` is the Settings window (⌘,) over
  `engine::settings::Settings`; apply changes through `apply_settings`, never by writing fields.
  The window draws its own title bar (native macOS title bar hidden, traffic lights overlaid at
  the left). Keep every panel to what the design frame shows; anything extra goes into a menu or
  Settings, not the panel. Fonts are Manrope and IBM Plex Mono (OFL) bundled in
  `desktop/assets/fonts`.
- `engine/`: pure Rust command store, session model, DSP, audio devices and documents.
  `engine/src/control.rs` is the public command registry (`control_app.rs` holds the view, preset,
  settings, audio, ui, app and agent families); `control/wire.rs` the loopback protocol. `tools/`
  builds `ondera-cli` and `ondera-mcp` as thin clients of that registry, and `desktop/src/control.rs`
  serves it from the window between frames, implementing `Host::live` for window-only actions
  (screenshots, panels, devices, updates, the agent). A new user-facing action goes into the
  registry so the window, the CLI, MCP and the built-in agent get it together; the CLI help, MCP
  tool list and agent tools are generated from it. Agent permissions (`settings.agent.permissions`)
  are enforced in `run_control_command` for every agent-flagged request.
- `sdk/` is `ondera-plugin`: the `Plugin` trait, DSP primitives and the frozen C ABI (`ffi.rs`,
  ABI version 1, never change a `repr(C)` layout without bumping it). `engine/src/host/native.rs`
  loads libraries and adapts vtables to `Editor`/`Processor`; `engine/src/stock.rs` is written on
  the trait and served through the same vtables. `plugins/gain` is the example bundle used by tests.
- Preferences live in `engine/src/settings.rs` (`settings.json`, 0600, secrets masked by
  `redacted()`); presets in `engine/src/preset.rs`; recovery snapshot naming in `engine/src/recovery.rs`.
- 0.7 work (decided 2026-09-19, owner delegated the calls): the registry is the contract for GUI
  parity, so a new window interaction lands as a command first (`control_edit.rs` for edits and
  `session.batch`, `control_plugins.rs` for the plugin library, live-only ones in
  `desktop/src/control.rs`) and the frontend calls that name; do not add private `web.*` handlers
  for things a script could want. Plugins are browsed by sound folder: `control_plugins.rs` files
  every descriptor (`automatic_folder`, ordered `EFFECT_RULES`), favourites/recents/overrides live in
  `settings.plugins`, and the frontend maps a folder to a `fam*` colour token in `theme/families.ts`.
  Motion is a theme token group (`motion` in `schema.ts`); `theme/motion.css` is the only place that
  says what moves, components opt in with `data-motion`, and drags are never eased. The count-in
  lives in the renderer (`Renderer::count_in`), the capture callback drops frames while
  `Telemetry::counting_in` is set, and `InputMeter` holds the input open only while an audio track
  is armed. Native plugin calls are panic-guarded in `sdk/src/ffi.rs` (`Guarded`); test plugins with
  `ondera_plugin::testing::Bench`. Continuous controls are coalesced in `NativeStore`
  (`CONTINUOUS`); add a command there when a new dial dispatches on pointer move.
- Every persistent UI edit dispatches `store::Command`. Keep drag previews local and group
  continuous edits with `Store::set_gesture`. Preserve undo and source/clip alignment.
- No allocations, deallocations, blocking, I/O or logging in the audio callback. Compile graphs
  on workers, transfer through bounded queues, and reclaim old graphs outside the callback.
- Plugins (`engine/src/plugin.rs`, `engine/src/host/`, `engine/src/stock.rs`): every insert and
  instrument is an `Instance` (main-thread `Editor` + audio-thread `Processor`). Processors live
  in the callback's `Rack`, keyed by insert id, and survive renderer rebuilds; a new `Renderer`
  must `adopt` the old one so held notes are released or chased. Create, activate, save state
  and destroy plugins on the UI thread only; unmount through the queue and wait for retirement
  before dropping an editor. Parameter values are document state (`Insert.params`) so they undo;
  external plugin state is captured into `Insert.blob` on save and bounce. Scan bundles only in
  the `--scan-plugin` child process. New stock DSP goes in `stock.rs` behind the same traits.
- Platform streams belong to their owning workers; never force Send with an unsafe impl.
- File operations must preserve the old file on failure. Keep v1 `.ondera` loading covered by tests.
- Run fmt, clippy with warnings denied, and workspace tests. Check a real native window after
  UI changes. Distinguish tests, builds, actual device checks and public signing/notarization.
- Work on a branch. Do not merge or publish a release without the owner's request.
- Releases: bump the workspace `version` in `Cargo.toml`, add `docs/releases/X.Y.Z.md`, then push
  a matching `vX.Y.Z` tag. `.github/workflows/release.yml` builds all platforms, writes and signs
  `SHA256SUMS` (Ed25519, secret `ONDERA_SIGNING_KEY`, public key in
  `desktop/assets/update-signing.pub`) and publishes the GitHub release that `desktop/src/update.rs`
  installs from after verifying the signature, the download host and the new binaries' versions.
  Keep the asset names in `update::asset_name` and the workflow in sync. The secret key stays in
  `~/.ondera/keys/update-signing.key` on the owner's machine; never commit it. Builds are ad-hoc
  signed, not notarized.
- `legacy/` is reference material, not the active implementation.
