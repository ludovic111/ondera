/**
 * Colour maths for canvas code, which cannot use CSS color-mix() or oklch().
 * Parses hex / rgb(a) / oklch() strings, mixes in OKLab, and formats rgba.
 */

export interface Rgb {
  r: number;
  g: number;
  b: number;
  a: number;
}

interface Lab {
  L: number;
  a: number;
  b: number;
}

const clamp01 = (x: number) => Math.min(1, Math.max(0, x));

function srgbToLinear(c: number): number {
  return c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
}

function linearToSrgb(c: number): number {
  const v = c <= 0.0031308 ? 12.92 * c : 1.055 * Math.pow(c, 1 / 2.4) - 0.055;
  return clamp01(v);
}

function labToRgb({ L, a, b }: Lab): Rgb {
  const l_ = L + 0.3963377774 * a + 0.2158037573 * b;
  const m_ = L - 0.1055613458 * a - 0.0638541728 * b;
  const s_ = L - 0.0894841775 * a - 1.291485548 * b;
  const l = l_ * l_ * l_;
  const m = m_ * m_ * m_;
  const s = s_ * s_ * s_;
  return {
    r: linearToSrgb(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s),
    g: linearToSrgb(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s),
    b: linearToSrgb(-0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s),
    a: 1,
  };
}

function rgbToLab({ r, g, b }: Rgb): Lab {
  const lr = srgbToLinear(r);
  const lg = srgbToLinear(g);
  const lb = srgbToLinear(b);
  const l = Math.cbrt(
    0.4122214708 * lr + 0.5363325363 * lg + 0.0514459929 * lb,
  );
  const m = Math.cbrt(
    0.2119034982 * lr + 0.6806995451 * lg + 0.1073969566 * lb,
  );
  const s = Math.cbrt(
    0.0883024619 * lr + 0.2817188376 * lg + 0.6299787005 * lb,
  );
  return {
    L: 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    a: 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    b: 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  };
}

const cache = new Map<string, Rgb>();

/** Parses #rgb, #rrggbb, rgb()/rgba(), oklch(L C H [/ a]). */
export function parseColor(input: string): Rgb {
  const hit = cache.get(input);
  if (hit) return hit;
  const s = input.trim();
  let out: Rgb;
  if (s.startsWith("#")) {
    const hex = s.slice(1);
    const n =
      hex.length === 3
        ? hex
            .split("")
            .map((c) => c + c)
            .join("")
        : hex;
    out = {
      r: parseInt(n.slice(0, 2), 16) / 255,
      g: parseInt(n.slice(2, 4), 16) / 255,
      b: parseInt(n.slice(4, 6), 16) / 255,
      a: n.length === 8 ? parseInt(n.slice(6, 8), 16) / 255 : 1,
    };
  } else if (s.startsWith("rgb")) {
    const parts = s
      .slice(s.indexOf("(") + 1, s.lastIndexOf(")"))
      .split(/[\s,/]+/)
      .filter(Boolean);
    out = {
      r: Number(parts[0]) / 255,
      g: Number(parts[1]) / 255,
      b: Number(parts[2]) / 255,
      a: parts[3] !== undefined ? Number(parts[3]) : 1,
    };
  } else if (s.startsWith("oklch")) {
    const body = s.slice(s.indexOf("(") + 1, s.lastIndexOf(")"));
    const [lch, alpha] = body.split("/");
    const parts = (lch ?? "").trim().split(/\s+/);
    const L = Number(parts[0]);
    const C = Number(parts[1]);
    const H = (Number(parts[2]) * Math.PI) / 180;
    out = labToRgb({ L, a: C * Math.cos(H), b: C * Math.sin(H) });
    out.a = alpha !== undefined ? Number(alpha.trim()) : 1;
  } else {
    throw new Error(`parseColor: unsupported colour "${input}"`);
  }
  cache.set(input, out);
  return out;
}

export function formatRgba({ r, g, b, a }: Rgb, alpha = a): string {
  return `rgba(${Math.round(r * 255)},${Math.round(g * 255)},${Math.round(b * 255)},${alpha})`;
}

/** Equivalent of CSS color-mix(in oklch, a p%, b). Returns an rgba string. */
export function mix(a: string, b: string, aPercent: number): string {
  const la = rgbToLab(parseColor(a));
  const lb = rgbToLab(parseColor(b));
  const t = aPercent / 100;
  return formatRgba(
    labToRgb({
      L: la.L * t + lb.L * (1 - t),
      a: la.a * t + lb.a * (1 - t),
      b: la.b * t + lb.b * (1 - t),
    }),
  );
}

export function withAlpha(colour: string, alpha: number): string {
  return formatRgba(parseColor(colour), alpha);
}

/** Same hue and chroma as `colour`, at lightness L (0..1). For note gradients. */
export function withLightness(colour: string, L: number): string {
  const lab = rgbToLab(parseColor(colour));
  return formatRgba(labToRgb({ ...lab, L }));
}
