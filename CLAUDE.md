# ryolune native Rust branch

Renamed from Ondera on 2026-09-28 (0.11, owner's decision; the brand is written in lowercase).
Compatibility kept on purpose, do not "clean it up": `document::LEGACY_EXTENSION` (`.ondera`)
and the `ondera-session` format still load; `host::scan::data_dir` adopts the old `Ondera`
data folder once and native plugin folders under the old name are still scanned; the SDK
exports `ondera_plugin_entry*` beside `ryolune_plugin_entry*` (`LEGACY_ENTRY_SYMBOL*`) and the
host accepts both; `plugins/abi1-fixture` keeps its old crate name, symbol and id; the GitHub
secret is still named `ONDERA_SIGNING_KEY`; release notes before 0.11 and `legacy/` keep the old
name as history.

Since 0.4 the window is Tauri 2 with the React renderer in `frontend/` (`docs/TAURI_MIGRATION.md`);
`desktop/src/web.rs` hosts it and the egui painting code below is kept as reference only. UI work
happens in `frontend/src`: `state/actions.ts` is the one table behind menus, shortcuts, the command
palette and the shortcut sheet; window panels (mixer, help, settings…) are toggled through
`ui.showPanel` so the CLI, MCP and agent can drive them. Check with `npm --prefix frontend test`,
`npm --prefix frontend run build`, then the Rust checks. The public page is lsuite.xyz/ryolune, in the lsuite repo (ludovic111/lsuite); ryolune.com redirects there with the same path, so `/support` and `/download/<platform>` links keep working. `site/` is the former standalone site, no longer deployed; its launch film (`site/video/`) is made in `marketing/` (see its README).

Theme (0.12, owner's decision 2026-10-01: "one theme, dark or light, ultra premium"): the six
themes of 0.6-0.11 are gone. `frontend/src/theme` is the only place visual values live. `schema.ts`
types the theme, `ryolune.ts` returns the full `ThemeSpec` for `dark` (graphite, design source) and
`light` (porcelain): one neutral ladder, crisp 1 px edges, a fine top highlight and a short drop,
one accent (lunar gold, amber by day), meters mint/amber, channel keys `mute`/`solo`/`danger`.
`tokens.ts` keeps live groups that `setTheme(mode)` refills (canvas code reads them at paint time);
`applyAppearance(mode)` emits the CSS properties and sets `data-theme="ryolune"` / `data-mode`.
Structure the material classes cannot say goes in `theme/ryolune.css`, coloured by the `--ryo-*`
vars from `ryolune.ts` (frame, focus, displays, primary keys). Add a token to `schema.ts` and both
modes, never a colour in a component. `engine/src/settings.rs` `THEMES` is `["ryolune"]`; any
older `interface.appearance` migrates to it and keeps its mode. `appearance.test.ts` enforces
contrast on both modes: fix the palette, not the threshold. Stock plugin panels are
`components/plugin` (`response.ts` mirrors the engine DSP). `npm --prefix frontend run dev` in a
plain browser serves a fixture song through `src/dev/mockHost.ts` (`?mode=&panel=`); run
`node scripts/gen-site-tokens.mjs` after changing the dark mode (`site/` tokens and captures are reused
by the lsuite page; they wear only dark, light appears as a capture).

The owner requested a complete Rust rewrite on 2026-09-12, including the interface.
This supersedes the former Electron / TypeScript architecture in `legacy/CLAUDE.md`.

- `desktop/`: native egui interface with wgpu, no webview. `desktop/src/theme.rs` holds every visual
  token and the skeuomorphic material recipes from `design/ryolune Arrangement.dc.html` (spec sheet 02:
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
  builds `ryolune-cli` and `ryolune-mcp` as thin clients of that registry, and `desktop/src/control.rs`
  serves it from the window between frames, implementing `Host::live` for window-only actions
  (screenshots, panels, devices, updates, the agent). A new user-facing action goes into the
  registry so the window, the CLI, MCP and the built-in agent get it together; the CLI help, MCP
  tool list and agent tools are generated from it. Agent permissions (`settings.agent.permissions`)
  are enforced in `run_control_command` for every agent-flagged request.
- `sdk/` is `ryolune-plugin`: the `Plugin` trait, DSP primitives and the frozen C ABI (`ffi.rs`,
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
  `ryolune_plugin::testing::Bench`. Continuous controls are coalesced in `NativeStore`
  (`CONTINUOUS`); add a command there when a new dial dispatches on pointer move.
- 0.8 (released 2026-09-23; the owner delegated lossy export and MIDI CC scope): input monitoring
  is a bounded ring (`device::monitor_ring`) from the one input stream to the output callback's `MonitorTap`, which resamples, waits for one input buffer before it
  starts, skips a backlog and counts drops and underruns in `Telemetry`; `Renderer::render_monitored`
  mixes it into monitoring tracks ahead of their inserts with a 5 ms ramp. `Track.monitor` is
  absent from the file when off. Built-in microphone into built-in speakers is decided by device
  names (`device::feedback_risk`) and stays muted until `audio.allowSpeakerMonitoring`.
  `settings.audio.bufferFrames` sets both device buffers. FLAC is `engine/src/flac.rs`, written by
  hand (fixed predictors, Rice, mid/side, MD5) so no dependency was added; a mix takes its
  container from the path, stems from `ExportOptions::container`. MP3 was not added: no clean
  encoder fits the licence and the build; Ogg Vorbis is the lossy format. Plugin ABI 2 never touches an ABI 1
  layout: `ryolune_plugin_entry_v2` returns `PluginVTable2 { size, base, .. }`, the macro exports
  both symbols, the host tries v2 then v1, `sdk/src/ffi.rs` asserts every frozen offset at compile
  time, and `plugins/abi1-fixture` (no SDK dependency, never "update" it) is loaded by
  `engine/tests/abi1_plugin.rs`. Native state is saved from a main-thread model instance and
  restored by swapping a freshly loaded instance in on the audio thread. The window's private
  handlers are the allow-lists in `desktop/src/web.rs` tests (`PRIVATE_HANDLERS`, `PRIVATE_TAURI`);
  anything else is a registry command, and work that waits on the network or renders offline is
  a live job through `Ryolune::start_worker`. The clipboard and the lane width belong to the host
  (`Host::clipboard`, `Host::lane_width`). Browser rows fold channel layouts
  (`control_plugins::layout_of`); when extending `EFFECT_RULES`, diff every plugin's folder before
  and after on a real library, because a new word in an early group steals from later ones.
- 0.8 additions. Input: `device::LiveInput` is the only input stream (meter + monitor ring +
  takes), opened by `Ryolune::poll_input`; monitoring (`LiveInput::monitor`) and takes
  (`LiveInput::record` returns a `Recorder`) attach through a control ring, and the callback
  (`InputCallback::process`) hands them back through a garbage ring its worker frees. Test the
  callback with `live_input(..)`, no device needed; it reopens only for a new device or buffer size,
  never during a take. Parameters: `plugin::ParamChange` carries `frame`, `Rack::set_param_at` keeps
  changes sorted; a processor with `Processor::timed_params()` (CLAP, VST3, native ABI 2) gets the
  whole block, any other is split at each change by the Rack; the renderer sends a lane's value on
  frame 0, on breakpoint frames and every `render::AUTOMATION_GRAIN` (32) frames while it moves.
  Events: `Processor::process` takes `&[plugin::Event]` (notes, controllers, bend, pressure); ABI 1
  gets notes only; stock instruments come from `TABLES_V2` (ABI 2), stock effects stay ABI 1. Clip
  controllers are `model::Controller` in `ClipData::Midi.controllers` (absent when empty), helpers in
  `controllers.rs` (`window`, `reverse`, `playback`, `recorded`), commands in
  `control_controllers.rs`; the renderer remembers what each instrument was last sent (`applied`),
  chases only differences on locate and rests bend/pedal/pressure at stop. Live MIDI controllers
  are `Message::RoutedControl`. CLAP gets controllers as MIDI only when its note port speaks MIDI;
  VST3 through `IMidiMapping` (`Shared.midi_map`), one queue point per value; AU through
  `Event::to_midi`. Lane UI: `canvas/controllerLane.ts`, `components/editor/ControllerLane.tsx`,
  `ui.showPanel panel=controllers`. Inserts hear a MIDI track's controllers (never its notes) when
  `Processor::accepts_events` says so (native ABI 2, CLAP note port, VST3 event bus, AU music
  effect); they ride the same per-track list, so chase and rest reach them. Channels: `Note` and
  `Controller` carry `channel` 0-15, absent when 0; a lane is (kind, number, channel); the
  renderer keeps voices and `applied` per channel and `Message::RoutedNote` carries it. Poly
  pressure is `ControllerKind::PolyPressure` (`number` = key) inside `controllers`, but the file
  writes it to its own `polyPressure` list (`ClipDataFile`/`ClipDataOut` in `model.rs`) so older
  versions and the lane UI never see the kind; it is played, chased onto notes a locate restarts
  (`Renderer::chase_poly`, keys without a note go to 0), rested at stop and ranked after note-ons. VST3 mapped
  controllers also reach the edit controller through `Shared.mapped` (atomics, read in `idle`).
  Audio clips carry `fade_in`/`fade_out` (seconds), `fade_curve`
  and `gain_db`, absent when default; build them with `ClipData::audio(src, offset)`;
  `Command::PutClip` clamps fades (`model::clamp_fades`) and `render.rs` `clip_envelope` applies
  fades, gain and the 3 ms edge ramp per sample; `frontend/src/core/fade.ts` mirrors the curves.
  `Session.markers` (bar order, absent when empty) change only through `Command::PutMarker` /
  `RemoveMarker` from `control_arrange.rs`; marker navigation is gated by the agent's transport
  permission. Ogg is `Container::Ogg` through `vorbis_rs` (C built by `cc`, no system packages),
  block by block at `ExportOptions::quality` (0 to 1, default 0.6); `Container::for_path` refuses
  extensions ryolune does not write. Plugin folders: `control_plugins::AutoFolders` decides per
  product (vendor, kind, name without layout): name first, then `PRODUCTS`/`PRIORITY_RULES`, then
  `EFFECT_RULES`, the category last. `Store` restores redo on `cancel_gesture`; background captures
  skip over redo or an open gesture. `NativeStore.request()` is `run()` without the error dialog, for
  forms that show their own errors. `atomic_write` keeps the target's mode (0644 when new) and writes
  through symlinks. Engine regression tests live in `engine/tests/regressions.rs`. The agent's
  Changes list records only document edits that did not come from the window.
- Agent control (decided 2026-09-25): an agent starts from `session.overview`
  (`control_overview.rs`, bounded; the built-in agent gets a compact one each turn) and
  `ui.state` (live). `control::call` runs `control_refs::resolve` first, so every `trackId`,
  `clipId` and `markerId` also takes a unique name, and a wrong one lists what exists.
  Plugin parameters and programs live in `control_params.rs`: read through
  `Host::loaded_editor` (the window's instance) or a fresh one, set by name, plain value,
  0-1 or display text (`Editor::parse_text`), programs through `Editor::programs` (VST3
  program-change parameter, AU factory presets loaded into a fresh instance and saved as
  state). `docs/AGENT_PARITY.md` is the audit of window interactions against the registry;
  `engine/tests/agent_parity.rs` fails when an `actions.ts` action has no entry in
  `docs/agent-parity.json`, when the frontend sends an unknown name, or when a command has
  no real description. A new window interaction adds its row there.
- 0.9 (2026-09-25, owner asked to "improve the app" and delegated): stock voices are scaled by
  `dsp::HEADROOM` (0.5, -6 dB) because loops and chords clipped at unity; old songs play 6 dB
  quieter on stock instruments, accepted. `store::empty()` starts Drums (Drum Machine), Bass
  (Analog Bass) and Vocals (audio); strips exist only once edited, so set them with
  `entry().or_default()`. `Host::loaded_editor(insert)` serves the window's instance only when
  its plugin id and blob match the document (a new song reuses insert keys; reconcile runs a
  frame later), otherwise callers read a fresh instance. A rebuilt `Renderer` glides a
  sounding clip's envelope from the old graph (`glide_from`, 5 ms), never touching playback
  without a rebuild. `plugin.list` rows fold formats and layouts (vendor + name; CLAP, VST3, AU
  order, others under `formats`); search also matches `folder_words` and stock descriptions.
  Side panels shrink to `size.*Min` floors so the arrangement keeps `arrangementMin` at the
  1120 px minimum. `ui.screenshot` finishes running animations first (`settleMotion` in
  `main.tsx`). Docs: `docs/COMMANDS.md` and `docs/SHORTCUTS.md` are generated and checked by
  tests (`RYOLUNE_BLESS=1` regenerates); `USER_GUIDE.md`, `AI_CONTROL.md` and `DEVELOPMENT.md`
  are written by hand, keep them true when behaviour changes.
- 0.10 (2026-09-27, owner asked for the next update and delegated): tempo changes live in
  `Session.tempo_changes` (`TempoPoint { bar, bpm, ramp }`, bar order, after bar 0, absent when
  empty); `transport.tempo` is the starting tempo. `engine/src/tempo.rs` `TempoMap` (steps and
  ramps linear in beats, closed-form seconds both ways) is the only place beats become seconds:
  use `Session::bars_seconds` / `seconds_bars` / `tempo_map()`, never `60 / tempo`. The renderer
  advances `position` by the segment's tempo each frame, keeps `seconds` for audio clips
  (`Scheduled.start_seconds`), resnaps it at segment ends and fills `frame_beats` for
  automation and the click. `frontend/src/core/tempo.ts` mirrors the map; the tempo track is
  `TempoRow.tsx` + `canvas/tempoLane.ts` (`ui.showPanel panel=tempo`). Commands are
  `control_tempo.rs`; MIDI export writes ramps as sixteenth steps of equal duration. Meter
  changes inside a song are not supported. Buses: a track of kind `bus` (no clips, never armed)
  sums tracks routed to it (`Track.output`) or sending to it (`Send.bus`; sends 0/1 default to
  bus-a/bus-b, up to `MAX_SENDS` 4). Tracks feed bus tracks, bus tracks feed A, B and the Stereo
  Out only (`Session::validate_routing`); `prune_routing` on track removal. The renderer's
  `order` runs tracks then buses; track outputs to the Stereo Out/A/B wait in `early_*` buffers
  delayed by the slowest bus so every path meets (PDC stages: tracks, buses, A/B, master).
  Commands in `control_routing.rs`. Audio Unit CF objects from `AudioUnitGetProperty` are the
  caller's to release (factory preset arrays, PresentPreset names); `Editor::current_program`.
- 0.12 agent and generation (owner asked 2026-10-01 for better agent integration, more agents, no
  Rhythm Lab, and generating music and instruments through provider APIs; delegated). Providers:
  `settings::Provider` has 13 variants; every one after Anthropic runs through `agent/openai.rs`
  (Chat Completions) with `Settings::base_url` / `api_key`; `Provider::hosted()` holds the fixed
  address, env names, key page and `strict` (Mistral and DeepSeek get `max_tokens`, no stream
  usage). The frontend mirror is `providers` in `components/agent/connection.ts`; keep both,
  `SECRET_PATHS` and `validate`'s key loop in sync. Outside agents: `agent/clients.rs` builds the
  per-client MCP recipes (`agent.mcp`) and install links (`agent.openClient`, refused to agents).
  Generation: `control_generate.rs` shapes requests from the song (loops get tempo/key and are
  fitted to their bars), keeps results in `<data dir>/generated` with a JSON note, and places them
  (`place_audio` in control.rs, or Sample Keys via `load_sample`); the network call is
  `desktop/src/generate.rs` (ElevenLabs, Stability, fal, custom contract in docs/AI_CONTROL.md),
  run by `start_generation` in desktop control.rs and placed on the interface thread from
  `LiveWait::Generation`. `settings.generation` is agent-protected like `agent.*` and `control.*`;
  `permissions.generation` gates `generate.audio`. Sample Keys (`sample_keys.rs`, stock index 34)
  keeps its sound in its state: the insert blob is the native host's `{values, state}` document
  (`sample_keys::insert_blob`). The agent panel tabs are Chat, Generate, Changes, Takes; Rhythm Lab's
  UI is gone, its commands stay.
- Parallel worktrees must not share `CARGO_TARGET_DIR`: cargo can link another worktree's
  `ryolune-engine` into yours. The site: `site/server.js` swaps each `?v=` on `.js`/`.css` for a
  content hash (immutable caching), serves `/sitemap.xml` and hides its own sources; fonts are
  self-hosted in `site/fonts` and the CSP allows only the site's origin; each release updates the
  site's "New in" section, hero pill, changelog, limits and version (checklist in `site/README.md`).
- Money (owner's decision, 2026-09-29): ryolune is MIT and free forever, every update included; the
  only income is optional donations, once or monthly, through GitHub Sponsors behind
  lsuite.xyz/ryolune/support (`SUPPORT_URL` in desktop control.rs, since 2026-10-01; the retired
  `site/server.js` defaulted ryolune.com/support to `SPONSORS_URL`). Nothing
  is sold or locked, so copy says donate or sponsor, never pay, price or checkout. The app asks once,
  after the third export (`SUPPORT_AFTER_EXPORTS`); a quiet Sponsor key sits at the right of the
  title bar (`app.openGuide guide=support`), and `.github/FUNDING.yml` shows GitHub's Sponsor button.
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
- File operations must preserve the old file on failure. Keep v1 `.ryolune` loading covered by tests.
- Run fmt, clippy with warnings denied, and workspace tests. Check a real native window after
  UI changes. Distinguish tests, builds, actual device checks and public signing/notarization.
- Work on a branch. Do not merge or publish a release without the owner's request.
- Releases: bump the workspace `version` in `Cargo.toml`, add `docs/releases/X.Y.Z.md`, then push
  a matching `vX.Y.Z` tag. `.github/workflows/release.yml` builds all platforms, writes and signs
  `SHA256SUMS` (Ed25519, secret `RYOLUNE_SIGNING_KEY`, public key in
  `desktop/assets/update-signing.pub`) and publishes the GitHub release that `desktop/src/update.rs`
  installs from after verifying the signature, the download host and the new binaries' versions.
  Keep the asset names in `update::asset_name` and the workflow in sync. The secret key stays in
  `~/.ryolune/keys/update-signing.key` on the owner's machine; never commit it. Builds are ad-hoc
  signed, not notarized.
- `legacy/` is reference material, not the active implementation.

## lsuite: bring ryolune up to the suite standard (next session; notes updated 2026-10-01)

ryolune is part of **lsuite** (lowercase), the free open-source creative suite with kimchi
(video) and zenith (hub). Two documents in ludovic111/lsuite (locally `../lsuite/`) are the
contract: `STANDARD.md` (every action a command, CLI + MCP + built-in agent on one registry,
signed auto-update, apps that work together) and `design/DESIGN.md` (the shared design system,
live at lsuite.xyz/design). ryolune is the reference implementation of the standard.

Done: 0.12.0 released and notarized (2026-10-01; the Apple developer agreement had to be
accepted). The notarization key is now "ryolune notarization" (`APPLE_API_KEY_*` secrets); the old
"Ondera notarization" key is revoked. The public page is lsuite.xyz/ryolune; ryolune.com is a
Porkbun 301 there with the path kept.

Still to do:

- [ ] **Design system** (`../lsuite/design/`): ryolune's signature color is **teal, hue 185**
      (`--ls-ryolune-*`, accent `#00c5b4` dark / `#009586` light), matching its icon. 0.12 uses a
      lunar gold accent: move the accent to the teal scale (playhead, focus, lit keys, what the agent
      touched), keep meters/mute/solo/record colors. Map `frontend/src/theme` onto the `--ls-*`
      tokens (copy `tokens.css`; the theme layer stays the only place values live), put the chrome
      (title bar, browser, inspector, agent panel, transport, menus, dialogs) on the three glass
      tiers over `.ls-backdrop`, keep the arrangement, editors and mixer strips solid, use macOS
      window vibrancy (Tauri `window-vibrancy`), and keep `appearance.test.ts` passing with the
      glass tiers in the contrast check. Redraw the app icon from the lsuite template.
- [ ] **Discovery**: write `~/.lsuite/apps/ryolune.json` at start (version, paths of the app,
      `ryolune-cli`, `ryolune-mcp`, bridge port while running, data folder). ryolune is the first
      app to do it, so design the format (small, versioned) and document it in `../lsuite/STANDARD.md`.
- [ ] **Hand-offs**: export a mix or stems straight onto a kimchi project's audio track
      (read kimchi's discovery file, use its CLI/MCP), and accept a cut from kimchi (audio,
      length, markers) to score. Each as a registry command.
- [ ] **Shared command names** with the other apps where the concept matches (`app.version`,
      `app.checkUpdates`, `history.*`, `export.*`); add aliases rather than breaking scripts.
- [x] **Site** (0.12 page done 2026-10-01): keep updating lsuite.xyz/ryolune
      (`../lsuite/ryolune/index.html`) with every release: what's new, features, captures
      (`../lsuite/assets/img/ryolune/`). The version shown comes from the latest GitHub release.
- [ ] Point `SUPPORT_URL` (desktop/src/control.rs) at `https://lsuite.xyz/ryolune/support` in the
      next release.

When done, tick these, and update the status table at the end of `../lsuite/STANDARD.md`.
