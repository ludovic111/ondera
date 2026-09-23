import { useCallback, useEffect, useRef, useState } from "react";
import { useStore } from "../../state/session";
import styles from "./AgentPanel.module.css";
interface Takes {
  active: string | null;
  takes: { id: string; name: string; tracks: number; clips: number }[];
}
export function TakePanel({
  busy,
  onVariation,
}: {
  busy: boolean;
  onVariation: () => void;
}) {
  const store = useStore();
  const [data, setData] = useState<Takes>({ active: null, takes: [] });
  const [name, setName] = useState("");
  const [working, setWorking] = useState(false);
  const [error, setError] = useState("");
  const locked = useRef(false);
  const refresh = useCallback(async () => {
    try {
      setData((await store.request("take.list")) as Takes);
    } catch (e) {
      setError(String(e));
    }
  }, [store]);
  useEffect(() => {
    if (!busy) void refresh();
  }, [busy, refresh]);
  const run = async (action: () => Promise<unknown>) => {
    if (busy || locked.current) return;
    locked.current = true;
    setWorking(true);
    setError("");
    try {
      await action();
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      locked.current = false;
      setWorking(false);
    }
  };
  return (
    <section className={styles.takes} aria-label="Creative takes">
      <h2>Try another direction</h2>
      <p>
        Keep the original. Explore a new take, then switch between versions to
        compare. Switching stops playback and can be undone.
      </p>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void run(async () => {
            if (!data.takes.length)
              await store.request("take.create", { name: "Original" });
            await store.request("take.create", {
              name:
                name.trim() || `Variation ${Math.max(1, data.takes.length)}`,
            });
            setName("");
            onVariation();
          });
        }}
      >
        <label>
          New take
          <input
            aria-label="New take name"
            placeholder="Warmer chorus, half-time groove…"
            value={name}
            maxLength={120}
            onChange={(e) => setName(e.target.value)}
            disabled={busy || working}
          />
        </label>
        <button
          className="m-button aero-primary"
          disabled={busy || working || data.takes.length >= 8}
        >
          Create & explore
        </button>
      </form>
      {error && <p role="alert">{error}</p>}
      <div className={styles.takeList}>
        {data.takes.map((take) => (
          <div key={take.id}>
            <button
              className="m-button"
              aria-pressed={data.active === take.id}
              disabled={busy || working || data.active === take.id}
              onClick={() =>
                void run(() => store.request("take.select", { id: take.id }))
              }
            >
              <strong>{take.name}</strong>
              <span>
                {data.active === take.id
                  ? "Current version"
                  : `${take.tracks} tracks · ${take.clips} regions`}
              </span>
            </button>
            {data.active !== take.id && (
              <button
                aria-label={`Remove ${take.name}`}
                title="Remove this saved take (undoable)"
                disabled={busy || working}
                onClick={() =>
                  void run(() => store.request("take.remove", { id: take.id }))
                }
              >
                ×
              </button>
            )}
          </div>
        ))}
      </div>
      <p>
        Up to eight takes. Save your project to keep them all, including plugin
        settings and audio. Edits stay with the active take.
      </p>
      <button
        className="m-button"
        disabled={busy || working}
        onClick={() => store.fire("transport.play")}
      >
        Listen to current take
      </button>
    </section>
  );
}
