# Tauri frontend migration

This branch replaces the egui window with Tauri 2 and a React/TypeScript renderer. Its visual
reference is the existing Ondera interface: the same transport, browser, arrangement, MIDI
editor, inspector and agent rail. It is not a redesign or a new document format.

## Ownership and audio

`desktop/src/web.rs` keeps the existing `Ondera` host on the OS main thread. This is also where
external plugin editors must live. WebView commands enter the existing validated command
registry. The engine still owns the session, undo/redo, plugin states, decoding, recording,
exports and the real-time audio callback. CLI and MCP use that same host.

The frontend has a read-only document mirror. Native-generated IDs are resolved before
subsequent queued edits; failed commands are shown and do not optimistically mutate the
session. Pointer gestures group edits into a single undo step. Document revisions publish
snapshots with plugin state blobs omitted. Small position/meter packets are checked at 30 Hz
and published on change. A slow main thread can have at most one queued host tick.

Waveforms use peaks from the Rust audio library, with each source's actual peak sampling rate.
They are refreshed when decoded buffers change. The frontend never synthesizes audio or fake
waveforms. Keyboard note-offs are also sent on focus loss.

The former egui painting methods and dependencies remain in the Rust host as migration
reference, alongside existing regression tests. Tauri does not invoke their window or painting
loop. Removing this inactive code is a separate cleanup, not required to run the Tauri window.

## Building

```sh
npm --prefix frontend ci
npm --prefix frontend run build
cargo build --release --workspace --locked
bash scripts/package-macos.sh  # macOS only
```

The default `custom-protocol` feature embeds `frontend/dist` in the executable. A packaged app
needs neither Node nor a Vite server. Rebuild the frontend before compiling Rust after UI edits.
For hot reload, use two terminals:

```sh
npm --prefix frontend run dev
cargo run --no-default-features
```

Tauri's [system prerequisites](https://v2.tauri.app/start/prerequisites/) apply: WebKitGTK 4.1 on
Linux and WebView2 on Windows. CI builds the frontend before the locked Cargo checks on macOS
ARM/Intel, Ubuntu and Windows. Existing signing and publication gates still apply.

## Verification

```sh
npm --prefix frontend test
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python3 scripts/verify-song.py  # Python 3.12+
```

For the live DAW workflow, set `ONDERA_CONTROL` to an isolated test window's discovery file and
run `scripts/verify-song.py --live`. This replaces that test window's document. Use a separate
`ONDERA_DATA_DIR` when launching it to keep real preferences, plugin cache and recovery files
out of QA runs.

Local evidence is written to the ignored `artifacts/tauri-migration/` directory. Build success
and the headless song test do not prove UI interaction parity or cross-platform runtime parity.
Those require the live test, visual inspection, and the corresponding CI/running platforms.
No quantified performance gain or loss is claimed without equivalent release-build measurements.

### Local run, 2026-09-13

- 193 Rust tests and 10 frontend bridge tests passed; locked Clippy with warnings denied passed.
- The optimized macOS ARM64 application was built and packaged with an ad-hoc signature.
- The song verification passed both headless and through CLI/MCP against an isolated running
  Tauri window: five tracks, 17 clips, save/reopen, MIDI interchange, stereo and stem exports.
- The standalone window was observed with real waveforms, MIDI notes, the inspector and
  transport. `ui.status` reported `frontendReady: true`, no error, and Ready with Vite stopped;
  the WebView URL was `tauri://localhost`. The owner resumed interactive testing in the preview.
  A complete manual parity checklist is still pending; compilation and the live command
  workflow alone do not prove every gesture or dialog.
- Linux, Windows and macOS Intel are assigned to CI; local macOS results do not validate those
  platforms. Subsequent installation and release preparation are recorded in the 0.4.0 notes.
