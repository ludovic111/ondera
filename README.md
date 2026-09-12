# Ondera

An open-source, cross-platform DAW (macOS, Linux, Windows) with Logic-like UX and a skeuomorphic
UI. Every action is a named command, so the same session can be driven from the GUI, a CLI, or an
MCP server by an AI agent, at the same time.

**Status: Phase 1.** Static UI shell with mock data and no audio. See `CLAUDE.md` for the
architecture, the command-layer rule, the tokens rule and the git workflow.

## Run it

```bash
pnpm install
pnpm dev
```

`pnpm check` runs the TypeScript typecheck for every package and the tokens lint.

Shortcuts in the shell: space toggles play, Return jumps to bar 1, C toggles cycle. Pinch or
ctrl/cmd + wheel zooms the arrangement around the cursor; horizontal wheel scrolls it. Click the
ruler to locate.

## Layout

```
packages/
  core/     @ondera/core   command layer + session model. Pure TS, no DOM, no Electron.
    src/model/        Session, Track, Clip, Note, Transport, View types; bar/beat/SMPTE maths
    src/commands/     one file per family (transport, track, clip, view, agent), typed params
    src/registry.ts   every command, introspectable (future CLI + MCP are generated from it)
    src/store.ts      getState / subscribe / dispatch(command)
    src/engine/       EngineClient interface + MockEngine (fake playhead timer)
    src/mock/         the 8-track mock session and deterministic waveform peaks
  app/      @ondera/app    Electron + Vite + React
    electron/         main.ts, preload.ts
    src/theme/        tokens.ts (THE tokens file), materials.css, colour maths
    src/state/        React bindings: SessionProvider, useSession(), useDispatch()
    src/canvas/       pure drawing: lanes, clips, waveforms, ruler, piano roll
    src/components/   DOM chrome: titlebar, transport, browser, arrangement, editor, inspector, agent
design/     Claude Design export, the visual source of truth
engine/     reserved for the Rust audio engine (not started)
scripts/    check-tokens.mjs enforces the tokens rule
```
