import { useCallback, useEffect, useRef, useState } from "react";
import { native } from "../../state/native";
import { useStore } from "../../state/session";
import styles from "./GeneratePanel.module.css";

type Kind = "loop" | "song" | "sound" | "instrument";
interface Service {
  id: string;
  label: string;
  ready: boolean;
  makes: string;
}
interface Sound {
  id: string;
  name: string;
  description: string;
  kind: Kind;
  service: string;
  seconds: number;
  created: number;
}

const KINDS: { id: Kind; label: string; hint: string; placeholder: string }[] =
  [
    {
      id: "loop",
      label: "Loop",
      hint: "Loops on your bars, at the song's tempo and key.",
      placeholder: "Dusty boom-bap drums with a lazy swing…",
    },
    {
      id: "song",
      label: "Song",
      hint: "A full idea to build on, placed at the playhead.",
      placeholder: "Dreamy synthwave with a driving bass and warm pads…",
    },
    {
      id: "sound",
      label: "Sound",
      hint: "A one-shot or effect: a riser, an impact, a foley hit.",
      placeholder: "Deep cinematic impact with a long metallic tail…",
    },
    {
      id: "instrument",
      label: "Instrument",
      hint: "One note, played across your keyboard by Sample Keys.",
      placeholder: "Felt piano, soft and intimate, close-miked…",
    },
  ];
const LENGTHS: Record<Exclude<Kind, "loop">, number[]> = {
  song: [30, 60, 120, 180],
  sound: [1, 3, 6, 12],
  instrument: [2, 3, 5],
};

const duration = (seconds: number) =>
  seconds >= 60
    ? `${Math.floor(seconds / 60)}:${String(Math.round(seconds % 60)).padStart(2, "0")}`
    : `${Math.round(seconds * 10) / 10} s`;

const ago = (created: number) => {
  const minutes = Math.round((Date.now() / 1000 - created) / 60);
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.round(minutes / 60);
  return hours < 24 ? `${hours} h ago` : `${Math.round(hours / 24)} d ago`;
};

/**
 * Make sounds from a description with the service in Settings > Generation, and turn them
 * into clips or instruments. Everything here is `generate.*`, so the agent, the CLI and MCP
 * clients do the same.
 */
export function GeneratePanel({ busy }: { busy: boolean }) {
  const store = useStore();
  const [services, setServices] = useState<Service[] | null>(null);
  const [chosen, setChosen] = useState("");
  const [kind, setKind] = useState<Kind>("loop");
  const [prompt, setPrompt] = useState("");
  const [bars, setBars] = useState(4);
  const [seconds, setSeconds] = useState<Record<string, number>>({
    song: 60,
    sound: 3,
    instrument: 3,
  });
  const [instrumental, setInstrumental] = useState(true);
  const [follow, setFollow] = useState(true);
  const [working, setWorking] = useState(false);
  const [started, setStarted] = useState(0);
  const [now, setNow] = useState(0);
  const [error, setError] = useState("");
  const [sounds, setSounds] = useState<Sound[]>([]);
  const [playing, setPlaying] = useState("");
  const audio = useRef<HTMLAudioElement | null>(null);

  const refresh = useCallback(async () => {
    const list = await native<{ sounds: Sound[] }>("generate.list", {
      limit: 30,
    }).catch(() => ({ sounds: [] as Sound[] }));
    setSounds(Array.isArray(list?.sounds) ? list.sounds : []);
  }, []);
  useEffect(() => {
    native<{ chosen: string; services: Service[] }>("generate.services")
      .then((value) => {
        setServices(Array.isArray(value?.services) ? value.services : []);
        setChosen(String(value?.chosen ?? ""));
      })
      .catch((reason) => setError(String(reason)));
    void refresh();
    return () => audio.current?.pause();
  }, [refresh]);
  useEffect(() => {
    if (!working) return;
    const timer = window.setInterval(() => setNow(Date.now()), 500);
    return () => window.clearInterval(timer);
  }, [working]);

  const ready = services?.filter((s) => s.ready) ?? [];
  const service = ready.find((s) => s.id === chosen) ?? ready[0] ?? null;
  const info = KINDS.find((k) => k.id === kind)!;

  const generate = async () => {
    if (!service || !prompt.trim() || working) return;
    setWorking(true);
    setStarted(Date.now());
    setNow(Date.now());
    setError("");
    try {
      await store.request("generate.audio", {
        prompt: prompt.trim(),
        kind,
        service: service.id,
        ...(kind === "loop" ? { bars } : { seconds: seconds[kind] ?? 3 }),
        ...(kind === "song" ? { instrumental } : {}),
        ...(kind === "loop" || kind === "song" ? { followSong: follow } : {}),
      });
      await refresh();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setWorking(false);
    }
  };
  const act = async (method: string, params: Record<string, unknown>) => {
    setError("");
    try {
      await store.request(method, params);
      await refresh();
    } catch (reason) {
      setError(String(reason));
    }
  };
  const preview = async (sound: Sound) => {
    audio.current?.pause();
    if (playing === sound.id) {
      setPlaying("");
      return;
    }
    try {
      const clip = await native<{ mime: string; base64: string }>(
        "generate.preview",
        { id: sound.id },
      );
      const player = new Audio(`data:${clip.mime};base64,${clip.base64}`);
      player.onended = () => setPlaying("");
      audio.current = player;
      setPlaying(sound.id);
      await player.play();
    } catch (reason) {
      setPlaying("");
      setError(String(reason));
    }
  };

  return (
    <section className={styles.generate} aria-label="Generate">
      <header>
        <h2>Make a sound</h2>
        <p>
          Describe it in your own words. ryolune asks your sound service and
          puts the result straight into the song.
        </p>
      </header>
      {services && ready.length === 0 && (
        <div className={styles.connect}>
          <strong>Connect a sound service</strong>
          <p>
            ElevenLabs, Stable Audio, fal.ai or your own endpoint. Add a key
            once in Settings; you pay the service directly, ryolune takes
            nothing.
          </p>
          <button
            type="button"
            className="primary-action m-button"
            onClick={() =>
              store.fire("ui.showPanel", {
                panel: "settings",
                section: "generation",
              })
            }
          >
            Set up generation
          </button>
        </div>
      )}
      <div className={styles.kinds} role="radiogroup" aria-label="What to make">
        {KINDS.map((k) => (
          <button
            key={k.id}
            type="button"
            role="radio"
            aria-checked={kind === k.id}
            className={kind === k.id ? styles.on : ""}
            onClick={() => setKind(k.id)}
          >
            {k.label}
          </button>
        ))}
      </div>
      <p className={styles.hint}>{info.hint}</p>
      <textarea
        aria-label="Describe the sound"
        placeholder={info.placeholder}
        value={prompt}
        maxLength={2000}
        onChange={(e) => setPrompt(e.target.value)}
        onKeyDown={(e) => {
          if (
            e.key === "Enter" &&
            (e.metaKey || e.ctrlKey) &&
            !e.nativeEvent.isComposing
          ) {
            e.preventDefault();
            void generate();
          }
        }}
      />
      <div className={styles.options}>
        <label>
          <span>Length</span>
          {kind === "loop" ? (
            <select
              value={bars}
              onChange={(e) => setBars(Number(e.target.value))}
            >
              {[1, 2, 4, 8, 16].map((n) => (
                <option key={n} value={n}>
                  {n} {n === 1 ? "bar" : "bars"}
                </option>
              ))}
            </select>
          ) : (
            <select
              value={seconds[kind]}
              onChange={(e) =>
                setSeconds({ ...seconds, [kind]: Number(e.target.value) })
              }
            >
              {LENGTHS[kind].map((n) => (
                <option key={n} value={n}>
                  {duration(n)}
                </option>
              ))}
            </select>
          )}
        </label>
        {ready.length > 1 && (
          <label>
            <span>Service</span>
            <select
              value={service?.id ?? ""}
              onChange={(e) => setChosen(e.target.value)}
            >
              {ready.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.label}
                </option>
              ))}
            </select>
          </label>
        )}
        {(kind === "loop" || kind === "song") && (
          <label className={styles.check}>
            <input
              type="checkbox"
              checked={follow}
              onChange={(e) => setFollow(e.target.checked)}
            />
            <span>Song's tempo and key</span>
          </label>
        )}
        {kind === "song" && (
          <label className={styles.check}>
            <input
              type="checkbox"
              checked={instrumental}
              onChange={(e) => setInstrumental(e.target.checked)}
            />
            <span>Instrumental</span>
          </label>
        )}
      </div>
      <button
        type="button"
        className={`m-button primary-action ${styles.go}`}
        disabled={!service || !prompt.trim() || working || busy}
        onClick={() => void generate()}
      >
        {working
          ? `Making it with ${service?.label ?? "your service"}… ${duration(Math.max(0, (now - started) / 1000))}`
          : `Generate ${info.label.toLowerCase()}`}
      </button>
      {error && (
        <p className={styles.error} role="alert">
          {error}
        </p>
      )}
      {sounds.length > 0 && (
        <div className={styles.library}>
          <h3 className="caps">Your sounds</h3>
          <ul aria-label="Generated sounds">
            {sounds.map((sound) => (
              <li key={sound.id}>
                <button
                  type="button"
                  className={styles.play}
                  aria-label={
                    playing === sound.id
                      ? `Stop ${sound.name}`
                      : `Listen to ${sound.name}`
                  }
                  aria-pressed={playing === sound.id}
                  onClick={() => void preview(sound)}
                >
                  {playing === sound.id ? "■" : "▶"}
                </button>
                <div className={styles.meta} title={sound.description}>
                  <strong>{sound.name}</strong>
                  <span>
                    {KINDS.find((k) => k.id === sound.kind)?.label} ·{" "}
                    {duration(sound.seconds)} · {ago(sound.created)}
                  </span>
                </div>
                <button
                  type="button"
                  title="Put it in the song as an audio clip at the playhead"
                  onClick={() =>
                    void act("generate.place", { id: sound.id, as: "audio" })
                  }
                >
                  Add
                </button>
                <button
                  type="button"
                  title="Play it from the keyboard on a new Sample Keys track"
                  onClick={() =>
                    void act("generate.place", {
                      id: sound.id,
                      as: "instrument",
                    })
                  }
                >
                  Keys
                </button>
                <button
                  type="button"
                  className={styles.remove}
                  aria-label={`Delete ${sound.name}`}
                  title="Delete from this computer (clips in songs keep their audio)"
                  onClick={() => void act("generate.delete", { id: sound.id })}
                >
                  ×
                </button>
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}
