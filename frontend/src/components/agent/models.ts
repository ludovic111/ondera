import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import type { Provider } from "./connection";

export interface ModelInfo {
  id: string;
  name: string;
  efforts: string[];
}
export interface ModelGroup {
  provider: Provider;
  label: string;
  models: ModelInfo[];
  error: string | null;
}
export const effortName = (value: string) =>
  ({
    xhigh: "Extra high",
    ultra: "Ultra",
    none: "Off",
    minimal: "Minimal",
    low: "Low",
    medium: "Medium",
    high: "High",
    max: "Max",
  })[value] ?? value;
export function useModels() {
  const [groups, setGroups] = useState<ModelGroup[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const request = useRef(0);
  const refresh = useCallback(async () => {
    const id = ++request.current;
    setLoading(true);
    setError("");
    try {
      const data = await invoke<ModelGroup[]>("daw_agent_models");
      if (id === request.current) setGroups(data);
    } catch {
      if (id === request.current)
        setError(
          "Could not load your models. Check your connections and retry.",
        );
    } finally {
      if (id === request.current) setLoading(false);
    }
  }, []);
  useEffect(
    () => () => {
      request.current++;
    },
    [],
  );
  return { groups, loading, error, refresh };
}
