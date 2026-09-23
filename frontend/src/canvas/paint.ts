import {
  font,
  fontSize,
  fontWeight,
  type CanvasShadowLayer,
} from "../theme/tokens";
import { formatRgba, parseColor } from "../theme/color";

/** Canvas font string from the type scale. */
export function uiFont(
  size: keyof typeof fontSize,
  weight: keyof typeof fontWeight = "regular",
): string {
  return `${fontWeight[weight]} ${fontSize[size]}px ${font.ui}`;
}

export function monoFont(
  size: keyof typeof fontSize,
  weight: keyof typeof fontWeight = "regular",
): string {
  return `${fontWeight[weight]} ${fontSize[size]}px ${font.mono}`;
}

/** Canvas needs rgba(); tokens may be oklch(). */
export function cc(colour: string): string {
  return colour.startsWith("oklch") ? formatRgba(parseColor(colour)) : colour;
}

export function roundRectPath(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  w: number,
  h: number,
  r: number,
): void {
  // A clip narrower than its inset (a 1/64-bar region zoomed out) gives a negative width, and
  // roundRect throws on a negative radius: the whole frame would stop painting.
  const width = Math.max(0, w);
  const height = Math.max(0, h);
  const rr = Math.max(0, Math.min(r, width / 2, height / 2));
  ctx.beginPath();
  ctx.roundRect(x, y, width, height, rr);
}

/** Paints `fillPath` once per shadow layer, back to front. */
export function withShadows(
  ctx: CanvasRenderingContext2D,
  layers: readonly CanvasShadowLayer[],
  paint: () => void,
): void {
  // A flat theme has no layers; the shape itself must still be painted.
  if (layers.length === 0) {
    paint();
    return;
  }
  for (const layer of layers) {
    ctx.save();
    ctx.shadowColor = cc(layer.color);
    ctx.shadowBlur = layer.blur;
    ctx.shadowOffsetX = 0;
    ctx.shadowOffsetY = layer.offsetY;
    paint();
    ctx.restore();
  }
}

/** 1px crisp horizontal line. */
export function hline(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  w: number,
  colour: string,
): void {
  ctx.fillStyle = cc(colour);
  ctx.fillRect(x, Math.round(y), w, 1);
}

/** 1px crisp vertical line. */
export function vline(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  h: number,
  colour: string,
): void {
  ctx.fillStyle = cc(colour);
  ctx.fillRect(Math.round(x), y, 1, h);
}
