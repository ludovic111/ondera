import { native } from "../state/native";
export const PEAKS_PER_SECOND = 400;
const peaks = new Map<string, Float32Array>();
const rates = new Map<string, number>();
let generation = 0;
const pending = new Map<string, Promise<void>>();
const listeners = new Set<() => void>();
export const library = {
  rateFor: (id: string) => rates.get(id) ?? PEAKS_PER_SECOND,
  peaksFor: (id: string) => peaks.get(id),
  onChange: (fn: () => void) => {
    listeners.add(fn);
    return () => {
      listeners.delete(fn);
    };
  },
  load: (id: string): Promise<void> => {
    if (peaks.has(id)) return Promise.resolve();
    const existing = pending.get(id);
    if (existing) return existing;
    const current = generation;
    const task = native<{ peaks: number[]; rate: number }>("web.peaks", {
      sourceId: id,
    })
      .then((data) => {
        if (current !== generation) return;
        rates.set(id, data.rate);
        peaks.set(id, Float32Array.from(data.peaks));
        for (const fn of listeners) fn();
      })
      .finally(() => {
        if (pending.get(id) === task) pending.delete(id);
      });
    pending.set(id, task);
    return task;
  },
  clear: () => {
    generation++;
    peaks.clear();
    rates.clear();
    pending.clear();
  },
};
