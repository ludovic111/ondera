# Ondera site

The marketing site at the root of this folder is plain HTML, CSS and JavaScript, served by a
dependency-free Node server (`server.js`). It deploys to Railway from this directory
(`site/` is the service's root directory), so nothing here depends on the pnpm workspace.

- **One look.** The site wears the app's Skeuomorphic dark theme (the design source) and has no
  theme switcher. The app's other themes are shown, not worn: the "Themes, in the app" section is
  a gallery of captures of the app's renderer, one per theme and mode.
- `tokens.css` and `tokens.js` are **generated** from the app's theme layer (`frontend/src/theme`)
  by `node scripts/gen-site-tokens.mjs` (run from the repo root after `npm --prefix frontend ci`).
  They hold Skeuomorphic dark only: `tokens.css` is one `:root` block, `tokens.js` the same values
  for the canvas code. Do not edit the generated files; regenerate them when that theme changes.
- Caching: `server.js` rewrites every `?v=` on a `.js`/`.css` reference (in `index.html` and in
  `main.js`'s `tokens.js` import) to a hash of that file, and serves `?v=` URLs as immutable, so a
  change is picked up on the next page load without a manual bump. Other files carry an ETag and
  are rechecked hourly; HTML is always revalidated.
- `fonts/` holds the app's typefaces (Manrope 400-700, IBM Plex Mono 400-600, latin subsets from
  `@fontsource`, SIL OFL in `fonts/LICENSE.txt`); the page makes no third-party request, and the
  CSP only allows `'self'`.
- `styles.css` holds the site-level scale (type sizes, section rhythm, hairlines) at the top, then
  composes everything else from the tokens. Text inside a well (time display, terminals, the build
  command) takes `accent-hi`, the accent that holds 4.5:1 on a well.
- `server.js` also redirects `/download` (by User-Agent) and `/download/<platform>` to the latest
  GitHub release asset; keep `ASSETS` in step with `update::asset_name`. It replaces `%ORIGIN%` in
  HTML with the request's origin for link previews and structured data, serves `/robots.txt` and
  `/sitemap.xml` for that origin, sets the security headers and CSP (no inline scripts), and does
  not serve its own files (`server.js`, `package.json`, this README) or dotfiles.
- Each release: update the "New in" section, the hero pill, the changelog entry (newest first; move
  the oldest expanded one into "Earlier releases"), "Not here yet" and "Next, and current limits",
  the version in the hero, the stats strip under the demo, downloads, footer and the Open Graph /
  JSON-LD tags. The copy is written from `docs/releases/`; keep it in plain words for musicians.
- `img/` holds captures of the real window, downscaled to WebP. Retake them when the interface
  changes and replace the files in place under the same names: `arrangement.webp` and `mixer.webp`
  at 2000x1250 (`ui.screenshot`; the `width`/`height` attributes reserve that 16:10 box), `og.png`
  at 1200x750 (declared in the Open Graph tags; update them if the size changes).
- `img/theme-<id>-<mode>.webp` (2000x1250) feed the theme gallery. They are the frontend renderer
  with its fixture song: `npm --prefix frontend run dev`, open `/?theme=<id>&mode=<mode>` at
  1600x1000 with a device scale of 1.25, screenshot, then `cwebp -q 74 -m 6`.
- **Adding an app theme to the site:** add a tab to `#theme-tabs` in `index.html` (`data-theme-id`,
  `data-theme-name`) and its `<p data-theme-desc>` line, add `img/theme-<id>-dark.webp` and
  `-light.webp`, and update the theme count. The count is written in exactly one place: the
  `#themes-title` heading ("Three themes, each in dark and light.", marked `THEME COUNT` in a
  comment). The 0.6 changelog entry names the original three and stays as history.
- `main.js` runs the interactive DAW mock on the hero: a tiny command store using the registry's
  command names (`transport.locate`, `transport.setCycle`, `track.setMonitor`, ...), canvas drawing
  for the arrangement (clip fades and gain), song markers as buttons over the ruler (click jumps,
  double-click or Shift+Enter loops the section), the agent panel (Conversation and Changes tabs,
  Revert through `history.undo`) and the hardware rack demo. Keep its copy in step with
  `docs/releases/` when the app changes. It also wires the page motion below.

## Motion

All motion is CSS (transform, opacity, filter) switched by classes from `main.js`, uses the
theme's `--motion-*` easings, and stops under `prefers-reduced-motion: reduce` (the view transition
and the tilt are skipped in script too). Hero and demo content hides only under
`@media (scripting: enabled)` with a 2.5 s failsafe; lower sections wait for `html.js`. The reveal
observer is wired at the top of `main.js` so a later error cannot leave the page hidden.

- Scroll reveals: each `.reveal` rises and un-blurs once, staggered by `--d` (grid items get their
  column automatically). The headline's lines rise out of a mask one after another.
- The hero's signal: level bars built by script, animated with `scaleY` only, paused off screen.
- Stats: digits pop in with blur and a spring (`--motion-settle`).
- Scroll progress: a 1 px accent line under the nav, `animation-timeline: scroll()` where
  supported, absent elsewhere. The nav underlines the section you are reading.
- Theme gallery: a pill slides between tabs; the picture is preloaded, then swapped inside
  `document.startViewTransition` with a blur crossfade (`::view-transition-*(theme-shot)`; the root
  does not take part, so the rest of the page stays live). Without the API it swaps instantly.
- Micro-interactions: a light sweep over lit buttons, arrows that nudge on hover, cards with a
  pointer-following glow, window captures with a small 3D tilt and glare, the FAQ opening by
  height (`::details-content` with `interpolate-size`, instant where unsupported) with a plus that
  folds into a minus, and the Copy button's label swapping with a blur.

## Design references

Patterns were studied, then re-implemented from scratch in plain CSS/JS (no code or assets copied):

- [transitions.dev](https://transitions.dev): "Texts reveal" (lines rise with offset stagger, used
  for the headline and section reveals), "Number pop-in" (stats), "Text
  states swap" (the Copy label), "Tabs sliding" (theme and mode pickers), "3D tilt"
  (window captures), "Accordion" (FAQ height and plus/minus morph), "Shimmer text" and "Get Pro
  button" (the lit button's light sweep), "Learn more hover" (arrows), "Page side-by-side" (the
  view transition idea, applied to the gallery swap since the site is one page).
- [Refero Styles](https://styles.refero.design/): the dark references in its Styles library.
  [Linear](https://styles.refero.design/style/90ce5883-bb24-4466-93f7-801cd617b0d1): near-black
  canvas, hairline borders instead of shadow stacks, tight negative tracking on display type, one
  chromatic call to action per view, decorative gradient kept to the hero.
  [Factory](https://styles.refero.design/style/13d6fc89-eba2-4724-ac37-20f4f2e5efec): two voices
  (sans for the page, mono uppercase for "instrument" labels), metric tiles divided by 1 px rules,
  a status dot before labels. [teenage engineering](https://styles.refero.design/style/aecf9dda-5cba-4dc7-9e73-59b65d895cdf):
  product as hero, spec-sheet grid of hairline cells (the features grid), a dark vitrine band for
  the thing you take home (the download panel).

Run it locally:

```bash
cd site && npm start          # or PORT=4318 npm start
```
