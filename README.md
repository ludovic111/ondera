# Ondera

A digital audio workstation for macOS, Linux and Windows. On this migration branch, Tauri
hosts a React/TypeScript interface using the existing Ondera layout and materials. The command
store, undo history, synthesizers, effects, audio I/O and file operations remain in Rust.
The system WebView draws the interface; audio does not run in JavaScript or Web Audio.
See [the Tauri migration](docs/TAURI_MIGRATION.md) for architecture and validation scope.

Version 0.3 adds the fourth way to control Ondera: a built-in agent panel that talks to Codex,
Claude Code, the Anthropic or OpenAI APIs or any compatible endpoint through the same command
registry as the CLI and MCP server; a Settings window; Ondera's own Rust plugin format with the
stock library rewritten on it; full CLI/MCP parity with the window; and signed, verified updates.
You can write MIDI parts, record audio and MIDI, arrange regions, mix through stock, native or
installed plugins, save a project and export a stereo WAV. It reads version-1 `.ondera`
sessions. See the [release notes](docs/releases/0.5.0.md), [the agent](docs/AGENT.md),
[native plugins](docs/NATIVE_PLUGINS.md) and the [migration status and limits](docs/RUST_MIGRATION.md).

## Build and run

Install Rust 1.88 or newer and Node.js 24. Node is needed to build the interface, not to run the packaged app.

```sh
npm --prefix frontend ci
npm --prefix frontend run build
cargo run --release
```

Use release mode for real-time audio. Debug builds prioritize diagnostics over audio performance.
On macOS, install the Xcode command-line tools. On Windows, install Visual Studio's C++ build
tools and Windows SDK; use the MSVC Rust toolchain. On Ubuntu/Debian, install:

```sh
sudo apt-get install build-essential pkg-config libasound2-dev libudev-dev \
  libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxi-dev \
  libxrandr-dev libxinerama-dev libegl1-mesa-dev libgl1-mesa-dev \
  libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libdbus-1-dev \
  libwebkit2gtk-4.1-dev libssl-dev librsvg2-dev
```

Linux file dialogs use the desktop portal (`xdg-desktop-portal` plus the appropriate desktop
backend). Tauri uses WebKit on macOS/Linux and WebView2 on Windows. Windows installations
need the Microsoft Edge WebView2 runtime. Audio uses CPAL's system backend. Choose input, output
and a hardware MIDI controller in **Settings → Audio & MIDI**;
the initial device selection follows the system defaults.

```sh
npm --prefix frontend run build
npm --prefix frontend test
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --release --workspace --locked
```

The executables are `target/release/ondera`, `ondera-cli` and `ondera-mcp` (`.exe` on Windows).
Keep all three together. On macOS,
`bash scripts/package-macos.sh` creates `dist/Ondera.app` with its icon and microphone usage
declaration, and `dist/Ondera-macos-<arch>.zip`. This is an ad-hoc signed test app, without
Developer ID notarization.

## Install and updates

Download the build for your computer from the
[latest release](https://github.com/ludovic111/ondera/releases/latest):
`Ondera-macos-arm64.zip` (Apple Silicon), `Ondera-macos-x86_64.zip` (Intel Mac),
`Ondera-linux-x86_64.zip` (also available as `.tar.gz`) or `Ondera-windows-x86_64.zip`. These include the application and
CLI/MCP companions. Extract all three executables into the same directory on Linux/Windows;
on macOS they are together inside `Ondera.app/Contents/MacOS`. Every release ships `SHA256SUMS`.
The separate `ondera-linux-x86_64` and `ondera-windows-x86_64.exe` assets contain only the desktop
executable and are retained for compatibility with the 0.1 updater.

On macOS, unzip and drag `Ondera.app` into Applications. The app is ad-hoc signed, not
notarized, so the first launch of a browser download is blocked by Gatekeeper: open
**System Settings > Privacy & Security** and choose **Open Anyway**, or clear the quarantine
flag from a terminal:

```sh
xattr -dr com.apple.quarantine /Applications/Ondera.app
```

Ondera checks GitHub for a newer release when it starts and offers it in the title bar and in
**Help > Check for updates…** (Settings > Updates can install automatically). Installing verifies
the release's `SHA256SUMS.sig` Ed25519 signature against the key built into the app, downloads
the asset from this repository only, verifies its checksum and that the new binaries report the
expected version, replaces the installed copy in place and relaunches; the previous copy is kept
until the new one starts. `ondera --update` does the same from a terminal. Set
`ONDERA_NO_UPDATE=1` or pass `--no-update-check` to skip the startup check.
The update replaces the application and both companions together. On Linux/Windows, it rejects
incomplete or unsafe ZIP entries, verifies all three binary versions before replacement, and
restores the previous files if an install step fails. Backups stay until the new version starts.

The release also includes `Ondera-Afterglow-demo.zip`: a complete stock-plugin session, its stereo
WAV mix, MIDI export and verification report. Extract it and open `Afterglow.ondera` to explore
the arrangement, instrument and effect settings, and automation. Audio is embedded in the project.

To publish a release: bump `version` in `Cargo.toml`, merge to `main`, then push a matching tag
(`git tag v0.5.0 && git push origin v0.5.0`). The `Release` workflow builds all four platforms,
writes and signs `SHA256SUMS` and creates the GitHub release; installed apps pick it up on their
next start. Signing needs the `ONDERA_SIGNING_KEY` repository secret: create a key pair once with
`ondera --release-keygen <file>`, commit the printed public key in
`desktop/assets/update-signing.pub` (already done for the current key) and store the secret
file's contents in the secret. Releases without a valid signature are refused by the app.

The exercised native song workflow, test evidence and remaining platform limits are recorded in
[docs/VERIFICATION.md](docs/VERIFICATION.md).

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
  the audio thread. Live CC64 sustain holds notes and extends recorded note lengths until pedal
  release. Retriggering, Stop and input disconnect release held notes; disconnect also stops the
  take while retaining its recorded notes. Pitch bend, MPE and general MIDI CC control are not
  implemented.
- **Record**: arm audio tracks to capture the input device, arm instrument tracks to capture
  MIDI from the keyboard or musical typing. Enable Rec, press Play, stop to keep the take. Audio
  takes need microphone permission; hardware latency compensation and loop takes are not
  implemented. Completed takes retain a separate float32 source WAV in the app data recordings
  folder. Interruptions preserve partial audio with a warning. If it cannot be inserted, the app
  shows its recovery path; Audio > Save recovered take retries a failed disk backup before closing.
  Active recording remains in memory until the take finishes.
- File > Import Audio or drop files to import mono/stereo WAV, AIFF, FLAC, MP3, Ogg/Vorbis,
  AAC/M4A and other formats supported by Symphonia. Unsupported codecs return an error.
- **MIDI files**: File > Import MIDI imports notes into new instrument tracks at the chosen
  bar, optionally using the file's initial tempo and time signature. File > Export MIDI writes
  selected instrument tracks as a Standard MIDI File. MIDI files contain notes, not rendered
  plugin audio; ignored controllers and later tempo changes are reported.
- **Mixing**: every track, the A / B aux buses and the master strip have eight insert slots, and
  tracks have two sends. Effects come from the stock library, from Ondera native plugins built
  with the Rust SDK, or from scanned CLAP, VST3 and Audio Unit plugins; instruments likewise.
  Click an insert for its parameters and presets, *Open plugin window* for the native editor on
  macOS. Static plugin delay compensation aligns parallel track and aux paths. See
  [docs/PLUGINS.md](docs/PLUGINS.md) and [docs/NATIVE_PLUGINS.md](docs/NATIVE_PLUGINS.md).
- **Automation**: Track/View > Automation opens editable lanes for track volume/pan, the master
  fader, and instrument/insert parameters, including aux and master plugins. Add points by
  double-clicking, drag to move, right-click to delete, or enter their beat/value numerically.
  Choose linear or stepped changes and enable/disable reading. Lanes save with the project and
  participate in undo. Touch/latch/write recording and clip-follow automation are not implemented.
- Audio > Output / Input device pick the interface; MIDI input picks the controller.
- Cmd/Ctrl+S saves, Cmd/Ctrl+O opens and Cmd/Ctrl+I imports audio. Cmd/Ctrl+B opens audio export
  settings: stereo mix or selected track stems, 44.1/48/96 kHz, 16/24-bit PCM or 32-bit float,
  optional PCM dither, full arrangement or a bar range, and 0–120 seconds of release tail.
  Stem settings choose track effects/sends and master processing; the result reports paths,
  clipping and warnings. Undo/redo uses Cmd/Ctrl+Z / Shift+Z. A drag or text edit is one undo gesture.
- **Agent**: the panel at the right edge (or `ondera --agents`) is a conversation with the
  built-in agent. Use **Set up agent** to connect a service, then type a musical request and
  press Enter (Shift+Enter for a newline). Watch it inspect and edit the session through
  the same commands as the CLI, one card per tool call and a Changes view with Undo. Settings > Agent
  chooses the provider: Codex CLI or Claude Code CLI with their own sign-in, the Anthropic or
  OpenAI API with a key, or any OpenAI-compatible endpoint; permissions gate file operations,
  transport, replacing the session, settings and application control. See [docs/AGENT.md](docs/AGENT.md).
- **Settings** (⌘,): audio and MIDI devices, interface scale, recovery interval, agent
  provider and permissions, plugin folders, the local bridge, update behaviour. Stored in
  `settings.json` next to the plugin cache and readable through `settings.get`.
- **Recovery**: edited sessions get a separate recovery copy every 30 seconds when no file,
  recording or editing gesture is active. File > Recover session lists generated snapshots;
  selecting one offers to save current edits, then opens a copy. Save chooses its destination.
  The recovery worker does not overwrite your original project. Recovery status and location
  are shown in the picker; snapshots can lag while the app is busy.

The current workflow does not include time stretching, comping or a notation
editor. VST2 and AAX are unsupported. Native external plugin editors are currently macOS-only;
generic parameter controls are available on all platforms. Hardware recording latency is not
measured or compensated. Supported formats do not establish compatibility with every plugin.

## Command line and MCP

The window, `ondera-cli`, `ondera-mcp` and the built-in agent share the command store and undo
history. The 131-command registry covers session files, transport and recording, tracks, clips,
notes, mixing, plugin state and parameters, presets, automation, the view, settings, audio
devices, interface actions (including `ui.screenshot`, so an agent can see the window), the
application and the agent itself. It lives in `engine/src/control.rs` and `control_app.rs`; CLI
help, MCP tools and the agent's tool list are generated from it. Bars and beats are zero-based;
note times are beats relative to their clip.

```sh
cargo build --release --workspace     # target/release/ondera-cli and ondera-mcp
ondera-cli commands                   # every command with its parameters (--json for the schema)
ondera-cli help clip.create
ondera-cli doctor                     # bridge, versions, companions, settings, plugin cache
ondera-cli batch < edits.jsonl        # {"command":"track.add","params":{...}} per line
```

For routine inspection, use `session.info` for counts and transport or `session.inspect` for the
arrangement, mixer and automation without opaque plugin state. Its clips are summaries by default;
use `clip.get` for one region's notes or opt into all notes with `--includeNotes true`. These queries
keep agent context smaller than the full document returned by `session.get`.

```sh
ondera-cli session.inspect
ondera-cli session.inspect --includeNotes true
ondera-cli session.catalog
ondera-cli plugin.list --kind instrument --format vst3 --query bass --limit 20 --offset 0
```

`session.catalog` contains the 34 stock plugins, presets and bundled loops. Search installed plugins
with `plugin.list`: `query` matches name, vendor or ID; `kind` accepts `instrument`/`effect`, and
`format` accepts `stock`/`native`/`clap`/`vst3`/`au`. Pages default to 50 entries, with a maximum
of 200. Pass the returned `nextOffset` as `offset` to continue; `null` means the last page.
`plugin.scan` refreshes the cache and returns a count and scan errors; `plugin.describe` reads one
plugin's parameters without placing it; `preset.list` / `preset.load` apply factory and user presets.

### Live control of the running app

While `ondera` runs it listens on 127.0.0.1 and writes the port and a random token to
`~/.ondera/control.json` (readable only by you; `$ONDERA_CONTROL` overrides the path). Clients
read that file and connect; document commands apply on the interface thread between frames,
with their own undo steps. File decoding, save, export and scanning run on workers; the result
returns after completion. Start the app with `--no-control`, or disable the bridge in Agents,
to refuse connections. Use the copied Agents configuration to target this window explicitly.

```sh
ondera-cli session.info
ondera-cli track.add --kind midi --name Bass --instrument "Sub Bass 808"
ondera-cli clip.create --trackId <id> --startBar 0 --lengthBars 2 \
    --notes '[{"start":0,"length":1,"pitch":36},{"start":2,"length":1,"pitch":43}]'
ondera-cli transport.play
ondera-cli history.undo
ondera-cli plugin.list
ondera-cli strip.setPlugin --trackId master --slot 7 --pluginId stock:Limiter
ondera-cli strip.parameters --trackId master --slot 7
ondera-cli master.setVolume --volume 0.75
ondera-cli session.exportAudio --path mix.wav --sampleRate 96000 --format float32 --tailSeconds 5
ondera-cli session.exportStems --directory song-stems --includeMaster false
ondera-cli session.importMidi --path melody.mid --startBar 4
ondera-cli session.exportMidi --path arrangement.mid
```

`strip.setParameter` takes a parameter ID and its plain value from `strip.parameters`.
`strip.getState` / `strip.setState` read persisted plugin state and restore it; save first to
capture changes from a native plugin editor. Strip commands accept
track IDs or `master`, `bus-a` and `bus-b`; insert slots are zero-based 0–7. Omit `slot` in
`strip.setPlugin` to set a MIDI track's instrument. See `ondera-cli help <command>` for parameters.
Use `track.setArmed` then `transport.record` to record in the live app, and `transport.stop`
to finish. Recording requires an available input and does not support cycle takes.

### Files without the app

```sh
ondera-cli --file song.ondera session.new
ondera-cli --file song.ondera clip.addLoop --name "Four Floor 124"
ondera-cli --file song.ondera session.bounce --path mix.wav
ondera-cli --file song.ondera session.importAudio --path vocal.wav --startBar 4
```

`--file` hosts the session in the CLI process and saves atomically after every change, so the
file is always the state. Playback needs the app; rendering does not.

MIDI interchange accepts SMF type 0/1 files with quarter-note timing and exports type 1 at
960 ticks per quarter note. SMPTE timing and type 2 files are rejected. Imported controller data,
including CC64 sustain, program changes, pitch bend, SysEx and later tempo/meter changes are
reported rather than imported. Live sustain is captured as extended note lengths.
Exports preserve the note arrangement; they do not encode automation lanes or convert audio.

### MCP server

`ondera-mcp` is a stdio Model Context Protocol server. Each command is a tool (`track.add` is
`track_add`); the session, catalog, plugins, presets, settings and app are readable as
`ondera://…` resources, and the `compose`, `mix-review` and `see-the-window` prompts start
common tasks. With the app running it controls the app live; otherwise it hosts a session in its
own process. Use `--live` to require the visible app; `--headless` and `--file <path>` choose an
independent session explicitly. Clips and notes an agent creates are marked and drawn with the
agent accent in the interface, and `history.undo` reverts them like any other edit. Settings >
Agent > Permissions applies to MCP clients too.

```sh
claude mcp add ondera -- /path/to/ondera/target/release/ondera-mcp --live
```

For a client using an `mcpServers` configuration:

```json
{"mcpServers":{"ondera":{"command":"/path/to/ondera-mcp","args":["--live"]}}}
```

The Agents tab copies this configuration with the discovered companion path and this window's
discovery location. Configure the companion's full path if it is installed elsewhere.

A saved project has one editing owner across desktop windows and CLI/MCP file mode. Use `--live`
to share the open desktop session. Ownership uses the sibling `.filename.ondera-lock` file; its
OS lock is released when the last owner exits or crashes. The small lock file intentionally
remains so another process cannot replace its inode. `ondera --validate` remains read-only.

The validation and export flags also remain on the app binary:

```sh
cargo run --release -- --validate song.ondera
cargo run --release -- --bounce song.ondera mix.wav
cargo run --release -- --scan-plugins
cargo run --release -- --plugins
cargo run --release -p ondera-engine --example benchmark
cargo run --release -p ondera-engine --example probe -- "clap:com.example.plugin"
```

Bouncing restores the session's saved plugin states and explicit parameters, then renders
through the same stock and external DSP as playback. Live save/export first capture the
loaded plugin states. Plugin load failures are reported rather than silently dropping an insert.

## Architecture

```
frontend/ React controls, CSS materials, Canvas arrangement and MIDI editor
    │     Tauri commands, document snapshots and transport/meter events
desktop/  Tauri window, Rust document/audio owner, settings, agent and native plugin windows
    │     shared command registry
tools/    ondera-cli and ondera-mcp: shared registry clients, live or on a file
sdk/      ondera-plugin: the Plugin trait, DSP primitives and the frozen C ABI
plugins/  example native plugin bundle (Trim, Tilt EQ)
engine/   command registry, session validation, undo/redo, document and audio library
    │     plugin hosts (stock and native through the SDK ABI, CLAP, VST3, Audio Units),
    │     scanning cache, settings, presets, MIDI input, prepared graphs, bounded queues
    └──   CPAL output callback → renderer + plugin rack → system audio
          CPAL input callback → bounded recording queue → worker
```

The audio callback performs no heap allocations, deallocations, file access or mutex locking.
Graph creation, decoding, generation and exports run off the UI/audio callback. Plugin
instances live in a rack that the callback owns; renderers are rebuilt on every edit and adopt
the previous renderer's transport and held notes, preserving continuity without re-instantiating
anything. Old graphs and unmounted processors are reclaimed on the main thread.
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
deploys to Railway from that folder on every push. It wears one look, the app's Skeuomorphic dark
theme: its `tokens.css` and `tokens.js` are generated from `frontend/src/theme` with
`node scripts/gen-site-tokens.mjs`; run `npm --prefix site start` to serve it locally.
The website is independent of the native Cargo build.
