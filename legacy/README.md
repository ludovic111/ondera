# Ondera

An open-source, cross-platform DAW (macOS, Linux, Windows) with Logic-like UX and a skeuomorphic
UI that is also fully controllable from a CLI and an MCP server. A human in the GUI and an AI agent
on the command line can work on the same session at the same time, because every action in Ondera
is a named command and the GUI is just one client of that command layer.

> **Status: Phase 1.5, usable.** You can make a song from start to finish: add tracks, draw MIDI
> with built-in synth presets, import or record audio, arrange clips, mix with inserts and sends,
> save the session and bounce a WAV. Audio runs on a Web Audio engine inside the app for now; the
> Rust engine will replace it behind the same interface. No third-party plugins, no agent yet.

## Why

Most DAWs are built GUI-first and bolt scripting on afterwards. Ondera is built API-first: the
session model and every operation on it live in a pure TypeScript package with no knowledge of the
screen. The desktop app, the future `ondera` CLI and the future MCP server all dispatch the same
typed commands into the same store. That is what makes it safe for an agent to edit a project while
you watch, and what makes every agent change revertable from the UI.

## Architecture

```
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│  Electron UI │  │  CLI (later) │  │  MCP (later) │      clients
└──────┬───────┘  └──────┬───────┘  └──────┬───────┘
       └────────────────┼─────────────────┘
                        ▼
              ┌───────────────────┐
              │   @ondera/core    │   command registry · session store · model
              └─────────┬─────────┘
                        │ EngineClient (IPC)
                        ▼
              ┌───────────────────┐
              │  Rust audio engine│   separate process, not started yet
              └───────────────────┘
```

- **`packages/core`**: the command layer. Session model, typed command registry, store with
  `dispatch()`, an `EngineClient` interface and a `MockEngine` that fakes the playhead.
- **`packages/app`**: Electron + Vite + React. Canvas for the timeline, waveforms and piano roll;
  DOM for chrome. One tokens file holds every colour, size and material recipe; a lint step fails
  the build if anything else hardcodes a colour, gradient, shadow or font. `src/audio/` is the
  interim Web Audio engine behind core's `EngineClient`.
- **`engine/`**: reserved for the Rust process.

The rules that keep this honest are in [CLAUDE.md](CLAUDE.md): no UI component mutates state
directly, and no visual constant lives outside the tokens file.

## Run it

Requirements: Node 22+, pnpm 10.

```bash
pnpm install
pnpm dev
```

`pnpm check` runs the TypeScript typecheck for every package plus the tokens lint. `pnpm build`
produces the Electron bundles under `packages/app/out`.

### Making a song

1. **Tracks.** `+` above the track headers (or ⌥⌘A / ⌥⌘S) adds an audio or MIDI track.
   Right-click a header for rename, colour, move, delete. Double-click the name to rename.
2. **Instruments.** Double-click an instrument in the browser to load it on the selected MIDI
   track (or create one). The Instrument row in the inspector switches presets.
3. **Notes.** Pick the pencil tool (2) and drag on a MIDI lane to draw a clip, then draw notes in
   the piano roll (drag, or double-click). Drag a note to move it, its right edge to resize,
   right-click for velocity. Step view toggles cells on the snap grid. Loops in the browser drop
   a ready-made pattern at the playhead.
4. **Audio.** File › Import Audio… (⌘I) or drop files onto a lane. To record, arm an audio track,
   press R then Space; stop to commit the take.
5. **Arrange.** Drag clips (also across tracks of the same kind), trim their edges, split with the
   scissors (3) or ⌘T at the playhead, ⌘D duplicates, ⌫ deletes. Drag in the ruler to set a cycle.
6. **Mix.** Track fader and pan in the header; the inspector has the channel fader, pan, two send
   knobs (reverb, delay) and four insert slots. Click an empty slot to add an effect, the LED
   toggles bypass.
7. **Save and export.** ⌘S saves a `.ondera` file, ⌘B bounces the mix to WAV.

Shortcuts are listed in every menu. Space play/stop, 0 stop, Return to start, `,` `.` nudge a
bar, C cycle, K click, R record, M/S/A mute/solo/arm the selected track, F follow, Z zoom to fit,
⌘= / ⌘- zoom, ⌘Z / ⇧⌘Z undo/redo, ⌘J agent panel, 1–4 tools. Pinch or ⌘ + wheel zooms around the
cursor, horizontal wheel scrolls, ⌥ while dragging disables snap.

## Repository layout

```
packages/
  core/                @ondera/core, pure TS, no DOM, no Electron
    src/model/         Session, Track, Clip, Note, Transport, View; bar/beat/SMPTE maths
    src/commands/      one file per family: transport, track, clip, view, agent
    src/registry.ts    every command, introspectable (CLI + MCP are generated from it later)
    src/store.ts       getState / subscribe / dispatch(command)
    src/engine/        EngineClient interface + MockEngine
    src/mock/          the 8-track mock session and deterministic waveform peaks
  app/                 @ondera/app, Electron + Vite + React
    electron/          main.ts, preload.ts
    src/theme/         tokens.ts (the tokens file), materials.css, colour maths
    src/state/         React bindings, the actions/shortcuts/menus table, the document layer
    src/audio/         Web Audio engine: graph, instruments, effects, library, bounce, recorder
    src/canvas/        pure drawing: lanes, clips, waveforms, ruler, piano roll
    src/components/    titlebar, transport, browser, arrangement, editor, inspector, agent
design/                Claude Design export, the visual source of truth
engine/                reserved for the Rust audio engine
scripts/               check-tokens.mjs enforces the tokens rule
```

## Roadmap

1. **Phase 1, UI shell**: layout, tokens, mock session, fake transport. Done.
2. **Phase 1.5, usable** (this): every control works, Web Audio engine, sessions, import, record, bounce.
3. **Phase 2, engine**: Rust audio process behind the same `EngineClient`, plugin hosting.
4. **Phase 3, agents**: `ondera` CLI and MCP server generated from the command registry, with a
   revertable change log in the agent panel.

## Contributing

Work on a branch, keep commits small, and open a pull request against `main`. Read `CLAUDE.md`
first; it is short and it is the contract. If something in the design is ambiguous, ask in the PR
rather than guessing.

## Author

Ludovic Marie, [@lu4ovic](https://twitter.com/lu4ovic) on Twitter.

## License

MIT, copyright Ludovic Marie. See [LICENSE](LICENSE).
