# Ondera native Rust branch

The owner requested a complete Rust rewrite on 2026-09-12, including the interface.
This supersedes the former Electron / TypeScript architecture in `legacy/CLAUDE.md`.

- `desktop/`: native egui interface with wgpu, no webview. `desktop/src/theme.rs` holds every visual
  token and the skeuomorphic material recipes from `design/Ondera Arrangement.dc.html` (spec sheet 02:
  raised, pressed, lit, groove, well, knob, fader cap, clip slab, glass). Paint with those helpers;
  never introduce colours, gradients or shadows elsewhere. `chrome.rs` is the title bar, transport,
  browser and inspector; `timeline.rs` the arrangement; `editor.rs` the region editor. Fonts are
  Manrope and IBM Plex Mono (OFL) bundled in `desktop/assets/fonts`.
- `engine/`: pure Rust command store, session model, DSP, audio devices and documents.
  `engine/src/control.rs` is the public command registry; `control/wire.rs` the loopback
  protocol. `tools/` builds `ondera-cli` and `ondera-mcp` as thin clients of that registry, and
  `desktop/src/control.rs` serves it from the window between frames. A new user-facing action
  goes into the registry so the window, the CLI and agents get it together; the CLI help and
  MCP tool list are generated from it.
- Every persistent UI edit dispatches `store::Command`. Keep drag previews local and group
  continuous edits with `Store::set_gesture`. Preserve undo and source/clip alignment.
- No allocations, deallocations, blocking, I/O or logging in the audio callback. Compile graphs
  on workers, transfer through bounded queues, and reclaim old graphs outside the callback.
- Platform streams belong to their owning workers; never force Send with an unsafe impl.
- File operations must preserve the old file on failure. Keep v1 `.ondera` loading covered by tests.
- Run fmt, clippy with warnings denied, and workspace tests. Check a real native window after
  UI changes. Distinguish tests, builds, actual device checks and public signing/notarization.
- Work on a branch. Do not merge or publish a release without the owner's request.
- `legacy/` is reference material, not the active implementation.
