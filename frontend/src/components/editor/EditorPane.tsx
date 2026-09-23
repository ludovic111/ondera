import {
  useCallback,
  useRef,
  useState,
  useSyncExternalStore,
  type MouseEvent,
  type PointerEvent as ReactPointerEvent,
} from "react";
import { commands, snapBeats, type EditorMode } from "@ondera/core";
import { useDispatch, useSession, useStore } from "../../state/session";
import { newId } from "../../state/ids";
import { useCanvasSurface } from "../../canvas/surface";
import {
  beatAtX,
  drawPianoRoll,
  editorClip,
  editorLowPitch,
  hitTestNote,
  isBlackKey,
  noteLabel,
  pitchAtY,
  rollGeometry,
  type RollOverlay,
} from "../../canvas/pianoRoll";
import { SegmentedControl } from "../primitives/SegmentedControl";
import { Button } from "../primitives/Button";
import { PopupMenu, type MenuState } from "../menu/PopupMenu";
import { size } from "../../theme/tokens";
import { ControllerLane } from "./ControllerLane";
import styles from "./EditorPane.module.css";

const MODES: { id: EditorMode; label: string; title?: string }[] = [
  { id: "pianoRoll", label: "Piano Roll" },
  {
    id: "score",
    label: "Score",
    title: "Score preview. Edit notes in Piano Roll or Step.",
  },
  { id: "step", label: "Step" },
];

const VELOCITIES = [40, 64, 80, 96, 112, 127];
const DRAG_THRESHOLD_PX = 3;

type Drag =
  | {
      kind: "move";
      noteId: string;
      grabBeat: number;
      origStart: number;
      origPitch: number;
      start: number;
      pitch: number;
      length: number;
      startX: number;
      startY: number;
      moved: boolean;
    }
  | {
      kind: "resize";
      noteId: string;
      start: number;
      pitch: number;
      length: number;
      origLength: number;
    }
  | {
      kind: "pencil";
      anchor: number;
      start: number;
      length: number;
      pitch: number;
    };

export function EditorPane() {
  const dispatch = useDispatch();
  const store = useStore();
  const mode = useSession((s) => s.view.editorMode);
  const clip = useSession((s) => editorClip(s));
  const track = useSession((s) =>
    clip ? (s.tracks.find((t) => t.id === clip.trackId) ?? null) : null,
  );
  const key = useSession((s) => s.transport.key);
  const snap = useSession((s) => s.transport.snapDivision);
  const selectedNote = useSession((s) => {
    const c = editorClip(s);
    return c?.data.kind === "midi"
      ? (c.data.notes.find((n) => n.id === s.view.selectedNoteId) ?? null)
      : null;
  });
  const [overlay, setOverlay] = useState<RollOverlay>({});
  const [velocity, setVelocity] = useState(100);
  const [menu, setMenu] = useState<MenuState | null>(null);
  const drag = useRef<Drag | null>(null);
  const gridRef = useRef<HTMLDivElement>(null);
  const canvasRef = useCanvasSurface(
    useCallback(
      (ctx, w, h) => drawPianoRoll(ctx, w, h, store.getState(), overlay),
      [store, overlay],
    ),
  );

  const laneOpen = useSyncExternalStore(
    store.subscribeMeta,
    () => store.ui.controllers ?? false,
  );
  const showLane = laneOpen && mode !== "score" && clip?.data.kind === "midi";

  const pitchOffset = useSession((s) => s.view.editorLowPitch);
  const low = pitchOffset ?? editorLowPitch(clip);
  const rows = Array.from(
    { length: size.keyRows },
    (_, r) => low + (size.keyRows - 1 - r),
  );

  const point = (e: { clientX: number; clientY: number }) => {
    const r = gridRef.current!.getBoundingClientRect();
    return { x: e.clientX - r.left, y: e.clientY - r.top, w: r.width };
  };
  const stepBeats = () => 4 / store.getState().transport.snapDivision;
  const snapB = (b: number, fine: boolean) =>
    fine ? b : snapBeats(b, store.getState().transport.snapDivision);

  const audition = (pitch: number, vel = velocity) => {
    if (track)
      store.fire("note.preview", { trackId: track.id, pitch, velocity: vel });
  };

  const onPointerDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (
      mode === "score" ||
      e.button !== 0 ||
      !clip ||
      clip.data.kind !== "midi"
    )
      return;
    const state = store.getState();
    const p = point(e);
    const geo = rollGeometry(state, p.w);
    if (p.y < geo.gridTop) return;
    const hit = hitTestNote(geo, p.x, p.y);
    const lengthBeats = clip.lengthBars * geo.bpb;
    if (hit) {
      dispatch(commands.note.select({ noteId: hit.note.id }));
      audition(hit.note.pitch, hit.note.velocity);
      if (mode === "step") {
        dispatch(
          commands.note.remove({ clipId: clip.id, noteId: hit.note.id }),
        );
        return;
      }
      drag.current = hit.edge
        ? {
            kind: "resize",
            noteId: hit.note.id,
            start: hit.note.start,
            pitch: hit.note.pitch,
            length: hit.note.length,
            origLength: hit.note.length,
          }
        : {
            kind: "move",
            noteId: hit.note.id,
            grabBeat: beatAtX(geo, p.x) - hit.note.start,
            origStart: hit.note.start,
            origPitch: hit.note.pitch,
            start: hit.note.start,
            pitch: hit.note.pitch,
            length: hit.note.length,
            startX: p.x,
            startY: p.y,
            moved: false,
          };
      e.currentTarget.setPointerCapture(e.pointerId);
      return;
    }
    dispatch(commands.note.select({}));
    const pitch = pitchAtY(geo, p.y);
    const beat = Math.max(0, snapB(beatAtX(geo, p.x), e.altKey));
    if (beat >= lengthBeats) return;
    if (mode === "step") {
      const length = Math.min(stepBeats(), lengthBeats - beat);
      dispatch(
        commands.note.add({
          clipId: clip.id,
          noteId: newId("note"),
          start: beat,
          length,
          pitch,
          velocity,
        }),
      );
      audition(pitch);
      return;
    }
    // Click on empty space adds a step-length note; dragging draws a longer one.
    drag.current = {
      kind: "pencil",
      anchor: beat,
      start: beat,
      length: 0,
      pitch,
    };
    e.currentTarget.setPointerCapture(e.pointerId);
  };

  const onPointerMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    const state = store.getState();
    const p = point(e);
    const geo = rollGeometry(state, p.w);
    const d = drag.current;
    if (!d) {
      const hit =
        clip && p.y >= geo.gridTop ? hitTestNote(geo, p.x, p.y) : null;
      e.currentTarget.style.cursor = hit
        ? hit.edge
          ? "ew-resize"
          : "default"
        : mode === "step"
          ? "cell"
          : "crosshair";
      return;
    }
    if (!clip) return;
    const lengthBeats = clip.lengthBars * geo.bpb;
    const beat = beatAtX(geo, p.x);
    const minLen = stepBeats();
    if (d.kind === "move") {
      if (
        !d.moved &&
        Math.hypot(p.x - d.startX, p.y - d.startY) < DRAG_THRESHOLD_PX
      )
        return;
      d.moved = true;
      d.start = Math.max(
        0,
        Math.min(lengthBeats - d.length, snapB(beat - d.grabBeat, e.altKey)),
      );
      const pitch = Math.max(0, Math.min(127, pitchAtY(geo, p.y)));
      if (pitch !== d.pitch) {
        d.pitch = pitch;
        audition(pitch);
      }
      setOverlay({
        ghost: { start: d.start, length: d.length, pitch: d.pitch },
      });
    } else if (d.kind === "resize") {
      d.length = Math.max(
        minLen,
        Math.min(lengthBeats - d.start, snapB(beat, e.altKey) - d.start),
      );
      setOverlay({
        ghost: { start: d.start, length: d.length, pitch: d.pitch },
      });
    } else {
      const b = Math.max(0, Math.min(lengthBeats, snapB(beat, e.altKey)));
      d.start = Math.min(d.anchor, b);
      d.length = Math.max(minLen, Math.abs(b - d.anchor));
      setOverlay({
        pencil: { start: d.start, length: d.length, pitch: d.pitch },
      });
    }
  };

  const onPointerUp = () => {
    const d = drag.current;
    drag.current = null;
    setOverlay({});
    if (!d || !clip) return;
    if (d.kind === "move") {
      if (d.moved && (d.start !== d.origStart || d.pitch !== d.origPitch)) {
        dispatch(
          commands.note.update({
            clipId: clip.id,
            noteId: d.noteId,
            start: d.start,
            pitch: d.pitch,
          }),
        );
      }
    } else if (d.kind === "resize") {
      if (d.length !== d.origLength)
        dispatch(
          commands.note.update({
            clipId: clip.id,
            noteId: d.noteId,
            length: d.length,
          }),
        );
    } else {
      const lengthBeats =
        clip.lengthBars *
        (4 *
          (store.getState().transport.timeSignature.numerator /
            store.getState().transport.timeSignature.denominator));
      const length = Math.min(
        d.length > 0 ? d.length : stepBeats(),
        lengthBeats - d.start,
      );
      if (length <= 0) return;
      dispatch(
        commands.note.add({
          clipId: clip.id,
          noteId: newId("note"),
          start: d.start,
          length,
          pitch: d.pitch,
          velocity,
        }),
      );
      audition(d.pitch);
    }
  };

  const onContextMenu = (e: MouseEvent<HTMLDivElement>) => {
    e.preventDefault();
    if (mode === "score" || !clip || clip.data.kind !== "midi") return;
    const p = point(e);
    const geo = rollGeometry(store.getState(), p.w);
    const hit = hitTestNote(geo, p.x, p.y);
    if (!hit) return;
    dispatch(commands.note.select({ noteId: hit.note.id }));
    setMenu({
      x: e.clientX,
      y: e.clientY,
      items: [
        ...VELOCITIES.map((v) => ({
          label: `Velocity ${v}`,
          checked: hit.note.velocity === v,
          onSelect: () =>
            dispatch(
              commands.note.update({
                clipId: clip.id,
                noteId: hit.note.id,
                velocity: v,
              }),
            ),
        })),
        { separator: true as const },
        {
          label: "Delete Note",
          shortcut: "⌫",
          onSelect: () =>
            dispatch(
              commands.note.remove({ clipId: clip.id, noteId: hit.note.id }),
            ),
        },
      ],
    });
  };

  const openVelocityMenu = (e: MouseEvent<HTMLElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    setMenu({
      x: r.left,
      y: r.bottom + 4,
      items: VELOCITIES.map((v) => ({
        label: `Velocity ${v}`,
        checked: velocity === v,
        onSelect: () => {
          setVelocity(v);
          if (clip && selectedNote)
            dispatch(
              commands.note.update({
                clipId: clip.id,
                noteId: selectedNote.id,
                velocity: v,
              }),
            );
        },
      })),
    });
  };

  return (
    <div className={`${styles.pane} ${showLane ? styles.withLane : ""}`}>
      <div className={styles.header} data-surface="editor-header">
        <SegmentedControl
          items={MODES}
          value={mode}
          onChange={(id) => dispatch(commands.view.setEditorMode({ mode: id }))}
        />
        {clip && track ? (
          <div className={styles.clipTitle}>
            <span
              className={`${styles.swatch} m-swatch`}
              style={{ background: track.color }}
            />
            {clip.name}
            <span className={styles.range}>
              · bars {clip.startBar + 1} – {clip.startBar + clip.lengthBars}
            </span>
          </div>
        ) : (
          <div className={styles.range}>
            Select a MIDI clip to edit it, or draw one with the pencil tool
          </div>
        )}
        {mode === "score" && (
          <span className={styles.range}>
            Preview · edit in Piano Roll or Step
          </span>
        )}
        <div className={styles.params}>
          <span>
            <span className={styles.dim}>Quantize</span> 1/{snap}
          </span>
          <Button
            size="auto"
            className={styles.paramButton}
            onClick={openVelocityMenu}
            title="Velocity for new notes"
          >
            <span className={styles.dim}>Velocity</span>&nbsp;
            {selectedNote ? selectedNote.velocity : velocity}
          </Button>
          <span>
            <span className={styles.dim}>Scale</span>{" "}
            {key.replace("min", "minor").replace("maj", "major")}
          </span>
        </div>
      </div>
      <div className={styles.body}>
        <div className={styles.keys}>
          <div className={styles.keysHeader} />
          {rows.map((pitch) => {
            const black = isBlackKey(pitch);
            return (
              <div
                key={pitch}
                className={`${styles.key} ${black ? styles.blackKey : styles.whiteKey}`}
                onPointerDown={() => audition(pitch)}
                title={noteLabel(pitch)}
              >
                {black ? "" : noteLabel(pitch)}
              </div>
            );
          })}
        </div>
        <div
          ref={gridRef}
          className={styles.grid}
          onWheel={(e) => {
            if (!e.ctrlKey && !e.metaKey && e.deltaY)
              store.setEditorPitch(
                Math.max(
                  0,
                  Math.min(
                    127 - size.keyRows + 1,
                    low - Math.round(e.deltaY / 10),
                  ),
                ),
              );
          }}
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={onPointerUp}
          onContextMenu={onContextMenu}
        >
          <canvas ref={canvasRef} className={styles.canvas} />
        </div>
      </div>
      {showLane && <ControllerLane />}
      {menu && (
        <PopupMenu
          items={menu.items}
          x={menu.x}
          y={menu.y}
          onClose={() => setMenu(null)}
        />
      )}
    </div>
  );
}
