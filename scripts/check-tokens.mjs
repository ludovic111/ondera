#!/usr/bin/env node
/**
 * Enforces the tokens rule: no hardcoded colours, shadows, gradients or font
 * families outside packages/app/src/theme/. Exits non-zero on violations.
 */
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';

const ROOT = new URL('..', import.meta.url).pathname;
const SCAN = [join(ROOT, 'packages/app/src'), join(ROOT, 'packages/app/electron')];
const ALLOW_DIR = join(ROOT, 'packages/app/src/theme');
const EXT = /\.(tsx?|css)$/;

const rules = [
  { name: 'hex colour', re: /(^|[^\w&])#[0-9a-fA-F]{3,8}\b(?![^{]*})/ },
  { name: 'rgb()/rgba()', re: /\brgba?\(/ },
  { name: 'oklch()/oklab()', re: /\bokl(?:ch|ab)\(/ },
  { name: 'hsl()', re: /\bhsla?\(/ },
  { name: 'color-mix()', re: /\bcolor-mix\(/ },
  { name: 'literal gradient', re: /\b(?:linear|radial|conic)-gradient\(/ },
  { name: 'literal font-family', re: /font-family\s*:(?!\s*(?:var\(|inherit))/ },
  { name: 'literal box-shadow', re: /box-shadow\s*:(?!\s*(?:var\(|none))/ },
];

function walk(dir, out = []) {
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) walk(full, out);
    else if (EXT.test(entry)) out.push(full);
  }
  return out;
}

const violations = [];
for (const dir of SCAN) {
  for (const file of walk(dir)) {
    if (file.startsWith(ALLOW_DIR)) continue;
    const lines = readFileSync(file, 'utf8').split('\n');
    lines.forEach((text, i) => {
      // CSS module class names and ids can legitimately contain '#': only flag colour-like tokens
      const stripped = text.replace(/\/\/.*$/, '').replace(/\/\*.*?\*\//g, '');
      for (const rule of rules) {
        if (rule.re.test(stripped)) {
          violations.push(`${relative(ROOT, file)}:${i + 1}  ${rule.name}: ${text.trim()}`);
        }
      }
    });
  }
}

if (violations.length) {
  console.error(`tokens rule: ${violations.length} violation(s)\n`);
  for (const v of violations) console.error('  ' + v);
  console.error('\nMove the value into packages/app/src/theme/tokens.ts and reference it.');
  process.exit(1);
}
console.log('tokens rule: OK');
