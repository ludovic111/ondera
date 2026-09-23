# Ondera site

The marketing site at the root of this folder is plain HTML, CSS and JavaScript, served by a
dependency-free Node server (`server.js`). It deploys to Railway from this directory
(`site/` is the service's root directory), so nothing here depends on the pnpm workspace.

- `tokens.css` and `tokens.js` are **generated** from the app's theme layer (`frontend/src/theme`)
  by `node scripts/gen-site-tokens.mjs` (run from the repo root after `npm --prefix frontend ci`).
  The site wears the same three themes as the window, in dark and light: `:root` holds the fixed
  scales and the design-source theme, and one `[data-theme][data-mode]` block per variant holds
  every themed property, so any subtree (the theme cards) can wear a theme of its own.
  Do not edit the generated files; regenerate them when a theme changes.
- `theme.js` is a classic script in `<head>` that sets `data-theme` and `data-mode` before first
  paint from the stored choice, falling back to the system's light/dark preference. `main.js`
  handles the switcher and refills the canvas tokens.
- Caching: `server.js` rewrites every `?v=` on a `.js`/`.css` reference (in `index.html` and in
  `main.js`'s `tokens.js` import) to a hash of that file, and serves `?v=` URLs as immutable, so a
  change is picked up on the next page load without a manual bump. Other files carry an ETag and
  are rechecked hourly; HTML is always revalidated.
- `fonts/` holds the app's typefaces (Manrope 400-700, IBM Plex Mono 400-600, latin subsets from
  `@fontsource`, SIL OFL in `fonts/LICENSE.txt`); the page makes no third-party request, and the
  CSP only allows `'self'`.
- `styles.css` holds the site-level scale (type sizes, section rhythm) at the top, then composes
  everything else from the tokens. Theme structure (Aero glass and lens, Modern without glow) is
  near the end; colours still come only from the tokens. The page background is `--site-page`:
  the desk in dark modes, the editor surface in light ones (the light desk is too dark for
  secondary ink). Text inside a well (time display, terminals, the build command) re-maps the inks
  to the well inks and the accent to `accent-hi`, because a well can be dark on a light theme.
- `server.js` also redirects `/download` (by User-Agent) and `/download/<platform>` to the latest
  GitHub release asset; keep `ASSETS` in step with `update::asset_name`. It replaces `%ORIGIN%` in
  HTML with the request's origin for link previews and structured data, serves `/robots.txt` and
  `/sitemap.xml` for that origin, sets the security headers and CSP (no inline scripts), and does
  not serve its own files (`server.js`, `package.json`, this README) or dotfiles.
- Each release: update the "New in" section, the hero pill, the changelog entry (newest first; move
  the oldest expanded one into "Earlier releases"), "Not here yet" and "Next, and current limits",
  the version in the hero, downloads, footer and the Open Graph / JSON-LD tags. The copy is written
  from `docs/releases/`; keep it in plain words for musicians.
- `img/` holds captures of the real window (`ui.screenshot`, downscaled to WebP). Retake them when
  the interface changes and replace the files in place under the same names: `arrangement.webp`
  and `mixer.webp` at 2000x1250 (the `width`/`height` attributes reserve that 16:10 box), `og.png`
  at 1200x750 (declared in the Open Graph tags; update them if the size changes).
- `main.js` runs the interactive DAW mock on the hero: a tiny command store using the registry's
  command names (`transport.locate`, `transport.setCycle`, `track.setMonitor`, ...), canvas drawing
  for the arrangement (clip fades and gain), song markers as buttons over the ruler (click jumps,
  double-click or Shift+Enter loops the section), the agent panel (Conversation and Changes tabs,
  Revert through `history.undo`) and the hardware rack demo. Keep its copy in step with
  `docs/releases/` when the app changes.

Run it locally:

```bash
cd site && npm start          # or PORT=4318 npm start
```
