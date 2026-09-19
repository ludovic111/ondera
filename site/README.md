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
  handles the switcher and refills the canvas tokens. Bump the `?v=` query on the four assets in
  `index.html` (and the `tokens.js` import) when they change, since they are cached for an hour.
- `styles.css` holds the site-level scale (type sizes, section rhythm) at the top, then composes
  everything else from the tokens. Theme structure (Aero glass and lens, Modern without glow) is
  the last block; colours still come only from the tokens.
- `server.js` also redirects `/download` (by User-Agent) and `/download/<platform>` to the latest
  GitHub release asset; keep `ASSETS` in step with `update::asset_name`. It replaces `%ORIGIN%` in
  HTML with the request's origin for link previews and sets the security headers and CSP (no
  inline scripts).
- `img/` holds captures of the real window (`ui.screenshot`, downscaled to WebP). Retake them when
  the interface changes.
- `main.js` runs the interactive DAW mock on the hero: a tiny command store, canvas drawing for
  the arrangement, the agent panel (Conversation and Changes tabs, Revert through `history.undo`)
  and the hardware rack demo. Keep its copy in step with `docs/releases/` when the app changes.

Run it locally:

```bash
cd site && npm start
```
