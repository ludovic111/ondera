# Ondera 0.3.0 verification

Verification date: 13 September 2026, on the development Mac (Apple Silicon, macOS 26). This
records exercised behaviour and the limits of the evidence; it is not a certification of every
audio interface, plugin, model provider or production workload.

## Registry parity, live

With the 0.3.0 debug build running, `ondera-cli doctor` reported the bridge, matching versions,
both companions, the settings file and the plugin cache. Over the bridge the CLI opened the
agent panel and the Settings window (`ui.showPanel`), zoomed the arrangement (`view.set`),
captured three PNG screenshots of the real window (`ui.screenshot`, 3200×2000), read and reset
preferences (`settings.get`, `settings.reset`), rescanned plugins (2237 descriptors, 0 errors,
including the two native example plugins), listed `plugin.list --format native`, reported
`agent.status` and `agent.providers` (Codex found at its ChatGPT.app path), loaded a factory
preset onto a Space insert (`preset.load` wrote the five Cathedral values), opened that plugin's
window (`ui.openPluginWindow`) and quit the app with `app.quit --discard true`. The captures
were inspected: faceplates with corner screws behind the Settings and plugin windows, LED
buttons, tick rings on knobs, ivory and lacquered piano keys, the conversation header of the
agent panel.

## Native plugins

`engine/tests/native_plugin.rs` exercises the example bundle through the in-process ABI, the
whole stock library through the same vtables (24 manifests, legacy JSON-array state) and the
built dynamic library through the scanner. `ondera --scan-plugin native <dylib>` probed the
example library in a child process and returned both descriptors. All 24 stock plugins are
instantiated through the ABI in the existing DSP regression tests; the bit-identical voice test
and the stock effect decay tests pass unchanged.

## Agent, settings, updater

Unit tests cover the Codex and Claude Code stream parsers, a fake CLI driven through stdin with
a non-zero exit, cancellation that kills a child holding the output pipes, tool results reaching
chat cards, bridge calls appearing in the chat while a task runs, agent permissions enforced only
for agent-flagged requests, settings round trips with masked secrets and 0600 permissions, the
release-URL host pinning, and Ed25519 key generation, signing and tamper detection. A real
provider turn (Codex, Claude Code, Anthropic or OpenAI) was not run in this verification.

`cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace` and `python3 scripts/tests/release_workflow.py` all pass.

# Ondera 0.2.0 verification

Verification date: 12 September 2026. This records exercised behavior and the limits of the
evidence; it is not a certification of every audio interface, plugin or production workload.

## A complete song

With Python 3.12 or newer, `python3 scripts/verify-song.py` builds **Afterglow** through a persistent MCP process and checks
the saved document through the CLI and desktop validator. It creates five tracks, 17 regions
and 384 notes, imports an embedded audio source, splits a region, exercises undo/redo, uses
eight-slot inserts, sends and the master chain, draws a saved automation fade, saves and reopens
the project, then renders a 41.4-second stereo 24-bit / 48 kHz WAV.

The same run exports 960-PPQ MIDI, imports it and undoes the import, exports a selected range at
96 kHz / 32-bit float, and exports five aligned 44.1 kHz / 16-bit stems into a new folder. It
checks decoded audio, non-silence, format, peak, clipping, persistence and the command results.
The release workflow runs this scenario on macOS ARM and Intel, Linux and Windows. The
`Ondera-Afterglow-demo.zip` release asset contains the stock-plugin project, mix, MIDI and report.

The native macOS run also passed with FabFilter Twin 3 in CLAP and Apple's AUDelay Audio Unit.
Playback advanced on the MacBook Pro speakers at the selected 44.1 kHz device rate; the same
session saved, reopened, bounced and exported through the live bridge. Before the Mac locked,
the actual Agents tab and native automation window were inspected and a double-click added an
automation point that was immediately readable through the CLI.

A real signed-in Codex subprocess then used the app's MCP bridge to create a requested
instrument track and two specified notes, without altering existing tracks. Runner tests cover
Run/Stop and process cleanup; switching sessions waits for cleanup and rejects queued late edits
before showing the normal unsaved-changes prompt.

## Plugins, audio and failure handling

`engine/examples/plugin_roundtrip.rs` exercised installed FabFilter Twin 3 and Pro-Q 4 in CLAP,
VST3 and Audio Unit formats: instantiate, change a parameter, save state, destroy, instantiate
fresh, restore, save/reopen a MIDI-plus-audio project and export audible finite stereo PCM.
This verifies those installed plugins on this Mac, not every plugin sold in those formats.

Regression coverage includes callback allocation checks, sample alignment, plugin retirement,
held-note graph changes, short MIDI taps, per-channel sustain-pedal playback and recording,
automation at cycle boundaries, plugin preset undo,
partial audio capture, recovery copies, transaction failures and competing file writers. Native
editor tests exercise pointer-based piano-roll, arrangement and automation editing as well as
the Agents Run button and export settings.

## Performance evidence

The optimized stock-demo benchmark rendered 30 seconds in 0.600 seconds (50 times real time).
Its worst measured 128-frame block was 0.200 ms against a 2.667 ms block budget at 48 kHz, with
zero voice overflows. In the 32-instrument stress scenario (four-note chords, 64 effects and 32
automation lanes), DSP caching reduced median block cost from 2.318 to 1.769 ms, about 24%.
The optimized run's 99th percentile was 1.943 ms, worst block 2.038 ms and no blocks exceeded
the budget. Regression tests compare 524,288 samples across all eight instruments, pitch,
parameter and sample-rate changes; optimized samples are bit-identical.

Locating near the end of a 128-track, 200,000-note score now takes a median 0.102–0.111 ms,
compared with 13.51–13.70 ms before the single-pass note chase. These are measured offline
processing costs on the development Mac; driver scheduling, third-party processing, CPU power
settings and larger projects can change the result.

The song script reports individual command timings. In the recorded native run, ordinary
commands had an 8.30 ms median and 16.47 ms 95th percentile. These include the UI dispatch and
validation path. Plugin initialization and file work are excluded from that percentile and
remain separately reported. `cargo run --release -p ondera-engine --example stress` probes
4/8/16/32 instrument tracks with two effects and an automation lane per track.

The 78-command registry is shared by the app, CLI and MCP. `session.inspect` omits opaque plugin
state and optionally includes MIDI notes; `session.catalog` returns the 24 stock plugins, and
`plugin.list` searches and paginates installed plugins. These avoid sending a complete project
or a large installed-plugin catalog to the model for routine inspection.

## Remaining qualification

No physical microphone/interface capture or external MIDI keyboard was available for an
attended recording test. The recorder's data handling is covered by deterministic tests; that
does not verify a user's drivers, hardware latency or a long recording session. Final native
gestures after the Mac locked are covered by automated UI tests, not a later hands-on pass.

See [plugin support](PLUGINS.md), [current feature limits](RUST_MIGRATION.md) and
[release notes](releases/0.2.0.md). Automation write modes, time stretching, comping and arbitrary
hardware routing remain outside this release. macOS distribution is ad-hoc signed and not
notarized; Windows distribution is unsigned. These limits remain relevant when qualifying the
app for a professional studio.
