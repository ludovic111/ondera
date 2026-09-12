/** MIDI patterns behind the browser's Loops tab. Beats relative to the clip start. */
export interface LoopNote {
  start: number;
  length: number;
  pitch: number;
  velocity: number;
}

export interface Loop {
  bars: number;
  instrument: string;
  notes: LoopNote[];
}

const n = (start: number, length: number, pitch: number, velocity = 100): LoopNote => ({ start, length, pitch, velocity });

function drumBar(kicks: number[], snares: number[], hats: number[], bar: number): LoopNote[] {
  const o = bar * 4;
  return [
    ...kicks.map((b) => n(o + b, 0.25, 36, 118)),
    ...snares.map((b) => n(o + b, 0.25, 38, 105)),
    ...hats.map((b) => n(o + b, 0.125, 42, b % 1 === 0 ? 70 : 50)),
  ];
}

const eighths = [0, 0.5, 1, 1.5, 2, 2.5, 3, 3.5];

const LOOPS: Record<string, Loop> = {
  'Boom Bap 92': {
    bars: 2,
    instrument: 'Drum Machine',
    notes: [...drumBar([0, 2.5], [1, 3], eighths, 0), ...drumBar([0, 1.75, 2.5], [1, 3, 3.75], eighths, 1)],
  },
  'Four Floor 124': {
    bars: 2,
    instrument: 'Drum Machine',
    notes: [0, 1].flatMap((bar) => drumBar([0, 1, 2, 3], [1, 3], [0.5, 1.5, 2.5, 3.5], bar)),
  },
  'Brushes Swing': {
    bars: 2,
    instrument: 'Drum Machine',
    notes: [0, 1].flatMap((bar) => drumBar([0, 2.66], [1, 3], [0, 0.66, 1, 1.66, 2, 2.66, 3, 3.66], bar)),
  },
  'Rhodes Comp Cm': {
    bars: 4,
    instrument: 'E-Piano Mk I',
    notes: [
      [60, 63, 67, 70],
      [56, 60, 63, 67],
      [53, 56, 60, 63],
      [55, 58, 62, 65],
    ].flatMap((chord, bar) => [
      ...chord.map((p) => n(bar * 4, 1.5, p, 90)),
      ...chord.map((p) => n(bar * 4 + 2.5, 1, p, 70)),
    ]),
  },
  'Analog Pad Swell': {
    bars: 4,
    instrument: 'Choir Pad',
    notes: [
      [48, 55, 60, 63],
      [44, 51, 56, 60],
    ].flatMap((chord, i) => chord.map((p) => n(i * 8, 8, p, 80))),
  },
  'Bass Pluck 120': {
    bars: 2,
    instrument: 'Sub Bass 808',
    notes: [
      n(0, 0.75, 36, 110),
      n(1, 0.5, 36, 90),
      n(1.5, 0.5, 43, 95),
      n(2.5, 0.75, 41, 105),
      n(3.5, 0.5, 39, 90),
      n(4, 0.75, 36, 110),
      n(5, 0.5, 48, 85),
      n(5.5, 0.5, 43, 95),
      n(6.5, 0.75, 41, 105),
      n(7.5, 0.5, 43, 90),
    ],
  },
  'Riser 1 bar': { bars: 1, instrument: 'Riser', notes: [n(0, 4, 60, 110)] },
  'Reverse Cymbal': { bars: 2, instrument: 'Riser', notes: [n(0, 8, 72, 90)] },
  'Vinyl Crackle': { bars: 4, instrument: 'Drum Machine', notes: Array.from({ length: 32 }, (_, i) => n(i * 0.5, 0.05, 42, 20 + ((i * 7) % 20))) },
};

export function loopNotes(name: string): Loop | null {
  return LOOPS[name] ?? null;
}
