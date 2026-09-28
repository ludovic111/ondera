# Prompt for the next session

Paste everything below the line into a new session opened on this repository.

---

Continue ryolune from the 0.10 work (branch `claude/next-update-413330`, not released unless the
owner has merged it and tagged `v0.10.0` since). Read `CLAUDE.md` first, above all the 0.10
bullet, then `docs/releases/0.10.0.md`. Build and live-check recipe: `docs/DEVELOPMENT.md`
("Checking the real window"). This Mac had no Rust toolchain before 0.10: rustup lives in
`~/.cargo` (not on the shell's PATH; use `~/.cargo/bin`) and the worktree pins 1.88.0 like CI.

## Ask the owner first
- How tempo changes and ramps feel in a real song with audio regions (they play at their own
  speed, so an audio loop does not follow a tempo change), and whether a ramp should also be
  offered as a curve.
- Whether buses should feed other buses (a drum group into a music bus): 0.10 refuses it to keep
  the graph one stage deep; nesting needs delay compensation per depth.
- How monitoring, the count-in and a take behave with a real audio interface, and a real MIDI
  keyboard's controllers on stock and third-party instruments (still tests only).

## Open engineering items
- Time signature changes inside a song (a meter map beside the tempo map; bars are bars today).
- Time stretching of audio regions (WSOLA or a phase vocoder written by hand) and take comping;
  both build on the tempo map.
- CLAP preset discovery is not hosted, so CLAP factory presets are unreachable from commands.
- Serum 2 (and any Audio Unit that does not restore its preset name with its state) reports no
  current preset once reopened; the window's own instance still knows it until then.
- UAD and Slate Audio Units do not load outside their host environment (the UAD one exits the
  process in a probe): scanning already isolates this, but loading one in the app would too.
- Inserts never receive notes; controllers to inserts are not delayed to match a latent
  instrument.
- symphonia's MP4 `n_frames` may be in the track timescale; only Vorbis is trimmed to it.
- Two interfaces on different clocks drift (an occasional tick, counted in `audio.status`).
- The site's theme gallery and window shots predate the tempo track and buses.
