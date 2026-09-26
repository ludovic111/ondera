# Controlling Ondera from AI and scripts

Everything a person can do in the Ondera window, an AI or a script can do too, through one
command registry. This page explains the four ways in, what an agent should call first, and how
to drive tracks, notes, mixing and external plugins. The full list of commands and parameters
is the generated [command reference](COMMANDS.md); the audit that maps every window interaction
to its command is [AGENT_PARITY.md](AGENT_PARITY.md).

## Four ways in, one registry

| Way in | For | How it connects |
| --- | --- | --- |
| The window | people | menus, shortcuts and drags call the registry by name |
| The built-in agent | talking to Ondera in your own words | Agent panel, see [AGENT.md](AGENT.md) |
| `ondera-cli` | scripts, terminals, other programs | the running app, or a song file |
| `ondera-mcp` | Claude Code, Claude Desktop, Codex, any MCP client | the running app, or its own session |

All four share the same undo history. Every edit is an ordinary undo step (a `session.batch` is
one step for many edits), so anything an AI does can be undone from the Edit menu or with
`history.undo`. Clips and notes an agent creates are drawn with the agent accent colour, and the
agent panel's **Changes** tab lists edits that came from the agent, the CLI or MCP.

## Start with the overview

`session.overview` returns the whole song in one bounded answer: tempo, meter, key, length and
sections; every track with its instrument (name, format, vendor), inserts with the parameters
changed from their defaults as the plugin displays them, sends, fader in dB, pan, mute, solo, arm
and monitoring; **problems** that keep a track silent (muted, excluded by a solo, zero fader,
bypassed instrument, a plugin that is not installed or failed to load); clips with bars, note
counts, pitch ranges, audio sources and fades; automation and controller lanes; the buses,
selection, takes and undo history; and, in the running app, what the window shows. When a big
song does not fit, `truncated` says what was left out and `next` names the command for each
detail.

```sh
ondera-cli session.overview
ondera-cli session.overview --trackId Bass       # one track in full
```

Then drill down only where needed: `clip.get` or `note.list` for notes, `strip.parameters` for a
plugin's parameters, `automation.list`, `controller.list`, `ui.state` for the window and
`ui.screenshot` to see it.

## Conventions

- Bars and beats are **zero-based**: bar 0 is the first bar. Note `start` and `length` are in
  beats relative to their clip; automation points are in absolute beats.
- Pitch 60 is C4; velocity is 1–127; a fader value of 0.75 is unity gain (0 dB).
- **Names work wherever ids do**: `trackId`, `clipId` and `markerId` accept a unique name
  (`--trackId Bass`). A wrong or ambiguous name is refused with the list of what exists and the
  closest match ("Unknown track `Bas`. Did you mean Bass?").
- Strip commands take a track, `master`, `bus-a` (reverb) or `bus-b` (delay); insert slots are
  0–7; omit `slot` for a MIDI track's instrument.
- Errors are written for the reader: they say what was expected and how to fix the call.

## The CLI

`ondera-cli` talks to the running app over a local bridge: the app writes a port and a random
token to `control.json` in its data folder (readable only by you; `ONDERA_CONTROL` overrides the
path) and accepts only clients that present the token. Without a running app, `--file` edits a
song directly and saves atomically after every change.

```sh
ondera-cli commands                     # every command (add --json for the full schema)
ondera-cli help strip.setParameter      # one command's parameters
ondera-cli doctor                       # bridge, versions, settings and plugin cache
ondera-cli --file song.ondera session.new
ondera-cli --file song.ondera session.exportAudio --path mix.flac
```

Parameters are `--name value` or `name=value`; arrays and objects are JSON. `--compact` prints
one line of JSON. `ondera-cli batch` reads one `{"command": …, "params": {…}}` per line from
standard input and stops at the first error (`--continue` keeps going).

A song open in the window belongs to the window: `--file` on the same file is refused. Use the
live commands instead.

## MCP

`ondera-mcp` is a stdio Model Context Protocol server. Each command is a tool, with the dot
replaced by an underscore (`track.add` is `track_add`), its description and JSON schema taken
from the registry.

```sh
# Claude Code
claude mcp add ondera -- /Applications/Ondera.app/Contents/MacOS/ondera-mcp --live
```

```json
{"mcpServers": {"ondera": {"command": "/path/to/ondera-mcp", "args": ["--live"]}}}
```

- `--live` requires the running app and controls it; `--headless` hosts an independent session;
  `--file <song.ondera>` edits a file. Without a flag it uses the app when it runs.
- **Resources**: `ondera://session/overview` (read it first), `ondera://session`,
  `ondera://session/info`, `ondera://session/inspect`, `ondera://catalog`, `ondera://plugins`,
  `ondera://presets`, `ondera://settings` and the app state.
- **Prompts**: `compose`, `mix-review` and `see-the-window` start common tasks.
- The Agents section of Settings copies a ready configuration with the right path.

## Permissions

Settings > Agent > Permissions decide what an agent may do: the built-in agent, MCP clients and
`ondera-cli --agent`. Editing the song is always allowed (and always undoable); these are
switches for the rest:

| Permission | Covers |
| --- | --- |
| File operations | open, save, import, export, bounce, plugin scan, writing a screenshot to a path |
| Transport | play, record, stop, locate, marker navigation |
| Replace the session | new session, open another song |
| Settings | `settings.set`, `settings.reset` |
| Application control | quit, install an update |

Connecting an AI service, signing in and changing these permissions stay with the person.

## Recipes

### Write a part

```sh
ondera-cli track.add --kind midi --name Keys --instrument "E-Piano Mk I"
ondera-cli clip.create --trackId Keys --startBar 0 --lengthBars 2 \
  --notes '[{"start":0,"length":2,"pitch":60,"velocity":90},{"start":0,"length":2,"pitch":64},{"start":0,"length":2,"pitch":67}]'
ondera-cli clip.addLoop --trackId Drums --name "Four Floor 124" --startBar 0
ondera-cli marker.add --bar 0 --name Intro
```

`clip.setNotes` replaces a clip's notes in one step; `clip.quantize`, `clip.transpose`,
`clip.humanize`, `clip.fitScale`, `clip.legato` and `clip.repeat` edit a whole clip.
`controller.setPoints` writes mod wheel, pitch bend, sustain or any CC into a clip.

### Many edits, one undo step

```sh
ondera-cli session.batch --commands '[
  {"command":"track.setVolume","params":{"trackId":"Drums","volume":0.6}},
  {"command":"track.setPan","params":{"trackId":"Keys","pan":-20}},
  {"command":"strip.setPlugin","params":{"trackId":"master","plugin":"Limiter","firstFreeSlot":true}}
]'
```

With `atomic` (the default) a failing command rolls back the ones before it.

### External plugins

Any installed CLAP, VST3, Audio Unit or Ondera native plugin can be found, loaded and set.

```sh
ondera-cli plugin.list --query reverb                  # by name, vendor or what it does
ondera-cli strip.setPlugin --trackId Vocals --plugin "pro q 3" --firstFreeSlot true
ondera-cli strip.parameters --trackId Vocals --slot 0 --query "band 1"
ondera-cli strip.setParameter --trackId Vocals --slot 0 --parameter "Band 1 Gain" --text "-4.5 dB"
ondera-cli strip.setParameters --trackId Vocals --slot 0 \
  --values '{"Band 1 Frequency":"250 Hz","Band 1 Q":{"normalized":0.3}}'
ondera-cli strip.programs --trackId Bass --slot 0         # the plugin's own factory programs
ondera-cli strip.setProgram --trackId Bass --slot 0 --name Cathedral
ondera-cli automation.create --target pluginParameter --trackId Vocals --slot 0 \
  --parameter "Band 1 Gain" --points '[{"beat":0,"value":0},{"beat":16,"value":-6}]'
```

- `plugin.list` searches names, vendors and what a plugin is for ("reverb", "compressor",
  "saturation"). A plugin installed in several formats loads as CLAP, then VST3, then AU.
- `strip.parameters` lists every parameter with its value, display text, range, 0–1 position,
  whether it can be automated and its automation lane; `query` filters by name and
  `changed=true` shows only what differs from the defaults.
- A value is set as a plain number (`value`), a 0–1 position (`normalized`) or what the plugin
  displays (`text`: "-6 dB", "2.5k", "50%", "On", "Hall").
- `strip.programs` / `strip.setProgram` reach VST3 program lists and Audio Unit factory presets;
  Ondera's own presets are `preset.list`, `preset.save` and `preset.load`.
- `strip.getState` / `strip.setState` read and restore a plugin's full saved state;
  `strip.setBypass`, `strip.moveInsert` and `strip.removeInsert` manage the chain;
  `ui.openPluginWindow` opens the plugin's own window (macOS).
- Scanning (`plugin.scan`) runs in a separate process, so a faulty plugin cannot crash the app.

### Check a mix

1. `session.overview`: read `problems` for silent tracks and the fader and insert summary.
2. `session.exportAudio --path /tmp/check.wav`: the report gives the peak and the number of
   clipped samples. `session.exportStems` does the same per track.
3. Adjust with `track.setVolume`, `strip.setParameter` or a limiter on `master`, then export again.

### See and steer the window

`ui.state` reports what the window shows: open panels and dialogs, a pending prompt, open plugin
windows, the editor's clip and mode, zoom and visible bars, the tool, the browser, the selection
and the theme. `ui.showPanel` opens the mixer, automation, controller lane, settings, help, the
command palette and more; `view.set` scrolls and zooms; `ui.setTool` picks a tool;
`ui.screenshot` saves a PNG of the window, captured once running animations have settled.

## What only a person does

A few things deliberately have no command: signing in to an AI service and changing the agent's
connection or permissions, the menu bar itself, the agent panel's own composer, and pure layout
(vertical track scroll, folding a browser folder). The reasons are listed in
[AGENT_PARITY.md](AGENT_PARITY.md).

## Limits

CLAP preset discovery and presets a plugin shows only inside its own window are not reachable;
`strip.programs` reports what the format exposes. External plugin windows open on macOS only.
Playback needs the running app; rendering and exports do not.
