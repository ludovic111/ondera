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
- Drag regions to move, including to another track of the same type. Drag edges to trim. Use
  Scissors or Cmd/Ctrl+T to split at the playhead; Cmd/Ctrl+D duplicates the selected region.
- Space plays/stops, Enter returns to the start, 0 stops, C toggles cycle, K toggles the click.
  Drag across the ruler to set the cycle. F toggles following; Z fits the arrangement.
- File > Import Audio or drop files to import mono/stereo WAV, AIFF, FLAC, MP3, Ogg/Vorbis,
  AAC/M4A and other formats supported by Symphonia. Unsupported codecs return an error.
- Arm an audio track, disable Cycle, enable Rec and press Play to capture a linear take.
  Microphone permission belongs to the operating system. The take is placed at the first input
  callback's playhead position; hardware latency compensation and loop takes are not implemented.
- Four inserts per channel, two effect sends, fader and stereo pan are available in the inspector.
- Cmd/Ctrl+S saves, Cmd/Ctrl+O opens, Cmd/Ctrl+I imports, Cmd/Ctrl+B exports stereo 48 kHz /
  24-bit WAV. Undo/redo uses Cmd/Ctrl+Z / Shift+Z. A drag or text edit is one undo gesture.

## Headless tools

These commands work without a window or an audio device:

```sh
cargo run --release -- --validate song.ondera
cargo run --release -- --bounce song.ondera mix.wav
cargo run --release -p ondera-engine --example benchmark
```

The command enum in `engine/src/store.rs` is serializable and shared by the UI and headless
Rust clients. A public CLI command registry, MCP connection and third-party plugin hosting are
not implemented; the former app also did not provide those features.

## Architecture

```
desktop/  egui native interface, wgpu rendering, native file dialogs
    │     typed commands + immutable session snapshots
engine/   session validation, undo/redo, document and audio library
    │     prepared graphs, bounded lock-free queues, atomic telemetry
    └──   CPAL output callback → native DSP → system audio
          CPAL input callback → bounded recording queue → worker
```

The audio callback performs no heap allocations, deallocations, file access or mutex locking.
Graph creation, decoding, generation and exports run off the UI/audio callback. Old audio graphs
are reclaimed outside the callback and graph replacements crossfade over 5 ms. Rendering runs
at the device's negotiated sample rate. Meter data is atomic; the GUI repaints at approximately
30 Hz during playback and less frequently while idle. The offline renderer shares the DSP code.

Capacity is explicit: 128 tracks, 256 simultaneous arrangement voices, 32 preview voices,
4 inserts/channel, 512 MiB per decoded source, 1 GiB of declared session audio, and 4-hour export.
These are implementation bounds, not a promise that every device can sustain maximum load.

`legacy/` preserves the previous Electron/TypeScript source for comparison and regression
fixtures. It is outside the Cargo workspace and is never built or loaded by the native app.
`design/` retains the original visual reference. `desktop/src/theme.rs` contains the native
colour and material tokens.

## License

MIT. Copyright Ludovic Marie. See [LICENSE](LICENSE).
