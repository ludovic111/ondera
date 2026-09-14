# Agent setup and everyday usability review

14 September 2026 · based on `bbc4821` (Ondera 0.4.0). These changes are a local
review branch, not a published release.

## Changes

- Provider-specific setup, automatic companion discovery and sign-in checks, explicit API-key
  saving, installation help, advanced overrides and understandable permission descriptions.
  Credentials remain with the CLI or in the existing private settings file.
- Separate states for a signed-in account, configuration-only API readiness, missing companions,
  unsupported Codex versions, missing keys/models/addresses and a disabled local bridge.
  Connection checks do not send a model request. Switching providers resets the model override.
- Musical starter prompts, Enter/Shift+Enter chat, preserved drafts, duplicate-send protection,
  understandable errors, editable retries and a confirmed new-conversation action. Changes
  explains the scope of undo and disables undo/redo while the agent is running.
- Direct Settings section routing through the shared `ui.showPanel` registry command. Update
  preferences now appear in Updates. Unsaved agent configuration survives section changes and
  asks before closing. Dialog bounds account for interface scaling.
- Keyboard shortcuts respect text composition, dialogs and keyboard-activated buttons; moving
  focus into a text field releases held musical-typing notes. Segmented controls are buttons.
- Empty-project actions lead to instruments, audio import, the demo and the agent. The existing
  score display is explicitly a preview; clicks no longer apply piano-roll coordinates to it.
- Export validates ranges, tails and stem selection before the picker, prevents duplicate
  submissions, disables irrelevant float dither, and shows actual output paths and warnings.
- Agent screenshots explicitly redraw canvases when a background WebView has paused animation
  frames. CLI connection checks drain bounded output and terminate their process groups on
  completion or timeout. Masked settings are safe for Unicode keys.

## Verification

- `npm --prefix frontend test`: 42 passing tests, including actual React interaction tests for
  setup, draft retention, Enter/IME handling, confirmation, export selection and keyboard focus.
- Frontend production build and TypeScript checks pass.
- `cargo test --workspace --locked`: 203 passing tests; one existing doctest ignored.
- `cargo fmt --all --check` and Clippy with warnings denied pass.
- Release workspace build and local macOS ARM64 package verified. Native screenshots show
  the real arrangement and notes even with the Mac locked; Settings remains in bounds at
  interface scale 1.75 with scrollable content.
- `scripts/verify-song.py` passes both headlessly and through a dedicated live desktop window:
  five tracks, 17 regions, 384 notes, embedded audio, effects, sends, automation, edits,
  undo/redo, save/reopen, MIDI interchange, stereo WAV, a float range and five aligned stems.
- A real signed-in Codex turn added exactly one E-Piano Mk I track, one four-bar region and
  four requested notes. An independent CLI comparison verified every original track, region,
  strip, audio source and automation lane remained unchanged. Two-step undo and redo restored
  the exact result. The test used an isolated project/settings/data directory.

## Evidence limits

The Mac was locked: Computer Use could not perform attended pointer interactions. The app's
own screenshot command and React interaction tests provide separate visual and behavioural
checks. No fresh account login, account creation or provider installation was performed.
Anthropic, OpenAI, Claude Code and compatible-server inference were not all exercised live;
connection configuration is never labelled as a successful inference request.

Windows, Linux and Intel Mac CI still need to run after the owner authorizes pushing the
branch. Existing limits such as time stretching, comping, latency compensation and arbitrary
third-party plugin compatibility are unchanged. No claim of complete DAW parity is made.
