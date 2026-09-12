# Rust migration — 2026-09-12

Branch: `codex/rust-cross-platform`, based on `phase-1/usable` at `772ff9e`.

The owner explicitly chose a fully native Rust interface. The active build is a Cargo workspace
with `ondera` (desktop) and `ondera-engine`. The former source is preserved unchanged in `legacy/`.

## Delivered behavior

| Area | Native implementation |
| --- | --- |
| Interface | egui/wgpu window, warm graphite palette, library, arrangement, piano roll, step/score views, inspector, menus and shortcuts |
| Commands | typed serializable commands, validated atomic batches, 200 undo steps, gesture grouping, dirty/save tracking |
| Tracks/regions | add, rename, reorder, colour, mute, solo, arm, fader, pan; region create/move/trim/split/duplicate/delete |
| MIDI | note draw/move/resize/velocity/delete, preset selection, previews, nine original loop patterns |
| Audio | device-driven transport, native synthesis, decoded file playback/resampling, inserts, post-mute sends, metering, metronome and cycle |
| Documents | v1 JSON projects, embedded lossless float WAV, validation before loading, atomic replacement on saving |
| Recording | default device input, bounded queue, native float samples; linear takes only |
| Export | worker-based streaming stereo 48 kHz / 24-bit WAV, same renderer as playback, 3-second tail |
| Platforms | native audio and graphics backends for macOS, Linux and Windows; four-runner CI matrix including Intel and Apple Silicon Macs |
| Control | introspectable command registry (`engine/src/control.rs`) shared by the window, `ondera-cli` and the stdio MCP server `ondera-mcp`; live loopback socket with a per-launch token, or headless file hosting |

## Evidence

- macOS Apple Silicon: release build and native window launched, demo screenshot inspected.
- 31 regression tests pass locally: 23 engine tests and 8 UI/device lifecycle tests. They include
  real-time allocation/deallocation counts, sample equality between playback DSP and exported
  WAV, real pointer gestures, keyboard undo and safe deferred quit after finishing a take.
- Local optimized benchmark: 30 seconds of Nightfall at 48 kHz / 128-frame blocks rendered
  in 0.550 seconds (54.6× real time, 1.83% average render time / audio time, p99 block
  0.071 ms against a 2.667 ms deadline; no voice overflow). This excludes UI/device/OS scheduling and does not
  establish a speedup over Electron. Re-run `cargo run --release -p ondera-engine --example benchmark`.
- Native GUI playback advanced the sample-driven position on a 48 kHz output and showed live
  DSP activity. Track rename and undo were exercised in the packaged app. Arrangement move,
  ruler selection and piano drawing also have event-driven GUI regression tests.
- Headless loading accepted the original 8-track/13-region Nightfall fixture. Its native bounce
  was decoded/inspected as 27 seconds of stereo PCM WAV, 48 kHz / 24-bit, including effect tail.
- An input-device initialization stall observed during GUI QA led to moving all stream ownership
  and initialization to dedicated workers. Finalizing a recording is also asynchronous; save,
  export and document replacement wait for the take. The UI remains separate from OS permission waits.

Local review artifacts (ignored by Git): `dist/Ondera.app`, `dist/Ondera-macos.zip`,
`artifacts/native-macos.png`, `artifacts/nightfall.ondera` and `artifacts/nightfall-native.wav`.
Cross-platform builds, test logs and binaries are attached to the
[Native Rust workflow](https://github.com/ludovic111/ondera/actions/workflows/native.yml).

The final delivery note and CI runs contain the latest platform verification results.

## Known limits

- This is a first native port, not a claim of exhaustive production DAW qualification.
- Native synths, EQ, reverb and dynamics differ from Web Audio. MIDI events and imported PCM
  survive migration, but old mixes and regenerated demo sound are not bit-identical.
- Stereo files are linearly resampled. No time stretching, high-quality offline resampling,
  MIDI hardware input, external plugin hosting or automation curves yet.
- Live control answers file commands (open, save, bounce, import) synchronously on the
  interface thread, so the window pauses for the duration of a large export. Recording is not
  exposed through control.
- Recording requires system microphone permission, uses the default input and captures linear
  takes. No loop-take comping or measured hardware latency compensation. A dropped/overflowed
  take reports an error instead of silently inserting corrupted audio.
- The score view is a pitch overview, not an engraving/notation editor.
- Arrangement voices are bounded; overflow is counted. This is not a hard-real-time OS guarantee.
- Device hot-unplug reports failure; reconnect through the Audio menu. Device matrices, long
  sessions, sleep/wake and audio-driver latency need physical checks on each target system.
- Local macOS packaging is ad-hoc signed. Public distribution still needs appropriate macOS
  signing/notarization and Windows signing/installer validation.
