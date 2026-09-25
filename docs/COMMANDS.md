# Command reference

<!-- Generated from the command registry by tools/tests/command_docs.rs. Do not edit by hand: run `ONDERA_BLESS=1 cargo test -p ondera-tools --test command_docs`. -->

Ondera has 177 commands. The window, `ondera-cli`, `ondera-mcp` and the built-in agent all run these same commands, with the same undo history. On the CLI a command is `ondera-cli <name> --param value`; in MCP it is the tool `<name>` with the dot replaced by an underscore (`track.add` is `track_add`); the agent sees the same tools.

Conventions: bars and beats are zero-based; note `start` and `length` are beats relative to their clip; pitch 60 is C4; velocity is 1–127; a fader value of 0.75 is unity gain. Strip commands accept a track id or `master`, `bus-a`, `bus-b`; insert slots are 0–7.

**Edits** marks a command that can change the song, the transport, settings or files (it is one undo step when it changes the song). **Needs the app** marks a command only the running window can serve; the others also work on a file (`ondera-cli --file song.ondera …`).

## Families

- [session](#session) — `session.info`, `session.get`, `session.inspect`, `session.catalog`, `session.commands`, `session.new`, `session.open`, `session.save`, `session.rename`, `session.bounce`, `session.importAudio`, `session.importMidi`, `session.exportMidi`, `session.exportAudio`, `session.exportStems`, `session.batch`, `session.saveRecoveredTake`, `session.snapshots`, `session.restoreSnapshot`
- [plugin](#plugin) — `plugin.list`, `plugin.scan`, `plugin.folders`, `plugin.setFavorite`, `plugin.setFolder`, `plugin.scaffold`, `plugin.install`, `plugin.describe`
- [transport](#transport) — `transport.play`, `transport.record`, `transport.stop`, `transport.locate`, `transport.returnToStart`, `transport.setTempo`, `transport.setTimeSignature`, `transport.setKey`, `transport.setCycle`, `transport.setMetronome`, `transport.setSnap`, `transport.punch`
- [track](#track) — `track.list`, `track.add`, `track.remove`, `track.rename`, `track.setMute`, `track.setSolo`, `track.setArmed`, `track.setMonitor`, `track.setVolume`, `track.setPan`, `track.setColor`, `track.move`, `track.select`, `track.duplicate`
- [clip](#clip) — `clip.list`, `clip.get`, `clip.create`, `clip.move`, `clip.resize`, `clip.rename`, `clip.split`, `clip.duplicate`, `clip.copy`, `clip.cut`, `clip.paste`, `clip.remove`, `clip.setNotes`, `clip.addLoop`, `clip.select`, `clip.trim`, `clip.deselect`, `clip.setFades`, `clip.setGain`, `clip.humanize`, `clip.velocityRamp`, `clip.fitScale`, `clip.reverseMidi`, `clip.legato`, `clip.repeat`, `clip.quantize`, `clip.transpose`
- [note](#note) — `note.list`, `note.add`, `note.update`, `note.remove`, `note.preview`, `note.hold`, `note.releaseAll`
- [strip](#strip) — `strip.get`, `strip.setInstrument`, `strip.setInsert`, `strip.setSendLevel`, `strip.setPlugin`, `strip.parameters`, `strip.setParameter`, `strip.setParameters`, `strip.setBypass`, `strip.getState`, `strip.setState`, `strip.moveInsert`
- [master](#master) — `master.setVolume`
- [history](#history) — `history.undo`, `history.redo`, `history.info`
- [source](#source) — `source.peaks`
- [marker](#marker) — `marker.list`, `marker.add`, `marker.rename`, `marker.move`, `marker.setColor`, `marker.remove`, `marker.goto`, `marker.next`, `marker.previous`, `marker.cycleSection`
- [automation](#automation) — `automation.list`, `automation.create`, `automation.setPoints`, `automation.setPoint`, `automation.removePoint`, `automation.setEnabled`, `automation.setInterpolation`, `automation.remove`
- [controller](#controller) — `controller.list`, `controller.add`, `controller.update`, `controller.remove`, `controller.setPoints`
- [rhythm](#rhythm) — `rhythm.create`, `rhythm.preview`
- [take](#take) — `take.list`, `take.create`, `take.select`, `take.remove`
- [view](#view) — `view.get`, `view.fit`, `view.set`
- [preset](#preset) — `preset.list`, `preset.save`, `preset.load`, `preset.delete`
- [settings](#settings) — `settings.get`, `settings.set`, `settings.reset`
- [audio](#audio) — `audio.devices`, `audio.status`, `audio.allowSpeakerMonitoring`, `audio.setOutput`, `audio.setInput`, `audio.setMidiInput`, `audio.reconnect`
- [ui](#ui) — `ui.screenshot`, `ui.showPanel`, `ui.openPluginWindow`, `ui.closePluginWindow`, `ui.dismissError`, `ui.closePluginWindows`, `ui.musicalTyping`, `ui.setTool`, `ui.status`
- [app](#app) — `app.info`, `app.checkUpdates`, `app.installUpdate`, `app.quit`, `app.confirm`, `app.openGuide`, `app.relaunch`
- [agent](#agent) — `agent.status`, `agent.configure`, `agent.providers`, `agent.models`, `agent.connection`, `agent.send`, `agent.stop`, `agent.transcript`, `agent.changes`, `agent.revert`, `agent.clear`

## session

### `session.info`

Summarise the open session: name, file, transport, counts, selection and history state.

### `session.get`

Return the complete session document as JSON (tracks, clips with notes, sources, strips, transport, view).

### `session.inspect`

Inspect the arrangement, mixer and automation without opaque plugin state. Clips are summaries by default; use clip.get for individual notes.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `includeNotes` | boolean |  | Include all MIDI notes instead of clip summaries (default false). |

### `session.catalog`

List built-in instruments, effects and bundled MIDI loops.

### `session.commands`

Describe every command with its parameters.

### `session.new`

*Edits*

Replace the open session with an empty one (or the bundled Nightfall demo). Unsaved changes are discarded.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `demo` | boolean |  | Load the Nightfall demo instead of an empty session. |

### `session.open`

*Edits*

Open a .ondera session file, replacing the current session. Unsaved changes are discarded.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `path` | string | yes | Path to a .ondera file. |

### `session.save`

*Edits*

Save the session as a .ondera file. Writes atomically; the old file survives a failed save.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `path` | string |  | Destination file. Defaults to the file the session was opened from. |

### `session.rename`

*Edits*

Set the session name.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `name` | string | yes | New session name. |

### `session.bounce`

*Edits*

Render the whole arrangement offline to a stereo 48 kHz / 24-bit WAV file, including a 3-second effect tail.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `path` | string | yes | Destination .wav path. |

### `session.importAudio`

*Edits*

Decode an audio file (WAV, AIFF, FLAC, MP3, Ogg, AAC) into the session and place it as a clip on an audio track.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `path` | string | yes | Audio file to import. |
| `trackId` | string |  | Audio track to place the clip on. Defaults to the selected audio track, or a new one. |
| `startBar` | number |  | Bar to place the clip at. Defaults to the playhead. |

### `session.importMidi`

*Edits*

Import SMF type 0/1 MIDI into new instrument tracks in one undo step. Quarter-note positions are preserved; reports unsupported controller/tempo-map data.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `path` | string | yes | Source .mid or .midi file. |
| `startBar` | number |  | Zero-based destination bar, default 0. |
| `importTempo` | boolean |  | Apply the file's initial tempo and meter to the entire session (default false). Later changes are reported and ignored. |

### `session.exportMidi`

*Edits*

Export arrangement notes as SMF type 1 at 960 PPQ with initial tempo/meter. Does not convert audio or embed plugins; includes muted tracks.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `path` | string | yes | Destination .mid file, replaced atomically after success. |
| `trackIds` | array |  | MIDI track IDs to export; defaults to all MIDI tracks. |

### `session.exportAudio`

*Edits*

Export an offline stereo WAV, AIFF, FLAC or Ogg Vorbis using the live plugin graph, with chosen rate, format, range and tail. The file type follows the path's extension and every type streams to disk. PCM clips above full scale; float retains headroom. Reports peak/clipping, and the bitrate for Ogg.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `path` | string | yes | Destination .wav, .aiff for AIFF, .flac for lossless FLAC (both pcm16 or pcm24 only) or .ogg for lossy Ogg Vorbis. Replaced atomically after success. |
| `sampleRate` | integer |  | 44100, 48000 (default), or 96000 Hz. |
| `format` | string |  | pcm16, pcm24 (default), or float32. |
| `startBar` | number |  | Zero-based range start bar, default 0. Use bars or beats, never both. |
| `endBar` | number |  | Exclusive end bar; defaults to arrangement end. |
| `startBeat` | number |  | Zero-based start in quarter-note beats instead of bars. |
| `endBeat` | number |  | Exclusive end in quarter-note beats instead of bars. |
| `tailSeconds` | number |  | Effect release tail after range end, 0–120 seconds, default 3. |
| `dither` | boolean |  | TPDF dither for integer PCM, default true. Ignored for float32. |
| `quality` | number |  | Ogg Vorbis quality 0–1, default 0.6 (about 192 kbit/s; 0.4 ≈ 128, 0.8 ≈ 256). Ignored by the other file types, as format and dither are by Ogg. |

### `session.exportStems`

*Edits*

Export one stereo file per track (WAV, or AIFF, FLAC or Ogg Vorbis with `container`) into a new folder, publishing the entire set only on success. Solo-rendered nonlinear/shared effects can prevent exact summation to the full mix.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `directory` | string | yes | New destination folder; an existing folder is never replaced. |
| `trackIds` | array |  | Track IDs to export; defaults to all tracks. Mute/solo are ignored. |
| `includeEffects` | boolean |  | Include track inserts and send/bus processing, default true. |
| `includeMaster` | boolean |  | Apply master inserts/fader to each stem, default false. |
| `sampleRate` | integer |  | 44100, 48000 (default), or 96000 Hz. |
| `container` | string |  | File type of every stem: wav (default), aiff, flac or ogg. aiff and flac need pcm16 or pcm24; ogg uses quality instead of format. |
| `format` | string |  | pcm16, pcm24 (default), or float32. |
| `startBar` | number |  | Zero-based range start bar, default 0. Use bars or beats, never both. |
| `endBar` | number |  | Exclusive end bar; defaults to arrangement end for every stem. |
| `startBeat` | number |  | Zero-based start in quarter-note beats instead of bars. |
| `endBeat` | number |  | Exclusive end in quarter-note beats instead of bars. |
| `tailSeconds` | number |  | Effect release tail after range end, 0–120 seconds, default 3. |
| `dither` | boolean |  | TPDF dither for integer PCM, default true. Ignored for float32. |
| `quality` | number |  | Ogg Vorbis quality 0–1 for container ogg, default 0.6 (about 192 kbit/s). |

### `session.batch`

*Edits*

Run many commands in one request. They share one undo step, and with atomic=true (default) a failing command rolls back every edit made before it. Far faster than separate calls: the window answers the whole list in one frame.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `commands` | array | yes | List of {"command": name, "params": {...}} in the order to run. File, history, application and agent commands are not accepted. |
| `atomic` | boolean |  | Roll back earlier edits when one command fails (default true). |

### `session.saveRecoveredTake`

*Edits · Needs the app*

Write a recording that could not be placed on a track to a WAV file, which frees the window to open other sessions. ui.status reports it as `recoveredTake`.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `path` | string | yes | Destination .wav. |

### `session.snapshots`

List recovery snapshots newest first, with paths, titles, times and sizes.

### `session.restoreSnapshot`

*Edits · Needs the app*

Open a recovery snapshot in the window as an unsaved copy.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `path` | string | yes | Snapshot path from session.snapshots. |

## plugin

### `plugin.list`

Search a page of installed plugins from the scanner cache. Use query/kind/format to avoid returning a large library; follow nextOffset for more. Channel layouts that a vendor registers as separate plugins ("C1 comp (m)", "(s)", "(m->s)") are one row: its id is the layout a stereo track wants and `layouts` lists the others.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `everyLayout` | boolean |  | List each channel layout as its own row instead (default false). |
| `query` | string |  | Case-insensitive name, vendor or plugin ID search. |
| `format` | string |  | stock, native, clap, vst3 or au. |
| `kind` | string |  | instrument or effect. |
| `folder` | string |  | Sound folder from plugin.folders, for example Synths, Drums, Dynamics or Space & Time. |
| `favorite` | boolean |  | Only favourites. |
| `sort` | string |  | name (default) or recent: most recently loaded first. |
| `offset` | integer |  | Zero-based result offset, default 0. |
| `limit` | integer |  | Page size 1-200, default 50. |

### `plugin.scan`

*Edits*

Scan installed plugin directories in isolated child processes and refresh the plugin cache. May take several minutes.

### `plugin.folders`

The sound folders of the plugin library with how many instruments and effects each holds, plus favourites and recents.

### `plugin.setFavorite`

*Edits*

Star or unstar a plugin so it shows under Favourites in the browser.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `pluginId` | string | yes | Plugin id from plugin.list. |
| `favorite` | boolean | yes | Star (true) or unstar (false). |

### `plugin.setFolder`

*Edits*

File a plugin under another sound folder, or a new one of your own. Omit folder to return it to the automatic one.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `pluginId` | string | yes | Plugin id from plugin.list. |
| `folder` | string |  | Folder name, 1-40 characters. |

### `plugin.scaffold`

*Edits*

Start a new Ondera native plugin in Rust: writes a crate with a working effect or instrument, a test that runs it through the real plugin ABI, and build notes. Build it with cargo, then plugin.install.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `path` | string | yes | Directory to create. It must not exist yet. |
| `name` | string | yes | Plugin display name, for example Warm Drive. |
| `kind` | string |  | effect (default) or instrument. |
| `vendor` | string |  | Your name or label, default My Studio. |

### `plugin.install`

*Edits*

Copy a built native plugin library (.dylib, .so, .dll or .onplug) into Ondera's plugin folder. Run plugin.scan afterwards to load it.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `path` | string | yes | The built library, for example target/release/libwarm_drive.dylib. |

### `plugin.describe`

Describe a plugin without placing it: format, vendor, category and every parameter with ids, ranges, units and defaults.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `pluginId` | string | yes | Descriptor ID from plugin.list, for example stock:Space. |

## transport

### `transport.play`

*Edits*

Start playback from the playhead. Needs the Ondera app (live mode).

### `transport.record`

*Edits*

Record armed audio and MIDI tracks in the running app. Disable cycle before recording.

### `transport.stop`

*Edits*

Stop playback and recording.

### `transport.locate`

*Edits*

Move the playhead. Give one of bar, beats or markerId.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `bar` | number |  | Zero-based bar position. |
| `beats` | number |  | Zero-based beat position. |
| `markerId` | string |  | A marker from marker.list: go to its bar. |

### `transport.returnToStart`

*Edits*

Move the playhead to the beginning.

### `transport.setTempo`

*Edits*

Set the tempo.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `bpm` | number | yes | Beats per minute, 20-400. |

### `transport.setTimeSignature`

*Edits*

Set the time signature.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `numerator` | integer | yes | Beats per bar, 1-32. |
| `denominator` | integer | yes | Beat unit: 1, 2, 4, 8, 16 or 32. |

### `transport.setKey`

*Edits*

Set the displayed song key.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `key` | string | yes | Key label such as "C minor". |

### `transport.setCycle`

*Edits*

Enable or disable cycle (loop) playback and optionally set its range.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `enabled` | boolean | yes | Cycle on or off. |
| `startBar` | number |  | Cycle start bar. |
| `endBar` | number |  | Cycle end bar; must be after startBar. |

### `transport.setMetronome`

*Edits*

Enable or disable the click.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `enabled` | boolean | yes | Metronome on or off. |

### `transport.setSnap`

*Edits*

Set the grid snap division.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `division` | integer | yes | Notes per bar: 1, 2, 4, 8, 16, 32 or 64. |

### `transport.punch`

*Edits · Needs the app*

Turn record on or off. While the transport is rolling this punches in or out on the armed tracks without stopping playback; while stopped it only sets the record button.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `enabled` | boolean | yes | Record on or off. |

## track

### `track.list`

List tracks in arrangement order with their instrument and clip count.

### `track.add`

*Edits*

Add a track at the end of the arrangement and select it.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `kind` | string | yes | "midi" for an instrument track or "audio". |
| `name` | string |  | Track name. Defaults to Instrument N / Audio N. |
| `color` | string |  | CSS colour: #rrggbb or oklch(l c h). Defaults to the palette. |
| `instrument` | string |  | Instrument for a MIDI track; see session.catalog. |

### `track.remove`

*Edits*

Delete a track and every clip on it.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `trackId` | string | yes | Track id, as listed by track.list. |

### `track.rename`

*Edits*

Rename a track.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `trackId` | string | yes | Track id, as listed by track.list. |
| `name` | string | yes | New name. |

### `track.setMute`

*Edits*

Mute or unmute a track.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `trackId` | string | yes | Track id, as listed by track.list. |
| `muted` | boolean | yes | Muted or not. |

### `track.setSolo`

*Edits*

Solo or unsolo a track.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `trackId` | string | yes | Track id, as listed by track.list. |
| `solo` | boolean | yes | Soloed or not. |

### `track.setArmed`

*Edits*

Arm or disarm an audio or MIDI track for recording.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `trackId` | string | yes | Track id, as listed by track.list. |
| `armed` | boolean | yes | Armed or not. |

### `track.setMonitor`

*Edits*

Hear the live input through an audio track's inserts, sends and fader. auto monitors while the track is armed and not playing back its own clip (and again while recording); on always; off never. audio.status reports whether the input is routed, the measured latency, and `blocked` when the built-in microphone would feed back through the built-in speakers.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `trackId` | string | yes | Track id, as listed by track.list. |
| `monitor` | string | yes | off, auto or on. |

### `track.setVolume`

*Edits*

Set the fader.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `trackId` | string | yes | Track id, as listed by track.list. |
| `volume` | number | yes | 0.0 (silent) to 1.0 (+6 dB); 0.75 is unity. |

### `track.setPan`

*Edits*

Set stereo pan.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `trackId` | string | yes | Track id, as listed by track.list. |
| `pan` | number | yes | -100 (left) to 100 (right). |

### `track.setColor`

*Edits*

Set the track colour.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `trackId` | string | yes | Track id, as listed by track.list. |
| `color` | string | yes | CSS colour: #rrggbb or oklch(l c h). |

### `track.move`

*Edits*

Move a track to another position.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `trackId` | string | yes | Track id, as listed by track.list. |
| `index` | integer | yes | Zero-based target index. |

### `track.select`

*Edits*

Select a track in the interface.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `trackId` | string | yes | Track id, as listed by track.list. |

### `track.duplicate`

*Edits*

Copy a track with its strip and clips right after the original.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `trackId` | string | yes | Track id, as listed by track.list. |
| `name` | string |  | Name for the copy. Defaults to the original name plus " copy". |

## clip

### `clip.list`

List clips (regions) without their notes.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `trackId` | string |  | Only clips on this track. |

### `clip.get`

Return one clip with its notes or audio reference.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `clipId` | string | yes | Clip id, as listed by clip.list. |

### `clip.create`

*Edits*

Create a MIDI or audio clip. Audio clips need sourceId (see session.get sources).

| Parameter | Type | Required | Description |
|---|---|---|---|
| `trackId` | string | yes | Track id, as listed by track.list. |
| `startBar` | number | yes | Zero-based start bar. |
| `lengthBars` | number | yes | Length in bars, greater than 0. |
| `name` | string |  | Clip name. |
| `notes` | array |  | Array of {start, length, pitch, velocity?} with start/length in beats relative to the clip, pitch 0-127 (60 = C4), velocity 1-127 (default 100). |
| `sourceId` | string |  | Audio source id for a clip on an audio track. |
| `offsetSeconds` | number |  | Seconds into the source where the audio clip starts (default 0). |

### `clip.move`

*Edits*

Move a clip to another bar and/or track of the same kind.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `clipId` | string | yes | Clip id, as listed by clip.list. |
| `startBar` | number |  | New zero-based start bar. |
| `trackId` | string |  | Destination track. |

### `clip.resize`

*Edits*

Change a clip's length.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `clipId` | string | yes | Clip id, as listed by clip.list. |
| `lengthBars` | number | yes | New length in bars. |

### `clip.rename`

*Edits*

Rename a clip.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `clipId` | string | yes | Clip id, as listed by clip.list. |
| `name` | string | yes | New name. |

### `clip.split`

*Edits*

Split a clip at a bar, keeping notes and audio offsets aligned.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `clipId` | string | yes | Clip id, as listed by clip.list. |
| `bar` | number | yes | Absolute bar inside the clip. |

### `clip.duplicate`

*Edits*

Duplicate a clip immediately after itself.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `clipId` | string | yes | Clip id, as listed by clip.list. |

### `clip.copy`

*Edits*

Copy a clip to the clipboard the window, the CLI and agents share. The session does not change.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `clipId` | string |  | Clip to copy; defaults to the selected clip. |

### `clip.cut`

*Edits*

Copy a clip to the shared clipboard and remove it, in one undo step.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `clipId` | string |  | Clip to cut; defaults to the selected clip. |

### `clip.paste`

*Edits*

Paste the clipboard as a new clip. MIDI goes on instrument tracks and audio on audio tracks.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `trackId` | string |  | Destination track. Defaults to the selected track when its kind fits, else the track the clip came from. |
| `bar` | number |  | Zero-based start bar; defaults to the bar the playhead is in. |

### `clip.remove`

*Edits*

Delete a clip.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `clipId` | string | yes | Clip id, as listed by clip.list. |

### `clip.setNotes`

*Edits*

Replace every note of a MIDI clip in one undo step.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `clipId` | string | yes | Clip id, as listed by clip.list. |
| `notes` | array | yes | Array of {start, length, pitch, velocity?} with start/length in beats relative to the clip, pitch 0-127 (60 = C4), velocity 1-127 (default 100). |

### `clip.addLoop`

*Edits*

Insert one of the bundled MIDI loops (see session.catalog) as a new clip, switching the track's instrument to the loop's.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `name` | string | yes | Loop name from session.catalog. |
| `trackId` | string |  | MIDI track. Defaults to the selected MIDI track, or a new one. |
| `startBar` | number |  | Start bar. Defaults to the playhead's bar. |

### `clip.select`

*Edits*

Select a clip and open it in the editor, optionally selecting one of its notes.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `clipId` | string | yes | Clip id, as listed by clip.list. |
| `noteId` | string |  | Note id from note.list to select inside the clip. |

### `clip.trim`

*Edits*

Move a clip's left edge while its content stays where it is on the timeline: notes keep their bar positions and audio keeps its alignment. The right edge does not move unless lengthBars is given.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `clipId` | string | yes | Clip id, as listed by clip.list. |
| `startBar` | number | yes | New absolute start bar of the clip. |
| `lengthBars` | number |  | New length in bars. Defaults to keeping the right edge in place. |

### `clip.deselect`

*Edits*

Clear the clip and note selection, like clicking an empty lane. The selected track stays.

### `clip.setFades`

*Edits*

Set an audio clip's fade in, fade out and fade curve, in one undo step. Omitted values keep theirs. Fades are seconds of audio and are kept inside the clip: when they would overlap they shrink in proportion. Split, trim and resize keep them sensible.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `clipId` | string | yes | Clip id, as listed by clip.list. |
| `fadeInSeconds` | number |  | Fade-in length in seconds; 0 removes it. |
| `fadeOutSeconds` | number |  | Fade-out length in seconds; 0 removes it. |
| `curve` | string |  | equalPower (default: keeps loudness through a crossfade), linear or exponential (slow start). |

### `clip.setGain`

*Edits*

Set an audio clip's gain, applied before the track's inserts and fader.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `clipId` | string | yes | Clip id, as listed by clip.list. |
| `gainDb` | number | yes | Gain in dB, -60 to +24; 0 is unchanged. |

### `clip.humanize`

*Edits*

Humanize MIDI timing and velocity reproducibly without changing pitch, inside region bounds. One undo step.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `clipId` | string | yes | Clip id, as listed by clip.list. |
| `timingMs` | number |  | Maximum timing offset, 0-100 ms (default 10). |
| `velocity` | integer |  | Maximum velocity offset, 0-32 (default 8). |
| `seed` | integer |  | Random seed, 0-4294967295 (default 1). |

### `clip.velocityRamp`

*Edits*

Shape MIDI dynamics from the first to last onset, preserving chords at equal velocity. One undo step.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `clipId` | string | yes | Clip id, as listed by clip.list. |
| `from` | integer | yes | Starting velocity 1-127. |
| `to` | integer | yes | Ending velocity 1-127. |

### `clip.fitScale`

*Edits*

Move MIDI pitches to the closest note in a scale, choosing down on ties. Timing and velocity stay intact.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `clipId` | string | yes | Clip id, as listed by clip.list. |
| `root` | integer | yes | Root pitch class 0-11, C=0. |
| `scale` | string | yes | major, minor, dorian, mixolydian, pentatonicMajor or pentatonicMinor. |

### `clip.reverseMidi`

*Edits*

Reverse MIDI note timing within the region, preserving pitch, duration and velocity.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `clipId` | string | yes | Clip id, as listed by clip.list. |

### `clip.legato`

*Edits*

Extend MIDI notes to the next distinct onset or region end. Simultaneous chord notes remain together.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `clipId` | string | yes | Clip id, as listed by clip.list. |

### `clip.repeat`

*Edits*

Repeat a MIDI or audio region immediately after itself in one undo step, assigning unique IDs.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `clipId` | string | yes | Clip id, as listed by clip.list. |
| `count` | integer | yes | Number of additional copies, 1-64. |

### `clip.quantize`

*Edits*

Snap every note start in a MIDI clip to the grid, in one undo step.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `clipId` | string | yes | Clip id, as listed by clip.list. |
| `division` | integer |  | Notes per bar: 1, 2, 4, 8, 16, 32 or 64. Defaults to the transport snap. |
| `strength` | number |  | How far to move toward the grid, 0-100 percent (default 100). |
| `lengths` | boolean |  | Also quantize note lengths (default false). |

### `clip.transpose`

*Edits*

Shift every note in a MIDI clip by semitones, clamped to 0-127.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `clipId` | string | yes | Clip id, as listed by clip.list. |
| `semitones` | integer | yes | Signed semitones, -48 to 48. |

## note

### `note.list`

List the notes of a MIDI clip.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `clipId` | string | yes | Clip id, as listed by clip.list. |

### `note.add`

*Edits*

Add a note to a MIDI clip.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `clipId` | string | yes | Clip id, as listed by clip.list. |
| `start` | number | yes | Start in beats relative to the clip. |
| `length` | number | yes | Length in beats, greater than 0. |
| `pitch` | integer | yes | MIDI pitch 0-127 (60 = C4). |
| `velocity` | integer |  | 1-127, default 100. |

### `note.update`

*Edits*

Change a note's timing, pitch or velocity.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `clipId` | string | yes | Clip id, as listed by clip.list. |
| `noteId` | string | yes | Note id from note.list. |
| `start` | number |  | Start in beats relative to the clip. |
| `length` | number |  | Length in beats. |
| `pitch` | integer |  | MIDI pitch 0-127. |
| `velocity` | integer |  | 1-127. |

### `note.remove`

*Edits*

Delete a note.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `clipId` | string | yes | Clip id, as listed by clip.list. |
| `noteId` | string | yes | Note id from note.list. |

### `note.preview`

*Edits · Needs the app*

Audition one note on a track's instrument, like clicking a piano-roll key.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `trackId` | string | yes | Track id, as listed by track.list. |
| `pitch` | integer | yes | MIDI pitch 0-127. |
| `velocity` | integer |  | 1-127, default 100. |

### `note.hold`

*Edits · Needs the app*

Hold or release a note on the selected instrument track, like a key on a MIDI keyboard. Held notes are recorded when the transport is recording. Always release what you hold.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `pitch` | integer | yes | MIDI pitch 0-127. |
| `on` | boolean | yes | true presses the key, false releases it. |
| `velocity` | integer |  | 1-127, default 100. |

### `note.releaseAll`

*Edits · Needs the app*

Release every note held with note.hold or musical typing.

## strip

### `strip.get`

Return a track or bus channel strip: instrument, eight inserts and two sends. Bus IDs: master, bus-a, bus-b.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `trackId` | string | yes | Track id, as listed by track.list. |

### `strip.setInstrument`

*Edits*

Choose the instrument of a MIDI track.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `trackId` | string | yes | Track id, as listed by track.list. |
| `instrument` | string | yes | Instrument name from session.catalog. |

### `strip.setInsert`

*Edits*

Load, bypass or clear an insert effect slot.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `trackId` | string | yes | Track id, as listed by track.list. |
| `slot` | integer | yes | Insert slot 0-7. |
| `effect` | string |  | Effect name from session.catalog. Omit it, and bypassed, to empty the slot. |
| `bypassed` | boolean |  | Bypass the effect instead of running it (default false). Without effect, bypasses or enables the effect already in the slot. |

### `strip.setSendLevel`

*Edits*

Set a send level to the reverb (A) or delay (B) bus.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `trackId` | string | yes | Track id, as listed by track.list. |
| `send` | integer | yes | 0 for A · Reverb, 1 for B · Delay. |
| `levelDb` | number |  | Level in dB, -100 to 0. Omit or null for off. |

### `strip.setPlugin`

*Edits*

Load a stock or installed external plugin. Omit slot for a MIDI instrument; pass slot 0-7 for an insert on a track or bus.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `trackId` | string | yes | Track id, as listed by track.list. |
| `slot` | integer |  | Insert slot 0-7. Omit for the instrument. |
| `pluginId` | string | yes | Stable descriptor ID from plugin.list, for example stock:Space. |

### `strip.parameters`

Read a plugin's parameter IDs, plain values, bounds and units. Omit slot for the MIDI instrument.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `trackId` | string | yes | Track id, as listed by track.list. |
| `slot` | integer |  | Insert slot 0-7. Omit for the instrument. |

### `strip.setParameter`

*Edits*

Set a plugin parameter using its plain value and ID from strip.parameters, in one undo step.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `trackId` | string | yes | Track id, as listed by track.list. |
| `slot` | integer |  | Insert slot 0-7. Omit for the instrument. |
| `parameterId` | integer | yes | Parameter ID from strip.parameters. |
| `value` | number | yes | Plain parameter value within its min and max. |

### `strip.setParameters`

*Edits*

Set several plugin parameters atomically in one undo step. Read strip.parameters first.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `trackId` | string | yes | Track id, as listed by track.list. |
| `slot` | integer |  | Insert slot 0-7. Omit for the instrument. |
| `values` | object | yes | Object mapping parameter IDs to plain numeric values. |

### `strip.setBypass`

*Edits*

Bypass or enable a plugin without replacing its settings.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `trackId` | string | yes | Track id, as listed by track.list. |
| `slot` | integer |  | Insert slot 0-7. Omit for the instrument. |
| `bypassed` | boolean | yes | Whether to bypass the processor. |

### `strip.getState`

Capture and read the selected plugin's current parameters and base64 state, including changes from its native editor.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `trackId` | string | yes | Track id, as listed by track.list. |
| `slot` | integer |  | Insert slot 0-7. Omit for the instrument. |

### `strip.setState`

*Edits*

Restore base64 state previously captured from this plugin, replacing its explicit parameter overrides.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `trackId` | string | yes | Track id, as listed by track.list. |
| `slot` | integer |  | Insert slot 0-7. Omit for the instrument. |
| `blob` | string | yes | Base64 plugin state from strip.getState or session.get. |

### `strip.moveInsert`

*Edits*

Move an insert to another slot on the same strip, shifting the others.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `trackId` | string | yes | Track id, as listed by track.list. |
| `from` | integer | yes | Slot 0-7 to move. |
| `to` | integer | yes | Destination slot 0-7. |

## master

### `master.setVolume`

*Edits*

Set the stereo output fader, including offline exports.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `volume` | number | yes | 0.0 (silent) to 1.0 (+6 dB); 0.75 is unity. |

## history

### `history.undo`

*Edits*

Undo the last document edit, or several.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `steps` | integer |  | How many edits to undo, 1-200 (default 1). |

### `history.redo`

*Edits*

Redo the last undone edit, or several.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `steps` | integer |  | How many edits to redo, 1-200 (default 1). |

### `history.info`

Report whether undo and redo are available and the current revision.

## source

### `source.peaks`

The waveform of an audio source as the window draws it: peak magnitudes 0-1, reduced to at most `points` values.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `sourceId` | string | yes | Audio source id, from clip.get or session.inspect. |
| `points` | integer |  | Most values to return, 16-4000 (default 400). |

## marker

### `marker.list`

List the song's markers (sections) in bar order, with the bar where each section ends.

### `marker.add`

*Edits*

Add a marker, the start of a song section, to the ruler.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `bar` | number |  | Zero-based bar. Defaults to the playhead, on the snap grid. |
| `name` | string |  | Section name such as "Verse 1" or "Chorus". Defaults to "Marker N". |
| `color` | string |  | CSS colour: #rrggbb or oklch(l c h). Defaults to the theme's marker colour. |

### `marker.rename`

*Edits*

Rename a marker.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `markerId` | string | yes | Marker id, as listed by marker.list. |
| `name` | string | yes | New name, 1-120 characters. |

### `marker.move`

*Edits*

Move a marker to another bar.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `markerId` | string | yes | Marker id, as listed by marker.list. |
| `bar` | number | yes | Zero-based bar. |

### `marker.setColor`

*Edits*

Colour a marker, or give it back the theme's colour.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `markerId` | string | yes | Marker id, as listed by marker.list. |
| `color` | string |  | CSS colour: #rrggbb or oklch(l c h). Omit or null for the theme's colour. |

### `marker.remove`

*Edits*

Delete a marker.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `markerId` | string | yes | Marker id, as listed by marker.list. |

### `marker.goto`

*Edits*

Move the playhead to a marker, by id or by name.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `markerId` | string |  | Marker id from marker.list. |
| `name` | string |  | Marker name, case-insensitive, when no id is given. |

### `marker.next`

*Edits*

Move the playhead to the next marker after it. Answers with marker null, and leaves the playhead, when there is none.

### `marker.previous`

*Edits*

Move the playhead to the marker before it. Answers with marker null, and leaves the playhead, when there is none.

### `marker.cycleSection`

*Edits*

Cycle one song section: set the cycle from a marker to the next marker (or the end of the song) and turn cycle on.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `markerId` | string |  | Marker that starts the section. Defaults to the section the playhead is in. |

## automation

### `automation.list`

List automation lanes and absolute beat points. Track/master gain and pan are sample-accurate; plugin changes occur at blocks of at most 256 frames.

### `automation.create`

*Edits*

Create read automation for trackVolume, trackPan, masterVolume or pluginParameter. One lane per target; values use the parameter's plain units.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `target` | string | yes | trackVolume, trackPan, masterVolume or pluginParameter |
| `trackId` | string |  | Track ID; master/bus-a/bus-b also accept plugin parameters |
| `slot` | integer |  | Plugin insert 0-7; omit for the track instrument |
| `parameterId` | integer |  | Plugin parameter ID from strip.parameters |
| `name` | string |  | Lane name |
| `interpolation` | string |  | linear (default) or step |
| `points` | array |  | Optional {beat,value,id?} points in absolute quarter-note beats |

### `automation.setPoints`

*Edits*

Replace all points in a lane atomically, with one undo step.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `laneId` | string | yes | Automation lane ID |
| `points` | array | yes | Array of {beat,value,id?}; beats must be distinct |

### `automation.setPoint`

*Edits*

Add or move one automation point.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `laneId` | string | yes | Automation lane ID |
| `pointId` | string |  | Existing point ID to move; omitted creates a point |
| `beat` | number | yes | Absolute quarter-note beat |
| `value` | number | yes | Plain parameter value |

### `automation.removePoint`

*Edits*

Delete an automation point.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `laneId` | string | yes | Automation lane ID |
| `pointId` | string | yes | Point ID |

### `automation.setEnabled`

*Edits*

Enable read automation or leave the manual control active.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `laneId` | string | yes | Automation lane ID |
| `enabled` | boolean | yes | Whether automation controls its target |

### `automation.setInterpolation`

*Edits*

Choose linear ramps or steps between points.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `laneId` | string | yes | Automation lane ID |
| `interpolation` | string | yes | linear or step |

### `automation.remove`

*Edits*

Delete an automation lane; undo restores it.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `laneId` | string | yes | Automation lane ID |

## controller

### `controller.list`

List a MIDI clip's controller points (control changes, pitch bend, channel pressure) and a summary of its lanes. Each value holds until the next point of its lane.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `clipId` | string | yes | Clip id, as listed by clip.list. |
| `kind` | string |  | Only this kind: cc, bend or pressure. |
| `number` | integer |  | Only this controller number (with kind cc). |

### `controller.add`

*Edits*

Add a controller point to a MIDI clip. A point already at that time in the same lane takes the new value instead.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `clipId` | string | yes | Clip id, as listed by clip.list. |
| `kind` | string | yes | cc (control change), bend (pitch bend) or pressure (channel pressure). |
| `number` | integer |  | Controller number 0-119 for kind cc: 1 mod wheel, 7 volume, 10 pan, 11 expression, 64 sustain pedal. Omit for bend and pressure. |
| `time` | number | yes | Beats from the clip start, inside the clip. |
| `value` | integer | yes | 0-127 for cc and pressure; -8192 (down) to 8191 (up) for bend, 0 centred. |

### `controller.update`

*Edits*

Move a controller point or change its value.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `clipId` | string | yes | Clip id, as listed by clip.list. |
| `controllerId` | string | yes | Point id from controller.list. |
| `time` | number |  | New time in beats from the clip start. |
| `value` | integer |  | 0-127 for cc and pressure; -8192 (down) to 8191 (up) for bend, 0 centred. |

### `controller.remove`

*Edits*

Delete a controller point.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `clipId` | string | yes | Clip id, as listed by clip.list. |
| `controllerId` | string | yes | Point id from controller.list. |

### `controller.setPoints`

*Edits*

Replace the points of one lane (kind and number) in one undo step: all of them, or only those from `from` up to `to` when given, which is how a drawn curve lands. An empty list clears the lane or the range.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `clipId` | string | yes | Clip id, as listed by clip.list. |
| `kind` | string | yes | cc (control change), bend (pitch bend) or pressure (channel pressure). |
| `number` | integer |  | Controller number 0-119 for kind cc: 1 mod wheel, 7 volume, 10 pan, 11 expression, 64 sustain pedal. Omit for bend and pressure. |
| `points` | array | yes | Array of {time, value, id?}: time in beats from the clip start, value as for controller.add. |
| `from` | number |  | Start of the range to replace, in beats (default: the clip start). |
| `to` | number |  | End of the range to replace, in beats, exclusive (default: the clip end). |

## rhythm

### `rhythm.create`

*Edits*

Create a Euclidean drum groove on a new Drum Machine track, in one undo step. Each lane has its own subdivision per bar.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `lanes` | array | yes | 1-8 objects with steps (1-64), pulses (0-steps), rotation (0-steps-1), pitch (0-127), velocity (1-127). |
| `bars` | integer | yes | Groove length, 1-16 bars. |
| `startBar` | number |  | Arrangement start, default zero. |
| `name` | string |  | Groove and track name. |

### `rhythm.preview`

*Edits*

Render a Euclidean groove to a WAV at the session's tempo and meter without creating anything: hear it before rhythm.create. In the app it runs as a job and answers when the file is written.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `lanes` | array | yes | As for rhythm.create. |
| `bars` | integer | yes | 1-4 bars, at most 30 seconds. |
| `path` | string |  | Destination .wav; defaults to one preview file in the data folder that each preview replaces. |
| `inline` | boolean |  | Also return the file as wavBase64 (default false). |

## take

### `take.list`

List creative takes saved inside this project, including the active one.

### `take.create`

*Edits*

Save the current music as a named creative take. Create Original then Variation before experimenting; edits follow the active take. Up to eight takes travel with the saved project.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `name` | string | yes | Take name, 1-120 characters. |

### `take.select`

*Edits*

Switch to a creative take, preserving edits in the current take. Stops playback; one Undo restores the previous arrangement.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `id` | string | yes | Take ID from take.list. |

### `take.remove`

*Edits*

Remove an inactive creative take. The active arrangement is preserved; undoable.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `id` | string | yes | Inactive take ID from take.list. |

## view

### `view.get`

Read the view: zoom in pixels per bar, first visible bar, the lane width in pixels, follow mode, editor mode, the clip open in the editor, the piano roll's lowest pitch, and the browser's tab and selected row.

### `view.fit`

*Edits*

Zoom the arrangement so the whole song, plus one bar, spans the lanes, and scroll to the first bar. Uses the lane width from view.get.

### `view.set`

*Edits*

Change the arrangement view and the editor. Omitted fields keep their values. Not an undo step.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `pixelsPerBar` | number |  | Arrangement zoom, 12-480 pixels per bar. |
| `scrollBar` | number |  | First visible bar, zero-based. |
| `followPlayhead` | boolean |  | Scroll with the playhead while playing. |
| `editorMode` | string |  | pianoRoll, score or step. |
| `editorClipId` | string |  | Clip to open in the editor; an empty string closes it. |
| `editorLowPitch` | integer |  | Lowest MIDI pitch the piano roll shows, 0-108: its vertical scroll. -1 lets it frame the open clip again. |
| `browserTab` | string |  | instruments, loops, plugins or files. |
| `browserSelection` | string |  | Name of the browser row to select; an empty string clears it. |
| `laneWidth` | number |  | Width of the arrangement lanes in pixels, 50-20000. The window reports it as it resizes; scripts rarely need to. |

## preset

### `preset.list`

List factory and user presets, optionally for one plugin.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `pluginId` | string |  | Only presets for this plugin ID. |

### `preset.save`

*Edits*

Save the plugin in a slot as a named preset: its parameters and, for external plugins, its captured state.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `trackId` | string | yes | Track id, as listed by track.list. |
| `slot` | integer |  | Insert slot 0-7. Omit for the instrument. |
| `name` | string | yes | Preset name, 1-120 characters. |

### `preset.load`

*Edits*

Apply a preset to the plugin in a slot, in one undo step. The preset must belong to the same plugin.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `trackId` | string | yes | Track id, as listed by track.list. |
| `slot` | integer |  | Insert slot 0-7. Omit for the instrument. |
| `name` | string | yes | Preset name from preset.list. |

### `preset.delete`

*Edits*

Delete a user preset. Factory presets cannot be deleted.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `pluginId` | string | yes | Plugin ID the preset belongs to. |
| `name` | string | yes | Preset name. |

## settings

### `settings.get`

Read preferences with secrets masked, or one dotted path such as agent.model.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `path` | string |  | Dotted setting path. Omit for everything. |

### `settings.set`

*Edits*

Change one preference and save it. The running window applies it at once.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `path` | string | yes | Dotted setting path, for example agent.provider or audio.outputDevice. |
| `value` | any | yes | New value: string, number, boolean, list or null. Strings are converted for numbers and booleans. |

### `settings.reset`

*Edits*

Reset one preference, a section, or everything to the defaults.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `path` | string |  | Dotted path or section name. Omit to reset everything. |

## audio

### `audio.devices`

List output devices, input devices and MIDI input ports, with the configured and, in live mode, the active selection.

### `audio.status`

*Needs the app*

The audio engine: device, sample rate, CPU load, master and selected-track peaks, MIDI port and live notes. `monitoring` reports input monitoring: state (off, on, blocked, failed), the input device and rate, the measured input and output buffer sizes, frames waiting in the ring, latencyMs computed from them, and frames dropped or underrun.

### `audio.allowSpeakerMonitoring`

*Edits · Needs the app*

Answer the feedback warning: monitoring the built-in microphone through the built-in speakers howls, so it stays muted (audio.status monitoring.state = blocked) until this is called with allow=true. Lasts until the app closes.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `allow` | boolean | yes | true to monitor anyway, false to mute it again. |

### `audio.setOutput`

*Edits · Needs the app*

Switch the output device and reconnect. Omit name for the system default.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `name` | string |  | Output device name from audio.devices. |

### `audio.setInput`

*Edits · Needs the app*

Choose the recording input. Omit name for the system default.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `name` | string |  | Input device name from audio.devices. |

### `audio.setMidiInput`

*Edits · Needs the app*

Connect a MIDI input port for live playing and recording. Omit port to disconnect.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `port` | string |  | MIDI port name from audio.devices. |

### `audio.reconnect`

*Edits · Needs the app*

Reopen the output device, for example after it was unplugged.

## ui

### `ui.screenshot`

*Edits · Needs the app*

Capture the window to a PNG so an agent can see the interface. Returns the file path and size.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `path` | string |  | Destination .png. Defaults to a timestamped file in the app data directory. |

### `ui.showPanel`

*Edits · Needs the app*

Show or hide an interface panel: agent, automation, mixer (every channel, in place of the region editor), controllers (the controller lane under the piano roll), palette (the command palette), settings, help, export, recovery, or master / bus-a / bus-b in the inspector.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `panel` | string | yes | agent, automation, mixer, controllers, settings, help, export, recovery, master, bus-a or bus-b. |
| `visible` | boolean |  | Show (default) or hide. |
| `section` | string |  | Settings section: general, audio, interface, agent, plugins, control, updates or about. |

### `ui.openPluginWindow`

*Edits · Needs the app*

Open a plugin's parameter panel in the window, or its native editor with native=true.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `trackId` | string | yes | Track id, as listed by track.list. |
| `slot` | integer |  | Insert slot 0-7. Omit for the instrument. |
| `native` | boolean |  | Open the plugin's own editor window when it has one. |

### `ui.closePluginWindow`

*Edits · Needs the app*

Close one plugin panel.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `id` | string | yes | Window id from ui.status pluginWindows. |

### `ui.dismissError`

*Edits · Needs the app*

Dismiss the error shown in the window.

### `ui.closePluginWindows`

*Edits · Needs the app*

Close every plugin panel and native editor.

### `ui.musicalTyping`

*Edits · Needs the app*

Turn musical typing (the computer keyboard as a piano) on or off.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `enabled` | boolean | yes | On or off. |

### `ui.setTool`

*Edits · Needs the app*

Choose the arrangement tool.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `tool` | string | yes | pointer, pencil or scissors. |

### `ui.status`

*Needs the app*

Window state: open panels, tool, musical typing, plugin panels, status line and any error being shown.

## app

### `app.info`

Version, platform, executable, data and settings paths, the control discovery file and the host mode.

### `app.checkUpdates`

*Edits · Needs the app*

Check GitHub for a newer release and report it.

### `app.installUpdate`

*Edits · Needs the app*

Download, verify and install the available update. Relaunching is confirmed in the window.

### `app.quit`

*Edits · Needs the app*

Ask the window to quit. Unsaved changes prompt in the window unless discard is true.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `discard` | boolean |  | Quit without saving (default false). |

### `app.confirm`

*Edits · Needs the app*

Answer the unsaved-changes prompt the window shows before New, Open, Quit or Relaunch. ui.status reports it as `prompt`.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `choice` | string | yes | save, discard or cancel. |

### `app.openGuide`

*Edits · Needs the app*

Open one of Ondera's guides in the web browser.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `guide` | string | yes | plugins: writing native plugins with the Rust SDK. |

### `app.relaunch`

*Edits · Needs the app*

Relaunch the app, for example after an update was installed. Unsaved changes prompt first.

## agent

### `agent.status`

*Needs the app*

The built-in agent: provider, model, whether a task is running, turn count and last reply.

### `agent.configure`

*Edits · Needs the app*

Select the agent provider, model and reasoning effort together. Only while idle.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `provider` | string | yes | codex, claude, anthropic, openai or compatible. |
| `model` | string | yes | Model ID; empty uses the provider default. |
| `reasoningEffort` | string | yes | Provider effort level; empty uses its default. |

### `agent.providers`

*Needs the app*

Available agent providers and whether each is configured.

### `agent.models`

*Needs the app*

Discover the models each connected provider offers, grouped by provider. Asks the providers over the network, so it runs as a job and answers when they have.

### `agent.connection`

*Needs the app*

Check that the configured agent provider can be reached and is signed in: provider, state and a message. Runs as a job, like agent.models.

### `agent.send`

*Edits · Needs the app*

Send a prompt to the built-in agent panel, like typing in the window.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `prompt` | string | yes | The request, in plain language. |

### `agent.stop`

*Edits · Needs the app*

Stop the running agent task; finished edits stay in Undo.

### `agent.transcript`

*Needs the app*

The agent conversation: user, assistant and tool entries.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `limit` | integer |  | Newest entries to return, default 40. |

### `agent.changes`

*Needs the app*

What the agent changed, one entry per command: sequence, title, the command as typed, its output, and whether it is currently applied.

### `agent.revert`

*Edits · Needs the app*

Undo back to just before one agent change, or redo up to it. Same as the buttons in the panel's Changes tab.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `sequence` | integer | yes | Change sequence from agent.changes. |
| `redo` | boolean |  | Redo up to the change instead of undoing it (default false). |

### `agent.clear`

*Edits · Needs the app*

Clear the agent conversation.
