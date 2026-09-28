// Records the real Ondera frontend (`npm --prefix frontend run dev`, fixture song from
// src/dev/mockHost.ts) frame by frame into the footage the composition plays. Every frame is a
// function of its time: the script sets the app's state through the same Tauri events the Rust
// host sends (telemetry, agent, document, settings), waits for the paint, then screenshots.
//
//   node record.mjs [shot ...]      shots: session composer mixer plugin themes
import { chromium } from "playwright-core";
import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, rmSync } from "node:fs";

const BASE = process.env.ONDERA_DEV ?? "http://127.0.0.1:1420/";
const FPS = 30;
const here = new URL(".", import.meta.url).pathname;
const footage = `${here}../composition/assets/footage/`;
mkdirSync(footage, { recursive: true });

// Per-frame loudness of the music bed, so the app's meters move with the soundtrack.
const env = JSON.parse(readFileSync(`${here}meters.json`, "utf8"));

// Composition time where each shot starts (brag-plan.md storyboard).
const SHOTS = {
// Scale 3 keeps the deepest zoom (about 2.4× in a 4K render downscaled to 1080p) above 1 source
// pixel per output pixel; the composer card is shown at 2.5× from its first frame.
  session: { at: 0, dur: 11.4, query: "agent=empty", dpr: 3 },
  composer: { at: 0, dur: 3.5, query: "agent=empty", dpr: 5, clip: [1220, 668, 380, 292] },
  mixer: { at: 11.2, dur: 2.6, query: "agent=empty&panel=mixer", dpr: 3 },
  plugin: { at: 13.6, dur: 2.6, query: "agent=empty&panel=plugin:Ondera Comp", dpr: 3 },
  themes: { at: 16.0, dur: 3.4, query: "agent=empty", dpr: 3 },
};

const PROMPT = "Add a sixteenth-note answer to the chords on bars 5 to 8.";
const PLAN = "I'll answer each chord on bars 5–8 with a sixteenth-note figure from its own notes, in C minor, on **Chords**.";
const DONE = "Done: 24 notes on bars 5–8, drawn in teal so you can tell them apart. ⌘Z takes all of them back.";

// The agent's notes: eighths from bar 5 to the end of bar 8, climbing through each bar's chord.
const CHORD_TONES = [
  [72, 75, 79, 84],
  [68, 72, 75, 80],
  [70, 75, 79, 82],
  [70, 74, 77, 82],
];
const AGENT_NOTES = Array.from({ length: 24 }, (_, i) => {
  const bar = Math.floor(i / 6);
  const tones = CHORD_TONES[bar];
  const shape = [0, 1, 2, 3, 2, 1][i % 6];
  return {
    id: `ag-${i}`,
    start: 8 + (i * 16) / 24,
    length: 0.5,
    pitch: tones[shape],
    velocity: 88 + ((i * 7) % 24),
    agent: true,
  };
});

const THEMES = [
  [0, "skeuo", "dark"],
  [0.52, "modern", "light"],
  [1.02, "aero", "dark"],
  [1.52, "console", "dark"],
  [2.02, "ink", "light"],
  [2.52, "neon", "dark"],
];

function stream(text, t, from, cps = 90) {
  const n = Math.max(0, Math.floor((t - from) * cps));
  return { text: text.slice(0, n), done: n >= text.length };
}
const step = (name, args, result, ok = true) => ({ role: "tool", text: "", tool: { name, args, ok, result } });

/** What the app should show at composition time `t` (session and composer shots). */
function agentAt(t) {
  const entries = [];
  let status = "Done";
  let running = false;
  if (t >= 3.2) {
    entries.push({ role: "user", text: PROMPT });
    running = true;
    status = "Reading the session…";
  }
  if (t >= 3.75) {
    entries.push(step("session.inspect", {}, { tracks: 6, clips: 9 }));
  }
  if (t >= 4.2) {
    const s = stream(PLAN, t, 4.2, 120);
    entries.push({ role: "assistant", text: s.text, streaming: !s.done });
    status = "Writing…";
  }
  if (t >= 5.35) {
    entries.push(step("clip.setNotes", { clipId: "c3", notes: AGENT_NOTES }, { noteCount: AGENT_NOTES.length }));
    status = "Running clip.setNotes…";
  }
  if (t >= 6.75) {
    const s = stream(DONE, t, 6.75, 110);
    entries.push({ role: "assistant", text: s.text, streaming: !s.done });
    status = s.done ? "Done" : "Writing…";
    running = !s.done;
  }
  return {
    status: { provider: "claude", model: "claude-sonnet-5", reasoningEffort: "medium", running, status, error: null, elapsedSeconds: Math.max(0, Math.floor(t - 3.2)) },
    transcript: { entries },
    changes: [],
  };
}
/** How many of the agent's notes are in the document at `t` (drawn left to right, then undone). */
function notesAt(t) {
  if (t >= 9.4) return 0;
  return Math.max(0, Math.min(AGENT_NOTES.length, Math.floor(((t - 5.35) / 1.2) * AGENT_NOTES.length)));
}

async function record(name) {
  const shot = SHOTS[name];
  const browser = await chromium.launch();
  const context = await browser.newContext({
    viewport: { width: 1600, height: 1000 },
    deviceScaleFactor: shot.dpr,
    reducedMotion: "reduce",
  });
  // The mock host reports a stopped transport every 120 ms; this script is the host now.
  await context.addInitScript(() => {
    const every = window.setInterval;
    window.setInterval = function (fn, ms, ...rest) {
      return ms === 120 ? 0 : every.call(this, fn, ms, ...rest);
    };
  });
  const page = await context.newPage();
  await page.goto(`${BASE}?${shot.query}`, { waitUntil: "networkidle" });
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(1200);
  // Hide the text caret and scrollbars; they blink on their own clock.
  await page.addStyleTag({ content: "* { caret-color: transparent !important; } ::-webkit-scrollbar { display: none; }" });

  const base = await page.evaluate(() => window.__TAURI_INTERNALS__.invoke("daw_command", { method: "web.document", params: {} }));
  const chords = base.clips.find((c) => c.id === "c3");
  const own = chords.data.notes.filter((n) => !n.agent);

  const dir = `${here}frames/${name}/`;
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  const frames = Math.round(shot.dur * FPS);
  let seq = 100000;
  let lastNotes = -1;
  let lastTheme = "skeuo";
  let lastTyped = null;
  let lastAgent = "";

  for (let k = 0; k < frames; k++) {
    const t = shot.at + k / FPS;
    const e = env.frames[Math.min(env.frames.length - 1, Math.round(t * FPS))];
    const telemetry = {
      position: 8 + t * 2,
      playing: true,
      recording: false,
      peaks: [e.l, e.r, e.keys, e.keys * 0.97],
      trackPeaks: [e.drums, e.bass, e.keys, e.pad, e.vox * (t > 6 ? 1 : 0.15), e.riser],
      cpu: 0.21 + 0.03 * e.l,
    };
    const updates = { telemetry };

    if (name === "session" || name === "composer") {
      const n = notesAt(t);
      if (n !== lastNotes) {
        lastNotes = n;
        const doc = structuredClone(base);
        const c3 = doc.clips.find((c) => c.id === "c3");
        c3.data.notes = [...own, ...AGENT_NOTES.slice(0, n)];
        c3.agent = n > 0;
        doc.snapshotSequence = ++seq;
        updates.document = doc;
      }
      const agent = JSON.stringify(agentAt(t));
      if (agent !== lastAgent) {
        lastAgent = agent;
        updates.agent = JSON.parse(agent);
      }
      const typed = t < 0.3 ? "" : t < 3.2 ? PROMPT.slice(0, Math.floor(((t - 0.3) / 2.5) * PROMPT.length)) : "";
      if (typed !== lastTyped) {
        lastTyped = typed;
        updates.typed = typed;
      }
    } else if (lastNotes === -1) {
      // Every other shot shows the session as it was after the undo.
      lastNotes = 0;
      const doc = structuredClone(base);
      const c3 = doc.clips.find((c) => c.id === "c3");
      c3.data.notes = own;
      c3.agent = false;
      doc.snapshotSequence = ++seq;
      updates.document = doc;
    }
    if (name === "themes") {
      const [, theme, mode] = THEMES.filter(([at]) => t - shot.at >= at).pop();
      if (theme !== lastTheme) {
        lastTheme = theme;
        updates.theme = [theme, mode];
      }
    }

    await page.evaluate(async (u) => {
      const I = window.__TAURI_INTERNALS__;
      const emit = (event, payload) => I.invoke("plugin:event|emit", { event, payload });
      if (u.document) await emit("daw:document", u.document);
      if (u.agent) await emit("daw:agent", u.agent);
      if (u.theme) {
        await I.invoke("daw_command", { method: "settings.set", params: { path: "interface.appearance", value: u.theme[0] } });
        await I.invoke("daw_command", { method: "settings.set", params: { path: "interface.mode", value: u.theme[1] } });
      }
      await emit("daw:telemetry", u.telemetry);
      if (u.typed !== undefined) {
        const box = document.querySelector(".agent-composer textarea");
        const set = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value").set;
        set.call(box, u.typed);
        box.dispatchEvent(new Event("input", { bubbles: true }));
      }
      await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
    }, updates);
    await page.screenshot({
      path: `${dir}${String(k).padStart(5, "0")}.png`,
      ...(shot.clip ? { clip: { x: shot.clip[0], y: shot.clip[1], width: shot.clip[2], height: shot.clip[3] } } : {}),
    });
  }
  await browser.close();

  execFileSync("ffmpeg", [
    "-v", "error", "-y", "-framerate", String(FPS), "-i", `${dir}%05d.png`,
    // Near-lossless 4:4:4: Hyperframes decodes footage with FFmpeg, and UI text loses its
    // coloured edges under 4:2:0.
    "-c:v", "libx264", "-preset", "slow", "-crf", "6", "-g", "15", "-pix_fmt", "yuv444p",
    "-movflags", "+faststart", `${footage}${name}.mp4`,
  ]);
  console.log(`${name}: ${frames} frames`);
}

const names = process.argv.slice(2);
for (const name of names.length ? names : Object.keys(SHOTS)) await record(name);
