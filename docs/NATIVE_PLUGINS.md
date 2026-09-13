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

`plugins/gain` in the repository is a complete example with two plugins (`Trim`, `Tilt EQ`).

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
saved), so any session that references a plugin restores exactly. `preset.save` and
`preset.load`, and the Presets menu in a plugin window, store named parameter sets in the
presets folder; the stock library ships factory presets.

## ABI

`ondera_plugin::ffi` freezes ABI version 1: an `ondera_plugin_entry()` symbol returns an
`Entry { abi_version, plugin_count, plugin(index) }`; each `PluginVTable` holds `manifest`
(JSON metadata), `create`, `destroy`, `set_param`, `process`, `reset`, `latency` and
`free_bytes`. Hosts other than Ondera can consume it; plugin authors never touch it directly.
A library built for another ABI version is refused with a clear message rather than guessed at.

## Limits in 0.3

- No custom editor window yet: Ondera draws knobs and menus from the parameter metadata.
- Stereo only, one audio bus, note input for instruments; no sidechain or MIDI output.
- State beyond parameters is not captured; keep everything the sound depends on in parameters.
