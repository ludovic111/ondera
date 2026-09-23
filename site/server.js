// Dependency-free static server for the Ondera site. Railway runs `npm start`.
import { createServer } from 'node:http';
import { readFile, stat } from 'node:fs/promises';
import { dirname, extname, join, normalize, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { gzipSync } from 'node:zlib';
import { createHash } from 'node:crypto';

const ROOT = dirname(fileURLToPath(import.meta.url));
const PORT = Number(process.env.PORT) || 3000;
const HOST = '0.0.0.0';

const TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.webp': 'image/webp',
  '.ico': 'image/x-icon',
  '.txt': 'text/plain; charset=utf-8',
  '.xml': 'application/xml; charset=utf-8',
  '.woff2': 'font/woff2',
};

// Keep in step with `update::asset_name` in desktop/src/update.rs and the release workflow.
const RELEASES = 'https://github.com/ludovic111/ondera/releases/latest';
const ASSETS = {
  'macos-arm64': 'Ondera-macos-arm64.zip',
  'macos-x86_64': 'Ondera-macos-x86_64.zip',
  'windows-x86_64': 'Ondera-windows-x86_64.zip',
  'linux-x86_64': 'Ondera-linux-x86_64.zip',
};

/** Best guess from the User-Agent. Macs report Intel even on Apple silicon, so default to arm64. */
export function platformFor(userAgent = '') {
  if (/Windows/i.test(userAgent)) return 'windows-x86_64';
  if (/Android|iPhone|iPad/i.test(userAgent)) return null;
  if (/Mac OS X|Macintosh/i.test(userAgent)) return 'macos-arm64';
  if (/Linux|X11/i.test(userAgent)) return 'linux-x86_64';
  return null;
}

const SECURITY = {
  'X-Content-Type-Options': 'nosniff',
  'Referrer-Policy': 'strict-origin-when-cross-origin',
  'X-Frame-Options': 'DENY',
  'Permissions-Policy': 'camera=(), microphone=(), geolocation=()',
  'Strict-Transport-Security': 'max-age=31536000',
  'Content-Security-Policy':
    "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; font-src 'self'; img-src 'self' data:; object-src 'none'; form-action 'none'; frame-ancestors 'none'; base-uri 'self'",
};

// Files that live beside the site but are not part of it.
const PRIVATE = new Set(['server.js', 'package.json', 'package-lock.json', 'README.md']);

const NOT_FOUND = `<!doctype html><meta charset="utf-8"><title>Ondera — not found</title>
<style>body{margin:0;min-height:100vh;display:grid;place-items:center;background:#141413;color:#a9a8a4;font:14px/1.5 Manrope,system-ui,sans-serif}a{color:#e8e7e4}</style>
<p>Nothing at this address. <a href="/">Back to Ondera</a></p>`;

function send(res, status, body, type, cache, req) {
  // A strong validator from the content, so a revisit costs a 304 instead of the file.
  const etag = status === 200 ? `"${createHash('sha1').update(body).digest('base64url').slice(0, 22)}"` : null;
  if (etag && req?.headers['if-none-match'] === etag) {
    res.writeHead(304, { ETag: etag, 'Cache-Control': cache, Vary: 'Accept-Encoding', ...SECURITY });
    return res.end();
  }
  // Text compresses six-fold (the token sheet most of all); images already are.
  const gzip =
    /^text\/|json|svg|xml/.test(type) &&
    Buffer.byteLength(body) > 1024 &&
    /\bgzip\b/.test(String(req?.headers['accept-encoding'] ?? ''));
  const payload = gzip ? gzipSync(body) : body;
  res.writeHead(status, {
    'Content-Type': type,
    'Content-Length': Buffer.byteLength(payload),
    'Cache-Control': cache,
    ...(etag ? { ETag: etag } : {}),
    ...(gzip ? { 'Content-Encoding': 'gzip' } : {}),
    Vary: 'Accept-Encoding',
    ...SECURITY,
  });
  res.end(req?.method === 'HEAD' ? undefined : payload);
}

// `?v=` on a script or stylesheet becomes a hash of that file, so versioned URLs can be cached
// for good and a forgotten bump in index.html can never serve a stale asset.
const stamps = new Map();
async function stampOf(path) {
  const file = normalize(join(ROOT, path));
  if (!file.startsWith(ROOT + sep)) return null;
  try {
    const { mtimeMs } = await stat(file);
    const hit = stamps.get(file);
    if (hit?.mtimeMs === mtimeMs) return hit.stamp;
    const stamp = createHash('sha1').update(await readFile(file)).digest('hex').slice(0, 10);
    stamps.set(file, { mtimeMs, stamp });
    return stamp;
  } catch {
    return null;
  }
}
async function stampVersions(text, fromDir) {
  const refs = [...text.matchAll(/(["'])(\.?\/?[\w./-]+\.(?:js|css))\?v=[\w.-]*\1/g)];
  for (const [whole, quote, path] of refs) {
    const stamp = await stampOf(join(fromDir, path));
    if (stamp) text = text.replaceAll(whole, `${quote}${path}?v=${stamp}${quote}`);
  }
  return text;
}

/** The origin the visitor used, for absolute URLs in link previews and the sitemap. */
function originOf(req) {
  const proto = String(req.headers['x-forwarded-proto'] ?? 'http').split(',')[0].trim();
  const host = String(req.headers.host ?? 'localhost').replace(/[^\w.:-]/g, '');
  return `${proto}://${host}`;
}

createServer(async (req, res) => {
  if (req.method !== 'GET' && req.method !== 'HEAD') {
    return send(res, 405, 'Method not allowed', 'text/plain; charset=utf-8', 'no-store');
  }
  const url = new URL(req.url ?? '/', 'http://localhost');
  if (url.pathname === '/health') return send(res, 200, 'ok', 'text/plain; charset=utf-8', 'no-store');

  if (url.pathname === '/download' || url.pathname.startsWith('/download/')) {
    const wanted = url.pathname.slice('/download/'.length) || platformFor(req.headers['user-agent']);
    const asset = ASSETS[wanted];
    res.writeHead(302, {
      Location: asset ? `${RELEASES}/download/${asset}` : RELEASES,
      'Cache-Control': 'no-store',
      ...SECURITY,
    });
    return res.end();
  }
  if (url.pathname === '/robots.txt') {
    const robots = `User-agent: *\nAllow: /\n\nSitemap: ${originOf(req)}/sitemap.xml\n`;
    return send(res, 200, robots, TYPES['.txt'], 'public, max-age=3600', req);
  }
  if (url.pathname === '/sitemap.xml') {
    const { mtime } = await stat(join(ROOT, 'index.html'));
    const sitemap = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url><loc>${originOf(req)}/</loc><lastmod>${mtime.toISOString().slice(0, 10)}</lastmod></url>
</urlset>
`;
    return send(res, 200, sitemap, TYPES['.xml'], 'public, max-age=3600', req);
  }

  let pathname;
  try {
    pathname = decodeURIComponent(url.pathname);
  } catch {
    return send(res, 400, 'Bad request', 'text/plain; charset=utf-8', 'no-store');
  }
  // Resolve inside ROOT only; anything that escapes, the server's own files and dotfiles become a 404.
  let file = normalize(join(ROOT, pathname));
  const rel = file.slice(ROOT.length + 1);
  if ((!file.startsWith(ROOT + sep) && file !== ROOT) || PRIVATE.has(rel) || rel.split(sep).some((part) => part.startsWith('.'))) {
    return send(res, 404, NOT_FOUND, TYPES['.html'], 'no-store');
  }
  try {
    let info = await stat(file);
    if (info.isDirectory()) {
      file = join(file, 'index.html');
      info = await stat(file);
    }
    const ext = extname(file).toLowerCase();
    const type = TYPES[ext] ?? 'application/octet-stream';
    // index.html names its scripts and styles with ?v=, so those can be kept for good;
    // everything else (images, fonts, unversioned URLs) is rechecked hourly.
    const cache =
      ext === '.html' ? 'no-cache' : url.searchParams.has('v') ? 'public, max-age=31536000, immutable' : 'public, max-age=3600';
    let body = await readFile(file);
    if (ext === '.html' || ext === '.js') {
      let text = await stampVersions(body.toString('utf8'), dirname(file).slice(ROOT.length));
      // Absolute URLs for link previews, whatever domain the site is served from.
      if (ext === '.html') text = text.replaceAll('%ORIGIN%', originOf(req));
      body = Buffer.from(text);
    }
    send(res, 200, body, type, cache, req);
  } catch {
    send(res, 404, NOT_FOUND, TYPES['.html'], 'no-store');
  }
}).listen(PORT, HOST, () => {
  console.log(`ondera site listening on http://${HOST}:${PORT}`);
});
