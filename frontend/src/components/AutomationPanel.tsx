import {
  useEffect,
  useState,
  useSyncExternalStore,
  type PointerEvent,
} from "react";
import { Modal } from "./Dialogs";
import { useStore } from "../state/session";
import { native, type AutomationLane } from "../state/native";
interface Lane extends AutomationLane {
  min: number;
  max: number;
  points: { id: string; beat: number; value: number }[];
}
export function AutomationPanel({ onClose }: { onClose: () => void }) {
  const store = useStore();
  const session = store.getState();
  const document = useSyncExternalStore(
    store.subscribeMeta,
    () => store.document,
  );
  const [lanes, setLanes] = useState<Lane[]>([]);
  const [selected, setSelected] = useState("");
  const [target, setTarget] = useState("trackVolume");
  const [drag, setDrag] = useState<{
    id: string;
    beat: number;
    value: number;
  } | null>(null);
  const refresh = () =>
    native<{ lanes: Lane[] }>("automation.list")
      .then((r) => setLanes(r.lanes))
      .catch(store.reportError);
  useEffect(() => {
    void refresh();
  }, [document]);
  const lane = lanes.find((l) => l.id === selected) ?? lanes[0];
  const end = Math.max(
    32,
    ...session.clips.map(
      (c) =>
        ((c.startBar + c.lengthBars) *
          session.transport.timeSignature.numerator *
          4) /
        session.transport.timeSignature.denominator,
    ),
  );
  const point = (e: PointerEvent<SVGSVGElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    return {
      beat: Math.max(0, Math.min(end, ((e.clientX - r.left) / r.width) * end)),
      value:
        lane.min +
        (1 - Math.max(0, Math.min(1, (e.clientY - r.top) / r.height))) *
          (lane.max - lane.min),
    };
  };
  const points = lane?.points.map((p) => (drag?.id === p.id ? drag : p)) ?? [];
  return (
    <Modal title="Automation" onClose={onClose}>
      <div className="plugin-toolbar">
        <select
          aria-label="Automation lane"
          value={lane?.id ?? ""}
          onChange={(e) => setSelected(e.target.value)}
        >
          {lanes.map((l) => (
            <option key={l.id} value={l.id}>
              {l.name}
            </option>
          ))}
        </select>
        <select
          aria-label="New automation target"
          value={target}
          onChange={(e) => setTarget(e.target.value)}
        >
          <option value="trackVolume">Track volume</option>
          <option value="trackPan">Track pan</option>
          <option value="masterVolume">Master volume</option>
        </select>
        <button
          onClick={() => {
            void store
              .run<{ id: string }>("automation.create", {
                target,
                ...(target === "masterVolume"
                  ? {}
                  : { trackId: session.view.selectedTrackId }),
              })
              .then(refresh)
              .catch(store.reportError);
          }}
        >
          Add lane
        </button>
      </div>
      {lane ? (
        <>
          <div className="plugin-toolbar">
            <label>
              <input
                type="checkbox"
                checked={lane.enabled}
                onChange={(e) => {
                  void store
                    .run("automation.setEnabled", {
                      laneId: lane.id,
                      enabled: e.target.checked,
                    })
                    .then(refresh)
                    .catch(store.reportError);
                }}
              />
              Read
            </label>
            <select
              aria-label="Interpolation"
              value={lane.interpolation}
              onChange={(e) => {
                void store
                  .run("automation.setInterpolation", {
                    laneId: lane.id,
                    interpolation: e.target.value,
                  })
                  .then(refresh)
                  .catch(store.reportError);
              }}
            >
              <option value="linear">Linear</option>
              <option value="step">Step</option>
            </select>
            <button
              onClick={() => {
                void store
                  .run("automation.remove", { laneId: lane.id })
                  .then(refresh)
                  .catch(store.reportError);
              }}
            >
              Remove lane
            </button>
          </div>
          <svg
            className="automation-graph"
            viewBox="0 0 800 200"
            preserveAspectRatio="none"
            onDoubleClick={(e) => {
              const r = e.currentTarget.getBoundingClientRect();
              void store
                .run("automation.setPoint", {
                  laneId: lane.id,
                  beat: Math.max(0, ((e.clientX - r.left) / r.width) * end),
                  value:
                    lane.min +
                    (1 - (e.clientY - r.top) / r.height) *
                      (lane.max - lane.min),
                })
                .then(refresh)
                .catch(store.reportError);
            }}
            onPointerMove={(e) => {
              if (drag) setDrag({ ...drag, ...point(e) });
            }}
            onPointerUp={() => {
              if (drag) {
                void store
                  .run("automation.setPoint", {
                    laneId: lane.id,
                    pointId: drag.id,
                    beat: drag.beat,
                    value: drag.value,
                  })
                  .then(refresh)
                  .catch(store.reportError);
                setDrag(null);
              }
            }}
            onPointerCancel={() => setDrag(null)}
          >
            <polyline
              fill="none"
              stroke="var(--color-accent)"
              strokeWidth="2"
              points={points
                .flatMap((p, i) =>
                  lane.interpolation === "step" && i > 0
                    ? [{ ...p, value: points[i - 1].value }, p]
                    : [p],
                )
                .map(
                  (p) =>
                    `${(p.beat / end) * 800},${200 - ((p.value - lane.min) / (lane.max - lane.min)) * 200}`,
                )
                .join(" ")}
            />
            {points.map((p) => (
              <circle
                key={p.id}
                cx={(p.beat / end) * 800}
                cy={200 - ((p.value - lane.min) / (lane.max - lane.min)) * 200}
                r="5"
                fill="var(--color-indicator)"
                onPointerDown={(e) => {
                  e.preventDefault();
                  e.currentTarget.ownerSVGElement?.setPointerCapture(
                    e.pointerId,
                  );
                  setDrag(p);
                }}
                onContextMenu={(e) => {
                  e.preventDefault();
                  void store
                    .run("automation.removePoint", {
                      laneId: lane.id,
                      pointId: p.id,
                    })
                    .then(refresh)
                    .catch(store.reportError);
                }}
              />
            ))}
          </svg>
          <p>
            Double-click to add a point. Drag to move. Right-click to delete.
          </p>
          <div className="automation-points">
            {lane.points.map((p) => (
              <div key={p.id}>
                <label>
                  Beat
                  <input
                    type="number"
                    min={0}
                    step="any"
                    defaultValue={p.beat}
                    key={`b${p.beat}`}
                    onBlur={(e) => {
                      void store
                        .run("automation.setPoint", {
                          laneId: lane.id,
                          pointId: p.id,
                          beat: Number(e.target.value),
                          value: p.value,
                        })
                        .then(refresh)
                        .catch(store.reportError);
                    }}
                  />
                </label>
                <label>
                  Value
                  <input
                    type="number"
                    min={lane.min}
                    max={lane.max}
                    step="any"
                    defaultValue={p.value}
                    key={`v${p.value}`}
                    onBlur={(e) => {
                      void store
                        .run("automation.setPoint", {
                          laneId: lane.id,
                          pointId: p.id,
                          beat: p.beat,
                          value: Number(e.target.value),
                        })
                        .then(refresh)
                        .catch(store.reportError);
                    }}
                  />
                </label>
              </div>
            ))}
          </div>
        </>
      ) : (
        <p>
          Select a track and add an automation lane. Plugin parameters can be
          automated from their parameter window.
        </p>
      )}
    </Modal>
  );
}
