# Marketing

## brag-output/ — launch video (made with [/brag](https://github.com/latent-spaces/brag) + [Hyperframes](https://hyperframes.heygen.com/))

- `brag.mp4` — 22.5 s, 1920×1080, −16 LUFS; frame 0 is the poster `brag.jpg`
- `brag-4k.mp4` — the same at 3840×2160
- `share-copy.txt` — the post text
- `brag-plan.md`, `composition-brief.md` — the plan and the brief handed to Hyperframes
- `composition/` — the Hyperframes project (`index.html`, footage, music, SFX)
- `work/` — how the footage was made: `record.mjs` drives the real frontend frame by frame,
  `meters.mjs` makes the app's meters follow the music

Not in git: the footage (`work/record.mjs` makes it), the music (copy
`happy-beats-business-moves-vol-1-by-ende-dot-app.mp3` from the /brag skill's `assets/music/` into
`composition/assets/music/`) and the final MP4s (published as `site/video/`).

To rebuild after a UI change (needs FFmpeg and Node 22):

```sh
npm --prefix frontend run dev                     # another terminal
cd marketing/brag-output/work && npm install
node meters.mjs && node record.mjs                # footage at 3x, near-lossless 4:4:4
cd ../composition && npx hyperframes check
npx hyperframes render --resolution landscape-4k --video-frame-format png --crf 8 --output ../work/master-4k.mp4
../work/finish.sh                                 # brag.mp4 (1080p) + brag-4k.mp4, poster, loudness
```

The site plays these files: copy `brag-4k.mp4` and `brag.mp4` to `site/video/ondera-<version>-4k.mp4`
and `-1080p.mp4`, and frame 0 to `site/img/film-poster.webp` (see `site/README.md`).

Keep `--video-frame-format png`: the default extracts footage as JPEG and blurs the UI text.
Music: "Happy Beats / Business Moves vol. 1" by ende.app, bundled with /brag. SFX: Kenney (CC0).
