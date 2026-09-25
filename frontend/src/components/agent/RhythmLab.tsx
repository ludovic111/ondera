import { useEffect, useRef, useState } from "react";
import { native } from "../../state/native";
import { useSession, useStore } from "../../state/session";
import styles from "./AgentPanel.module.css";
const initial = [
  { name: "Kick", steps: 16, pulses: 4, rotation: 0, pitch: 36, velocity: 112 },
  {
    name: "Snare",
    steps: 16,
    pulses: 2,
    rotation: 4,
    pitch: 38,
    velocity: 100,
  },
  {
    name: "Hi-hat",
    steps: 16,
    pulses: 8,
    rotation: 0,
    pitch: 42,
    velocity: 76,
  },
  {
    name: "Percussion",
    steps: 12,
    pulses: 3,
    rotation: 1,
    pitch: 46,
    velocity: 65,
  },
];
export function RhythmLab({ busy }: { busy: boolean }) {
  const store = useStore();
  const tempo = useSession((s) => s.transport.tempo);
  const meter = useSession((s) => s.transport.timeSignature);
  const [lanes, setLanes] = useState(initial);
  const [bars, setBars] = useState(2);
  const [name, setName] = useState("Euclidean groove");
  const [working, setWorking] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const locked = useRef(false);
  const sound = useRef<HTMLAudioElement | null>(null);
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      sound.current?.pause();
    };
  }, []);
  const params = () => ({
    lanes: lanes.map(({ name: _, ...lane }) => lane),
    bars,
    name,
  });
  const run = async (preview: boolean) => {
    if (busy || locked.current) return;
    locked.current = true;
    setWorking(true);
    setError("");
    setMessage("");
    sound.current?.pause();
    try {
      if (preview) {
        const { wavBase64: wav } = await native<{ wavBase64: string }>(
          "rhythm.preview",
          // A preview creates nothing, so it takes no name (the registry refuses one).
          { lanes: params().lanes, bars, inline: true },
        );
        if (!mounted.current) return;
        const audio = new Audio(`data:audio/wav;base64,${wav}`);
        sound.current = audio;
        audio.onended = () => {
          if (mounted.current && sound.current === audio)
            setMessage(
              "Preview complete. Create the groove to edit its notes.",
            );
        };
        await audio.play();
        setMessage("Previewing through your system audio output.");
      } else {
        const result = await store.request<{
          noteCount: number;
          excludedBySolo: boolean;
        }>("rhythm.create", params());
        setMessage(
          `Created ${result.noteCount} editable notes on a new drum track.${result.excludedBySolo ? " Another track is soloed: disable Solo to hear this groove." : ""}`,
        );
      }
    } catch (e) {
      if (mounted.current) setError(String(e));
    } finally {
      locked.current = false;
      if (mounted.current) setWorking(false);
    }
  };
  return (
    <section className={styles.rhythm} aria-label="Rhythm Lab">
      <h2>Build a drum groove</h2>
      <p>
        Each lane divides a bar into its own steps. Spread the hits, turn the
        pattern and hear the rhythms interlock.
      </p>
      <fieldset disabled={busy || working}>
        <label>
          Groove name
          <input
            value={name}
            maxLength={120}
            onChange={(e) => setName(e.target.value)}
          />
        </label>
        <label>
          Bars
          <select
            value={bars}
            onChange={(e) => setBars(Number(e.target.value))}
          >
            {[1, 2, 4].map((n) => (
              <option key={n}>{n}</option>
            ))}
          </select>
        </label>
        {lanes.map((lane, index) => (
          <div key={lane.name} className={styles.rhythmLane}>
            <strong>{lane.name}</strong>
            <div
              className={styles.grooveSteps}
              aria-label={`${lane.pulses} hits over ${lane.steps} steps`}
            >
              {Array.from({ length: lane.steps }, (_, i) => (
                <span
                  key={i}
                  data-hit={
                    (((i + lane.steps - lane.rotation) % lane.steps) *
                      lane.pulses) %
                      lane.steps <
                    lane.pulses
                  }
                />
              ))}
            </div>
            <div className={styles.laneControls}>
              {(
                [
                  ["steps", "Steps", 1, 32],
                  ["pulses", "Hits", 0, lane.steps],
                  ["rotation", "Rotate", 0, lane.steps - 1],
                  ["velocity", "Velocity", 1, 127],
                ] as const
              ).map(([key, label, min, max]) => (
                <label key={key}>
                  {label}
                  <input
                    type="number"
                    aria-label={`${lane.name} ${label}`}
                    min={min}
                    max={max}
                    value={lane[key]}
                    onChange={(e) => {
                      const value = Number(e.target.value);
                      if (!Number.isInteger(value)) return;
                      setLanes((old) =>
                        old.map((item, i) =>
                          i !== index
                            ? item
                            : {
                                ...item,
                                [key]: Math.min(max, Math.max(min, value)),
                                ...(key === "steps"
                                  ? {
                                      pulses: Math.min(
                                        item.pulses,
                                        Math.max(1, value),
                                      ),
                                      rotation:
                                        item.rotation % Math.max(1, value),
                                    }
                                  : {}),
                              },
                        ),
                      );
                    }}
                  />
                </label>
              ))}
            </div>
          </div>
        ))}
        <div className={styles.welcomeActions}>
          <button
            className="m-button"
            type="button"
            onClick={() => void run(true)}
          >
            ▶ Preview
          </button>
          <button
            className="m-button"
            type="button"
            onClick={() => sound.current?.pause()}
          >
            Stop preview
          </button>
          <button
            className="m-button aero-primary"
            type="button"
            disabled={!name.trim()}
            onClick={() => void run(false)}
          >
            Create MIDI groove
          </button>
        </div>
      </fieldset>
      {working && <p role="status">Preparing the groove…</p>}
      {message && <p role="status">{message}</p>}
      {error && <p role="alert">{error}</p>}
    </section>
  );
}
