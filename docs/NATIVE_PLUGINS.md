# Ondera native plugins

Ondera has its own plugin format. A native plugin is a Rust type that implements the
`Plugin` trait from the `ondera-plugin` SDK, compiled into a dynamic library that Ondera loads
through a small, frozen C ABI. The stock library that ships with Ondera is written the same
way and loaded through the same vtables, so the format is exercised in every session, not only
when a third-party plugin is installed. CLAP, VST3 and Audio Units keep working alongside.

## Why a native format

- **One trait, no boilerplate.** Declare `Info`, a parameter list and `process`; the SDK
  generates the entry point, metadata and state handling.
- **Parameters are document state.** Ondera stores plugin parameters in the session, so they
  undo, redo, save, automate and travel through the CLI, MCP and agents with no plugin code.
- **Real-time discipline is built in.** `process` receives a stereo block of at most 256 frames
  with sorted note events and a transport context; parameter changes arrive before the block.
- **Isolation.** Libraries are probed in a child process, like CLAP and VST3 bundles, so a
  crashing plugin cannot take a session down while scanning.

## Write one

The quickest start is the scaffold, which writes a crate with a working plugin and a test that
runs it through the real ABI:

```sh
ondera-cli plugin.scaffold path=./warm-drive name="Warm Drive" vendor="Night Owl"   # kind=instrument for a synth
cd warm-drive && cargo test && cargo build --release
ondera-cli plugin.install path=target/release/libwarm_drive.dylib
ondera-cli plugin.scan
```

By hand it is this much:

```toml
[package]
name = "my-plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
ondera-plugin = { git = "https://github.com/ludovic111/ondera", package = "ondera-plugin" }
```

```rust
use ondera_plugin::{export_plugins, prelude::*};

struct Gain { gain: Smoother }

impl Plugin for Gain {
    const INFO: Info = Info::effect("com.example.gain", "Gain", "Example", "Utility")
        .describe("A gain with click-free smoothing.");
    fn params() -> Vec<ParamSpec> { vec![param("Gain", -60.0, 12.0, 0.0, "dB")] }
    fn new(rate: f64) -> Self { Self { gain: Smoother::new(rate, 0.01, 1.0) } }
    fn set_param(&mut self, index: usize, value: f64) {
        if index == 0 { self.gain.set(db_to_gain(value)); }
    }
    fn process(&mut self, audio: &mut [[f32; 2]], _notes: &[NoteEvent], _ctx: &ProcessContext) {
        for frame in audio { let g = self.gain.step(); frame[0] *= g; frame[1] *= g; }
    }
}

export_plugins!(Gain);
```

- `Info::effect(id, name, vendor, category)` or `Info::instrument(id, name, vendor)`. The `id`
  is stored in sessions as `native:<id>`; keep it stable.
- Parameters: `param`, `hz` (logarithmic), `choice` (labelled, the value is the index) and
  `switch`. Values are plain units within the declared range; index order is parameter id.
- Instruments receive silence and add their output; effects transform in place. `notes` are
  sorted by frame. `ctx` carries tempo, position, time signature and the cycle range.
- `reset` silences tails; `latency` reports frames for delay compensation.
- Never allocate, block, lock or log in `process`. Preallocate in `new`.

`plugins/gain` in the repository is a complete example with three plugins: `Trim` and `Tilt EQ`
use only the methods above; `Bend Sine` uses everything in the next section.

## Beyond notes and knobs (plugin ABI 2)

Four more trait methods, each with a default, so a plugin written before them compiles and
behaves as it did:

```rust
fn process_events(&mut self, audio: &mut [[f32; 2]], events: &[Event], params: &[TimedParam], ctx: &ProcessContext);
fn save(&self) -> Vec<u8>;
fn load(&mut self, state: &[u8]) -> Result<(), String>;
fn tail_seconds(&self) -> f64;
```

- **Events.** `process_events` is what an ABI 2 host calls instead of `process`. `events` holds
  note on/off, controllers, pitch bend, channel pressure and poly pressure, sorted by `frame`
  within the block. `event.kind` is one of the `event::*` constants (`NOTE_ON`, `CONTROL`, ...);
  `key` is the pitch or the controller number, `value` the velocity, controller value or
  pressure (0-127), `bend_amount()` the wheel as -1..1. Ignore kinds you do not know: later
  hosts may send more.
- **Parameter changes with frame offsets.** `params` lists `TimedParam { frame, index, value }`,
  sorted by frame. If you do not override `process_events`, the default splits the block at
  every change and calls `set_param` then `process` on each stretch, so an ABI 1 style plugin
  gets sample-accurate parameters without any work. Override it when you want to ramp between
  points yourself, or to read controllers.
- **State.** Parameters are still saved by Ondera. `save` is for what parameters cannot express
  (a tuning table, a sample path, a versioned settings block); return an empty vector when there
  is nothing. It lands in the insert's blob as `{"values": [...], "state": "<base64>"}` next to
  CLAP and VST3 state. `load` must accept what older versions of your plugin wrote, and may
  refuse (`Err`) what it cannot read: the session then keeps the instance as it was.
  Threading: Ondera calls `save` on a main-thread instance that never processes audio, and
  calls `load` on a freshly made instance before handing it to the audio thread, so both may
  allocate. It follows that changes `process` makes to its own fields are not saved.
- **Tail.** `tail_seconds` says how long the output rings after the input stops
  (`f64::INFINITY` for a drone). The exporter warns when a plugin's tail is longer than the
  tail being rendered.
- **Latency changes.** Return the new value from `latency` when it changes. The ABI compares
  it after each block and raises `FLAG_LATENCY_CHANGED`; Ondera then rebuilds its delay
  compensation. Changing latency while playing causes one realignment, so do it on a
  parameter change, not continuously.
- If you override `process_events`, keep `process` working for hosts from before ABI 2: the
  scaffolded instrument shows the three-line delegation.

What Ondera itself sends today: notes, and parameter changes at frame 0 of each block of at
most 256 frames. Controllers, pitch bend and pressure travel through the ABI and
`testing::Bench` already, but the sequencer and the MIDI input do not route them to plugins yet.

## Install and scan

Build with `cargo build --release`. Copy the library (`.dylib`, `.so` or `.dll`) or an
`.onplug` bundle folder containing it into one of these folders, then rescan
(Settings > Plugins, Mix > Rescan plugins, or `ondera-cli plugin.scan`):

- Application data: `~/Library/Application Support/Ondera/plugins`, `~/.config/ondera/plugins`
  or `%APPDATA%\Ondera\plugins` (Settings > Plugins > Open plugin folder).
- macOS: `~/Library/Audio/Plug-Ins/Ondera`, `/Library/Audio/Plug-Ins/Ondera`.
- Linux: `~/.local/lib/ondera/plugins`, `/usr/lib/ondera/plugins`, `/usr/local/lib/ondera/plugins`.
- Windows: `%COMMONPROGRAMFILES%\Ondera\Plugins`.
- Extra folders from Settings > Plugins and the `ONDERA_PLUGIN_PATH` variable.

`ondera --scan-plugin native <path>` probes one library and prints its descriptors as JSON.
Native plugins appear in the browser as **Ondera Native** with an accent swatch and in
`plugin.list --format native`.

## State and presets

A native plugin's state is its parameter list (the same JSON array the stock library has always
saved), so any session that references a plugin restores exactly; a plugin with `save` data of
its own adds it as described above. `preset.save` and
`preset.load`, and the Presets menu in a plugin window, store named parameter sets in the
presets folder; the stock library ships factory presets.

## ABI

Plugin authors never touch this; hosts other than Ondera can consume it.

**ABI 1 (frozen).** An `ondera_plugin_entry()` symbol returns an
`Entry { abi_version: 1, plugin_count, plugin(index) }`; each `PluginVTable` holds `manifest`
(JSON metadata), `create`, `destroy`, `set_param`, `process`, `reset`, `latency` and
`free_bytes`. No `repr(C)` layout of ABI 1 ever changes: `sdk/src/ffi.rs` asserts the sizes and
offsets at compile time, and `plugins/abi1-fixture`, a plugin with its own frozen copy of the
declarations and no dependency on the SDK, is loaded by `engine/tests/abi1_plugin.rs`.

**ABI 2 (additive).** A second symbol, `ondera_plugin_entry_v2()`, returns an
`Entry2 { abi_version: 2, ... }` whose `PluginVTable2` is `size`, then the whole ABI 1 table as
`base`, then `process_events`, `save`, `load` and `tail_seconds`. `size` is the table's byte
length as the plugin was built, so later revisions can append functions without a third
symbol: a host calls only what fits.

`export_plugins!` exports both symbols. So:

| | ABI 1 host (Ondera 0.3 to 0.7) | ABI 2 host |
|---|---|---|
| plugin built with the 0.7 SDK or older | works | works, through its ABI 1 table |
| plugin built with the current SDK | works, through `process` | works, through `process_events` |

An entry that announces an ABI newer than the host knows is refused with a clear message rather
than guessed at, and the host falls back to the library's ABI 1 entry.

## Limits

- No custom editor window yet: Ondera draws knobs and menus from the parameter metadata.
- Stereo only, one audio bus; no sidechain or MIDI output.
- State is saved from a main-thread instance: what `process` changes in its own fields is not
  captured. Keep what the sound depends on in parameters or in what `load` was given.

## Testing

`ondera_plugin::testing::Bench` drives a plugin through the same vtable the host uses, in
host-sized blocks, with a moving transport:

```rust
use ondera_plugin::testing::Bench;

let mut bench = Bench::<Gain>::new(48_000.0);
bench.set("Gain", -6.0);                       // by display name; panics on a typo or a value out of range
let out = bench.sine(440.0, 0.5, 0.25);        // also: silence(seconds, notes), note(pitch, velocity, held, seconds)
Bench::<Gain>::assert_sane(&out);              // finite and not absurdly loud
assert!(Bench::<Gain>::peak(&out) < 0.3);
```

Those calls go through the ABI 1 table, as an older host would. ABI 2 has its own:

```rust
let ramp = [bench.at(0, "Gain", -60.0), bench.at(240, "Gain", 0.0)];       // frames from the start of the audio
let events = [Event::note_on(0, 60, 100), Event::pitch_bend(4800, 1.0), Event::control(9600, 1, 127)];
let out = bench.play(0.5, &events, &ramp);      // or process_events(&mut audio, &events, &params)
let state = bench.save();                       // empty when the plugin keeps none
bench.load(&state).unwrap();
assert_eq!(bench.tail_seconds(), 0.0);
assert!(!bench.latency_changed());              // true once after latency() returned a new value
```

## When a plugin panics

Every call into a plugin is guarded. A panic poisons that instance: the host never calls it
again, an effect passes audio through, an instrument falls silent, and the session keeps
playing. Fix the bug and reload; do not rely on the guard as control flow.

## Sound folders

The browser files plugins by sound. A native plugin chooses its folder by using one of these
as its category: Dynamics, EQ & Filter, Distortion, Modulation, Space & Time, Pitch,
Channel Strips, Mastering, Restoration, Utility. Instruments are filed by name (Synths, Keys,
Bass, Drums, Pads, Samplers, Textures); the user can re-file anything from the browser.
