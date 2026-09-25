# Ondera user guide

This guide covers everything you can do in the Ondera window, from a first beat to an exported
mix. Every action here also exists as a command, so the built-in agent, `ondera-cli` and MCP
clients can do the same things: see [AI_CONTROL.md](AI_CONTROL.md) and the generated
[command reference](COMMANDS.md). Keyboard shortcuts are listed in [SHORTCUTS.md](SHORTCUTS.md)
and in the app under Help > Shortcuts and Help (⌘/). ⌘ is Ctrl on Windows and Linux.

## Contents

1. [The window](#the-window)
2. [Your first song in five minutes](#your-first-song-in-five-minutes)
3. [Tracks](#tracks)
4. [Instruments, loops and the browser](#instruments-loops-and-the-browser)
5. [Regions in the arrangement](#regions-in-the-arrangement)
6. [Editing MIDI](#editing-midi)
7. [Recording](#recording)
8. [Mixing](#mixing)
9. [Automation](#automation)
10. [Plugins](#plugins)
11. [Files: save, import, export, recover](#files-save-import-export-recover)
12. [The agent](#the-agent)
13. [Themes and appearance](#themes-and-appearance)
14. [Settings](#settings)
15. [Limits](#limits)

## The window

From top to bottom and left to right:

- **Title bar**: the menus (File, Edit, Track, Mix, Agent, View, Help), the song's name (a `*`
  means unsaved changes) and update notices.
- **Transport**: go to start, rewind and forward one bar, Play, Stop, Record and Cycle; the
  display with position (bars · beats · ticks), SMPTE time, tempo, time signature and key (click
  or drag a value to change it); Click (metronome) and Snap; the master meter, CPU and the Agent
  button. On a narrow window the least important readouts hide first.
- **Browser** (left): tabs for Instruments, Loops, Plugins and Files, with a search field.
- **Arrangement** (centre): the toolbar (Pointer, Pencil and Scissors tools, grid, cycle range,
  Follow, Zoom), the ruler with markers and the cycle range, the track headers and the lanes.
- **Editor** (below the arrangement): Piano Roll, Score or Step for the selected MIDI region, with
  an optional controller lane. The Mixer (View > Mixer, `X`) takes its place when open.
- **Inspector** (right): the selected track's instrument, input and output, Channel EQ, the eight
  insert slots, the two sends, pan and fader, and the selected region's properties.
- **Agent panel** (right edge): a conversation with the built-in assistant. It folds into a thin
  rail when closed (`⌘J` toggles it).

The command palette (`⌘P` or View > Command Palette) finds any action by name.

## Your first song in five minutes

1. **File > New session** (`⌘N`). A new song has three tracks: **Drums** on the Drum Machine,
   **Bass** on Analog Bass and an audio track, **Vocals**, to record into.
2. **Add a beat**: open the browser's **Loops** tab and double-click *Four Floor 124*, or drag it
   onto the Drums lane. Loops are ordinary MIDI regions you can edit.
3. **Write a bass line**: choose the Pencil tool (`2`), drag across the Bass lane to draw a
   two-bar region, then click in the piano roll below to place notes. Drag a note to move it,
   drag its right edge to change its length.
4. **Hear it**: press Space. Press `C` to loop the cycle range; drag across the ruler to set it.
5. **Name the sections**: move the playhead and press `⇧M` to add a marker, double-click it to
   rename it (*Verse*, *Chorus*…).
6. **Mix**: select a track and use the inspector: pick an effect in an insert slot, turn the
   sends to the reverb (A) and delay (B) buses, set pan and level. `X` opens the full mixer.
7. **Save** with `⌘S`, then **export** with File > Export audio… (`⌘B`): WAV, AIFF, FLAC or Ogg
   Vorbis, as a stereo mix or one file per track.

## Tracks

- **Add** an instrument track or an audio track from the Track menu or the `+` above the track
  headers (`⌥⌘S` and `⌥⌘A`). Importing a file or double-clicking a loop also adds a track when
  needed.
- **Header controls**: M (mute), S (solo), record arm, and on audio tracks the monitoring button;
  the small fader and pan dial mirror the inspector.
- **Rename** in the inspector. Right-click a header to duplicate, recolour, move or delete the
  track, or to ask the agent about it.
- **Select** a track by clicking its header; the inspector and the editor follow the selection.
- **Duplicate** (Track > Duplicate) copies the regions, the instrument, the inserts and their
  settings.
- Keyboard: `M`, `S` and `A` mute, solo and arm the selected track; `I` cycles its monitoring.

## Instruments, loops and the browser

- **Instruments** tab: the stock instruments (Ondera Synth, E-Piano Mk I, Drum Machine, Sampler,
  Sub Bass 808, Glass Keys, Choir Pad, Riser, Tonewheel Organ, String Ensemble, Analog Bass) and
  every installed instrument plugin, filed in colour-coded sound folders. Double-click to put it
  on the selected instrument track (or a new one), or drag it onto a track.
- **Loops** tab: ready-made MIDI phrases, each with the instrument it was written for.
- **Plugins** tab: every effect and instrument (stock, Ondera native, CLAP, VST3 and Audio Units),
  by sound folder, with Favourites and Recent at the top. Click the star to favourite a plugin;
  right-click to move it to another folder, including folders you create.
- **Files** tab: audio and MIDI files to import.
- **Preview** (bottom of the browser) plays a sound before you choose it.
- Search matches names and vendors. One row stands for all of a plugin's channel layouts; Ondera
  loads the layout a stereo track needs.

## Regions in the arrangement

- **Create** a MIDI region by drawing with the Pencil tool or double-clicking an empty MIDI lane.
  Audio regions come from recordings, imports and dropped files.
- **Select** by clicking; **move** by dragging (also to another track of the same kind);
  **resize/trim** by dragging either edge; **split** at the playhead with `⌘T` or click with the
  Scissors tool; **duplicate** with `⌘D`; delete with `⌫`.
- **Copy, cut and paste** (`⌘C`, `⌘X`, `⌘V`) paste at the playhead on the selected track. The
  clipboard is shared with the CLI and the agent.
- **Open in the editor** with a double-click or `E`.
- **Audio fades**: drag the handles at an audio region's top corners, or type the fade lengths in
  the inspector. Choose equal power, linear or exponential curves. **Region gain** runs from −60
  to +24 dB; the waveform shows the result.
- **Markers**: `⇧M` adds one at the playhead, `⇧N` and `⇧B` jump to the next and previous, `⇧C`
  loops the section you are in. Drag a marker to move it, double-click to rename, right-click to
  recolour or delete.
- **Cycle**: drag across the ruler to set the range, `C` to toggle it.
- **Zoom and scroll**: the Zoom slider, `⌘=` / `⌘-`, `Z` to fit the whole song; `F` toggles
  following the playhead.
- **Edit menu** on the selected MIDI region: quantize, humanize, velocity crescendo and
  diminuendo, legato, reverse, fit to a scale, repeat, and transposition (`⌥↑`/`⌥↓` by a semitone,
  with `⇧` by an octave).

## Editing MIDI

- **Piano Roll**: click to add a note (at the editor's velocity), drag to move, drag the right
  edge to resize, right-click for velocity or deletion. Quantize and the scale guide are in the
  editor header.
- **Score** shows the region as notation; **Step** toggles notes on a grid, handy for drums.
- **Controller lane** (View > Controller Lane, `L`): choose Mod Wheel, Expression, Sustain, Pitch
  Bend, Pressure, Volume, Pan, Breath or any CC number. Click to add a point, drag to move, draw
  a curve by dragging on empty space, Option-click or right-click to delete.
- **Musical typing** (`⌘K`): the letter row from A to ; plays the selected instrument; Z and X
  change the octave. A hardware MIDI keyboard works the same way (Settings > Audio > MIDI Input):
  notes, the mod wheel, pitch bend, the sustain pedal, pressure and other controllers play live.

## Recording

1. Choose your interface in Settings > Audio (input, output, buffer size).
2. Arm the track (the round button, or `A`). Audio tracks record the input; instrument tracks
   record MIDI from the keyboard or from musical typing.
3. Press Record (`R`), then Play, or just Record: a count-in (Settings > Audio > Count In Bars,
   one bar by default) clicks before the take while the song stays parked. The status line shows
   "Count-in…".
4. Stop to keep the take. Audio takes are also kept as separate WAV files in Ondera's data folder.

**Monitoring** lets you hear the input through the track's inserts, sends and fader. The button
beside Arm cycles Off, Auto (while armed and while recording, until the track plays its own
region) and On; `I` does the same. A built-in microphone into built-in speakers would howl, so
that pairing stays muted until you choose *Monitor Anyway*; headphones are not affected.

MIDI takes keep controllers (mod wheel, bend, sustain, pressure…) in the region, where the
controller lane shows them.

## Mixing

- **Inspector**: Channel EQ, eight insert slots, sends A and B, pan and fader for the selected
  track. Click an insert to open its panel: stock plugins have a front panel with a live display;
  external plugins show their parameters and can open their own window.
- **Mixer** (`X`): one strip per track plus the reverb bus (A), the delay bus (B) and the master,
  each with inserts, sends, pan, fader, mute, solo and meter.
- **Buses**: A is a reverb and B a delay by default; any effect can go on them. Show their strips
  from the Track menu.
- **Master**: its own eight inserts (put a limiter last) and the master fader.
- **Faders**: 0 dB is unity. Double-click a dial to reset it; drag with ⇧ for fine steps.
- **Plugin delay compensation** keeps parallel paths aligned.
- Stock effects glide between parameter values in about 10 ms, so moving a dial or fast
  automation does not click.

## Automation

View > Automation opens lanes for track volume and pan, the master fader, and any parameter of
any instrument or insert, including those on the buses and the master. Double-click to add a
point, drag to move, right-click to delete, or type exact values. Each lane can be linear or
stepped and can be switched off. Parameter changes land on their exact sample for CLAP, VST3,
native and stock plugins; Audio Units follow in short slices. Hover a stock plugin's dial and
click its automation button to open that parameter's lane.

## Plugins

- **Formats**: stock plugins (built in), Ondera native plugins written in Rust with the SDK
  ([NATIVE_PLUGINS.md](NATIVE_PLUGINS.md)), CLAP, VST3 and, on macOS, Audio Units. VST2 and AAX
  are not supported.
- **Scanning** runs in a separate process, so a crashing plugin cannot take the app down. Mix >
  Rescan plugins refreshes the list; Settings > Plugins adds folders and can scan at start.
- **Presets**: every stock plugin has factory presets and you can save your own from its panel.
- **State**: an external plugin's own state is saved with the song, so its sound comes back when
  you reopen it and when you export.
- See [PLUGINS.md](PLUGINS.md) for hosting details and limits.

## Files: save, import, export, recover

- **Save** (`⌘S`) and **Save as** (`⇧⌘S`) write a `.ondera` file with the audio embedded. Saving
  never leaves a half-written file: the old one is kept if anything fails.
- **Open** (`⌘O`) and **Open demo** (File menu) load a song. One song has one editing owner at a
  time, across windows and scripts.
- **Import audio** (`⌘I`, or drop files): WAV, AIFF/AIFC, CAF, FLAC, MP3, AAC/M4A/MP4, Ogg Vorbis
  and the sound of video files (MKV, WebM…). Surround files are folded to stereo.
- **Import and export MIDI** (File menu): notes and controllers, optionally the file's tempo and
  time signature.
- **Export audio** (`⌘B`): a stereo mix or one stem per track; WAV, AIFF, FLAC or Ogg Vorbis;
  44.1, 48 or 96 kHz; 16 or 24-bit PCM or 32-bit float for WAV; optional dither; the whole song
  or a bar range; up to 120 seconds of release tail. The report lists the files, the peak and any
  clipping. There is no MP3 export.
- **Recovery**: an edited song gets a recovery copy every 30 seconds (Settings > General).
  File > Recover session… lists them; opening one never overwrites your original.

## The agent

The panel at the right edge is a music assistant that works inside your song. Choose a service in
Settings > Agent: Codex or Claude Code (they use their own sign-in), the Anthropic or OpenAI API
with a key, or any OpenAI-compatible endpoint. Then describe what you want in your own words and
language: "a busier bass line in the second verse", "glue the drums a little", "why is the keys
track silent?".

- Each step the agent takes appears in the conversation in plain words; the **Changes** tab lists
  every edit it made with Undo and Redo. Everything it does is an ordinary undo step.
- **Ask Agent About Selection** (`⇧⌘J`, or right-click a region, lane or track) sends what you
  selected along with your message.
- **Takes A/B** keeps a protected original while the agent explores a variation; **Rhythm Lab**
  builds drum patterns.
- **Permissions** (Settings > Agent) decide whether the agent may touch files, the transport,
  replace the session, change settings or control the application. They also apply to MCP
  clients.

Scripts and external AI tools control Ondera through the same commands: see
[AI_CONTROL.md](AI_CONTROL.md).

## Themes and appearance

Settings > Interface shows every theme as a live miniature, with a Dark / Light / Auto switch
(Auto follows the system). There are six themes, each in dark and light:

- **Modern**: flat and quiet, one cool neutral and one accent.
- **Skeuomorphic**: milled hardware; graphite by night, champagne aluminium by day.
- **Frutiger Aero**: glass, water and sky.
- **Console**: a warm analogue desk with walnut, brass keys and amber meters.
- **Ink**: paper and ink, flat and square, one red accent; selected things are shown reversed.
- **Neon**: violet glass lit from inside, magenta and cyan.

Scripts and the agent switch them too (`settings.set` with `interface.appearance` set to
`modern`, `skeuo`, `aero`, `console`, `ink` or `neon`, and `interface.mode` set to `dark`,
`light` or `auto`). The interface scale, tooltips and following the playhead are set there too.

## Settings

Settings (`⌘,`) is organised in sections:

- **General**: reopen the last song, confirm before quitting, recovery interval.
- **Interface**: theme and mode, scale, tooltips, open the agent panel at start, follow playhead.
- **Audio**: output and input devices, buffer size (64 to 2048 frames), count-in bars, input meter
  on armed tracks, MIDI input and connecting it at start.
- **Plugins**: extra CLAP, VST3 and native folders, scan at start.
- **Agent**: provider, model, reasoning effort, keys, custom instructions, limits and permissions.
- **Control**: the local bridge that `ondera-cli` and `ondera-mcp` use to reach the window.
- **Updates**: check at start, install automatically.

Settings live in `settings.json` in Ondera's data folder, readable only by you; keys are never
shown in full once saved.

## Limits

- No time stretching, comping, tempo changes inside a song or user-defined buses yet.
- Recording latency is not measured or compensated automatically.
- External plugin windows open on macOS; on Windows and Linux external plugins show their
  parameter list.
- Builds are signed ad hoc, not notarized: see the README for the first launch on macOS.
