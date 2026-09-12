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
| Audio Units (v2 API) | macOS | AudioToolbox | effects, music effects, instruments and generators; AUv3 units appear through the same registry |

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
  Each strip has eight insert slots; drag order with the context menu, bypass with the LED.
- Clicking an insert opens its parameter panel: knobs for every automatable parameter, drawn
  from the plugin's own metadata. Parameter values live in the session, so they undo, redo and
  save like every other edit.
- **Open plugin window** shows the plugin's native editor (macOS: an `NSWindow`; CLAP Cocoa,
  VST3 `NSView`, AU Cocoa view or the generic CoreAudioKit view). Native editors on Linux and
  Windows are not wired yet; the parameter panel works everywhere.
- Plugin state (presets, internal settings) is captured from every loaded plugin when saving or
  bouncing and restored when the session opens.

## Engine rules

- Plugin instances are created, activated and destroyed on the UI thread, which is the process
  main thread. Their processors are moved into the audio thread's **rack** through the command
  queue and come back through the retirement queue before they are destroyed.
- The rack outlives graph rebuilds. Editing a fader or a note rebuilds the lightweight renderer
  but never re-instantiates plugins, so reverb tails, synth voices and external state survive.
  When a renderer is replaced it adopts the old one's held notes and releases the notes that no
  longer exist.
- Nothing in the audio callback allocates, locks, logs or does I/O. Parameter changes and notes
  are delivered as events at block boundaries (256 frames maximum per block).
- Latency reported by plugins is available (`Editor::latency`) but not yet compensated.

## Stock library

Effects: Ondera Comp, Channel EQ, Tape Sat, Chorus, Space (reverb), Echo (tempo-synced delay),
Gate, Limiter, Filter (SVF), Phaser, Tremolo, Bitcrusher, Stereo Width, Utility, Overdrive,
Transient. Instruments: Ondera Synth (ADSR, filter envelope, three waves), E-Piano Mk I, Drum
Machine, Sampler, Sub Bass 808, Glass Keys, Choir Pad, Riser. The A bus carries Space and the
B bus carries Echo by default; both can be replaced.
