# Ondera

A digital audio workstation for macOS, Windows and Linux, with a music assistant built in and
every action open to scripts and AI tools.

Record while you hear yourself through your effects, write and edit MIDI with the full expression
of your keyboard, arrange with fades and song markers, mix on a real mixer with stock, CLAP, VST3
and Audio Unit plugins, automate anything, and export a mix or stems as WAV, AIFF, FLAC or Ogg
Vorbis. Ask the built-in agent for a bass line or a mix check in your own words: it works on the
same song, and every edit it makes is one undo away.

- **Audio engine in Rust**: sample-accurate automation and controllers, plugin delay
  compensation, input monitoring, count-in, crash-isolated plugin scanning. Nothing audio runs in
  JavaScript.
- **34 stock instruments and effects** with front panels that draw what the audio does, plus
  your own CLAP, VST3, Audio Unit (macOS) and Ondera native plugins.
- **Six themes**, each in dark and light: Modern, Skeuomorphic, Frutiger Aero, Console, Ink and
  Neon.
- **Built for AI control**: everything a person can do in the window is a command that the
  built-in agent, `ondera-cli` and any MCP client can call, with a one-call overview of the whole
  song and full access to external plugins' parameters and state.
- **Offline and private**: no account, no subscription, no telemetry. Songs are single `.ondera`
  files with the audio inside.

## Install

Download the file for your computer from the
[latest release](https://github.com/ludovic111/ondera/releases/latest):

| Computer | File |
| --- | --- |
| Mac with Apple silicon | `Ondera-macos-arm64.zip` |
| Mac with Intel | `Ondera-macos-x86_64.zip` |
| Windows | `Ondera-windows-x86_64.zip` |
| Linux | `Ondera-linux-x86_64.zip` (or `.tar.gz`) |

**macOS**: unzip and drag `Ondera.app` to Applications. The app is signed ad hoc, not notarized,
so the first launch is blocked: open System Settings > Privacy & Security and choose Open Anyway,
or run `xattr -dr com.apple.quarantine /Applications/Ondera.app`.
**Windows and Linux**: extract all three executables (`ondera`, `ondera-cli`, `ondera-mcp`) into
one folder. Windows needs the Microsoft Edge WebView2 runtime.

Ondera checks for updates when it starts (Help > Check for updates…). An update is installed
only after its Ed25519 signature, download host, checksum and binary versions are verified; the
previous copy is kept until the new one starts. `ONDERA_NO_UPDATE=1` turns the check off.

## Start

1. File > New session: Drums, Bass and an audio track are ready.
2. Double-click a loop in the browser, or draw a region with the Pencil tool (`2`) and click in
   the piano roll to add notes. Space plays.
3. Mix in the inspector or the mixer (`X`), save with `⌘S`, export with `⌘B`.

Or open File > Open demo to explore a finished song. The [user guide](docs/USER_GUIDE.md) walks
through everything.

## Control Ondera from scripts and AI

The window, the built-in agent, `ondera-cli` and `ondera-mcp` all run the same
[command registry](docs/COMMANDS.md), with the same undo history. While the app runs, clients
connect over a local, token-protected bridge; without it they edit a song file directly.

```sh
ondera-cli session.overview                    # the whole song in one call
ondera-cli track.add --kind midi --name Keys --instrument "E-Piano Mk I"
ondera-cli clip.create --trackId Keys --startBar 0 --lengthBars 2 \
    --notes '[{"start":0,"length":2,"pitch":60},{"start":2,"length":2,"pitch":64}]'
ondera-cli plugin.list --query reverb
ondera-cli history.undo
ondera-cli --file song.ondera session.exportAudio --path mix.flac
```

Add Ondera to Claude Code or any MCP client:

```sh
claude mcp add ondera -- /Applications/Ondera.app/Contents/MacOS/ondera-mcp --live
```

See [AI_CONTROL.md](docs/AI_CONTROL.md) for the agent, the CLI, MCP, permissions and recipes.

## Documentation

| Document | What it covers |
| --- | --- |
| [User guide](docs/USER_GUIDE.md) | The window, tracks, recording, editing, mixing, automation, files, themes, settings |
| [Keyboard shortcuts](docs/SHORTCUTS.md) | Every shortcut (generated from the app) |
| [AI control](docs/AI_CONTROL.md) | The built-in agent, `ondera-cli`, `ondera-mcp`, permissions, recipes |
| [Command reference](docs/COMMANDS.md) | Every command and parameter (generated from the registry) |
| [The agent panel](docs/AGENT.md) | Providers, sign-in, the conversation, Changes and Takes |
| [Plugins](docs/PLUGINS.md) | CLAP, VST3 and Audio Unit hosting |
| [Native plugins](docs/NATIVE_PLUGINS.md) | Writing plugins in Rust with the Ondera SDK |
| [Development](docs/DEVELOPMENT.md) | Code layout, building, checks, releases |
| [Release notes](docs/releases/) | What changed in each version |
| [Verification](docs/VERIFICATION.md) | What has been tested and how |

## Build from source

```sh
npm --prefix frontend ci
npm --prefix frontend run build
cargo run --release
```

Rust 1.88+ and Node.js 24 are needed. Platform prerequisites, checks and the release process are
in [DEVELOPMENT.md](docs/DEVELOPMENT.md).

## Limits

No time stretching, comping, tempo changes inside a song or user-defined buses yet. Recording
latency is not compensated automatically. External plugin windows open on macOS; elsewhere
external plugins show their parameter list. VST2 and AAX are not supported. There is no MP3
export. Builds are not notarized.

## License

MIT. Copyright Ludovic Marie. See [LICENSE](LICENSE).
