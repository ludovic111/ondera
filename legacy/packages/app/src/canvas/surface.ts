import { useEffect, useRef, type RefObject } from 'react';
import { useStore } from '../state/session';
import { library } from '../audio/library';

export type DrawFn = (ctx: CanvasRenderingContext2D, width: number, height: number) => void;

/**
 * Owns a canvas element: HiDPI sizing, resize tracking, and redraw
 * scheduling. Redraws on every store change (coalesced to one frame), on
 * resize, once fonts are ready, and whenever `draw` changes identity.
 * The draw function reads whatever state it needs from the store itself.
 */
export function useCanvasSurface(draw: DrawFn): RefObject<HTMLCanvasElement | null> {
  const ref = useRef<HTMLCanvasElement | null>(null);
  const drawRef = useRef(draw);
  const scheduleRef = useRef<() => void>(() => {});
  const store = useStore();

  useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    let raf = 0;

    const render = () => {
      raf = 0;
      const dpr = window.devicePixelRatio || 1;
      const w = canvas.clientWidth;
      const h = canvas.clientHeight;
      if (!w || !h) return;
      const W = Math.round(w * dpr);
      const H = Math.round(h * dpr);
      if (canvas.width !== W || canvas.height !== H) {
        canvas.width = W;
        canvas.height = H;
      }
      const ctx = canvas.getContext('2d');
      if (!ctx) return;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, w, h);
      drawRef.current(ctx, w, h);
    };

    const schedule = () => {
      if (!raf) raf = requestAnimationFrame(render);
    };
    scheduleRef.current = schedule;

    const ro = new ResizeObserver(schedule);
    ro.observe(canvas);
    const off = store.subscribe(schedule);
    const offLibrary = library.onChange(schedule);
    void document.fonts?.ready.then(schedule);
    schedule();

    return () => {
      ro.disconnect();
      off();
      offLibrary();
      if (raf) cancelAnimationFrame(raf);
      scheduleRef.current = () => {};
    };
  }, [store]);

  useEffect(() => {
    drawRef.current = draw;
    scheduleRef.current();
  }, [draw]);

  return ref;
}
