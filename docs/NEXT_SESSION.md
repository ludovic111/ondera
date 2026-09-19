# Prompt for the next session

Paste everything below the line into a new session opened on this repository.

---

Continue Ondera from 0.7.0 (merged to `main`, tagged `v0.7.0`). Read `CLAUDE.md` first, above all
the "0.7 work" bullet, and `docs/releases/0.7.0.md`. Work on a branch; do not merge or release
until I ask. Build setup and the live-check recipe are in your memory notes
(`live-check-recipe-tauri`, `shared-cargo-target-dir`, `xcode-license-build-workaround`).

Finish what 0.7.0 left open, in this order. Each item lists what exists and what "done" means.

## 1. Input monitoring (hear yourself while recording)
Exists: `engine/src/device.rs` has `InputMeter` (meter-only input stream on its own worker),
`Recorder`/`LocalRecorder` (capture into an `rtrb` ring), `Capture` with `producer: Option<…>`, and
`Telemetry::input_peak` / `counting_in`. The output callback is `Callback` in the same file; the
renderer is `engine/src/render.rs`. Nothing routes input to output.
Build: a second bounded `rtrb` ring from the input callback to the output callback, mixed into
the armed audio track's strip (so its inserts and sends apply) when monitoring is on. Add
`Track.monitor` (`off | auto | on`; auto = while armed and not playing back a clip on that track),
a `track.setMonitor` registry command, a button beside Arm in `TrackHeader.tsx`, the mixer and the
inspector, and `settings.audio.monitorLatencyMs` only if it is actually needed. Rules: no
allocation, locks or logging in either callback; streams stay on their owning workers (never an
unsafe `Send`); handle input/output sample-rate mismatch (resample or refuse with a clear error);
drop rather than block when the ring is full and count the drops in telemetry; mute monitoring
when input and output are the built-in mic and speakers unless the user confirms (feedback).
Done means: a unit test that a block pushed into the monitor ring comes out of `render` through
the track's inserts, plus a real-device check by me. Tell me exactly what to listen for and
report round-trip latency from the measured buffer sizes.

## 2. Verify recording on a real microphone
I have not confirmed the 0.7.0 count-in and input meter with an actual mic. Give me a five-step
manual test (arm, watch the meter, record with a one-bar count-in, confirm the first word is
intact and the clip starts on the bar, punch out), then fix whatever I report. Also fix: the
status line still says "Recording MIDI…" after a take ends (`desktop/src/app.rs`,
`start_recording`/`finish_recording`), and show "Count-in…" as the status during the count.

## 3. FLAC and MP3 export
Exists: `engine/src/export.rs` has a streaming `Sink` enum (WAV via hound, hand-written AIFF)
chosen by file extension; `export::mix` and `export::stems` use it; dialogs offer WAV and AIFF
(`desktop/src/web.rs` `daw_pick`, `desktop/src/export.rs`, `frontend/src/components/Dialogs.tsx`).
Build: a `Sink::Flac` that streams (encode fixed-size blocks as they are rendered; do not hold the
song in memory; `flacenc` exposes per-frame encoding, or write the encoder by hand if its
streaming API is unsuitable) for 16 and 24-bit. For MP3, check licensing and crate quality first
and tell me your recommendation before adding a dependency; if there is no clean pure-Rust
encoder, propose Ogg Vorbis or Opus instead. Stems must honour the chosen format
(`export.rs` currently hard-codes `.wav` in the stem file name). Done means: a round-trip test
like `aiff_export_holds_the_same_audio_as_wav` in `engine/tests/media.rs`, the format offered in
every export dialog, the registry docs in `engine/src/control_media.rs` updated, and a test that
memory stays bounded for a long export.

## 4. Plugin ABI 2: state, MIDI beyond notes, sample-accurate parameters
Exists: `sdk/src/ffi.rs` (`PluginVTable`, `Entry`, ABI_VERSION 1, calls guarded by `Guarded<P>`),
`sdk/src/plugin.rs` (`Plugin` trait, `NoteEvent`, `ParamChange` which is not yet delivered over
the ABI), `engine/src/host/native.rs` (loads libraries; synthesises state from parameters).
Build ABI 2 without breaking ABI 1 libraries: never change an existing `repr(C)` layout; add a
second, larger vtable reached through a new entry symbol or a versioned accessor, and have the
host pick by `abi_version`. Add: `save`/`load` of an opaque blob (goes into `Insert.blob` like
CLAP/VST3), an event list replacing the note slice (note on/off, CC, pitch bend, channel and
poly pressure, with frame offsets), parameter changes with frame offsets inside `process`,
`tail_seconds`, and a latency-changed notification. Keep the `Plugin` trait source-compatible
with default methods. Update `sdk/src/testing.rs` (`Bench`), both scaffold templates in
`engine/src/control_plugins.rs`, `plugins/gain`, `docs/NATIVE_PLUGINS.md`, and keep an ABI 1
fixture library under test so old plugins provably still load. MIDI CC also needs a path from
`engine/src/midi.rs` and the piano roll; scope that with me before building an editor for it.

## 5. The remaining GUI-only actions (target: 100 % parity)
Each needs a registry command so the CLI, MCP and the agent get it; the frontend then calls that
name (no new private `web.*` handlers):
- Browser tab and selection, and piano-roll vertical scroll: they live only in
  `frontend/src/state/native.ts` `localView`. Add them to `View` in `engine/src/model.rs` and to
  `view.get`/`view.set` in `engine/src/control_app.rs`.
- Command palette: add `palette` to `ui.showPanel`.
- Clipboard: `clip.copy`, `clip.cut`, `clip.paste(trackId?, bar?)` held by the host instead of
  `let clipboard` in `frontend/src/state/actions.ts`, so a paste works across CLI and window.
- Lane viewport width in `view.get`, so `zoomToFit` can be computed outside the window; add
  `view.fit`.
- `agent.models` and `agent.connection`: today Tauri commands `daw_agent_models` and
  `daw_agent_connection` in `desktop/src/web.rs` that block on the network. Make them live jobs
  that answer `{"status":"running"}` and complete later, like exports.
- `rhythm.preview` (render a groove to a temporary WAV without committing), replacing
  `daw_rhythm_preview`.
- The selection variants `web.quantize` and `web.transpose` should call `clip.quantize` /
  `clip.transpose` with the selected clip id and then be deleted.
Finish with a test that fails when a `web.*` method exists without a registry equivalent, apart
from an explicit allow-list (`web.ready`, `web.document`, `web.capture`, `web.file` pickers,
`web.liveNote` for musical typing).

## 6. Smaller things I noticed and did not fix
- Waves Audio Units list every channel layout as its own plugin ("(m)", "(s)", "(m->s)"): collapse
  them to one browser row per plugin and pick the layout from the track.
- 348 of about 2,200 effects on this Mac still land in "Other Effects"; extend `EFFECT_RULES` in
  `engine/src/control_plugins.rs` from what is actually in that folder, and keep the
  `well_known_third_party_names_find_their_folder` test growing with it.
- `session.batch` records one agent Changes entry per inner command but they share one undo
  step, so per-change Revert inside a batch steps the whole batch. Record a batch as one change.
- Opening a browser folder does not animate its rows; the `data-motion="reveal"` hook exists in
  `frontend/src/theme/motion.css` but the rows have no wrapper to carry it.
- The mock host (`frontend/src/dev/mockHost.ts`) has no agent transcript with tool entries, so
  the new agent steps cannot be checked in the browser preview; add a fixture conversation.

For every item: tests first where the behaviour is checkable without hardware, `cargo fmt`,
`cargo clippy --workspace --all-targets -- -D warnings`, workspace tests,
`npm --prefix frontend test` and `run build`, then a check in the real window. Say plainly what
you verified by test, what in the live window, and what only I can verify on hardware.
