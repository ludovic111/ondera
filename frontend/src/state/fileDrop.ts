import { useEffect, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useStore } from "./session";

/**
 * Files dragged from the file manager onto the window. The host decodes them;
 * the webview only forwards the paths Tauri reports. Returns true while a drag hovers.
 */
export function useFileDrop(): boolean {
  const store = useStore();
  const [hovering, setHovering] = useState(false);
  useEffect(() => {
    let off: (() => void) | undefined;
    let cancelled = false;
    try {
      void getCurrentWebview()
        .onDragDropEvent(({ payload }) => {
          if (payload.type === "enter" || payload.type === "over")
            setHovering(true);
          else setHovering(false);
          if (payload.type === "drop" && payload.paths.length)
            store.fire("web.file", {
              action: "importPaths",
              paths: payload.paths,
            });
        })
        .then((unlisten) => {
          if (cancelled) unlisten();
          else off = unlisten;
        })
        .catch(() => {});
    } catch {
      // Outside Tauri (tests, plain browser) there is no webview to listen on.
    }
    return () => {
      cancelled = true;
      off?.();
    };
  }, [store]);
  return hovering;
}
