import { useEffect, type RefObject } from "react";
import { commands } from "@ondera/core";
import { useStore } from "../../state/session";
import { laneGeometry, xToBar } from "../../canvas/timeline";

const ZOOM_SENSITIVITY = 0.01;

/**
 * Horizontal wheel scrolls the arrangement; ctrl/cmd + wheel (and trackpad
 * pinch, which Chromium reports as ctrl + wheel) zooms around the cursor.
 * Vertical wheel is left to the native scroller. Non-passive so the
 * default zoom/scroll can be prevented.
 */
export function useTimelineWheel(ref: RefObject<HTMLElement | null>): void {
  const store = useStore();
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      const state = store.getState();
      const geo = laneGeometry(state);
      if (e.ctrlKey || e.metaKey) {
        e.preventDefault();
        const x = e.clientX - el.getBoundingClientRect().left;
        store.dispatch(
          commands.view.zoomBy({
            factor: Math.exp(-e.deltaY * ZOOM_SENSITIVITY),
            anchorBar: xToBar(x, geo),
            anchorPx: x,
          }),
        );
        return;
      }
      const horizontal = e.shiftKey || Math.abs(e.deltaX) > Math.abs(e.deltaY);
      if (!horizontal) return;
      e.preventDefault();
      const dx = e.deltaX !== 0 ? e.deltaX : e.deltaY;
      store.dispatch(commands.view.scrollBy({ bars: dx / geo.ppb }));
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, [ref, store]);
}
