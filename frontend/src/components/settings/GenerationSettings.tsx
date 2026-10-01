import { useEffect, useState } from "react";
import { native, type Params } from "../../state/native";
import { useStore } from "../../state/session";
import styles from "./GenerationSettings.module.css";

type ServiceId = "elevenlabs" | "stability" | "fal" | "custom";
const SERVICES: {
  id: ServiceId;
  name: string;
  blurb: string;
  keyPath: string;
}[] = [
  {
    id: "elevenlabs",
    name: "ElevenLabs",
    blurb:
      "Songs and loops with Eleven Music; one-shots and effects up to 30 s.",
    keyPath: "elevenlabsApiKey",
  },
  {
    id: "stability",
    name: "Stable Audio",
    blurb: "Stability AI's model: loops, songs up to 3 min and sound design.",
    keyPath: "stabilityApiKey",
  },
  {
    id: "fal",
    name: "fal.ai",
    blurb: "Any of fal's audio models: Stable Audio, Lyria, ACE-Step…",
    keyPath: "falApiKey",
  },
  {
    id: "custom",
    name: "Your endpoint",
    blurb: "Any HTTP service that follows ryolune's simple contract.",
    keyPath: "customApiKey",
  },
];

/**
 * Settings > Generation: the service `generate.audio` calls and its key. Keys are saved only
 * when the person saves, never shown back (the settings mask them), and an agent may not
 * change any of this.
 */
export function GenerationSettings({
  settings,
  refresh,
}: {
  settings: Params;
  refresh: () => Promise<void>;
}) {
  const store = useStore();
  const [service, setService] = useState<ServiceId>(
    (String(settings.service ?? "elevenlabs") as ServiceId) || "elevenlabs",
  );
  const [key, setKey] = useState("");
  const [falModel, setFalModel] = useState(String(settings.falModel ?? ""));
  const [customUrl, setCustomUrl] = useState(String(settings.customUrl ?? ""));
  const [ready, setReady] = useState<Record<string, boolean>>({});
  const [notice, setNotice] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const info = SERVICES.find((s) => s.id === service) ?? SERVICES[0]!;
  const hasKey = Boolean(settings[info.keyPath]);
  const check = () =>
    native<{ services: { id: string; ready: boolean }[] }>("generate.services")
      .then((value) =>
        setReady(
          Object.fromEntries(
            (Array.isArray(value?.services) ? value.services : []).map((s) => [
              s.id,
              s.ready,
            ]),
          ),
        ),
      )
      .catch(() => {});
  useEffect(() => {
    void check();
  }, []);
  const save = async (entries: [string, unknown][]) => {
    setBusy(true);
    setError("");
    setNotice("");
    try {
      for (const [path, value] of entries)
        await store.request("settings.set", {
          path: `generation.${path}`,
          value,
        });
      await refresh();
      await check();
      setKey("");
      setNotice("Saved.");
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  };
  return (
    <section className={styles.generation} aria-label="Sound generation">
      <div>
        <h2>Make sounds from a description</h2>
        <p>
          Loops, songs, one-shots and playable instruments, from the Generate
          tab of the agent panel or by asking the agent. Connect a service once;
          you pay it directly and ryolune takes nothing.
        </p>
      </div>
      <div className={styles.cards} role="radiogroup" aria-label="Service">
        {SERVICES.map((s) => (
          <button
            key={s.id}
            type="button"
            role="radio"
            aria-checked={service === s.id}
            className={styles.card}
            onClick={() => {
              setService(s.id);
              setKey("");
              setNotice("");
            }}
          >
            <span className={styles.name}>
              {s.name}
              {ready[s.id] && <span className={styles.ready}>Connected</span>}
            </span>
            <span className={styles.blurb}>{s.blurb}</span>
          </button>
        ))}
      </div>
      <form
        className={styles.form}
        onSubmit={(event) => {
          event.preventDefault();
          void save([
            ["service", service],
            ...(key.trim()
              ? [[info.keyPath, key.trim()] as [string, unknown]]
              : []),
            ...(service === "fal"
              ? [["falModel", falModel.trim()] as [string, unknown]]
              : []),
            ...(service === "custom"
              ? [["customUrl", customUrl.trim()] as [string, unknown]]
              : []),
          ]);
        }}
      >
        <fieldset disabled={busy}>
          {service === "custom" && (
            <label>
              <span>Endpoint address</span>
              <input
                type="url"
                required
                value={customUrl}
                onChange={(e) => setCustomUrl(e.target.value)}
                placeholder="https://my-server.example/generate"
                spellCheck={false}
              />
            </label>
          )}
          <label>
            <span>API key{service === "custom" ? " (optional)" : ""}</span>
            <input
              type="password"
              autoComplete="off"
              spellCheck={false}
              value={key}
              onChange={(e) => setKey(e.target.value)}
              placeholder={
                hasKey
                  ? "Key saved · enter a replacement"
                  : "Paste your API key"
              }
            />
          </label>
          {service === "fal" && (
            <label>
              <span>Model</span>
              <input
                value={falModel}
                onChange={(e) => setFalModel(e.target.value)}
                placeholder="fal-ai/stable-audio"
                spellCheck={false}
              />
            </label>
          )}
          <div className={styles.actions}>
            <button type="submit" className="primary">
              {busy ? "Saving…" : "Use this service"}
            </button>
            {hasKey && (
              <button
                type="button"
                onClick={() => void save([[info.keyPath, ""]])}
              >
                Remove saved key
              </button>
            )}
            <button
              type="button"
              className={styles.link}
              onClick={() => store.fire("app.openGuide", { guide: service })}
            >
              {service === "custom"
                ? "How an endpoint answers ↗"
                : "Get a key ↗"}
            </button>
          </div>
          {notice && <p role="status">{notice}</p>}
          {error && <p role="alert">{error}</p>}
        </fieldset>
      </form>
      <p className={styles.privacy}>
        Your description, and for loops the song's tempo and key, go to the
        service you choose. Sounds are kept on this computer until you delete
        them. The agent can generate only while “Generate sounds” is allowed in
        Settings › Agent.
      </p>
    </section>
  );
}
