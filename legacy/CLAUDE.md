# Ondera

Ondera is an open-source, cross-platform DAW (macOS, Linux, Windows) with Logic-like UX and a
skeuomorphic UI. It is also 100% controllable from a CLI and an MCP server, so AI agents can drive a
session while a human works in the GUI at the same time.

## Architecture (decided, do not re-litigate)

Three layers. Every line of code belongs to exactly one of them.

1. **UI**: Electron app, React + TypeScript, Vite. Renders the session and captures user intent.
2. **Core command layer** (`packages/core`): pure TypeScript, no DOM, no Electron, no Node-only APIs.
   Owns the session model and every operation on it. This is the product's API.
3. **Audio engine**: a separate native process written in Rust, talking to core over IPC.
   It does not exist yet. Core talks to an `EngineClient` interface. Until the Rust process lands,
   `packages/app/src/audio/` ships a **Web Audio implementation** of that interface (decided
   2026-09-12 so the app can make a song end to end): synth presets for MIDI tracks, buffer playback
   for audio clips, inserts, sends, metering, offline bounce, microphone recording. Core's
   `MockEngine` stays for tests and headless clients. The Rust engine replaces the Web Audio one
   behind the same interface; nothing above the interface changes.

The GUI, the CLI and the MCP server are all *clients* of the core command layer. None of them is
privileged. Anything the GUI can do, the CLI and an agent can do with the same command.

### The command-layer rule

**No UI component may mutate state directly. It dispatches a command.**

- Every user action becomes a named command with typed params
  (for example `transport.play`, `track.setMute { trackId, muted }`, `view.zoom { pixelsPerBar }`).
- Commands are declared once in `packages/core/src/commands/` with a param schema, so the registry
  is introspectable. The CLI and MCP server are generated from that registry later, not hand-written.
- The renderer reads state through subscriptions and writes state only through `dispatch(command)`.
  Local, ephemeral UI state (hover, drag-in-progress, an open dropdown) may live in React state.
  Anything that would need to survive a reload, be visible to an agent, or be undoable is session
  state and goes through a command.
- Commands must be deterministic given (state, params). No `Date.now()` or randomness inside a
  command; pass those in as params.
- If you are about to write `setState(...)` or mutate an object from the session model inside a
  component, stop and add a command instead.

## Phase 1 scope

Phase 1 proper (static shell, mock data) shipped in PR #1. **Phase 1.5, "usable" (decided
2026-09-12):** almost everything visible does something, and a song can be made from A to Z:

1. Tracks: add, remove, reorder, rename, colour, mute/solo/arm, volume, pan.
2. Clips: draw (pencil), move, trim, split (scissors or ⌘T), duplicate, rename, delete.
3. Notes: piano roll (draw, move, resize, velocity), step view (toggle cells), score view (read).
4. Transport: play/stop/record, cycle range drag in the ruler, tempo/signature/key/snap edits,
   follow playhead, metronome, real meters.
5. Sound: MIDI tracks play synth presets, audio tracks play imported files and recordings,
   inserts and sends are live, bounce renders the mix to WAV.
6. Sessions: single-file `.ondera` (JSON, imported/recorded audio embedded as base64 WAV).
   Generated demo audio is rebuilt from its seed, never stored.
7. Still inert on purpose: third-party plugins in the browser, the agent (the panel logs your
   request and says no agent is connected), the File menu's Bounce is the only export.

Undo rule: commands that describe "now" rather than the document (selection, view, transport
position, engine ticks, meters) are `transient` and never enter the undo stack. Every other
command is one undo step, so a drag dispatches once on release, not on every pointer move.

Rendering rule: **Canvas for the timeline, waveforms and piano-roll grid. DOM for chrome.**
Canvas drawing code lives in pure functions that take a context and tokens; no React inside them.
Drag previews are an `overlay` argument to those functions, never session state.

Menus and shortcuts rule: one table (`packages/app/src/state/actions.ts`) defines every UI action
with its label, shortcut and enabled/checked predicates. Title-bar menus, context menus and the
keyboard handler are all generated from it. Add an action there; never bind a key or a menu row
by hand. File operations (new/open/save/import/bounce) talk to the host and live in
`state/document.ts`; they are the one thing that is not a command.

## Tokens rule

There is exactly one source of visual truth: `packages/app/src/theme/tokens.ts`.

- It holds colors, the type scale, spacing and radii, panel dimensions, and the shadow/highlight
  recipe for the material system (raised, pressed, lit, well, knob, fader cap, clip, meter, glass).
- Everything else references tokens. **No hardcoded colors, font sizes, shadows or magic pixel
  values anywhere else**, including canvas drawing code and inline styles.
- Tokens are also emitted as CSS custom properties (`--color-…`, `--shadow-…`) for stylesheets, so
  CSS and canvas read the same numbers.
- If a value from the design is missing from the tokens file, add it to the tokens file. Do not
  inline it "for now".
- The material system assumes one light source from directly above. Every raised object has the
  same four layers in the same order: top-edge specular highlight, gradient face darkening downward,
  bottom-edge contact line, soft drop shadow. Pressed states swap the drop shadow for an inner
  shadow and darken the face. Nothing else moves. Compose these from the recipe tokens; never
  hand-write a `box-shadow` in a component.

## Design reference

The Claude Design export lives in `design/` and is the source of truth for Phase 1:

- `design/Ondera Arrangement.dc.html`: variant 01/02, skeuomorphic material system
  (Manrope + IBM Plex Mono, gradients, bevels, contact shadows, LED meters, frosted glass).
- `design/Ondera Arrangement v2.dc.html`: variant v2, "Swiss / flat system, same anatomy"
  (Geist + Geist Mono, hairlines, outlined-at-rest / inverted-when-active, no shadows).
- `design/support.js`: the design runtime; not app code, never import it.

Both variants share the same anatomy: 1600×1000 reference window, panels 220 / 184 / 240 / 380 px
(browser / track header / inspector / agent), 70 px track rows, 48 px per bar at default zoom,
4 px base grid, 8 mock tracks. Each file ends with a spec sheet (color tokens, type scale, grid,
material recipes) in the `renderVals()` script block; extract tokens from there, not from eyeballing.

**Variant 01/02 (skeuomorphic) is authoritative for styling**, decided 2026-09-12: the brief asks
for bevels, contact shadows and a single light source from above, and only that file defines the
recipe. v2 is kept for reference only. Switching later is a tokens-file change, not a rebuild.

When something in the design is ambiguous, ask rather than invent.

## Git workflow

- Never commit to `main`. Work on a branch (`phase-1/…`, `feat/…`, `fix/…`).
- Small commits, one logical change each, imperative subject line.
- When a chunk of work is done, stop and say so; the owner reviews before anything is merged.
- Commit messages end with:
  `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`

## Repo layout

See the folder structure section in the root `README.md` once scaffolded. Short version:
`packages/core` (command layer, pure TS), `packages/app` (Electron + React), `design/` (reference),
`engine/` (reserved for the Rust process, empty in Phase 1).
