# Prompt for the next session

Paste everything below the line into a new session opened on this repository.

---

Continue Ondera on branch `feat/0.8-monitoring` (six commits on top of `v0.7.0`, not merged, not
released). Read `CLAUDE.md` first, above all the "0.8 work" bullet, and the draft
`docs/releases/0.8.0.md`. Build setup and the live-check recipe are in your memory notes.

## Waiting on the owner
1. The real-device checks of input monitoring and of recording with a microphone (the two
   checklists are in the 0.8 hand-off message and summarised in `docs/releases/0.8.0.md`). Fix
   whatever they report before anything else.
2. A decision on a lossy export format: none, Ogg Vorbis (`vorbis_rs`, BSD, C libraries), or
   MP3 through LAME (LGPL, C). Nothing was added.
3. The scope of MIDI CC: ABI 2 plugins can receive controllers, pitch bend and pressure, but
   nothing sends them. Proposed order: (a) `engine/src/midi.rs` forwards CC, bend and pressure
   from the MIDI input to the armed or selected track as `Event`s, live only; (b) record them into
   MIDI clips as a `controllers` list beside `notes` and play them back; (c) only then a lane
   editor under the piano roll. (a) needs `Renderer` to queue `Event`s instead of `NoteEvent`s,
   which also touches the CLAP, VST3 and AU adapters. Agree the scope before building (c).

## Open engineering items
- One shared input stream for the meter, monitoring and recording, so monitoring does not drop
  out when a take starts (`desktop/src/app.rs` `poll_input_meter` / `start_recording`,
  `engine/src/device.rs` `InputMeter` / `Recorder`).
- Mid-block automation points: the ABI carries frame offsets, the renderer still sends one
  value per block of at most 256 frames (`Rack::set_param`, `render.rs` `plugin_automation`).
- The agent's Changes list also records the window's own reads (`plugin.list`, `view.set`);
  consider recording only requests that did not come from the interface.
- `Other Effects` still holds names too plain to guess (Warm, Punch, Micro, Metamorph, Mic Mod,
  Serum 2 FX, Waves Gemstones).
