# Ondera

A native Rust digital audio workstation for macOS, Linux and Windows. The desktop interface,
command store, undo history, synthesizers, effects, audio I/O and file operations are Rust.
The application does not embed a browser, Electron, React, JavaScript or Web Audio.

This branch is the first native port, under validation. It preserves the existing arrangement /
piano-roll / inspector workflow and reads version-1 `.ondera` sessions. Synth and effect DSP has
been rewritten: an old session's composition and imported audio are preserved, but its mix will
not sound bit-identical to the former Web Audio engine. See [migration status](docs/RUST_MIGRATION.md).

## Build and run

Install Rust 1.88 or newer. Node and pnpm are not required.

```sh
cargo run --release
```

Use release mode for real-time audio. Debug builds prioritize diagnostics over audio performance.
On macOS, install the Xcode command-line tools. On Windows, install Visual Studio's C++ build
tools and Windows SDK; use the MSVC Rust toolchain. On Ubuntu/Debian, install:

```sh
sudo apt-get install build-essential pkg-config libasound2-dev libudev-dev \
  libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxi-dev \
  libxrandr-dev libxinerama-dev libegl1-mesa-dev libgl1-mesa-dev \
  libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libdbus-1-dev
```

Linux file dialogs use the desktop portal (`xdg-desktop-portal` plus the appropriate desktop
backend). Graphics use wgpu with Metal, DirectX 12, Vulkan or OpenGL ES backends. Audio uses
CPAL's system backend and the default output/input selected in system settings. Device changes
are applied with **Audio > Reconnect output**.

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --release --locked
```

The executable is `target/release/ondera` (`ondera.exe` on Windows). On macOS,
`bash scripts/package-macos.sh` creates `dist/Ondera.app` with its microphone usage declaration,
and a ZIP. This is an ad-hoc signed test app, without Developer ID notarization.

## Working in Ondera

- Add audio or instrument tracks above the arrangement. Rename in the inspector; right-click a
  track name to reorder, recolour or delete it. M / S / A toggle mute, solo and arm.
- Double-click a sound or loop in the library. Double-click an empty MIDI lane to create a
  region, or use the Pencil tool. Click/drag in the piano roll to draw notes; drag a note to move
  it or its right edge to resize. Right-click for velocity and deletion. Step mode toggles notes.
  Edit > Quantize / Transpose act on the selected region.
- Drag regions to move, including to another track of the same type. Drag edges to trim. Use
  Scissors or Cmd/Ctrl+T to split at the playhead; Cmd/Ctrl+D duplicates the selected region.
- Space plays/stops, Enter returns to the start, 0 stops, C toggles cycle, K toggles the click.
  Drag across the ruler to set the cycle. F toggles following; Z fits the arrangement.
- **Play live**: Cmd/Ctrl+K turns on musical typing (A–L play notes, Z / X change octave) on the
  selected instrument track. Audio > MIDI input connects a hardware keyboard; notes go straight to
  the audio thread.
- **Record**: arm audio tracks to capture the input device, arm instrument tracks to capture
  MIDI from the keyboard or musical typing. Enable Rec, press Play, stop to keep the take. Audio
  takes need microphone permission; hardware latency compensation and loop takes are not
  implemented.
- File > Import Audio or drop files to import mono/stereo WAV, AIFF, FLAC, MP3, Ogg/Vorbis,
  AAC/M4A and other formats supported by Symphonia. Unsupported codecs return an error.
- **Mixing**: every track, the A / B aux buses and the master strip have eight insert slots, and
  tracks have two sends. Effects come from the stock library or from scanned CLAP, VST3 and
  Audio Unit plugins; instruments likewise. Click an insert for its parameters, *Open plugin
  window* for the native editor. See [docs/PLUGINS.md](docs/PLUGINS.md).
- Audio > Output / Input device pick the interface; MIDI input picks the controller.
- Cmd/Ctrl+S saves, Cmd/Ctrl+O opens, Cmd/Ctrl+I imports, Cmd/Ctrl+B exports stereo 48 kHz /
  24-bit WAV. Undo/redo uses Cmd/Ctrl+Z / Shift+Z. A drag or text edit is one undo gesture.

## Command line and MCP

The window, `ondera-cli` and `ondera-mcp` are peers: all three dispatch the same commands to
the same store, so anything a person can do in the interface, a script or an AI agent can do
with the same undo history. The registry lives in `engine/src/control.rs`; the CLI help and the
MCP tool list are generated from it. Bars and beats are zero-based; note times are beats
relative to their clip.

```sh
cargo build --release --workspace     # target/release/ondera-cli and ondera-mcp
ondera-cli commands                   # every command with its parameters
ondera-cli help clip.create
```

### Live control of the running app

While `ondera` runs it listens on 127.0.0.1 and writes the port and a random token to
`~/.ondera/control.json` (readable only by you; `$ONDERA_CONTROL` overrides the path). Clients
read that file and connect; each command is applied on the interface thread between frames,
never concurrently with a drag. Start the app with `--no-control` to refuse connections.

```sh
ondera-cli session.info
ondera-cli track.add --kind midi --name Bass --instrument "Sub Bass 808"
ondera-cli clip.create --trackId <id> --startBar 0 --lengthBars 2 \
    --notes '[{"start":0,"length":1,"pitch":36},{"start":2,"length":1,"pitch":43}]'
ondera-cli transport.play
ondera-cli history.undo
```

### Files without the app

```sh
ondera-cli --file song.ondera session.new
ondera-cli --file song.ondera clip.addLoop --name "Four Floor 124"
ondera-cli --file song.ondera session.bounce --path mix.wav
ondera-cli --file song.ondera session.importAudio --path vocal.wav --startBar 4
```

`--file` hosts the session in the CLI process and saves atomically after every change, so the
file is always the state. Playback needs the app; rendering does not.

### MCP server

`ondera-mcp` is a stdio Model Context Protocol server. Each command is a tool (`track.add` is
`track_add`) and the session is readable as `ondera://session`, `ondera://session/info` and
`ondera://catalog`. With the app running it controls the app live; otherwise it hosts a session
in its own process. `--live`, `--headless` and `--file <path>` choose explicitly. Clips and
notes an agent creates are marked and drawn with the agent accent in the interface, and
`history.undo` reverts them like any other edit.

```sh
claude mcp add ondera -- /path/to/ondera/target/release/ondera-mcp
```

For Claude Desktop or another client: `{"mcpServers": {"ondera": {"command": "/path/to/ondera-mcp"}}}`.

Recording is not exposed through control: arm a track in the window and press Rec. The
validation and export flags remain on the app binary:

```sh
cargo run --release -- --validate song.ondera
cargo run --release -- --bounce song.ondera mix.wav
cargo run --release -- --scan-plugins
cargo run --release -- --plugins
cargo run --release -p ondera-engine --example benchmark
cargo run --release -p ondera-engine --example probe -- "clap:com.example.plugin"
```

Bouncing instantiates the session's plugins on the worker thread and restores their saved
state. The public command registry is shared by the desktop, CLI and MCP server.

## Architecture

```
desktop/  egui native interface, wgpu rendering, native file dialogs, plugin windows
    │     typed commands + immutable session snapshots
tools/    ondera-cli and ondera-mcp: shared registry clients, live or on a file
engine/   command registry, session validation, undo/redo, document and audio library
    │     plugin hosts (stock, CLAP, VST3, Audio Units), scanning cache, MIDI input
    │     prepared graphs, bounded lock-free queues, atomic telemetry
    └──   CPAL output callback → renderer + plugin rack → system audio
          CPAL input callback → bounded recording queue → worker
```

The audio callback performs no heap allocations, deallocations, file access or mutex locking.
Graph creation, decoding, generation and exports run off the UI/audio callback. Plugin
instances live in a rack that the callback owns; renderers are rebuilt on every edit and adopt
the previous renderer's transport and held notes, so replacements are click-free without
re-instantiating anything. Old graphs and unmounted processors are reclaimed on the main thread.
Rendering runs at the device's negotiated sample rate in blocks of at most 256 frames. Meter data
is atomic; the GUI repaints at approximately 30 Hz during playback and less frequently while
idle. The offline renderer shares the DSP code and the plugin hosts.

Capacity is explicit: 128 tracks, 256 simultaneous arrangement events, 32 voices per stock
instrument, 8 inserts per strip, 1024 rack slots, 512 MiB per decoded source, 1 GiB of declared
session audio, and 4-hour export. These are implementation bounds, not a promise that every
device can sustain maximum load.

`legacy/` preserves the previous Electron/TypeScript source for comparison and regression
fixtures. It is outside the Cargo workspace and is never built or loaded by the native app.
`design/` retains the original visual reference. `desktop/src/theme.rs` contains the native
colour and material tokens.

## License

MIT. Copyright Ludovic Marie. See [LICENSE](LICENSE).

## Website

`site/` is the marketing site (plain HTML, CSS and JS behind a dependency-free Node server). It
deploys to Railway from that folder on every push. Its `tokens.css` and `tokens.js` are generated
from the preserved design tokens in `legacy/packages/app/src/theme/tokens.ts` with
`node scripts/gen-site-tokens.mjs`; run `npm --prefix site start` to serve it locally.
The website is independent of the native Cargo build.
