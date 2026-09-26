# Developing Ondera

How the code is laid out, how to build and check it, and how a release is published. The rules
contributors (human or AI) follow are in [`CLAUDE.md`](../CLAUDE.md) at the repository root.

## Layout

```
frontend/  React + TypeScript interface: controls, CSS materials, canvas arrangement and editors,
    │      themes (frontend/src/theme), the action table behind menus and shortcuts
    │      Tauri commands, document snapshots, transport and meter events
desktop/   Tauri 2 window: owns the document and audio, settings, the agent runtime
    │      (desktop/src/agent), native plugin windows, live-only commands, updates
tools/     ondera-cli and ondera-mcp: thin clients of the command registry, live or on a file
engine/    command registry (control*.rs), session model and validation, undo/redo, documents,
    │      DSP and stock plugins, plugin hosts (native ABI, CLAP, VST3, Audio Units), scanning,
    │      settings, presets, MIDI, devices, rendering and export
    └──    CPAL output callback → renderer + plugin rack → system audio
           CPAL input callback → meter, monitor ring and takes → workers
sdk/       ondera-plugin: the Plugin trait, DSP primitives and the frozen C ABI
plugins/   example native plugin bundle (plugins/gain) and the ABI 1 fixture
site/      the marketing site (plain HTML/CSS/JS behind a small Node server)
legacy/    the previous Electron/TypeScript source, reference only, never built
```

**The command registry is the contract.** Every user-facing action is a command in
`engine/src/control.rs` and its `control_*.rs` families (window-only ones are served by
`desktop/src/control.rs`). The window, `ondera-cli`, `ondera-mcp` and the built-in agent all
call it; the CLI help, the MCP tool list, the agent's tools and [COMMANDS.md](COMMANDS.md) are
generated from it. A new action lands as a command first, then the interface calls it.

**Audio thread rules.** The callback performs no allocation, deallocation, locking, file access
or logging. Graphs are compiled on workers and handed over through bounded queues; old graphs and
unmounted plugins are reclaimed off the audio thread. Plugin processors live in a rack the
callback owns; a rebuilt renderer adopts the old one (transport, held notes, controller state,
the envelope of sounding clips) so edits during playback are seamless.

**Capacity** is explicit: 128 tracks, 256 simultaneous arrangement events, 32 voices per stock
instrument, 8 inserts per strip, 1024 rack slots, 512 MiB per decoded source, 1 GiB of session
audio and 4-hour exports.

## Build

Install Rust 1.88 or newer and Node.js 24 (Node builds the interface; the packaged app does not
need it). The desktop crate embeds `frontend/dist`, so build the frontend first:

```sh
npm --prefix frontend ci
npm --prefix frontend run build
cargo run --release
```

Use release builds for real-time audio. Platform prerequisites:

- **macOS**: the Xcode command-line tools.
- **Windows**: Visual Studio C++ build tools and the Windows SDK, MSVC Rust toolchain, and the
  Edge WebView2 runtime.
- **Ubuntu/Debian**:

  ```sh
  sudo apt-get install build-essential pkg-config libasound2-dev libudev-dev \
    libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxi-dev \
    libxrandr-dev libxinerama-dev libegl1-mesa-dev libgl1-mesa-dev \
    libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libdbus-1-dev \
    libwebkit2gtk-4.1-dev libssl-dev librsvg2-dev
  ```

  File dialogs use the desktop portal (`xdg-desktop-portal` and a backend).

The executables are `target/release/ondera`, `ondera-cli` and `ondera-mcp`; keep them together.
`bash scripts/package-macos.sh` builds `dist/Ondera.app` and a zip (ad-hoc signed, not notarized).

## Checks

Run all of these before a pull request:

```sh
npm --prefix frontend test
npm --prefix frontend run build
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Point the Rust tests at scratch settings so they never touch your own:
`ONDERA_SETTINGS=/tmp/ondera-test/settings.json ONDERA_DATA_DIR=/tmp/ondera-test/data`.

Generated documentation is checked by tests. After changing a command or a shortcut:

```sh
ONDERA_BLESS=1 cargo test -p ondera-tools --test command_docs     # docs/COMMANDS.md
ONDERA_BLESS=1 npm --prefix frontend test -- shortcutsDoc          # docs/SHORTCUTS.md
```

After changing a theme, `node scripts/gen-site-tokens.mjs` refreshes the site's tokens (the site
uses the Skeuomorphic dark theme only).

## Checking the real window

The interface can be checked in a plain browser: `npm --prefix frontend run dev` serves a
fixture song through `src/dev/mockHost.ts` (`?theme=<id>&mode=dark|light&panel=mixer|settings|…`).
For the real window, run the app with a scratch data folder and drive it with the CLI:

```sh
export ONDERA_DATA_DIR=/tmp/ondera-check ONDERA_CONTROL=/tmp/ondera-check/control.json
ONDERA_NO_UPDATE=1 target/debug/ondera &
target/debug/ondera-cli app.info                       # repeat until it answers
target/debug/ondera-cli session.overview
target/debug/ondera-cli ui.screenshot --path /tmp/ondera-check/window.png
target/debug/ondera-cli app.quit --discard true
```

The app binary also validates and renders without a window:

```sh
cargo run --release -- --validate song.ondera
cargo run --release -- --bounce song.ondera mix.wav
cargo run --release -- --scan-plugins
cargo run --release -p ondera-engine --example probe -- "clap:com.example.plugin"
```

## Releases

1. Bump `version` in the workspace `Cargo.toml` and add `docs/releases/X.Y.Z.md`.
2. Update the site's release content (checklist in `site/README.md`).
3. Merge to `main`, then push a matching tag: `git tag vX.Y.Z && git push origin vX.Y.Z`.

The `Release` workflow builds macOS (arm64 and x86_64), Linux and Windows, writes and signs
`SHA256SUMS` (Ed25519, repository secret `ONDERA_SIGNING_KEY`, public key in
`desktop/assets/update-signing.pub`) and publishes the GitHub release. Installed apps verify the
signature, the download host and the new binaries' versions before replacing themselves. A new
key pair comes from `ondera --release-keygen <file>`. Builds are ad-hoc signed, not notarized.

## Website

`site/` deploys to Railway from `main`. Run it locally with `npm --prefix site start`. See
`site/README.md` for its structure, design references and the per-release checklist.
