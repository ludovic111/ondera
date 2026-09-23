import { useSyncExternalStore } from "react";
import { commands, type Monitor, type Track } from "@ondera/core";
import { useDispatch, useStore } from "../../state/session";
import { Button } from "./Button";
import { MonitorSmallIcon } from "./Icons";

const NEXT: Record<Monitor, Monitor> = { off: "auto", auto: "on", on: "off" };
const TITLES: Record<Monitor, string> = {
  off: "Input monitoring off · click for Auto",
  auto: "Input monitoring Auto: heard while armed, until the track plays its own clip · click for On",
  on: "Input monitoring On: always heard · click for Off",
};

/** Beside Arm on audio tracks: off, auto (shows A), on. Sunk while it monitors. */
export function MonitorButton({ track }: { track: Track }) {
  const dispatch = useDispatch();
  const store = useStore();
  const blocked =
    useSyncExternalStore(store.subscribeMeta, store.getUi).monitorBlocked ===
    true;
  const monitor = track.monitor ?? "off";
  return (
    <Button
      size="sm"
      title={
        blocked && monitor !== "off"
          ? `${TITLES[monitor]} · muted: built-in speakers would feed back`
          : TITLES[monitor]
      }
      pressed={monitor !== "off"}
      onClick={() =>
        dispatch(
          commands.track.setMonitor({
            trackId: track.id,
            monitor: NEXT[monitor],
          }),
        )
      }
    >
      {monitor === "auto" ? "A" : <MonitorSmallIcon />}
    </Button>
  );
}
