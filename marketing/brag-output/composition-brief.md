# Hyperframes Composition Brief: Ondera

## Objective
Create a short launch-style brag video for Ondera 0.10.

## Output
- Composition directory: `marketing/brag-output/composition/`
- Rendered video: `marketing/brag-output/brag.mp4`
- Format: landscape — 1920x1080, 30 fps
- Duration: 22.5 seconds

## Source Material
- Project root: the Ondera repository
- Primary files read: `site/index.html`, `site/tokens.css`, `site/styles.css`, `README.md`, `frontend/src/dev/mockHost.ts`, `frontend/src/state/native.ts`, `CLAUDE.md`
- Product name: Ondera
- Tagline / strongest claim: "Finish the song. Hand the busywork to an agent." / "Every edit it makes is one undo away."
- Key UI moment: the real window, recorded frame by frame from the frontend (`work/record.mjs`) into `assets/footage/`: `composer.mp4` (the agent composer at 4x), `session.mp4` (typing → agent steps → teal notes land → ⌘Z at 9.4s), `mixer.mp4`, `plugin.mp4` (Ondera Comp), `themes.mp4` (six themes switch on 16.52, 17.02, 17.52, 18.02, 18.52 composition time). Each file starts at its composition start time; the window is 1600 × 1000 CSS px.
- Copy that must appear verbatim:
  - "A free DAW you can finish songs in."
  - "Every edit is one ⌘Z away." (site: "every edit it makes is one undo away")
  - "34 stock plugins" and "VST3, CLAP and AU"
  - github.com/ludovic111/ondera

## Creative Direction
- Tone preset: default
- Creative direction: confident producer's launch clip, playful but musical
- Interpretation: snappy camera moves over the live app, every caption held long enough to read, the product carries each scene.
- Angle, hook, outro: see `brag-plan.md`.
- Avoid: generic SaaS language, abstract filler, redesigning the app.

## Visual Identity
- Background: #141413; panel #2c2c2b
- Text: #f2f1ee; secondary #a9a8a4
- Accent: rgb(71,214,207) (hi rgb(118,239,231))
- Display font: Manrope 600/700 (assets/fonts, OFL)
- Body font: IBM Plex Mono 400/500 for labels
- Visual references: the site's hairline grid and LED mark (square plate, teal dot, ring)

## Storyboard
1. Hook — 0–3.2s — composer close-up types the request; "Your DAW takes requests now."
2. Agent works — 3.2–8.2s — zoom-out to the live window; steps tick in; teal notes land. "It does the busywork."
3. Undo — 8.2–11.2s — Chords region and piano roll; ⌘Z at 9.4s. "Every edit is one ⌘Z away."
4. Real DAW — 11.2–16.0s — mixer, then Ondera Comp. "A real mixer." / "34 stock plugins. Plus yours."
5. Themes — 16.0–19.2s — themes switch on the beat. "Six themes."
6. Outro — 19.2–22.5s — lockup on 20.02s, tagline, chips, URL.

## Audio
- Audio role: warm upbeat bed with motion-matched UI accents
- Music: `assets/music/happy-beats-business-moves-vol-1-by-ende-dot-app.mp3`, 0.34, fade out 21.3–22.5s
- Music cue guidance: preset `happy-beats-business-moves-vol-1-by-ende-dot-app.music-cues.json` (120.19 BPM, beats at .02 + 0.5k from 3.02s). Strong cues: 16.02 (themes), 20.02 (logo).
- Audio-reactive: subtle; `assets/data/audio.js` (RMS + bass per frame) warms the desk glow and the logo LED.
- Audio-coupled moments: typed prompt (thinned key ticks), Enter click at 3.2, step drops at 3.75 and 5.35, soft impact as notes land, ⌘Z click + soft impact at 9.4, card slides at 11.2 and 13.6, switch on 16.52 and 18.52, bell at 20.02.
- SFX guidance: `sfx-analysis.md` low-risk picks; SFX 0.35–0.7.
