# Ondera site

The marketing site at the root of this folder is plain HTML, CSS and JavaScript, served by a
dependency-free Node server (`server.js`). It deploys to Railway from this directory
(`site/` is the service's root directory), so nothing here depends on the pnpm workspace.

- `tokens.css` and `tokens.js` are **generated** from `legacy/packages/app/src/theme/tokens.ts` by
  `node scripts/gen-site-tokens.mjs` (run from the repo root). These preserve the site's original
  colours, radii and material recipes. The desktop interface uses `frontend/src/theme`.
  Do not edit the generated files.
- `styles.css` holds the site-level scale (type sizes, section rhythm) at the top, then composes
  everything else from the tokens.
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
