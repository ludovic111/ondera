// Dependency-free static server for the Ondera site. Railway runs `npm start`.
import { createServer } from 'node:http';
import { readFile, stat } from 'node:fs/promises';
import { dirname, extname, join, normalize, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

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

const NOT_FOUND = `<!doctype html><meta charset="utf-8"><title>Ondera — not found</title>
<style>body{margin:0;min-height:100vh;display:grid;place-items:center;background:#141413;color:#a9a8a4;font:14px/1.5 Manrope,system-ui,sans-serif}a{color:#e8e7e4}</style>
<p>Nothing at this address. <a href="/">Back to Ondera</a></p>`;

function send(res, status, body, type, cache) {
  res.writeHead(status, {
    'Content-Type': type,
    'Content-Length': Buffer.byteLength(body),
    'Cache-Control': cache,
    'X-Content-Type-Options': 'nosniff',
  });
  res.end(body);
}

createServer(async (req, res) => {
  if (req.method !== 'GET' && req.method !== 'HEAD') {
    return send(res, 405, 'Method not allowed', 'text/plain; charset=utf-8', 'no-store');
  }
  const url = new URL(req.url ?? '/', 'http://localhost');
  if (url.pathname === '/health') return send(res, 200, 'ok', 'text/plain; charset=utf-8', 'no-store');

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
    const body = await readFile(file);
    send(res, 200, body, type, cache);
  } catch {
    send(res, 404, NOT_FOUND, TYPES['.html'], 'no-store');
  }
}).listen(PORT, HOST, () => {
  console.log(`ondera site listening on http://${HOST}:${PORT}`);
});
