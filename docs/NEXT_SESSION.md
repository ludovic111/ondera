# Prompt for the next session

Paste everything below the line into a new session opened on this repository.

---

Continue Ondera from `main` at `v0.8.0` (released 2026-09-23). Read `CLAUDE.md` first, above all
the two 0.8 bullets, and `docs/releases/0.8.0.md`. Build setup and the live-check recipe are in
your memory notes.

## Ask the owner first
- How monitoring, the count-in and a take behave with a real audio interface and with the built-in
  microphone on headphones. The shared input stream (`device::LiveInput`) is covered by tests that
  drive its callback directly, never by a real device.
- How a real MIDI keyboard's mod wheel, pitch bend and sustain pedal feel on the stock synths and
  on one third-party CLAP, VST3 and AU instrument.

## Open engineering items
- Controllers reach only a track's instrument: route them to insert effects that accept events
  (ABI 2 effects, CLAP note ports on effects), and keep the MIDI channel instead of sending all on
  channel 1. Polyphonic aftertouch is not recorded.
- VST3 editors are not told about parameter changes that came from mapped controllers.
- Stock effects do not smooth parameters, so fast automation moves in 32-sample steps; add
  one-pole smoothing to their parameters in `engine/src/stock.rs`.
- Undone audio imports still count toward the 1 GiB import budget.
- `session.importMidi importTempo=true` does not move automation when it changes the meter.
- Agent transcript items are keyed by array index in React.
- At the 1120 px minimum width with the agent panel open the arrangement is about 60 px wide.
- The playhead's position bubble covers the first marker's name at bar 1.
- Editing a region's fade or gain while playing rebuilds the renderer, as any edit does: not
  click-free.
- symphonia's decoded Ogg runs a few milliseconds past the true end; trim to the stream's length on
  import if it matters.
