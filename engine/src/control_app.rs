//! Registry commands that give the CLI, MCP and agents the rest of the window: view state,
//! region tools, track duplication, presets, preferences, audio devices, interface actions
//! and application/agent control. Everything that needs a window goes through
//! [`Host::live`]; the rest works on files too.

use crate::{
    control::{self, edit, opt, query, req, selection, Args, Host, Kind, Spec, CLIP_ID, TRACK_ID},
    host as plugin_host,
    model::*,
    preset::{self, PluginPreset},
    recovery, settings,
    store::Command,
    Result,
};
use serde_json::{json, Value};
use std::path::PathBuf;

const SLOT: control::Param = opt(
    "slot",
    Kind::Integer,
    "Insert slot 0-7. Omit for the instrument.",
);

pub const SPECS: &[Spec] = &[
    edit("rhythm.create", "Create a Euclidean drum groove on a new Drum Machine track, in one undo step. Each lane has its own subdivision per bar.", &[req("lanes", Kind::Array, "1-8 objects with steps (1-64), pulses (0-steps), rotation (0-steps-1), pitch (0-127), velocity (1-127)."), req("bars", Kind::Integer, "Groove length, 1-16 bars."), opt("startBar", Kind::Number, "Arrangement start, default zero."), opt("name", Kind::String, "Groove and track name.")]),
    edit("clip.humanize", "Humanize MIDI timing and velocity reproducibly without changing pitch, inside region bounds. One undo step.", &[CLIP_ID, opt("timingMs", Kind::Number, "Maximum timing offset, 0-100 ms (default 10)."), opt("velocity", Kind::Integer, "Maximum velocity offset, 0-32 (default 8)."), opt("seed", Kind::Integer, "Random seed, 0-4294967295 (default 1).")]),
    edit("clip.velocityRamp", "Shape MIDI dynamics from the first to last onset, preserving chords at equal velocity. One undo step.", &[CLIP_ID, req("from", Kind::Integer, "Starting velocity 1-127."), req("to", Kind::Integer, "Ending velocity 1-127.")]),
    edit("clip.fitScale", "Move MIDI pitches to the closest note in a scale, choosing down on ties. Timing and velocity stay intact.", &[CLIP_ID, req("root", Kind::Integer, "Root pitch class 0-11, C=0."), req("scale", Kind::String, "major, minor, dorian, mixolydian, pentatonicMajor or pentatonicMinor.")]),
    edit("clip.reverseMidi", "Reverse MIDI note timing within the region, preserving pitch, duration and velocity.", &[CLIP_ID]),
    edit("clip.legato", "Extend MIDI notes to the next distinct onset or region end. Simultaneous chord notes remain together.", &[CLIP_ID]),
    edit("clip.repeat", "Repeat a MIDI or audio region immediately after itself in one undo step, assigning unique IDs.", &[CLIP_ID, req("count", Kind::Integer, "Number of additional copies, 1-64.")]),
    query("take.list", "List creative takes saved inside this project, including the active one.", &[]),
    edit("take.create", "Save the current music as a named creative take. Create Original then Variation before experimenting; edits follow the active take. Up to eight takes travel with the saved project.", &[req("name", Kind::String, "Take name, 1-120 characters.")]),
    edit("take.select", "Switch to a creative take, preserving edits in the current take. Stops playback; one Undo restores the previous arrangement.", &[req("id", Kind::String, "Take ID from take.list.")]),
    edit("take.remove", "Remove an inactive creative take. The active arrangement is preserved; undoable.", &[req("id", Kind::String, "Inactive take ID from take.list.")]),
    query("view.get", "Read the view: zoom in pixels per bar, first visible bar, follow mode, editor mode and the clip open in the editor.", &[]),
    edit("view.set", "Change the arrangement view and the editor. Omitted fields keep their values. Not an undo step.", &[
        opt("pixelsPerBar", Kind::Number, "Arrangement zoom, 12-480 pixels per bar."),
        opt("scrollBar", Kind::Number, "First visible bar, zero-based."),
        opt("followPlayhead", Kind::Boolean, "Scroll with the playhead while playing."),
        opt("editorMode", Kind::String, "pianoRoll, score or step."),
        opt("editorClipId", Kind::String, "Clip to open in the editor; an empty string closes it."),
    ]),
    edit("clip.quantize", "Snap every note start in a MIDI clip to the grid, in one undo step.", &[
        CLIP_ID,
        opt("division", Kind::Integer, "Notes per bar: 1, 2, 4, 8, 16, 32 or 64. Defaults to the transport snap."),
        opt("strength", Kind::Number, "How far to move toward the grid, 0-100 percent (default 100)."),
        opt("lengths", Kind::Boolean, "Also quantize note lengths (default false)."),
    ]),
    edit("clip.transpose", "Shift every note in a MIDI clip by semitones, clamped to 0-127.", &[
        CLIP_ID,
        req("semitones", Kind::Integer, "Signed semitones, -48 to 48."),
    ]),
    edit("track.duplicate", "Copy a track with its strip and clips right after the original.", &[
        TRACK_ID,
        opt("name", Kind::String, "Name for the copy. Defaults to the original name plus \" copy\"."),
    ]),
    edit("strip.moveInsert", "Move an insert to another slot on the same strip, shifting the others.", &[
        TRACK_ID,
        req("from", Kind::Integer, "Slot 0-7 to move."),
        req("to", Kind::Integer, "Destination slot 0-7."),
    ]),
    query("plugin.describe", "Describe a plugin without placing it: format, vendor, category and every parameter with ids, ranges, units and defaults.", &[
        req("pluginId", Kind::String, "Descriptor ID from plugin.list, for example stock:Space."),
    ]),
    query("preset.list", "List factory and user presets, optionally for one plugin.", &[
        opt("pluginId", Kind::String, "Only presets for this plugin ID."),
    ]),
    edit("preset.save", "Save the plugin in a slot as a named preset: its parameters and, for external plugins, its captured state.", &[
        TRACK_ID, SLOT,
        req("name", Kind::String, "Preset name, 1-120 characters."),
    ]),
    edit("preset.load", "Apply a preset to the plugin in a slot, in one undo step. The preset must belong to the same plugin.", &[
        TRACK_ID, SLOT,
        req("name", Kind::String, "Preset name from preset.list."),
    ]),
    edit("preset.delete", "Delete a user preset. Factory presets cannot be deleted.", &[
        req("pluginId", Kind::String, "Plugin ID the preset belongs to."),
        req("name", Kind::String, "Preset name."),
    ]),
    query("settings.get", "Read preferences with secrets masked, or one dotted path such as agent.model.", &[
        opt("path", Kind::String, "Dotted setting path. Omit for everything."),
    ]),
    edit("settings.set", "Change one preference and save it. The running window applies it at once.", &[
        req("path", Kind::String, "Dotted setting path, for example agent.provider or audio.outputDevice."),
        req("value", Kind::Any, "New value: string, number, boolean, list or null. Strings are converted for numbers and booleans."),
    ]),
    edit("settings.reset", "Reset one preference, a section, or everything to the defaults.", &[
        opt("path", Kind::String, "Dotted path or section name. Omit to reset everything."),
    ]),
    query("audio.devices", "List output devices, input devices and MIDI input ports, with the configured and, in live mode, the active selection.", &[]),
    query("audio.status", "The audio engine: device, sample rate, CPU load, master and selected-track peaks, MIDI port and live notes. `monitoring` reports input monitoring: state (off, on, blocked, failed), the input device and rate, the measured input and output buffer sizes, frames waiting in the ring, latencyMs computed from them, and frames dropped or underrun.", &[]),
    edit("audio.allowSpeakerMonitoring", "Answer the feedback warning: monitoring the built-in microphone through the built-in speakers howls, so it stays muted (audio.status monitoring.state = blocked) until this is called with allow=true. Lasts until the app closes.", &[
        req("allow", Kind::Boolean, "true to monitor anyway, false to mute it again."),
    ]),
    edit("audio.setOutput", "Switch the output device and reconnect. Omit name for the system default.", &[
        opt("name", Kind::String, "Output device name from audio.devices."),
    ]),
    edit("audio.setInput", "Choose the recording input. Omit name for the system default.", &[
        opt("name", Kind::String, "Input device name from audio.devices."),
    ]),
    edit("audio.setMidiInput", "Connect a MIDI input port for live playing and recording. Omit port to disconnect.", &[
        opt("port", Kind::String, "MIDI port name from audio.devices."),
    ]),
    edit("audio.reconnect", "Reopen the output device, for example after it was unplugged.", &[]),
    edit("note.preview", "Audition one note on a track's instrument, like clicking a piano-roll key.", &[
        TRACK_ID,
        req("pitch", Kind::Integer, "MIDI pitch 0-127."),
        opt("velocity", Kind::Integer, "1-127, default 100."),
    ]),
    edit("note.hold", "Hold or release a note on the selected instrument track, like a key on a MIDI keyboard. Held notes are recorded when the transport is recording. Always release what you hold.", &[
        req("pitch", Kind::Integer, "MIDI pitch 0-127."),
        req("on", Kind::Boolean, "true presses the key, false releases it."),
        opt("velocity", Kind::Integer, "1-127, default 100."),
    ]),
    edit("note.releaseAll", "Release every note held with note.hold or musical typing.", &[]),
    edit("transport.punch", "Turn record on or off. While the transport is rolling this punches in or out on the armed tracks without stopping playback; while stopped it only sets the record button.", &[
        req("enabled", Kind::Boolean, "Record on or off."),
    ]),
    edit("ui.screenshot", "Capture the window to a PNG so an agent can see the interface. Returns the file path and size.", &[
        opt("path", Kind::String, "Destination .png. Defaults to a timestamped file in the app data directory."),
    ]),
    edit("ui.showPanel", "Show or hide an interface panel: agent, automation, mixer (every channel, in place of the region editor), settings, help, export, recovery, or master / bus-a / bus-b in the inspector.", &[
        req("panel", Kind::String, "agent, automation, mixer, settings, help, export, recovery, master, bus-a or bus-b."),
        opt("visible", Kind::Boolean, "Show (default) or hide."),
        opt("section", Kind::String, "Settings section: general, audio, interface, agent, plugins, control, updates or about."),
    ]),
    edit("ui.openPluginWindow", "Open a plugin's parameter panel in the window, or its native editor with native=true.", &[
        TRACK_ID, SLOT,
        opt("native", Kind::Boolean, "Open the plugin's own editor window when it has one."),
    ]),
    edit("ui.closePluginWindow", "Close one plugin panel.", &[
        req("id", Kind::String, "Window id from ui.status pluginWindows."),
    ]),
    edit("ui.dismissError", "Dismiss the error shown in the window.", &[]),
    edit("ui.closePluginWindows", "Close every plugin panel and native editor.", &[]),
    edit("ui.musicalTyping", "Turn musical typing (the computer keyboard as a piano) on or off.", &[
        req("enabled", Kind::Boolean, "On or off."),
    ]),
    edit("ui.setTool", "Choose the arrangement tool.", &[
        req("tool", Kind::String, "pointer, pencil or scissors."),
    ]),
    query("ui.status", "Window state: open panels, tool, musical typing, plugin panels, status line and any error being shown.", &[]),
    query("app.info", "Version, platform, executable, data and settings paths, the control discovery file and the host mode.", &[]),
    edit("app.checkUpdates", "Check GitHub for a newer release and report it.", &[]),
    edit("app.installUpdate", "Download, verify and install the available update. Relaunching is confirmed in the window.", &[]),
    edit("app.quit", "Ask the window to quit. Unsaved changes prompt in the window unless discard is true.", &[
        opt("discard", Kind::Boolean, "Quit without saving (default false)."),
    ]),
    edit("app.confirm", "Answer the unsaved-changes prompt the window shows before New, Open, Quit or Relaunch. ui.status reports it as `prompt`.", &[
        req("choice", Kind::String, "save, discard or cancel."),
    ]),
    edit("app.relaunch", "Relaunch the app, for example after an update was installed. Unsaved changes prompt first.", &[]),
    edit("session.saveRecoveredTake", "Write a recording that could not be placed on a track to a WAV file, which frees the window to open other sessions. ui.status reports it as `recoveredTake`.", &[
        req("path", Kind::String, "Destination .wav."),
    ]),
    query("session.snapshots", "List recovery snapshots newest first, with paths, titles, times and sizes.", &[]),
    edit("session.restoreSnapshot", "Open a recovery snapshot in the window as an unsaved copy.", &[
        req("path", Kind::String, "Snapshot path from session.snapshots."),
    ]),
    query("agent.status", "The built-in agent: provider, model, whether a task is running, turn count and last reply.", &[]),
    edit("agent.configure", "Select the agent provider, model and reasoning effort together. Only while idle.", &[
        req("provider", Kind::String, "codex, claude, anthropic, openai or compatible."),
        req("model", Kind::String, "Model ID; empty uses the provider default."),
        req("reasoningEffort", Kind::String, "Provider effort level; empty uses its default."),
    ]),
    query("agent.providers", "Available agent providers and whether each is configured.", &[]),
    edit("agent.send", "Send a prompt to the built-in agent panel, like typing in the window.", &[
        req("prompt", Kind::String, "The request, in plain language."),
    ]),
    edit("agent.stop", "Stop the running agent task; finished edits stay in Undo.", &[]),
    query("agent.transcript", "The agent conversation: user, assistant and tool entries.", &[
        opt("limit", Kind::Integer, "Newest entries to return, default 40."),
    ]),
    query("agent.changes", "What the agent changed, one entry per command: sequence, title, the command as typed, its output, and whether it is currently applied.", &[]),
    edit("agent.revert", "Undo back to just before one agent change, or redo up to it. Same as the buttons in the panel's Changes tab.", &[
        req("sequence", Kind::Integer, "Change sequence from agent.changes."),
        opt("redo", Kind::Boolean, "Redo up to the change instead of undoing it (default false)."),
    ]),
    edit("agent.clear", "Clear the agent conversation.", &[]),
];

/// Commands that only a window can serve.
pub fn is_live_only(name: &str) -> bool {
    matches!(name.split('.').next().unwrap_or(""), "ui" | "agent")
        || matches!(
            name,
            "audio.status"
                | "audio.allowSpeakerMonitoring"
                | "audio.setOutput"
                | "audio.setInput"
                | "audio.setMidiInput"
                | "audio.reconnect"
                | "note.preview"
                | "note.hold"
                | "note.releaseAll"
                | "transport.punch"
                | "app.confirm"
                | "app.relaunch"
                | "session.saveRecoveredTake"
                | "app.checkUpdates"
                | "app.installUpdate"
                | "app.quit"
                | "session.restoreSnapshot"
        )
}

/// Agent permission check from Settings > Agent. `None` when a command is allowed.
pub fn denied_for_agent(name: &str, permissions: &settings::Permissions) -> Option<String> {
    let deny = |what: &str, setting: &str| {
        Some(format!(
            "{name} is not allowed for agents: {what} is off in Settings > Agent ({setting})."
        ))
    };
    match name {
        "session.new" | "session.open" | "session.restoreSnapshot"
            if !permissions.replace_session =>
        {
            deny("replacing the session", "replaceSession")
        }
        "session.save"
        | "session.bounce"
        | "session.importAudio"
        | "session.importMidi"
        | "session.exportMidi"
        | "session.exportAudio"
        | "session.exportStems"
        | "plugin.scan"
        | "preset.save"
        | "preset.delete"
        | "session.saveRecoveredTake"
        | "plugin.scaffold"
        | "plugin.install"
            if !permissions.file_operations =>
        {
            deny("file operations", "fileOperations")
        }
        "transport.play"
        | "transport.record"
        | "transport.stop"
        | "transport.locate"
        | "transport.returnToStart"
        | "note.preview"
        | "note.hold"
        | "transport.punch"
            if !permissions.transport =>
        {
            deny("transport control", "transport")
        }
        "settings.set"
        | "settings.reset"
        | "audio.setOutput"
        | "audio.setInput"
        | "audio.setMidiInput"
        | "audio.allowSpeakerMonitoring"
            if !permissions.settings =>
        {
            deny("changing settings", "settings")
        }
        "app.quit" | "app.installUpdate" | "app.relaunch" | "app.confirm"
            if !permissions.app_control =>
        {
            deny("application control", "appControl")
        }
        _ => None,
    }
}

pub(crate) fn call(host: &mut dyn Host, name: &str, a: &Args, agent: bool) -> Result<Value> {
    if is_live_only(name) {
        return host.live(name, &args_value(a));
    }
    match name {
        "rhythm.create" => crate::rhythm::call(host, a, agent),
        "clip.humanize" | "clip.velocityRamp" | "clip.fitScale" | "clip.reverseMidi"
        | "clip.legato" | "clip.repeat" => crate::midi_tools::call(host, name, a, agent),
        "take.list" | "take.create" | "take.select" | "take.remove" => {
            crate::takes::call(host, name, a)
        }
        "view.get" => Ok(view_json(host)),
        "view.set" => {
            let mut view = host.store().session().view.clone();
            if let Some(v) = a.opt_f64("pixelsPerBar") {
                if !(12.0..=480.0).contains(&v) {
                    return Err("pixelsPerBar must be between 12 and 480".into());
                }
                view.pixels_per_bar = v as f32;
            }
            if let Some(v) = a.opt_f64("scrollBar") {
                if !valid_time(v) {
                    return Err("scrollBar must be between 0 and 1,000,000".into());
                }
                view.scroll_bars = v;
            }
            if let Some(v) = a.opt_bool("followPlayhead") {
                view.follow_playhead = v;
            }
            if let Some(mode) = a.opt_str("editorMode") {
                if !["pianoRoll", "score", "step"].contains(&mode) {
                    return Err("editorMode must be pianoRoll, score or step".into());
                }
                view.editor_mode = mode.into();
            }
            if let Some(clip) = a.opt_str("editorClipId") {
                if clip.is_empty() {
                    view.editor_clip_id = None;
                } else {
                    let clip = control::find_clip(host.store().session(), clip)?;
                    view.selected_track_id = Some(clip.track_id.clone());
                    view.selected_clip_id = Some(clip.id.clone());
                    view.editor_clip_id = Some(clip.id.clone());
                    view.selected_note_id = None;
                }
            }
            host.dispatch(Command::SetView(view))?;
            host.view_changed();
            Ok(view_json(host))
        }
        "clip.quantize" | "clip.transpose" => {
            let s = host.store().session();
            let mut clip = control::find_clip(s, a.str("clipId")?)?.clone();
            let bpb = s.beats_per_bar();
            let length = clip.length_bars * bpb;
            let snap = s.transport.snap_division;
            let ClipData::Midi { notes } = &mut clip.data else {
                return Err("Only MIDI clips hold notes".into());
            };
            let mut moved = 0;
            if name == "clip.quantize" {
                let division = a.opt_int("division").unwrap_or(snap as i64);
                if ![1, 2, 4, 8, 16, 32, 64].contains(&division) {
                    return Err("division must be 1, 2, 4, 8, 16, 32 or 64".into());
                }
                let strength = a.opt_f64("strength").unwrap_or(100.0);
                if !(0.0..=100.0).contains(&strength) {
                    return Err("strength must be between 0 and 100".into());
                }
                let step = 4.0 / division as f64;
                let lengths = a.opt_bool("lengths").unwrap_or(false);
                for n in notes.iter_mut() {
                    let mut target = (n.start / step).round() * step;
                    if target >= length {
                        // Rounding past the end lands on the last grid line inside the clip.
                        target = (target - step).max(0.0);
                    }
                    let next = (n.start + (target - n.start) * strength / 100.0)
                        .clamp(0.0, (length - 1e-6).max(0.0));
                    if lengths {
                        let target = ((n.length / step).round() * step).max(step);
                        n.length = n.length + (target - n.length) * strength / 100.0;
                    }
                    n.length = n.length.min(length - next).max(1e-3);
                    if (next - n.start).abs() > 1e-9 {
                        moved += 1;
                    }
                    n.start = next;
                }
            } else {
                let semitones = a.int("semitones")?;
                if !(-48..=48).contains(&semitones) {
                    return Err("semitones must be between -48 and 48".into());
                }
                for n in notes.iter_mut() {
                    let next = (n.pitch as i64 + semitones).clamp(0, 127) as u8;
                    if next != n.pitch {
                        moved += 1;
                    }
                    n.pitch = next;
                }
            }
            let id = clip.id.clone();
            host.dispatch(Command::PutClip(clip))?;
            let mut summary =
                control::clip_summary(control::find_clip(host.store().session(), &id)?);
            summary["changedNotes"] = json!(moved);
            Ok(summary)
        }
        "track.duplicate" => {
            let s = host.store().session();
            let source = control::find_track(s, a.str("trackId")?)?.clone();
            if s.tracks.len() >= 128 {
                return Err("The session already has 128 tracks".into());
            }
            let index = s.tracks.iter().position(|t| t.id == source.id).unwrap_or(0);
            let mut track = source.clone();
            track.id = control::new_id("track");
            track.name = a
                .opt_str("name")
                .map(str::to_string)
                .unwrap_or_else(|| format!("{} copy", source.name));
            track.armed = false;
            let mut strip = s.strips.get(&source.id).cloned().unwrap_or_default();
            for insert in strip.inserts.iter_mut().chain(strip.synth.iter_mut()) {
                if !insert.is_empty() {
                    insert.id = control::new_id("insert");
                }
            }
            let mut commands = vec![
                Command::AddTrack(track.clone()),
                Command::SetStrip {
                    track: track.id.clone(),
                    strip,
                },
            ];
            for clip in s.clips.iter().filter(|c| c.track_id == source.id) {
                let mut copy = clip.clone();
                copy.id = control::new_id("clip");
                copy.track_id = track.id.clone();
                copy.agent = agent;
                if let ClipData::Midi { notes } = &mut copy.data {
                    for n in notes {
                        n.id = control::new_id("note");
                    }
                }
                commands.push(Command::PutClip(copy));
            }
            commands.push(Command::MoveTrack {
                id: track.id.clone(),
                index: index + 1,
            });
            host.dispatch(Command::Batch(commands))?;
            let s = host.store().session();
            Ok(control::track_json(s, control::find_track(s, &track.id)?))
        }
        "strip.moveInsert" => {
            let id = a.str("trackId")?;
            control::check_strip(host.store().session(), id)?;
            let (from, to) = (a.int("from")?, a.int("to")?);
            let valid = 0..MAX_INSERTS as i64;
            if !valid.contains(&from) || !valid.contains(&to) {
                return Err(format!("Slots must be 0-{}", MAX_INSERTS - 1));
            }
            let mut strip = control::full_strip(host.store().session(), id);
            let insert = strip.inserts.remove(from as usize);
            strip.inserts.insert(to as usize, insert);
            host.dispatch(Command::SetStrip {
                track: id.into(),
                strip,
            })?;
            Ok(control::strip_json(host.store().session(), id))
        }
        "plugin.describe" => {
            let plugin_id = a.str("pluginId")?;
            let descriptor = plugin_host::scan::installed()
                .into_iter()
                .find(|d| d.id == plugin_id)
                .ok_or_else(|| {
                    format!("Unknown plugin `{plugin_id}`. Run plugin.scan, then plugin.list.")
                })?;
            let instance = plugin_host::instantiate(&descriptor.id, &descriptor.name, 48000)?;
            Ok(json!({
                "descriptor": descriptor,
                "latency": instance.editor.latency(),
                "hasGui": instance.editor.has_gui(),
                "parameters": instance.editor.params().iter().map(|p| json!({
                    "id": p.id, "name": p.name, "min": p.min, "max": p.max, "default": p.default,
                    "unit": p.unit, "steps": p.steps, "logarithmic": p.log, "labels": p.labels,
                })).collect::<Vec<_>>(),
                "presets": preset::list(Some(plugin_id))?.iter().map(|p| &p.name).collect::<Vec<_>>(),
            }))
        }
        "preset.list" => Ok(json!({ "presets": preset::list(a.opt_str("pluginId"))? })),
        "preset.save" => {
            let track = a.str("trackId")?;
            let slot = control::plugin_slot(a)?;
            host.capture_states()?;
            let insert = control::selected_plugin(host.store().session(), track, slot)?;
            let preset = PluginPreset {
                name: a.str("name")?.trim().to_string(),
                plugin_id: insert.plugin_id(),
                plugin_name: insert.name.clone(),
                params: insert.params.clone(),
                blob: insert.blob.clone(),
                factory: false,
            };
            let path = preset::save(&preset)?;
            Ok(json!({ "preset": preset, "path": path }))
        }
        "preset.load" => {
            let track = a.str("trackId")?;
            let slot = control::plugin_slot(a)?;
            let mut insert = control::selected_plugin(host.store().session(), track, slot)?;
            let preset = preset::load(&insert.plugin_id(), a.str("name")?)?;
            if preset.plugin_id != insert.plugin_id() {
                return Err(format!(
                    "Preset `{}` belongs to {}, not {}",
                    preset.name,
                    preset.plugin_id,
                    insert.plugin_id()
                ));
            }
            let mut instance = plugin_host::instantiate(&insert.plugin_id(), &insert.name, 48000)?;
            if !preset.blob.is_empty() {
                instance
                    .editor
                    .load(&plugin_host::decode_blob(&preset.blob)?)?;
            }
            for (id, value) in &preset.params {
                let p = instance
                    .editor
                    .params()
                    .iter()
                    .find(|p| p.id == *id)
                    .ok_or_else(|| {
                        format!("Preset parameter {id} does not exist on this plugin")
                    })?;
                if !(p.min..=p.max).contains(value) {
                    return Err(format!("Preset value for {} is out of range", p.name));
                }
            }
            insert.blob = preset.blob.clone();
            insert.params = preset.params.clone();
            let mut strip = control::full_strip(host.store().session(), track);
            if let Some(slot) = slot {
                strip.inserts[slot] = insert;
            } else {
                strip.synth = Some(insert);
            }
            host.dispatch(Command::SetStrip {
                track: track.into(),
                strip,
            })?;
            let mut result = control::strip_json(host.store().session(), track);
            result["preset"] = json!(preset.name);
            Ok(result)
        }
        "preset.delete" => {
            preset::delete(a.str("pluginId")?, a.str("name")?)?;
            Ok(json!({ "deleted": a.str("name")? }))
        }
        "settings.get" => host.settings().get(a.opt_str("path")),
        "settings.set" => {
            let path = a.str("path")?;
            let value = a.get("value").cloned().unwrap_or(Value::Null);
            let mut settings = host.settings();
            settings.set(path, value)?;
            host.update_settings(settings.clone())?;
            settings
                .get(Some(path))
                .map(|value| json!({ "path": path, "value": value }))
        }
        "settings.reset" => {
            let mut settings = host.settings();
            settings.reset(a.opt_str("path"))?;
            host.update_settings(settings.clone())?;
            Ok(settings.redacted())
        }
        "audio.devices" => {
            let settings = host.settings();
            let mut value = json!({
                "outputs": crate::device::output_devices(),
                "inputs": crate::device::input_devices(),
                "midiInputs": crate::midi::ports(),
                "configured": {
                    "output": settings.audio.output_device,
                    "input": settings.audio.input_device,
                    "midiInput": settings.audio.midi_input,
                },
            });
            if let Ok(live) = host.live("audio.status", &json!({})) {
                value["active"] = live;
            }
            Ok(value)
        }
        "app.info" => {
            let mut value = json!({
                "version": env!("CARGO_PKG_VERSION"),
                "os": std::env::consts::OS,
                "arch": std::env::consts::ARCH,
                "pid": std::process::id(),
                "executable": std::env::current_exe().ok(),
                "dataDir": plugin_host::scan::data_dir(),
                "settingsPath": settings::Settings::path(),
                "pluginCache": plugin_host::scan::cache_path(),
                "presetsDir": preset::directory(),
                "recoveryDir": recovery::directory(),
                "discoveryPath": control::wire::discovery_path(),
                "mode": host.mode(),
                "session": host.path().map(|p| p.to_path_buf()),
                "commands": control::COMMANDS.len(),
                "pluginAbi": ondera_plugin::ABI_VERSION,
            });
            if let Ok(live) = host.live("app.status", &json!({})) {
                if let Some(map) = live.as_object() {
                    for (k, v) in map {
                        value[k] = v.clone();
                    }
                }
            }
            Ok(value)
        }
        "session.snapshots" => Ok(json!({
            "directory": recovery::directory(),
            "snapshots": recovery::list(&recovery::directory())?,
        })),
        _ => Err(format!(
            "Command `{name}` is registered but not implemented"
        )),
    }
}

fn args_value(a: &Args) -> Value {
    let mut map = serde_json::Map::new();
    for p in a.spec().params {
        if let Some(v) = a.get(p.name) {
            map.insert(p.name.into(), v.clone());
        }
    }
    Value::Object(map)
}
fn view_json(host: &dyn Host) -> Value {
    let (zoom, scroll) = host.view_state();
    let v = &host.store().session().view;
    let mut value = selection(host);
    value["pixelsPerBar"] = json!(zoom);
    value["scrollBar"] = json!(scroll);
    value["followPlayhead"] = json!(v.follow_playhead);
    value["editorMode"] = json!(v.editor_mode);
    value["editorClipId"] = json!(v.editor_clip_id);
    value
}

/// Where `ui.screenshot` writes when no path is given.
pub fn default_screenshot_path() -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    plugin_host::scan::data_dir()
        .join("screenshots")
        .join(format!("ondera-{stamp}.png"))
}
