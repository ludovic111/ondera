# Brag Plan: ryolune

## What is this app?
ryolune is a free, open-source DAW for macOS, Windows and Linux with a built-in agent that edits the same session you do, and every edit it makes is one undo away.

## The angle
A DAW that takes requests. Most "AI music" pitches generate a song for you; ryolune does the opposite: you keep writing, and you hand the agent the busywork (quantising, variations, gain staging). The proof is the real window: you type a request, the agent's steps tick in, teal notes land in the piano roll, and ⌘Z takes them straight back out. Then a fast tour proves it is a complete DAW, and the six themes flip on the beat.

## Hook (first 2-3 seconds)
A close-up of ryolune's real agent composer. A request types itself out with key clicks, "Add a sixteenth-note answer to the chords on bars 5 to 8.", while the headline slams in: "Your DAW takes requests now."

## Key moments (the middle)
- Enter is pressed and the camera pulls back to the whole live window: the playhead is running, meters move with the music, and the agent's steps arrive one by one ("Looked at your project", "Wrote 24 notes").
- The agent's notes land in the real piano roll and the Chords region, drawn in teal, left to right.
- ⌘Z: every teal note disappears at once. "Every edit is one ⌘Z away."
- Fast proof it is a real DAW: the mixer with live meters, then the ryolune Comp plugin window. "34 stock plugins. Plus your VST3, CLAP and AU."
- Six themes switch live, one per beat, on the track's lift at 16s.

## Outro / punchline
The ryolune mark and wordmark lock up on the 20s hit: "A free DAW you can finish songs in." with "macOS · Windows · Linux · MIT" and github.com/ludovic111/ryolune.

## User flow worth showing
1. Type a request into the agent composer (entry).
2. The agent inspects the session and writes notes; the steps tick in and the notes appear in the arrangement and piano roll (key action).
3. Undo with ⌘Z and the session is back as it was (result, and the trust story).

## Tone
- Preset: default
- Creative direction: confident producer's launch clip, playful but musical
- Interpretation: 5-6 scenes, snappy entrances and camera moves, every line held long enough to read, the product itself carries every scene.

## Format: landscape — 1920x1080
## Duration: 22.5s

## Visual identity (from the project)
- Background: #141413 (desk), panels #2c2c2b
- Accent: rgb(71,214,207) (hi rgb(118,239,231), lo rgb(0,169,162))
- Text: #f2f1ee (ink-bright), #a9a8a4 (ink300)
- Display font: Manrope (600/700), self-hosted in site/fonts
- Body font: IBM Plex Mono for labels and metadata
- Strongest visual element: the skeuomorphic window itself (lit accent LED, agent-marked teal notes, LCD transport), plus the site's hairline grid and LED mark

All UI is the real frontend (`frontend/`) running its fixture song through `src/dev/mockHost.ts`, recorded frame by frame. No personal data appears; the agent conversation is written for the video and uses real tool names (`session.inspect`, `clip.setNotes`).

## Share copy (draft)
ryolune 0.10 is a free, open-source DAW with an agent that takes requests, and ⌘Z takes back anything it did. macOS, Windows, Linux.

## Audio direction
- Role: warm, upbeat bed with motion-matched UI accents
- Music: happy-beats-business-moves-vol-1-by-ende-dot-app.mp3 (120.19 BPM), from 0s
- Music treatment: 0.34 bed, short fade-in, fade out under the last 1.2s after the logo hit
- Music cue guidance: bundled preset read. Beat grid on .02 + 0.5s steps from 3.02s. Strong cues: 16.02s (section lift, theme switching starts), 20.02s (logo lock-up). Theme switches on 16.02, 16.52, 17.02, 17.52, 18.02, 18.52 (accents, not text; the "Six themes." line holds across them).
- Audio-reactive treatment: subtle; bass warms the desk glow behind the window and the logo LED. No bars or waveforms (the app's own meters already move with the track).
- SFX posture: moderate, motion-matched
- Audio-coupled moments: typed prompt (key ticks), Enter (click), each agent step (soft drop), notes landing (soft impact), ⌘Z (key + soft impact), theme flips (quiet switch on the first and last), logo (bell)
- Restraint rule: no stacked hits on the busy steps sequence; repeated sounds stay low; nothing bright over the typing.

## Storyboard

### Scene 1 — Hook — 0.0–3.2s
Close-up of the real agent composer (model chip, context chips, text box). The request types out character by character. Headline over the lower third: "Your DAW takes requests now." Small kicker top-left: "ryolune 0.10".
Sequential/interaction: yes, typed prompt.
Audio intent: curiosity; clicks sit under the bed.
Audio-coupled idea: randomized key ticks per character, thinned.
Transition mood: clean, camera pull-back → Scene 2

### Scene 2 — The agent works — 3.2–8.2s
Enter (click). Camera pulls back to the full live window: playhead running, meters moving. The conversation fills in: the request, "Looked at your project", the plan, "Wrote 24 notes", the reply. Teal notes sweep into the piano roll and the Chords region. Caption: "It does the busywork. You finish the song."
Sequential/interaction: yes, steps one by one, notes drawn left to right.
Audio intent: momentum.
Audio-coupled idea: soft drop per step (first and last accented), one soft impact as notes land.
Transition mood: camera push → Scene 3

### Scene 3 — Undo — 8.2–11.2s
Camera pushes into the Chords region and piano roll. A ⌘Z keycap presses; every teal note vanishes. Caption: "Every edit is one ⌘Z away."
Sequential/interaction: yes, key press.
Audio intent: a satisfying reversal.
Audio-coupled idea: key click, then a soft reverse-feeling impact as notes clear.
Transition mood: clean wipe → Scene 4

### Scene 4 — A real DAW — 11.2–16.0s
Live mixer with moving meters (11.2–13.6): "A real mixer. Buses and groups." Then the ryolune Comp window over the session (13.6–16.0): "34 stock plugins. Plus your VST3, CLAP and AU."
Sequential/interaction: two cards, each held ≥1.8s.
Audio intent: confident.
Audio-coupled idea: soft drop on each card.
Transition mood: hard cut on 16.02 → Scene 5

### Scene 5 — Six themes — 16.0–19.2s
The full window switches theme live on each beat: Skeuomorphic, Modern, Aero, Console, Ink, Neon. "Six themes." holds throughout; the theme name updates as a small mono label.
Sequential/interaction: yes, beat-grid theme flips.
Audio intent: lift with the track.
Audio-coupled idea: quiet switch on the first and last flip only.
Transition mood: dramatic zoom-out → Scene 6

### Scene 6 — Outro — 19.2–22.5s
Mark and wordmark lock up on 20.02s. "A free DAW you can finish songs in." Chips: macOS · Windows · Linux, MIT licensed, No account. URL: github.com/ludovic111/ryolune.
Sequential/interaction: lockup then lines.
Audio intent: payoff, then let the bed fade.
Audio-coupled idea: bell on the lockup.

**Music mood for this video:** upbeat
**Audio summary:** a bright 120 BPM bed from the first frame, UI clicks carrying the typing and the agent's steps, a lift at 16s for the themes, a bell on the logo and a clean fade.
