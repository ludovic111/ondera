# Plugins in Ondera

Ondera hosts external audio plugins alongside its own stock library. Every insert and every
instrument is the same kind of object to the engine: a `Processor` that runs on the audio thread
and an `Editor` that stays on the main thread (parameters, state, native window).

## Formats

| Format | Platforms | Loader | Notes |
| --- | --- | --- | --- |
| Ondera stock | all | in process | 16 effects and 8 instruments, see below |
| CLAP | macOS, Linux, Windows | `clap-sys` bindings, `clap_entry` | main-thread / audio-thread split as the spec defines |
| VST3 | macOS, Linux, Windows | `vst3` COM bindings, `GetPluginFactory` | component + edit controller, `IPlugView` editors |
| Audio Units | macOS | AudioToolbox component API | effects, music effects, instruments and generators exposed by the system registry |

VST2 and AAX are not supported. Use a plugin's CLAP, VST3 or supported Audio Unit installation.
The binary architecture must match Ondera: Apple Silicon and Intel Mac builds are separate.
Format support and a successful scan do not guarantee compatibility with every installed unit.

Plugin ids are stable strings stored in the session: `stock:<name>`, `clap:<plugin id>`,
`vst3:<class id hex>` and `au:<type>:<subtype>:<manufacturer>`. VST3 class ids are the bytes the
plugin reports on the platform that scanned it.

## Scanning

**Audio > Rescan plugins** (or the *Scan plugins* button under the Instruments and Effects tabs)
looks in the standard directories plus `CLAP_PATH` / `VST3_PATH`:

- macOS: `~/Library/Audio/Plug-Ins/{CLAP,VST3}`, `/Library/Audio/Plug-Ins/{CLAP,VST3}`, the Audio Unit registry
- Linux: `~/.clap`, `/usr/lib/clap`, `/usr/local/lib/clap`, `~/.vst3`, `/usr/lib/vst3`, `/usr/local/lib/vst3`
- Windows: `%COMMONPROGRAMFILES%\{CLAP,VST3}`, `%LOCALAPPDATA%\Programs\Common\{CLAP,VST3}`

CLAP and VST3 bundles are probed in a child process (`ondera --scan-plugin <format> <path>`) with
a 30-second timeout, so a crashing or hanging plugin only loses its own entry. Results are cached
in `plugins.json` under the application data directory (`~/Library/Application Support/Ondera`,
`~/.config/ondera` or `%APPDATA%\Ondera`; override with `ONDERA_DATA_DIR`). Unchanged bundles are
not probed again. `ondera --plugins` prints the cache; `ondera --scan-plugins` rescans from a
terminal.

## Using plugins

- **Instruments tab**: double-click loads the plugin on the selected instrument track (or a new
  one). The inspector's *Instrument* row switches between stock presets and external instruments.
- **Effects tab**: double-click inserts on the selected strip: a track, the **A** / **B** aux
  buses or the **master** (Track menu, the transport's *Master* label, or double-click a send).
  Each strip has eight insert slots; reorder with the context menu, bypass with the LED.
- Clicking an insert opens its parameter panel: knobs for every automatable parameter, drawn
  from the plugin's own metadata. Parameter values live in the session, so they undo, redo and
  save like every other edit.
- **Open plugin window** shows the plugin's native editor (macOS: an `NSWindow`; CLAP Cocoa,
  VST3 `NSView`, AU Cocoa view or the generic CoreAudioKit view). Native editors on Linux and
  Windows are not wired yet; the parameter panel works everywhere.
- Plugin state (presets, internal settings) is captured from every loaded external plugin when
  saving, bouncing or checking whether to close/replace the document, and restored when the
  session opens. Changed native-editor state becomes one undoable document edit, so native-only
  changes also trigger the save/discard prompt. Later parameter undo returns to the captured
  preset's value. Capturing an opaque native state can mark the project dirty while read
  automation is moving plugin controls; manual parameter overrides are preserved so reopening
  or disabling a lane restores its manual value.

## CLI, MCP and Agents

The Agents tab, CLI and MCP expose the same plugin registry. First scan, then search the cache
and copy a stable ID from the returned page:

```sh
ondera-cli plugin.scan
ondera-cli plugin.list --kind effect --format stock --query limiter
ondera-cli strip.setPlugin --trackId master --slot 7 --pluginId stock:Limiter
ondera-cli strip.parameters --trackId master --slot 7
```

`plugin.scan` returns a count and any scan errors. `plugin.list` accepts `query` (name, vendor or
ID), `kind` (`instrument` or `effect`) and `format` (`stock`, `clap`, `vst3` or `au`). It returns
50 entries by default; `limit` accepts 1–200, and the returned `nextOffset` can be passed as
`offset` for the next page. `session.catalog` lists only the 24 stock plugins, presets and loops.

Use `strip.setParameter --trackId master --slot 7 --parameterId <id> --value <plain-value>`
with an ID and the bounds returned by `strip.parameters`. Values are in plugin units, not a
generic normalized 0–1 range. `strip.setBypass` preserves the plugin's settings. All eight slots
are available, indexed 0–7; targets are track IDs, `master`, `bus-a` or `bus-b`. To install an
instrument on a MIDI track, call `strip.setPlugin` with that track ID and omit `slot`.

`strip.getState` reads the persisted parameters and base64 plugin state; save first to capture
changes made in a native plugin editor. `strip.setState` restores the matching plugin's blob
and clears earlier explicit parameter overrides. State formats belong to the plugin and are
not interchangeable between different plugin IDs or formats.

Live save/export capture plugin state before the worker begins. Scans and file operations
return their result after completion while the window continues to repaint. Headless exports
load the required stock/external processors and restore saved state; a missing or failed plugin
reports an error rather than silently exporting a different mix.

## Engine rules

- During live playback, plugin instances are created and activated by the UI host. Their
  processors are moved into the audio thread's **rack** through the command
  queue and come back through the retirement queue before they are destroyed.
- The rack outlives graph rebuilds. Editing a fader or a note rebuilds the lightweight renderer
  but never re-instantiates plugins, so reverb tails, synth voices and external state survive.
  When a renderer is replaced it adopts the old one's held notes and releases the notes that no
  longer exist.
- Nothing in the audio callback allocates, locks, logs or does I/O. Parameter changes and notes
  are delivered as events at block boundaries (256 frames maximum per block).
- Static plugin delay compensation uses the latencies reported when a graph is prepared.
  It aligns track paths before sends/mixing and aligns dry/aux paths before the master. Offline
  export discards the initial plugin latency so the exported song starts on its musical timeline.
  The compensation plan is bounded to ten seconds of total latency and 64 MiB of delay buffers;
  excessive reports return an error. Dynamic latency-change notifications, arbitrary sidechain
  routing and audio-interface recording latency compensation are not implemented.

## Stock library

Effects: Ondera Comp, Channel EQ, Tape Sat, Chorus, Space (reverb), Echo (tempo-synced delay),
Gate, Limiter, Filter (SVF), Phaser, Tremolo, Bitcrusher, Stereo Width, Utility, Overdrive,
Transient. Instruments: Ondera Synth (ADSR, filter envelope, three waves), E-Piano Mk I, Drum
Machine, Sampler, Sub Bass 808, Glass Keys, Choir Pad, Riser. The A bus carries Space and the
B bus carries Echo by default; both can be replaced.
