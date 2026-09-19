// Dependency-free static server for the Ondera site. Railway runs `npm start`.
import { createServer } from 'node:http';
import { readFile, stat } from 'node:fs/promises';
import { dirname, extname, join, normalize, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { gzipSync } from 'node:zlib';

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
    "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; font-src https://fonts.gstatic.com; img-src 'self' data:; frame-ancestors 'none'; base-uri 'self'",
};

const NOT_FOUND = `<!doctype html><meta charset="utf-8"><title>Ondera — not found</title>
<style>body{margin:0;min-height:100vh;display:grid;place-items:center;background:#141413;color:#a9a8a4;font:14px/1.5 Manrope,system-ui,sans-serif}a{color:#e8e7e4}</style>
<p>Nothing at this address. <a href="/">Back to Ondera</a></p>`;

function send(res, status, body, type, cache, req) {
  // Text compresses six-fold (the token sheet most of all); images already are.
  const gzip =
    /^text\/|json|svg/.test(type) &&
    Buffer.byteLength(body) > 1024 &&
    /\bgzip\b/.test(String(req?.headers['accept-encoding'] ?? ''));
  const payload = gzip ? gzipSync(body) : body;
  res.writeHead(status, {
    'Content-Type': type,
    'Content-Length': Buffer.byteLength(payload),
    'Cache-Control': cache,
    ...(gzip ? { 'Content-Encoding': 'gzip' } : {}),
    Vary: 'Accept-Encoding',
    ...SECURITY,
  });
  res.end(req?.method === 'HEAD' ? undefined : payload);
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
    return send(res, 200, 'User-agent: *\nAllow: /\n', TYPES['.txt'], 'public, max-age=3600');
  }

  let pathname;
  try {
    pathname = decodeURIComponent(url.pathname);
  } catch {
    return send(res, 400, 'Bad request', 'text/plain; charset=utf-8', 'no-store');
  }
  // Resolve inside ROOT only; anything that escapes becomes a 404.
  let file = normalize(join(ROOT, pathname));
  if (!file.startsWith(ROOT + sep) && file !== ROOT) {
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
    const cache = ext === '.html' ? 'no-cache' : 'public, max-age=3600';
    let body = await readFile(file);
    if (ext === '.html') {
      // Absolute URLs for link previews, whatever domain the site is served from.
      const proto = String(req.headers['x-forwarded-proto'] ?? 'http').split(',')[0].trim();
      const host = String(req.headers.host ?? 'localhost').replace(/[^\w.:-]/g, '');
      body = Buffer.from(body.toString('utf8').replaceAll('%ORIGIN%', `${proto}://${host}`));
    }
    send(res, 200, body, type, cache, req);
  } catch {
    send(res, 404, NOT_FOUND, TYPES['.html'], 'no-store');
  }
}).listen(PORT, HOST, () => {
  console.log(`ondera site listening on http://${HOST}:${PORT}`);
});
