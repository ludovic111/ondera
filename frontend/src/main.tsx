import "@fontsource/manrope/400.css";
import "@fontsource/manrope/500.css";
import "@fontsource/manrope/600.css";
import "@fontsource/manrope/700.css";
import "@fontsource/ibm-plex-mono/400.css";
import "@fontsource/ibm-plex-mono/500.css";
import "./theme/global.css";
import "./theme/materials.css";
import "./theme/native.css";
import "./theme/aero.css";
import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { NativeStore, native } from "./state/native";
import { applyTokens } from "./theme/applyTokens";
import { SessionProvider } from "./state/session";
import { App } from "./App";
applyTokens();
const root = createRoot(document.getElementById("root")!);
const store = new NativeStore();
root.render(<div className="startup">Opening Ondera…</div>);
async function start() {
  await listen("daw:capture", async () => {
    try {
      await document.fonts.ready;
      window.dispatchEvent(new Event("ondera:before-capture"));
      if (store.platform === "macos") {
        const png = await invoke<string>("daw_snapshot");
        await native("web.capture", { png });
        return;
      }
      const { toPng } = await import("html-to-image");
      const png = await toPng(document.getElementById("root")!, {
        pixelRatio: window.devicePixelRatio,
        cacheBust: false,
      });
      await native("web.capture", { png: png.split(",")[1] });
    } catch {
      await native("web.captureError");
    }
  });
  await store.connect();
  root.render(
    <SessionProvider store={store}>
      <App />
    </SessionProvider>,
  );
}
void start().catch((error) =>
  root.render(
    <div className="startup" role="alert">
      Ondera could not connect to its audio engine.<pre>{String(error)}</pre>
      <button onClick={() => location.reload()}>Retry</button>
    </div>,
  ),
);
