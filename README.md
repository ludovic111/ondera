# Ondera

An open-source, cross-platform DAW (macOS, Linux, Windows) with Logic-like UX and a skeuomorphic
UI that is also fully controllable from a CLI and an MCP server. A human in the GUI and an AI agent
on the command line can work on the same session at the same time, because every action in Ondera
is a named command and the GUI is just one client of that command layer.

> **Status: Phase 1, UI shell.** Mock data, no audio, no file loading, no plugins. The point of
> this phase is that the app looks like the design and the layout holds up when resized. Do not
> expect it to make sound yet.

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
  the build if anything else hardcodes a colour, gradient, shadow or font.
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

In the shell: space toggles play, Return jumps to bar 1, C toggles cycle. Pinch or ctrl/cmd + wheel
zooms the arrangement around the cursor, horizontal wheel scrolls it, and clicking the ruler
locates. Mute, solo, arm, volume, selection, browser and editor tabs, and the agent panel all work
and all go through commands. Everything else is inert for now.

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
    src/state/         React bindings: SessionProvider, useSession(), useDispatch()
    src/canvas/        pure drawing: lanes, clips, waveforms, ruler, piano roll
    src/components/    titlebar, transport, browser, arrangement, editor, inspector, agent
design/                Claude Design export, the visual source of truth
engine/                reserved for the Rust audio engine
scripts/               check-tokens.mjs enforces the tokens rule
```

## Roadmap

1. **Phase 1, UI shell** (this): layout, tokens, mock session, fake transport.
2. **Phase 2, engine**: Rust audio process, real transport, audio and MIDI playback.
3. **Phase 3, agents**: `ondera` CLI and MCP server generated from the command registry, with a
   revertable change log in the agent panel.

## Contributing

Work on a branch, keep commits small, and open a pull request against `main`. Read `CLAUDE.md`
first; it is short and it is the contract. If something in the design is ambiguous, ask in the PR
rather than guessing.

## Author

Ludovic Marie, [@lu4ovic](https://twitter.com/lu4ovic) on Twitter.

## License

MIT, copyright Ludovic Marie. See [LICENSE](LICENSE).
