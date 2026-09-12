// Ondera site runtime. Three parts: a tiny command store that mirrors the app's
// dispatch(command) pattern, canvas drawing for the arrangement mock, and the
// hardware rack demo. Every visual constant comes from tokens.js.
import { tokens as T, white, black, accentAlpha } from './tokens.js';

document.documentElement.classList.add('js');
const reducedMotion = matchMedia('(prefers-reduced-motion: reduce)').matches;
const $ = (sel, root = document) => root.querySelector(sel);
const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];
const clamp = (v, lo, hi) => Math.min(hi, Math.max(lo, v));

// ---------------------------------------------------------------------------
// Mock session (mirrors packages/core/src/mock/session.ts and model/palette.ts)
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

const tracks = [
  { id: 'drums', name: 'Drums', kind: 'audio', volume: 0.78, mute: false, solo: false, armed: false, agent: false },
  { id: 'bass', name: 'Bass', kind: 'midi', volume: 0.7, mute: false, solo: false, armed: false, agent: false },
  { id: 'keys', name: 'Keys', kind: 'midi', volume: 0.62, mute: false, solo: false, armed: false, agent: true },
  { id: 'pad', name: 'Pad', kind: 'midi', volume: 0.5, mute: false, solo: false, armed: false, agent: false },
  { id: 'vox', name: 'Lead Vox', kind: 'audio', volume: 0.82, mute: false, solo: false, armed: true, agent: false },
  { id: 'bgv', name: 'BGV', kind: 'audio', volume: 0.55, mute: true, solo: false, armed: false, agent: false },
  { id: 'guitar', name: 'Guitar', kind: 'audio', volume: 0.66, mute: false, solo: false, armed: false, agent: false },
  { id: 'riser', name: 'Riser FX', kind: 'midi', volume: 0.6, mute: false, solo: false, armed: false, agent: false },
];
const audio = (id, trackId, name, start, length, seed, wave = 'tonal') => ({ id, trackId, name, start, length, kind: 'audio', seed, wave, agent: false });
const midi = (id, trackId, name, start, length, notes, agent = false) => ({ id, trackId, name, start, length, kind: 'midi', notes, agent });
const clips = [
  audio('drums-1', 'drums', 'Drums_take3', 0, 12, 1, 'drums'),
  midi('bass-1', 'bass', 'Bass intro', 0, 4, scatterNotes(11, 4, false)),
  midi('bass-2', 'bass', 'Bass verse', 4, 8, bassVerseNotes(8)),
  midi('keys-1', 'keys', 'Keys A', 0, 4, scatterNotes(21, 4, false)),
  midi('keys-2', 'keys', 'Keys B', 4, 4, scatterNotes(22, 4, true), true),
  midi('keys-3', 'keys', 'Keys B', 8, 4, scatterNotes(23, 4, false)),
  midi('pad-1', 'pad', 'Pad swell', 4, 8, scatterNotes(31, 8, false)),
  audio('vox-1', 'vox', 'LV_v2_comp', 4, 4, 2),
  audio('vox-2', 'vox', 'LV_v2_comp', 8, 4, 3),
  audio('bgv-1', 'bgv', 'BGV stack', 8, 4, 4),
  audio('guitar-1', 'guitar', 'Gtr DI', 2, 6, 5),
  audio('guitar-2', 'guitar', 'Gtr DI', 8, 3, 6),
  midi('riser-1', 'riser', 'Riser', 7, 1, scatterNotes(41, 1, false)),
];

// ---------------------------------------------------------------------------
// Store: state is only ever changed through dispatch(name, params).
// ---------------------------------------------------------------------------

const state = {
  playing: false,
  recording: false,
  cycle: true,
  position: 0, // bars, fractional
  tracks,
  agentEdit: 'pending', // 'pending' | 'kept' | 'reverted'
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
  'transport.setCycle': (s, { enabled }) => { s.cycle = enabled; },
  'transport.tick': (s, { deltaSeconds }) => {
    s.position += (deltaSeconds * TEMPO) / 60 / 4;
    if (s.position >= BARS) s.position = s.cycle ? s.position - BARS : 0;
    if (!s.cycle && s.position === 0) s.playing = false;
  },
  'track.setMute': (s, { trackId, muted }) => { track(trackId).mute = muted; },
  'track.setSolo': (s, { trackId, solo }) => { track(trackId).solo = solo; },
  'track.setArmed': (s, { trackId, armed }) => { track(trackId).armed = armed; },
  'agent.keep': (s) => { s.agentEdit = 'kept'; },
  'agent.toggleRevert': (s) => { s.agentEdit = s.agentEdit === 'reverted' ? 'kept' : 'reverted'; },
};
const SILENT = new Set(['transport.tick']);

const subscribers = new Set();
function dispatch(name, params = {}) {
  const run = commands[name];
  if (!run) throw new Error(`unknown command ${name}`);
  run(state, params);
  if (!SILENT.has(name)) {
    state.undoDepth += 1;
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
          <button class="hbtn" data-cmd="track.setMute" data-track="${t.id}" aria-label="Mute ${t.name}">M</button>
          <button class="hbtn" data-cmd="track.setSolo" data-track="${t.id}" aria-label="Solo ${t.name}">S</button>
          <button class="hbtn" data-cmd="track.setArmed" data-track="${t.id}" aria-label="Arm ${t.name}">R</button>
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
    case 'agent.keep': dispatch(name, { clipId: 'keys-2' }); break;
    case 'agent.toggleRevert': dispatch(name, { clipId: 'keys-2' }); break;
    default: dispatch(name);
  }
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
}

subscribe((s, name) => {
  if (name === 'transport.tick') return;
  const play = $('#play');
  if (play) { play.setAttribute('aria-pressed', String(s.playing)); play.setAttribute('aria-label', s.playing ? 'Stop' : 'Play'); }
  $('#rec')?.setAttribute('aria-pressed', String(s.recording));
  $('#cycle')?.setAttribute('aria-pressed', String(s.cycle));
  for (const btn of $$('.hbtn[data-track]')) {
    const t = track(btn.dataset.track);
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

  const card = $('#agent-card');
  if (card && name && name.startsWith('agent.')) {
    card.classList.add('agent__card--done');
    $('#agent-text').textContent = s.agentEdit === 'kept'
      ? 'Applied to Keys B: 20 notes quantised, 7 velocities lifted. Undo is one step back, like any other edit.'
      : 'Reverted. Keys B is back to the take you recorded; the agent\'s proposal stays in the log.';
    const actions = $('.agent__actions', card);
    actions.innerHTML = s.agentEdit === 'kept'
      ? '<button class="btn btn--xs btn--raised" data-cmd="agent.toggleRevert">Revert</button>'
      : '<button class="btn btn--xs btn--lit" data-cmd="agent.toggleRevert">Re-apply</button>';
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

function roundRect(c, x, y, w, h, r) {
  c.beginPath();
  c.roundRect(x, y, w, h, r);
}

function drawClip(c, clip, x, y, w, h, color, dim, agentPending) {
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
  // content
  const top = y + T.size.clipTitle + 3, bottom = y + h - 3, ch = bottom - top, mid = top + ch / 2;
  if (clip.kind === 'audio') {
    c.fillStyle = T.line.waveformMid;
    c.fillRect(x + 2, mid, w - 4, 1);
    c.fillStyle = T.line.waveform;
    const step = 2;
    for (let px = 3; px < w - 3; px += step) {
      const a = peak(clip, px / w) * (ch / 2) * 0.92;
      c.fillRect(x + px, mid - a, 1, Math.max(1, a * 2));
    }
  } else {
    const noteH = T.size.clipNoteH;
    for (const n of clip.notes) {
      const nx = x + (n.start / 4 / clip.length) * w;
      const nw = Math.max(2, (n.length / 4 / clip.length) * w - 1);
      const ny = top + (1 - (n.pitch - 36) / 36) * (ch - noteH);
      if (n.agent && agentPending) {
        c.save();
        c.shadowBlur = T.canvasShadow.agentNote[1].blur; c.shadowColor = T.canvasShadow.agentNote[1].color;
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
  c.fillStyle = white(0.22); c.fillRect(x, y, w, 1);
  c.fillStyle = black(0.4); c.fillRect(x, y + h - 1, w, 1);
  c.restore();
  if (agentPending) {
    c.save();
    c.shadowBlur = T.canvasShadow.agentRing[0].blur; c.shadowColor = T.canvasShadow.agentRing[0].color;
    c.strokeStyle = T.color.accent; c.lineWidth = 1;
    roundRect(c, x + 0.5, y + 0.5, w - 1, h - 1, r); c.stroke();
    c.restore();
  }
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
  if (state.cycle) { c.fillStyle = T.fill.cycleLane; c.fillRect(0, rulerH, BARS * ppb, H - rulerH); }
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
    const agentPending = clip.agent && state.agentEdit === 'pending';
    drawClip(c, clip, x, y, w, h, PALETTE[t.id], dim, agentPending);
  }
  // ruler
  c.fillStyle = T.color.ruler; c.fillRect(0, 0, W, rulerH);
  if (state.cycle) { c.fillStyle = T.fill.cycleRuler; c.fillRect(0, 0, BARS * ppb, rulerH); c.fillStyle = T.line.cycleEdge; c.fillRect(0, 0, 1, rulerH); c.fillRect(Math.round(BARS * ppb) - 1, 0, 1, rulerH); }
  c.font = `500 ${T.fontSize.small}px ${T.font.mono}`;
  c.textBaseline = 'alphabetic';
  for (let b = 0; b <= BARS + 1; b++) {
    const x = Math.round(b * ppb);
    c.fillStyle = T.line.rulerBar; c.fillRect(x, rulerH - T.timeline.rulerTickH, 1, T.timeline.rulerTickH);
    for (let q = 1; q < 4; q++) { c.fillStyle = T.line.rulerTick; c.fillRect(Math.round((b + q / 4) * ppb), rulerH - 3, 1, 3); }
    c.fillStyle = T.color.ink500; c.fillText(String(b + 1), x + 5, 12);
  }
  c.fillStyle = T.line.rulerBottom; c.fillRect(0, rulerH - 1, W, 1);
  // playhead
  const px = Math.round(state.position * ppb) + 0.5;
  c.save();
  for (const layer of T.canvasShadow.playhead) {
    c.shadowBlur = layer.blur; c.shadowColor = layer.color;
    c.fillStyle = T.color.accent; c.fillRect(px - 0.5, 0, 1, H);
  }
  c.restore();
  c.save();
  c.shadowBlur = T.canvasShadow.playheadFlag[0].blur; c.shadowColor = T.canvasShadow.playheadFlag[0].color;
  c.fillStyle = T.color.accent;
  c.beginPath();
  c.moveTo(px - T.timeline.playheadFlagW / 2, rulerH - T.timeline.playheadFlagH - 1);
  c.lineTo(px + T.timeline.playheadFlagW / 2, rulerH - T.timeline.playheadFlagH - 1);
  c.lineTo(px, rulerH - 1);
  c.closePath(); c.fill();
  c.restore();
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
    $('#knob-value').textContent = v === 0 ? '-∞ dB' : `${db >= 0 ? '+' : ''}${db.toFixed(1)} dB`;
    knob.setAttribute('aria-valuenow', db.toFixed(1));
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
  const apply = () => { fader.style.setProperty('--v', v); fader.setAttribute('aria-valuenow', Math.round(v * 100)); };
  dragVertical(fader, () => v, (nv) => { v = nv; apply(); }, T.size.faderH);
  apply();
  let phase = 0, rackVisible = false, shown = 0;
  new IntersectionObserver(([e]) => { rackVisible = e.isIntersecting; }, { threshold: 0.1 }).observe(meter);
  const tickMeter = () => {
    if (rackVisible) {
      phase += 0.06;
      const wobble = reducedMotion ? 0 : 0.12 * Math.sin(phase * 1.7) + 0.08 * Math.sin(phase * 5.3) + 0.05 * Math.sin(phase * 13.1);
      const target = clamp(v * (0.88 + wobble), 0, 1) * segs.length;
      shown = target > shown ? target : shown - 0.6;
      segs.forEach((el, i) => { el.classList.toggle('on', i < shown); el.classList.toggle('hot', i >= segs.length - 2); });
    }
    setTimeout(tickMeter, 50);
  };
  tickMeter();
}

const segmented = $('#segmented');
segmented?.addEventListener('click', (e) => {
  const btn = e.target.closest('button');
  if (!btn) return;
  for (const b of $$('button', segmented)) b.setAttribute('aria-selected', String(b === btn));
});

// Copy clone command
$('#copy')?.addEventListener('click', async (e) => {
  const btn = e.currentTarget;
  try {
    await navigator.clipboard.writeText($('#clone-cmd').textContent);
    btn.textContent = 'Copied';
  } catch {
    btn.textContent = 'Select it';
  }
  setTimeout(() => { btn.textContent = 'Copy'; }, 1400);
});

// Reveal on scroll
const revealer = new IntersectionObserver((entries) => {
  for (const e of entries) if (e.isIntersecting) { e.target.classList.add('in'); revealer.unobserve(e.target); }
}, { threshold: 0.08, rootMargin: '0px 0px -6% 0px' });
for (const el of $$('.reveal')) revealer.observe(el);
