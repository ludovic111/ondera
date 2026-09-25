// Ondera site runtime: a tiny command store that mirrors the app's dispatch(command)
// pattern, canvas drawing for the arrangement mock, the hardware rack demo, the theme
// gallery and the page's motion. Every visual constant comes from tokens.js (the app's
// Skeuomorphic dark theme, the one the site wears).
import { tokens as T } from './tokens.js?v=0.8';

document.documentElement.classList.add('js');
const reducedMotion = matchMedia('(prefers-reduced-motion: reduce)').matches;
const $ = (sel, root = document) => root.querySelector(sel);
const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];
const clamp = (v, lo, hi) => Math.min(hi, Math.max(lo, v));

// Reveals are wired first, so a failure further down never leaves the page hidden.
// Grid items rise one after another: each gets its column as a stagger step.
for (const grid of $$('.news, .features, .downloads, .clients, .switch, .shots__grid')) {
  $$(':scope > .reveal', grid).forEach((el, i) => { if (!el.style.getPropertyValue('--d')) el.style.setProperty('--d', i % 3); });
}

// Reveal on scroll.
const revealer = new IntersectionObserver((entries) => {
  for (const e of entries) if (e.isIntersecting) { e.target.classList.add('in'); revealer.unobserve(e.target); }
}, { threshold: 0.08, rootMargin: '0px 0px -6% 0px' });
for (const el of $$('.reveal, .display')) revealer.observe(el);

// Number pop-in: each digit of a stat gets its own step; the text stays whole for screen readers.
for (const dd of $$('[data-pop]')) {
  const text = dd.textContent;
  dd.innerHTML = `<span class="sr">${text}</span><span class="pop" aria-hidden="true">${[...text].map((ch, i) => `<i style="--i:${i}">${ch}</i>`).join('')}</span>`;
}

// ---------------------------------------------------------------------------
// Mock session: eight tracks, song markers, and audio clips with fades and clip gain.
// ---------------------------------------------------------------------------

const PALETTE = {
  drums: 'oklch(0.72 0.14 40)',
  bass: 'oklch(0.72 0.13 300)',
  keys: 'oklch(0.75 0.13 250)',
  pad: 'oklch(0.75 0.12 330)',
  vox: 'oklch(0.78 0.14 85)',
  bgv: 'oklch(0.75 0.12 130)',
  guitar: 'oklch(0.72 0.13 20)',
  riser: 'oklch(0.75 0.14 60)',
};
const withAlpha = (oklch, a) => oklch.replace(')', ` / ${a})`);

const TEMPO = 120;
const BARS = 12;
const SMPTE_FPS = 30;

function lcg(seed) {
  let s = seed >>> 0;
  return () => {
    s = (s * 1664525 + 1013904223) >>> 0;
    return s / 4294967296;
  };
}
function scatterNotes(seed, lengthBars, agent) {
  const rnd = lcg(seed);
  const notes = [];
  for (let i = 0; i < lengthBars * 5; i++) {
    notes.push({
      start: rnd() * lengthBars * 4,
      length: 0.25 + rnd() * 0.75,
      pitch: 36 + Math.floor(rnd() * 36),
      agent: agent && i % 3 === 0,
    });
  }
  return notes;
}
function bassVerseNotes(lengthBars) {
  const roots = [0, 0, 8, 8, 5, 5, 7, 7];
  const thirds = [3, 3, 4, 4, 3, 3, 4, 4];
  const notes = [];
  for (let bar = 0; bar < lengthBars; bar++) {
    const r = roots[bar % roots.length];
    const t = thirds[bar % thirds.length];
    const x = bar * 4;
    for (const [st, len, semis] of [[0, 1.5, r], [1.5, 0.5, r + 12], [2, 1, r + 7], [3, 0.75, r], [3.5, 0.5, r + t]]) {
      notes.push({ start: x + st, length: len, pitch: 36 + semis, agent: false });
    }
  }
  return notes;
}

// monitor: off | auto | on, like track.setMonitor (audio tracks only).
const tracks = [
  { id: 'drums', name: 'Drums', kind: 'audio', volume: 0.78, mute: false, solo: false, armed: false, monitor: 'off', agent: false },
  { id: 'bass', name: 'Bass', kind: 'midi', volume: 0.7, mute: false, solo: false, armed: false, agent: false },
  { id: 'keys', name: 'Keys', kind: 'midi', volume: 0.62, mute: false, solo: false, armed: false, agent: true },
  { id: 'pad', name: 'Pad', kind: 'midi', volume: 0.5, mute: false, solo: false, armed: false, agent: false },
  { id: 'vox', name: 'Lead Vox', kind: 'audio', volume: 0.82, mute: false, solo: false, armed: true, monitor: 'auto', agent: false },
  { id: 'bgv', name: 'BGV', kind: 'audio', volume: 0.55, mute: true, solo: false, armed: false, monitor: 'off', agent: false },
  { id: 'guitar', name: 'Guitar', kind: 'audio', volume: 0.66, mute: false, solo: false, armed: false, monitor: 'off', agent: false },
  { id: 'riser', name: 'Riser FX', kind: 'midi', volume: 0.6, mute: false, solo: false, armed: false, agent: false },
];
const MONITOR_NEXT = { off: 'auto', auto: 'on', on: 'off' };
const MONITOR_LABEL = { off: 'Off', auto: 'Auto', on: 'On' };
// Song sections on the ruler, in bars from zero.
const MARKERS = [
  { name: 'Intro', bar: 0 },
  { name: 'Verse', bar: 4 },
  { name: 'Chorus', bar: 8 },
];
const sectionEnd = (i) => (MARKERS[i + 1]?.bar ?? BARS);
// fade: lengths in bars and a curve (linear, exp, sCurve); gainDb is the clip gain.
const audio = (id, trackId, name, start, length, seed, wave = 'tonal', extra = {}) => ({ id, trackId, name, start, length, kind: 'audio', seed, wave, agent: false, fadeIn: 0, fadeOut: 0, curve: 'linear', gainDb: 0, ...extra });
const midi = (id, trackId, name, start, length, notes, agent = false) => ({ id, trackId, name, start, length, kind: 'midi', notes, agent });
const clips = [
  audio('drums-1', 'drums', 'Drums_take3', 0, 12, 1, 'drums', { fadeOut: 0.5 }),
  midi('bass-1', 'bass', 'Bass intro', 0, 4, scatterNotes(11, 4, false)),
  midi('bass-2', 'bass', 'Bass verse', 4, 8, bassVerseNotes(8)),
  midi('keys-1', 'keys', 'Keys A', 0, 4, scatterNotes(21, 4, false)),
  midi('keys-2', 'keys', 'Keys B', 4, 4, scatterNotes(22, 4, true), true),
  midi('keys-3', 'keys', 'Keys B', 8, 4, scatterNotes(23, 4, false)),
  midi('pad-1', 'pad', 'Pad swell', 4, 8, scatterNotes(31, 8, false)),
  audio('vox-1', 'vox', 'LV_v2_comp', 4, 4, 2, 'tonal', { fadeIn: 0.5, curve: 'exp' }),
  audio('vox-2', 'vox', 'LV_v2_comp', 8, 4, 3, 'tonal', { fadeOut: 1, curve: 'sCurve' }),
  audio('bgv-1', 'bgv', 'BGV stack', 8, 4, 4, 'tonal', { fadeIn: 1, gainDb: -4.5 }),
  audio('guitar-1', 'guitar', 'Gtr DI', 2, 6, 5, 'tonal', { fadeIn: 0.75, curve: 'sCurve' }),
  audio('guitar-2', 'guitar', 'Gtr DI', 8, 3, 6, 'tonal', { fadeOut: 1.5, curve: 'exp' }),
  midi('riser-1', 'riser', 'Riser', 7, 1, scatterNotes(41, 1, false)),
];

// ---------------------------------------------------------------------------
// Store: state is only ever changed through dispatch(name, params).
// ---------------------------------------------------------------------------

const state = {
  playing: false,
  recording: false,
  cycle: true,
  cycleStart: 0, // bars
  cycleEnd: BARS,
  position: 0, // bars, fractional
  tracks,
  agentApplied: true, // the agent's quantise of Keys B; Revert walks the undo stack
  undoDepth: 12,
  log: [],
};

const track = (id) => state.tracks.find((t) => t.id === id);

const commands = {
  'transport.play': (s) => { s.playing = true; },
  'transport.stop': (s) => { s.playing = false; },
  'transport.togglePlay': (s) => { s.playing = !s.playing; },
  'transport.returnToStart': (s) => { s.position = 0; },
  'transport.setRecording': (s, { recording }) => { s.recording = recording; },
  'transport.setCycle': (s, { enabled, startBar, endBar }) => {
    s.cycle = enabled;
    if (startBar !== undefined && endBar > startBar) { s.cycleStart = startBar; s.cycleEnd = endBar; }
  },
  'transport.locate': (s, { bar }) => { s.position = clamp(bar, 0, BARS); },
  'transport.tick': (s, { deltaSeconds }) => {
    s.position += (deltaSeconds * TEMPO) / 60 / 4;
    if (s.cycle && s.position >= s.cycleEnd) s.position = s.cycleStart + (s.position - s.cycleEnd);
    else if (s.position >= BARS) { s.position = 0; s.playing = false; }
  },
  'track.setMute': (s, { trackId, muted }) => { track(trackId).mute = muted; },
  'track.setSolo': (s, { trackId, solo }) => { track(trackId).solo = solo; },
  'track.setArmed': (s, { trackId, armed }) => { track(trackId).armed = armed; },
  'track.setMonitor': (s, { trackId, monitor }) => { track(trackId).monitor = monitor; },
  'history.undo': (s) => { s.agentApplied = false; },
  'history.redo': (s) => { s.agentApplied = true; },
};
const SILENT = new Set(['transport.tick']);

const subscribers = new Set();
function dispatch(name, params = {}) {
  const run = commands[name];
  if (!run) throw new Error(`unknown command ${name}`);
  run(state, params);
  if (!SILENT.has(name)) {
    state.undoDepth += name === 'history.undo' ? -1 : 1;
    state.log.unshift({ name, params });
    state.log.length = Math.min(state.log.length, 6);
  }
  for (const fn of subscribers) fn(state, name);
}
const subscribe = (fn) => { subscribers.add(fn); fn(state, null); };

// ---------------------------------------------------------------------------
// Track headers (DOM)
// ---------------------------------------------------------------------------

const headers = $('#headers');
if (headers) {
  const corner = document.createElement('div');
  corner.className = 'th__corner';
  corner.textContent = 'Tracks';
  headers.append(corner);
  for (const t of state.tracks) {
    const el = document.createElement('div');
    el.className = 'th' + (t.agent ? ' th--agent' : '');
    el.style.setProperty('--track', PALETTE[t.id]);
    el.dataset.track = t.id;
    el.innerHTML = `
      <span class="th__strip"></span>
      <div class="th__main">
        <div class="th__top">
          <span class="th__name">${t.name}</span>
          <span class="th__kind">${t.kind}</span>
          ${t.agent ? '<span class="led led--accent th__agentdot" title="An agent is editing this track"></span>' : ''}
        </div>
        <div class="th__bottom">
          <button type="button" class="hbtn" data-cmd="track.setMute" data-track="${t.id}" aria-label="Mute ${t.name}">M</button>
          <button type="button" class="hbtn" data-cmd="track.setSolo" data-track="${t.id}" aria-label="Solo ${t.name}">S</button>
          <button type="button" class="hbtn" data-cmd="track.setArmed" data-track="${t.id}" aria-label="Arm ${t.name}">R</button>
          ${t.kind === 'audio'
            ? `<button type="button" class="hbtn" data-cmd="track.setMonitor" data-track="${t.id}" title="Input monitoring: Off, Auto, On">I</button>`
            : '<span class="hbtn hbtn--spacer" aria-hidden="true"></span>'}
          <div class="slider" aria-hidden="true"><div class="slider__rail"></div><div class="slider__thumb" style="--v:${t.volume}"></div></div>
        </div>
      </div>`;
    headers.append(el);
  }
}

// Every button with data-cmd dispatches. Toggles compute their new value from
// state so the log shows the canonical setter, exactly like the app.
document.addEventListener('click', (e) => {
  const btn = e.target.closest('[data-cmd]');
  if (!btn) return;
  const name = btn.dataset.cmd;
  const trackId = btn.dataset.track;
  switch (name) {
    case 'transport.togglePlay': dispatch(state.playing ? 'transport.stop' : 'transport.play'); break;
    case 'transport.setRecording': dispatch(name, { recording: !state.recording }); break;
    case 'transport.setCycle': dispatch(name, { enabled: !state.cycle }); break;
    case 'track.setMute': dispatch(name, { trackId, muted: !track(trackId).mute }); break;
    case 'track.setSolo': dispatch(name, { trackId, solo: !track(trackId).solo }); break;
    case 'track.setArmed': dispatch(name, { trackId, armed: !track(trackId).armed }); break;
    case 'track.setMonitor': dispatch(name, { trackId, monitor: MONITOR_NEXT[track(trackId).monitor] }); break;
    case 'history.undo': dispatch(name, { steps: 1 }); break;
    case 'history.redo': dispatch(name, { steps: 1 }); break;
    default: dispatch(name);
  }
});

// Agent panel tabs (Conversation / Changes), pure view state like the app.
function selectTab(tab) {
  for (const t of $$('[data-tab]')) {
    const on = t === tab;
    t.setAttribute('aria-selected', String(on));
    t.tabIndex = on ? 0 : -1;
    const panel = document.getElementById(t.getAttribute('aria-controls'));
    if (panel) panel.hidden = !on;
  }
}
document.addEventListener('click', (e) => {
  const tab = e.target.closest('[data-tab]');
  if (tab) selectTab(tab);
});

// Arrow keys move within a tab list or a radio group, as the ARIA patterns expect.
document.addEventListener('keydown', (e) => {
  const group = e.target.closest?.('[role="tablist"], [role="radiogroup"]');
  if (!group || !['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown', 'Home', 'End'].includes(e.key)) return;
  const items = $$('[role="tab"], [role="radio"]', group);
  const i = items.indexOf(e.target.closest('[role="tab"], [role="radio"]'));
  if (i < 0) return;
  e.preventDefault();
  const next = e.key === 'Home' ? 0 : e.key === 'End' ? items.length - 1
    : (i + (e.key === 'ArrowRight' || e.key === 'ArrowDown' ? 1 : -1) + items.length) % items.length;
  items[next].focus();
  items[next].click();
});

// ---------------------------------------------------------------------------
// Chrome bindings (DOM reads state; never writes it)
// ---------------------------------------------------------------------------

const fmtParams = (p) => {
  const parts = Object.entries(p).map(([k, v]) => `${k}: ${typeof v === 'string' ? `"${v}"` : v}`);
  return parts.length ? `{ ${parts.join(', ')} }` : '';
};
const pad = (n, w) => String(n).padStart(w, '0');

function renderTime() {
  const pos = state.position;
  const bar = Math.floor(pos) + 1;
  const beat = Math.floor((pos % 1) * 4) + 1;
  const tick = Math.floor((((pos * 4) % 1) + 1) % 1 * 960) + 1;
  const posEl = $('#pos');
  if (posEl) posEl.innerHTML = `${pad(bar, 3)}<i>.</i>${pad(beat, 2)}<i>.</i>${pad(tick, 3)}`;
  const seconds = (pos * 4 * 60) / TEMPO;
  const h = Math.floor(seconds / 3600), m = Math.floor(seconds / 60) % 60, s = Math.floor(seconds) % 60;
  const f = Math.floor((seconds % 1) * SMPTE_FPS);
  const smpte = $('#smpte');
  if (smpte) smpte.textContent = `${pad(h, 2)}:${pad(m, 2)}:${pad(s, 2)}:${pad(f, 2)}`;
  // The section the playhead is in, on the transport and on its marker.
  const current = MARKERS.findLastIndex((mk) => mk.bar <= pos + 1e-9);
  if (current !== shownSection) {
    shownSection = current;
    const section = $('#section');
    if (section) section.textContent = MARKERS[current]?.name ?? '';
    $$('.marker').forEach((el, i) => el.setAttribute('aria-current', String(i === current)));
  }
}
let shownSection = -1;

subscribe((s, name) => {
  if (name === 'transport.tick') return;
  const play = $('#play');
  if (play) { play.setAttribute('aria-pressed', String(s.playing)); play.setAttribute('aria-label', s.playing ? 'Stop' : 'Play'); }
  $('#rec')?.setAttribute('aria-pressed', String(s.recording));
  $('#cycle')?.setAttribute('aria-pressed', String(s.cycle));
  for (const btn of $$('.hbtn[data-track]')) {
    const t = track(btn.dataset.track);
    if (btn.dataset.cmd === 'track.setMonitor') {
      btn.dataset.monitor = t.monitor;
      btn.setAttribute('aria-pressed', String(t.monitor === 'on'));
      btn.setAttribute('aria-label', `Input monitoring for ${t.name}: ${MONITOR_LABEL[t.monitor]}`);
      continue;
    }
    const on = btn.dataset.cmd === 'track.setMute' ? t.mute : btn.dataset.cmd === 'track.setSolo' ? t.solo : t.armed;
    btn.setAttribute('aria-pressed', String(on));
  }
  for (const el of $$('[data-bind="bgv-muted"]')) el.textContent = String(track('bgv').mute);
  for (const el of $$('[data-bind="undo-count"]')) el.textContent = String(s.undoDepth);

  const log = $('#log');
  if (log) {
    log.innerHTML = s.log.length
      ? s.log.map((e, i) => `<li class="${i === 0 ? 'live' : ''}"><span class="log__name">${e.name}</span><span class="log__params">${fmtParams(e.params)}</span></li>`).join('')
      : '<li class="log__empty">nothing dispatched yet · press play</li>';
  }

  const change = $('#agent-change');
  if (change) {
    change.classList.toggle('agent__change--reverted', !s.agentApplied);
    const btn = $('#agent-revert');
    btn.dataset.cmd = s.agentApplied ? 'history.undo' : 'history.redo';
    btn.textContent = s.agentApplied ? 'Undo from here' : 'Redo to here';
    btn.classList.toggle('btn--lit', !s.agentApplied);
    btn.classList.toggle('btn--raised', s.agentApplied);
  }
  renderTime();
});

// ---------------------------------------------------------------------------
// Timeline canvas. Pure drawing functions over (ctx, tokens, state).
// ---------------------------------------------------------------------------

const canvas = $('#timeline');
const wrap = $('#timeline-wrap');
const ctx = canvas?.getContext('2d');
let W = 0, H = 0, dpr = 1;

// Deterministic peaks by (seed, normalised time), so zoom never reshuffles them.
function hash(seed, i) {
  let x = (seed * 374761393 + i * 668265263) >>> 0;
  x = (x ^ (x >>> 13)) * 1274126177 >>> 0;
  return ((x ^ (x >>> 16)) >>> 0) / 4294967296;
}
function peak(clip, t) {
  const i = Math.floor(t * 2048);
  const n = hash(clip.seed, i);
  if (clip.wave === 'drums') {
    const sixteenth = (t * clip.length * 16) % 1;
    const step = Math.floor(t * clip.length * 16);
    const accent = step % 4 === 0 ? 1 : step % 2 === 0 ? 0.62 : 0.4;
    return Math.min(1, accent * Math.exp(-sixteenth * 7) * (0.7 + n * 0.5) + n * 0.06);
  }
  const slow = 0.55 + 0.35 * Math.sin(t * Math.PI * 2 * (2 + clip.seed % 3) + clip.seed) * Math.sin(t * 17 + clip.seed * 3);
  return clamp(slow * (0.75 + n * 0.5), 0.04, 1);
}

// Flat themes have no shadow layers; a missing layer paints without a glow.
const NO_GLOW = { blur: 0, offsetY: 0, color: 'transparent' };
const layerOf = (list, i) => list[i] ?? NO_GLOW;
const layersOf = (list) => (list.length ? list : [NO_GLOW]);

function roundRect(c, x, y, w, h, r) {
  c.beginPath();
  c.roundRect(x, y, w, h, r);
}

function drawClip(c, clip, x, y, w, h, color, dim, agentHighlight) {
  const r = T.radius.clip;
  // 4. soft drop shadow (painted back to front)
  c.save();
  for (const layer of T.canvasShadow.clipDrop) {
    c.shadowBlur = layer.blur; c.shadowOffsetY = layer.offsetY; c.shadowColor = layer.color;
    c.fillStyle = T.color.panel;
    roundRect(c, x, y, w, h, r); c.fill();
  }
  c.restore();
  c.save();
  if (dim) c.globalAlpha = 0.45;
  // 2. face: track colour mixed over the panel, lighter at the top
  roundRect(c, x, y, w, h, r);
  c.fillStyle = T.color.panel; c.fill();
  const g = c.createLinearGradient(0, y, 0, y + h);
  g.addColorStop(0, withAlpha(color, T.clipMix.faceTop / 100));
  g.addColorStop(1, withAlpha(color, T.clipMix.faceBottom / 100));
  c.fillStyle = g; c.fill();
  c.clip();
  // title bar
  c.fillStyle = T.fill.clipTitle;
  c.fillRect(x, y, w, T.size.clipTitle);
  c.fillStyle = T.line.clipTitleBottom;
  c.fillRect(x, y + T.size.clipTitle, w, 1);
  c.fillStyle = T.line.clipName;
  c.font = `500 ${T.fontSize.kind}px ${T.font.mono}`;
  c.textBaseline = 'middle';
  c.fillText(clip.name, x + 6, y + T.size.clipTitle / 2 + 0.5, w - 10);
  if (clip.gainDb) {
    const label = `${clip.gainDb > 0 ? '+' : '−'}${Math.abs(clip.gainDb).toFixed(1)} dB`;
    c.textAlign = 'right';
    c.fillText(label, x + w - 6, y + T.size.clipTitle / 2 + 0.5);
    c.textAlign = 'left';
  }
  // content
  const top = y + T.size.clipTitle + 3, bottom = y + h - 3, ch = bottom - top, mid = top + ch / 2;
  if (clip.kind === 'audio') {
    c.fillStyle = T.line.waveformMid;
    c.fillRect(x + 2, mid, w - 4, 1);
    c.fillStyle = T.line.waveform;
    const step = 2;
    const clipGain = 10 ** (clip.gainDb / 20);
    for (let px = 3; px < w - 3; px += step) {
      const a = peak(clip, px / w) * (ch / 2) * 0.92 * clipGain * fadeGain(clip, px / w);
      c.fillRect(x + px, mid - a, 1, Math.max(1, a * 2));
    }
    drawFades(c, clip, x, y, w, h, top, bottom);
  } else {
    const noteH = T.size.clipNoteH;
    for (const n of clip.notes) {
      const nx = x + (n.start / 4 / clip.length) * w;
      const nw = Math.max(2, (n.length / 4 / clip.length) * w - 1);
      const ny = top + (1 - (n.pitch - 36) / 36) * (ch - noteH);
      if (n.agent && agentHighlight) {
        c.save();
        c.shadowBlur = layerOf(T.canvasShadow.agentNote, 1).blur; c.shadowColor = layerOf(T.canvasShadow.agentNote, 1).color;
        c.fillStyle = T.color.accent;
        c.fillRect(nx, ny, nw, noteH);
        c.restore();
      } else {
        c.fillStyle = T.line.midiNote;
        c.fillRect(nx, ny, nw, noteH);
      }
    }
  }
  // 1 + 3. specular top edge and contact line
  c.fillStyle = T.line.clipHighlight; c.fillRect(x, y, w, 1);
  c.fillStyle = T.line.clipContact; c.fillRect(x, y + h - 1, w, 1);
  c.restore();
  if (agentHighlight) {
    c.save();
    c.shadowBlur = layerOf(T.canvasShadow.agentRing, 0).blur; c.shadowColor = layerOf(T.canvasShadow.agentRing, 0).color;
    c.strokeStyle = T.color.accent; c.lineWidth = 1;
    roundRect(c, x + 0.5, y + 0.5, w - 1, h - 1, r); c.stroke();
    c.restore();
  }
}

// Fade gain at normalised clip time t (0..1), with the clip's curve.
const CURVES = {
  linear: (u) => u,
  exp: (u) => u * u,
  sCurve: (u) => 0.5 - 0.5 * Math.cos(Math.PI * u),
};
function fadeGain(clip, t) {
  const bars = t * clip.length, shape = CURVES[clip.curve] ?? CURVES.linear;
  let g = 1;
  if (clip.fadeIn > 0 && bars < clip.fadeIn) g *= shape(bars / clip.fadeIn);
  if (clip.fadeOut > 0 && bars > clip.length - clip.fadeOut) g *= shape((clip.length - bars) / clip.fadeOut);
  return g;
}
// The fade curve over the clip body, the region it silences dimmed, and a handle at its end.
function drawFades(c, clip, x, y, w, h, top, bottom) {
  const ppbClip = w / clip.length;
  const handle = (hx) => { c.fillStyle = T.line.clipName; c.fillRect(hx - 2.5, y + T.size.clipTitle + 1, 5, 5); };
  for (const [len, from, dir] of [[clip.fadeIn, 0, 1], [clip.fadeOut, clip.length, -1]]) {
    if (!(len > 0)) continue;
    const x0 = x + from * ppbClip, x1 = x0 + dir * len * ppbClip;
    const pts = [];
    for (let i = 0; i <= 24; i++) {
      const u = i / 24;
      pts.push([x0 + (x1 - x0) * u, bottom - (bottom - top) * (CURVES[clip.curve] ?? CURVES.linear)(u)]);
    }
    c.save();
    c.beginPath();
    c.moveTo(x0, top);
    for (const [px, py] of pts) c.lineTo(px, py);
    c.lineTo(x1, top);
    c.closePath();
    c.globalAlpha = 0.35;
    c.fillStyle = T.fill.clipTitle;
    c.fill();
    c.globalAlpha = 1;
    c.beginPath();
    pts.forEach(([px, py], i) => (i ? c.lineTo(px, py) : c.moveTo(px, py)));
    c.strokeStyle = T.line.clipName; c.lineWidth = 1; c.stroke();
    c.restore();
    handle(x1);
  }
}

// Markers are buttons over the ruler (focusable, with names); the canvas skips bar
// numbers they cover and draws their line down the lanes.
let markerSpans = [];
function placeMarkers(ppb) {
  const layer = $('#markers');
  if (!layer) return;
  markerSpans = $$('.marker', layer).map((el, i) => {
    const left = Math.round(MARKERS[i].bar * ppb) + 2;
    el.style.left = `${left}px`;
    return [left, left + el.offsetWidth];
  });
}

function draw() {
  if (!ctx || W === 0) return;
  const c = ctx;
  c.setTransform(dpr, 0, 0, dpr, 0, 0);
  const rulerH = T.size.ruler, rowH = T.size.trackRow;
  const ppb = Math.max(24, W / (BARS + 0.25));
  const anySolo = state.tracks.some((t) => t.solo);

  // lanes
  c.fillStyle = T.color.timelineEmpty; c.fillRect(0, 0, W, H);
  state.tracks.forEach((t, i) => {
    const y = rulerH + i * rowH;
    c.fillStyle = t.agent ? T.color.timelineAgent : T.color.timeline;
    c.fillRect(0, y, W, rowH);
    c.fillStyle = T.line.laneTop; c.fillRect(0, y, W, 1);
    c.fillStyle = T.line.laneBottom; c.fillRect(0, y + rowH - 1, W, 1);
  });
  // cycle range shading on lanes
  const cx0 = state.cycleStart * ppb, cx1 = state.cycleEnd * ppb;
  if (state.cycle) { c.fillStyle = T.fill.cycleLane; c.fillRect(cx0, rulerH, cx1 - cx0, H - rulerH); }
  // grid
  for (let b = 0; b <= BARS + 1; b++) {
    const x = Math.round(b * ppb) + 0.5;
    c.fillStyle = T.line.barLine; c.fillRect(x - 0.5, rulerH, 1, H - rulerH);
    for (let q = 1; q < 4; q++) { c.fillStyle = T.line.beatLine; c.fillRect(Math.round((b + q / 4) * ppb), rulerH, 1, H - rulerH); }
  }
  // clips
  for (const clip of clips) {
    const ti = state.tracks.findIndex((t) => t.id === clip.trackId);
    const t = state.tracks[ti];
    const x = clip.start * ppb + 1, w = clip.length * ppb - 2;
    const y = rulerH + ti * rowH + T.size.clipInset, h = rowH - 2 * T.size.clipInset;
    const dim = t.mute || (anySolo && !t.solo);
    const agentApplied = clip.agent && state.agentApplied;
    drawClip(c, clip, x, y, w, h, PALETTE[t.id], dim, agentApplied);
  }
  // marker lines
  c.save();
  c.globalAlpha = 0.45;
  c.fillStyle = T.color.accent;
  for (const mk of MARKERS) if (mk.bar > 0) c.fillRect(Math.round(mk.bar * ppb), rulerH, 1, H - rulerH);
  c.restore();
  // ruler
  c.fillStyle = T.color.ruler; c.fillRect(0, 0, W, rulerH);
  if (state.cycle) { c.fillStyle = T.fill.cycleRuler; c.fillRect(cx0, 0, cx1 - cx0, rulerH); c.fillStyle = T.line.cycleEdge; c.fillRect(Math.round(cx0), 0, 1, rulerH); c.fillRect(Math.round(cx1) - 1, 0, 1, rulerH); }
  placeMarkers(ppb);
  c.font = `500 ${T.fontSize.small}px ${T.font.mono}`;
  c.textBaseline = 'alphabetic';
  for (let b = 0; b <= BARS + 1; b++) {
    const x = Math.round(b * ppb);
    c.fillStyle = T.line.rulerBar; c.fillRect(x, rulerH - T.timeline.rulerTickH, 1, T.timeline.rulerTickH);
    for (let q = 1; q < 4; q++) { c.fillStyle = T.line.rulerTick; c.fillRect(Math.round((b + q / 4) * ppb), rulerH - 3, 1, 3); }
    const label = String(b + 1), lx = x + 5, lw = c.measureText(label).width;
    if (markerSpans.some(([l, r]) => lx < r + 2 && lx + lw > l - 2)) continue;
    c.fillStyle = T.color.ink500; c.fillText(label, lx, 12);
  }
  c.fillStyle = T.line.rulerBottom; c.fillRect(0, rulerH - 1, W, 1);
  // playhead
  const px = Math.round(state.position * ppb) + 0.5;
  c.save();
  for (const layer of layersOf(T.canvasShadow.playhead)) {
    c.shadowBlur = layer.blur; c.shadowColor = layer.color;
    c.fillStyle = T.color.accent; c.fillRect(px - 0.5, 0, 1, H);
  }
  c.restore();
  c.save();
  c.shadowBlur = layerOf(T.canvasShadow.playheadFlag, 0).blur; c.shadowColor = layerOf(T.canvasShadow.playheadFlag, 0).color;
  c.fillStyle = T.color.accent;
  c.beginPath();
  c.moveTo(px - T.timeline.playheadFlagW / 2, rulerH - T.timeline.playheadFlagH - 1);
  c.lineTo(px + T.timeline.playheadFlagW / 2, rulerH - T.timeline.playheadFlagH - 1);
  c.lineTo(px, rulerH - 1);
  c.closePath(); c.fill();
  c.restore();
}

const markerLayer = $('#markers');
if (markerLayer) {
  MARKERS.forEach((mk, i) => {
    const el = document.createElement('button');
    el.type = 'button';
    el.className = 'marker';
    el.textContent = mk.name;
    el.title = `${mk.name}: click to jump, double-click or Shift+Enter to loop this section`;
    el.setAttribute('aria-label', `${mk.name}, bar ${mk.bar + 1}. Jump here; Shift+Enter loops the section`);
    el.addEventListener('click', (e) => { if (!e.shiftKey) dispatch('transport.locate', { bar: mk.bar }); });
    const loop = () => {
      dispatch('transport.setCycle', { enabled: true, startBar: mk.bar, endBar: sectionEnd(i) });
      dispatch('transport.locate', { bar: mk.bar });
    };
    el.addEventListener('dblclick', loop);
    el.addEventListener('keydown', (e) => { if (e.key === 'Enter' && e.shiftKey) { e.preventDefault(); loop(); } });
    markerLayer.append(el);
  });
  shownSection = -1;
  renderTime();
}

if (canvas && wrap) {
  const resize = () => {
    const r = wrap.getBoundingClientRect();
    W = Math.max(1, Math.round(r.width)); H = Math.max(1, Math.round(r.height)); dpr = window.devicePixelRatio || 1;
    canvas.width = W * dpr; canvas.height = H * dpr;
    draw();
  };
  new ResizeObserver(resize).observe(wrap);
  resize();
  // Marker widths depend on the web font; measure again once it has loaded.
  document.fonts?.ready.then(draw);
  subscribe((s, name) => { if (name !== 'transport.tick') draw(); });
}

// ---------------------------------------------------------------------------
// Playback clock. The clock dispatches transport.tick with the elapsed time,
// so the command itself stays deterministic.
// ---------------------------------------------------------------------------

let last = 0;
let rafId = 0;
function frame(now) {
  if (!state.playing) { rafId = 0; return; }
  const dt = last ? Math.min(0.1, (now - last) / 1000) : 0;
  last = now;
  dispatch('transport.tick', { deltaSeconds: dt });
  draw();
  renderTime();
  rafId = requestAnimationFrame(frame);
}
subscribe((s) => {
  if (s.playing && !rafId) { last = 0; rafId = requestAnimationFrame(frame); }
});

// CPU LEDs
const cpu = $('#cpu');
if (cpu) {
  cpu.innerHTML = '<i></i>'.repeat(12);
  const segs = $$('i', cpu);
  let level = 2;
  setInterval(() => {
    if (document.hidden || reducedMotion || !dawVisible) return;
    const target = state.playing ? 4 + Math.random() * 4 : 1 + Math.random() * 1.2;
    level += (target - level) * 0.5;
    segs.forEach((el, i) => { el.classList.toggle('on', i < Math.round(level)); el.classList.toggle('hot', i >= 10); });
  }, 140);
}

// Space toggles play while the window is on screen, like the app.
let dawVisible = false;
const daw = $('#daw');
if (daw) new IntersectionObserver(([e]) => { dawVisible = e.isIntersecting; }, { threshold: 0.35 }).observe(daw);
document.addEventListener('keydown', (e) => {
  if (!dawVisible || e.target !== document.body) return;
  if (e.code === 'Space') { e.preventDefault(); dispatch(state.playing ? 'transport.stop' : 'transport.play'); }
  else if (e.code === 'Enter') { dispatch('transport.returnToStart'); }
  else if (e.key === 'c' || e.key === 'C') { dispatch('transport.setCycle', { enabled: !state.cycle }); }
});

// ---------------------------------------------------------------------------
// Hardware rack
// ---------------------------------------------------------------------------

function dragVertical(el, getValue, setValue, travelPx) {
  let startY = 0, startV = 0;
  el.addEventListener('pointerdown', (e) => {
    startY = e.clientY; startV = getValue();
    el.setPointerCapture(e.pointerId);
    el.classList.add('dragging');
  });
  el.addEventListener('pointermove', (e) => {
    if (!el.hasPointerCapture(e.pointerId)) return;
    setValue(clamp(startV + (startY - e.clientY) / travelPx, 0, 1));
  });
  el.addEventListener('pointerup', (e) => { el.releasePointerCapture(e.pointerId); el.classList.remove('dragging'); });
  el.addEventListener('keydown', (e) => {
    const step = e.shiftKey ? 0.1 : 0.02;
    if (e.key === 'ArrowUp' || e.key === 'ArrowRight') { e.preventDefault(); setValue(clamp(getValue() + step, 0, 1)); }
    if (e.key === 'ArrowDown' || e.key === 'ArrowLeft') { e.preventDefault(); setValue(clamp(getValue() - step, 0, 1)); }
  });
}

const knob = $('#knob');
if (knob) {
  let v = (60 - 6) / 66;
  const apply = () => {
    knob.style.setProperty('--angle', `${-135 + v * 270}deg`);
    const db = -60 + v * 66;
    const text = v === 0 ? '-∞ dB' : `${db >= 0 ? '+' : ''}${db.toFixed(1)} dB`;
    $('#knob-value').textContent = text;
    knob.setAttribute('aria-valuenow', db.toFixed(1));
    knob.setAttribute('aria-valuetext', text);
  };
  dragVertical(knob, () => v, (nv) => { v = nv; apply(); }, 160);
  apply();
}

const fader = $('#fader');
const meter = $('#meter');
if (fader && meter) {
  let v = 0.78;
  meter.innerHTML = '<i></i>'.repeat(20);
  const segs = $$('i', meter);
  const apply = () => {
    fader.style.setProperty('--v', v);
    fader.setAttribute('aria-valuenow', Math.round(v * 100));
    fader.setAttribute('aria-valuetext', `${Math.round(v * 100)} %`);
  };
  dragVertical(fader, () => v, (nv) => { v = nv; apply(); }, T.size.faderH);
  apply();
  let phase = 0, rackVisible = false, shown = 0;
  new IntersectionObserver(([e]) => { rackVisible = e.isIntersecting; }, { threshold: 0.1 }).observe(meter);
  const tickMeter = () => {
    if (rackVisible && !document.hidden) {
      phase += 0.06;
      const wobble = reducedMotion ? 0 : 0.12 * Math.sin(phase * 1.7) + 0.08 * Math.sin(phase * 5.3) + 0.05 * Math.sin(phase * 13.1);
      const target = clamp(v * (0.88 + wobble), 0, 1) * segs.length;
      shown = target > shown ? target : shown - 0.6;
      segs.forEach((el, i) => { el.classList.toggle('on', i < shown); el.classList.toggle('hot', i >= segs.length - 2); });
    }
    setTimeout(tickMeter, rackVisible && !document.hidden ? 50 : 400);
  };
  tickMeter();
}

const segmented = $('#segmented');
segmented?.addEventListener('click', (e) => {
  const btn = e.target.closest('button');
  if (!btn) return;
  for (const b of $$('button', segmented)) b.setAttribute('aria-pressed', String(b === btn));
});

// ---------------------------------------------------------------------------
// Page motion. Patterns after transitions.dev (texts reveal, number pop-in, text
// states swap, tabs sliding, 3D tilt), written from scratch. CSS owns what moves;
// this only sets classes and custom properties. Reduced motion skips all of it.
// ---------------------------------------------------------------------------

/** Text states swap: the old label blurs out, the new one blurs in. */
function swapText(el, text) {
  if (reducedMotion) { el.textContent = text; return; }
  el.classList.add('is-out');
  setTimeout(() => { el.textContent = text; el.classList.remove('is-out'); }, 140);
}

// Copy clone command
$('#copy')?.addEventListener('click', async (e) => {
  const label = $('[data-label]', e.currentTarget);
  try {
    await navigator.clipboard.writeText($('#clone-cmd').textContent);
    swapText(label, 'Copied');
  } catch {
    swapText(label, 'Select it');
  }
  setTimeout(() => swapText(label, 'Copy'), 1600);
});

// Sound folders light in order; cards carry the pointer position for their sheen.
$$('.families__row li').forEach((li, i) => li.style.setProperty('--i', i));
const finePointer = matchMedia('(hover: hover) and (pointer: fine)').matches;
if (!reducedMotion && finePointer) {
  for (const card of $$('.feature, .switch__col, .news__item, .card, .dl, .release')) {
    card.addEventListener('pointermove', (e) => {
      const r = card.getBoundingClientRect();
      card.style.setProperty('--mx', `${e.clientX - r.left}px`);
      card.style.setProperty('--my', `${e.clientY - r.top}px`);
    }, { passive: true });
  }
  // 3D tilt with a glare that follows the pointer, on the window captures.
  for (const frame of $$('.shot__frame, .gallery__frame')) {
    frame.addEventListener('pointermove', (e) => {
      const r = frame.getBoundingClientRect();
      const x = (e.clientX - r.left) / r.width, y = (e.clientY - r.top) / r.height;
      frame.style.setProperty('--ry', `${(x - 0.5) * 5}deg`);
      frame.style.setProperty('--rx', `${(0.5 - y) * 4}deg`);
      frame.style.setProperty('--gx', `${x * 100}%`);
      frame.style.setProperty('--gy', `${y * 100}%`);
      frame.classList.add('is-tilting');
    }, { passive: true });
    frame.addEventListener('pointerleave', () => {
      frame.classList.remove('is-tilting');
      frame.style.setProperty('--rx', '0deg');
      frame.style.setProperty('--ry', '0deg');
    });
  }
}

// ---------------------------------------------------------------------------
// The hero's signal: a row of level bars that breathe like a meter bridge.
// Pure CSS animation (transform only); it pauses off screen.
// ---------------------------------------------------------------------------

const signal = $('#signal');
if (signal) {
  const rnd = lcg(7);
  const build = () => {
    const count = Math.max(24, Math.min(128, Math.floor(signal.clientWidth / 8)));
    if (signal.childElementCount === count) return;
    let html = '';
    for (let i = 0; i < count; i++) {
      const u = i / (count - 1);
      // A phrase-like envelope: louder in the middle, two swells, never silent.
      const env = 0.25 + 0.75 * Math.sin(Math.PI * u) * (0.7 + 0.3 * Math.sin(u * 9.4 + 1.3));
      const hi = Math.max(0.14, env * (0.65 + rnd() * 0.35));
      const lo = hi * (0.25 + rnd() * 0.3);
      html += `<i style="--hi:${hi.toFixed(3)};--lo:${lo.toFixed(3)};--dur:${(0.9 + rnd() * 1.1).toFixed(2)}s;--delay:-${(rnd() * 2).toFixed(2)}s"></i>`;
    }
    signal.innerHTML = html;
  };
  build();
  new ResizeObserver(build).observe(signal);
  new IntersectionObserver(([e]) => signal.classList.toggle('is-paused', !e.isIntersecting)).observe(signal);
}

// ---------------------------------------------------------------------------
// Tabs with a sliding pill (the theme gallery's theme and mode pickers).
// ---------------------------------------------------------------------------

function placePill(group) {
  const on = $('[aria-selected="true"], [aria-checked="true"]', group);
  const pill = $('.tabs__pill', group);
  if (!on || !pill) return;
  pill.style.setProperty('--x', `${on.offsetLeft}px`);
  pill.style.setProperty('--w', `${on.offsetWidth}px`);
}
const pillGroups = $$('.tabs');
const placeAll = () => pillGroups.forEach(placePill);
if (pillGroups.length) {
  placeAll();
  document.fonts?.ready.then(placeAll);
  new ResizeObserver(placeAll).observe(pillGroups[0].parentElement);
  requestAnimationFrame(() => pillGroups.forEach((g) => g.classList.add('is-ready')));
}

// ---------------------------------------------------------------------------
// Theme gallery: captures of the app's renderer in every theme and mode. The site
// itself keeps one look; this only swaps a picture, crossfaded with a view
// transition where the browser has one.
// ---------------------------------------------------------------------------

const galleryImg = $('#gallery-img');
if (galleryImg) {
  const themeTabs = $$('#theme-tabs [role="tab"]');
  const modeTabs = $$('#mode-tabs [role="radio"]');
  let theme = 'skeuo', mode = 'dark', request = 0;

  const select = (list, attr, value, key) => {
    for (const b of list) {
      const on = b.dataset[key] === value;
      b.setAttribute(attr, String(on));
      b.tabIndex = on ? 0 : -1;
    }
  };
  async function show() {
    const id = ++request;
    const tab = themeTabs.find((b) => b.dataset.themeId === theme);
    const src = `img/theme-${theme}-${mode}.webp`;
    const alt = `The Ondera window in the ${tab.dataset.themeName} theme, ${mode} mode: arrangement, piano roll, channel inspector and agent panel`;
    // Load before the swap so the transition never waits on the network.
    const next = new Image();
    next.src = src;
    try { await next.decode(); } catch { /* shown anyway */ }
    if (id !== request) return;
    const swap = () => {
      galleryImg.src = src;
      galleryImg.alt = alt;
      for (const p of $$('[data-theme-desc]')) p.hidden = p.dataset.themeDesc !== theme;
    };
    if (document.startViewTransition && !reducedMotion) {
      document.startViewTransition(async () => {
        swap();
        try { await galleryImg.decode(); } catch { /* ignore */ }
      });
    } else swap();
  }
  for (const b of themeTabs) b.addEventListener('click', () => {
    if (b.dataset.themeId === theme) return;
    theme = b.dataset.themeId;
    select(themeTabs, 'aria-selected', theme, 'themeId');
    placeAll();
    b.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: reducedMotion ? 'auto' : 'smooth' });
    show();
  });
  for (const b of modeTabs) b.addEventListener('click', () => {
    if (b.dataset.modeId === mode) return;
    mode = b.dataset.modeId;
    select(modeTabs, 'aria-checked', mode, 'modeId');
    placeAll();
    show();
  });
  // Warm the cache for the other captures once the gallery is near.
  new IntersectionObserver(([e], io) => {
    if (!e.isIntersecting) return;
    io.disconnect();
    const warm = () => {
      for (const t of themeTabs) for (const m of ['dark', 'light']) {
        const src = `img/theme-${t.dataset.themeId}-${m}.webp`;
        if (!galleryImg.src.endsWith(src)) new Image().src = src;
      }
    };
    (window.requestIdleCallback ?? setTimeout)(warm);
  }, { rootMargin: '400px 0px' }).observe(galleryImg);
}

// ---------------------------------------------------------------------------
// The section link you are reading is marked in the nav.
// ---------------------------------------------------------------------------

const navFor = new Map($$('.nav__links a[href^="#"]').map((a) => [a.getAttribute('href').slice(1), a]));
const sectionSpy = new IntersectionObserver((entries) => {
  for (const e of entries) {
    const link = navFor.get(e.target.id);
    if (!link) continue;
    if (e.isIntersecting) {
      for (const a of navFor.values()) a.removeAttribute('aria-current');
      link.setAttribute('aria-current', 'location');
    } else if (link.hasAttribute('aria-current')) link.removeAttribute('aria-current');
  }
}, { rootMargin: '-45% 0px -50% 0px' });
for (const id of navFor.keys()) { const s = document.getElementById(id); if (s) sectionSpy.observe(s); }

// ---------------------------------------------------------------------------
// Section menu on narrow screens.
// ---------------------------------------------------------------------------

const menuBtn = $('.nav__menu');
const navLinks = $('#nav-links');
if (menuBtn && navLinks) {
  const setOpen = (open) => {
    navLinks.classList.toggle('is-open', open);
    menuBtn.setAttribute('aria-expanded', String(open));
  };
  menuBtn.addEventListener('click', () => setOpen(!navLinks.classList.contains('is-open')));
  navLinks.addEventListener('click', (e) => { if (e.target.closest('a')) setOpen(false); });
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && navLinks.classList.contains('is-open')) { setOpen(false); menuBtn.focus(); }
  });
  document.addEventListener('click', (e) => {
    if (navLinks.classList.contains('is-open') && !e.target.closest('.nav')) setOpen(false);
  });
  matchMedia('(min-width: 1041px)').addEventListener('change', (e) => { if (e.matches) setOpen(false); });
}

// ---------------------------------------------------------------------------
// Downloads: name the visitor's platform on the buttons. The links already work
// without this (the server redirects /download by User-Agent).
// ---------------------------------------------------------------------------

async function detectPlatform() {
  const ua = navigator.userAgent;
  if (/Android|iPhone|iPad/i.test(ua)) return null;
  if (/Windows/i.test(ua)) return 'windows-x86_64';
  if (/Mac/i.test(ua)) {
    // Safari and Firefox report Intel on every Mac; only Chromium can tell them apart.
    try {
      const { architecture } = await navigator.userAgentData.getHighEntropyValues(['architecture']);
      if (architecture === 'x86') return 'macos-x86_64';
    } catch { /* not Chromium */ }
    return 'macos-arm64';
  }
  if (/Linux|X11/i.test(ua)) return 'linux-x86_64';
  return null;
}
detectPlatform().then((platform) => {
  if (!platform) return;
  const names = { 'macos-arm64': 'macOS', 'macos-x86_64': 'macOS (Intel)', 'windows-x86_64': 'Windows', 'linux-x86_64': 'Linux' };
  $$('[data-download]').forEach((a) => { a.href = `/download/${platform}`; });
  $$('[data-download-label]').forEach((a) => { a.textContent = `Download for ${names[platform]}`; });
  $(`.dl[data-os="${platform}"]`)?.classList.add('is-yours');
});
