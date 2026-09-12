# Ondera site

The marketing site at the root of this folder is plain HTML, CSS and JavaScript, served by a
dependency-free Node server (`server.js`). It deploys to Railway from this directory
(`site/` is the service's root directory), so nothing here depends on the pnpm workspace.

- `tokens.css` and `tokens.js` are **generated** from `legacy/packages/app/src/theme/tokens.ts` by
  `node scripts/gen-site-tokens.mjs` (run from the repo root). These preserve the site's original
  colours, radii and material recipes. The native app uses `desktop/src/theme.rs`.
  Do not edit the generated files.
- `styles.css` holds the site-level scale (type sizes, section rhythm) at the top, then composes
  everything else from the tokens.
- `main.js` runs the interactive DAW mock on the hero: a tiny command store, canvas drawing for
  the arrangement, and the hardware rack demo.

Run it locally:

```bash
cd site && npm start
```
