// Per-frame levels of the music bed (0-24 s), for the app's meters in the recorded footage.
import { execFileSync } from "node:child_process";
import { writeFileSync } from "node:fs";

const here = new URL(".", import.meta.url).pathname;
const SR = 24000;
const FPS = 30;
const raw = execFileSync(
  "ffmpeg",
  ["-v", "error", "-i", `${here}../composition/assets/music/happy-beats-business-moves-vol-1-by-ende-dot-app.mp3`, "-t", "24", "-ac", "1", "-ar", String(SR), "-f", "f32le", "-"],
  { maxBuffer: 1 << 28 },
);
const x = new Float32Array(raw.buffer, raw.byteOffset, raw.length / 4);
// Split into lows and highs with a one-pole filter; drums and bass follow the lows.
const low = new Float32Array(x.length);
let y = 0;
for (let i = 0; i < x.length; i++) low[i] = y += 0.035 * (x[i] - y);
const per = SR / FPS;
const bands = [];
for (let f = 0; f * per < x.length; f++) {
  let a = 0, l = 0, h = 0, p = 0;
  for (let i = f * per; i < Math.min(x.length, (f + 1) * per); i++) {
    a += x[i] * x[i];
    l += low[i] * low[i];
    h += (x[i] - low[i]) ** 2;
    p = Math.max(p, Math.abs(low[i]));
  }
  bands.push([Math.sqrt(a / per), Math.sqrt(l / per), Math.sqrt(h / per), p]);
}
const norm = (i) => Math.max(...bands.map((b) => b[i]));
const n = [0, 1, 2, 3].map(norm);
// Meters read linear peaks; keep them between -30 and -3 dBFS so they look like a real mix.
const meter = (v, floor = 0.03) => Math.min(0.71, floor + 0.68 * Math.pow(v, 0.8));
let hold = 0;
const frames = bands.map(([a, l, h, p], f) => {
  const all = a / n[0], lo = l / n[1], hi = h / n[2], punch = p / n[3];
  hold = Math.max(punch, hold * 0.8);
  return {
    l: meter(all), r: meter(all * 0.96),
    drums: meter(hold), bass: meter(lo * 0.9), keys: meter(hi * 0.85),
    pad: meter(all * 0.7), vox: meter(hi * 0.75 + 0.05 * Math.sin(f / 5)), riser: meter(hi * 0.4),
  };
});
writeFileSync(`${here}meters.json`, JSON.stringify({ fps: FPS, frames }));
console.log(`meters.json ${frames.length} frames`);
