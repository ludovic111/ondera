# Prompt for the next session

Paste everything below the line into a new session opened on this repository.

---

Continue Ondera from the 0.9 work (branch `claude/ameliorer-lapp-089e02`, not yet released unless
the owner has tagged `v0.9.0` since). Read `CLAUDE.md` first, above all the 0.8 bullets, "Agent
control" and the 0.9 bullet, then `docs/releases/0.9.0.md`. Build setup and the live-check recipe
are in your memory notes and in `docs/DEVELOPMENT.md`.

## Ask the owner first
- How monitoring, the count-in and a take behave with a real audio interface and with the built-in
  microphone on headphones (still only covered by tests that drive the input callback).
- How a real MIDI keyboard's mod wheel, pitch bend, sustain and aftertouch feel on the stock
  synths and on one third-party CLAP, VST3 and AU instrument; whether a controller-aware effect
  reacts, and whether multi-channel controllers land on the right channel.
- Whether the 6 dB of stock-instrument headroom sounds right in their existing songs.

## Open engineering items
- `strip.programs` reports `current: null` after an Audio Unit factory preset is loaded; read
  `kAudioUnitProperty_PresentPreset` (or remember the index in the insert) to report it.
- The Audio Unit factory preset list is not released after reading (some plugins own it); a
  small leak per read. Decide per the AU documentation and test with several vendors.
- CLAP preset discovery is not hosted, so CLAP factory presets are unreachable from commands.
- Browser rows fold formats but offer no way to pick another format in the window; the
  row's `formats` list is there for a context-menu choice.
- Inserts never receive notes (a vocoder-like effect needs voice bookkeeping for inserts);
  controllers to inserts are not delayed to match a latent instrument.
- MIDI file import still records channels as separate tracks, so imported notes play on
  channel 1.
- Poly pressure is not chased on locate or reset at stop.
- symphonia's MP4 `n_frames` may be in the track timescale; only Vorbis is trimmed to it.
- Two interfaces on different clocks drift (an occasional tick, counted in `audio.status`).
