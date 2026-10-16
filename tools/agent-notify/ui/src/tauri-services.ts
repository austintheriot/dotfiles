import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { HOVER_EVENT, VIEW_MODEL_EVENT } from "./bindings/events";
import type { HoverPayload } from "./bindings/HoverPayload";
import type { PanelViewModel } from "./bindings/PanelViewModel";
import type { UiBootstrap } from "./bindings/UiBootstrap";
import type { UiServices } from "./services";

export function tauriServices(): UiServices {
  return {
    bootstrap: () => invoke<UiBootstrap>("bootstrap"),
    send: (message) => {
      invoke("ui_message", { message }).catch((error: unknown) => console.error("ui_message failed", error));
    },
    onViewModel: (listener) => listen<PanelViewModel>(VIEW_MODEL_EVENT, (event) => listener(event.payload)),
    onHover: (listener) => listen<HoverPayload>(HOVER_EVENT, (event) => listener(event.payload.hovered)),
    now: () => Date.now(),
    setTimer: (callback, delayMs) => {
      const timer = setTimeout(callback, delayMs);
      return () => clearTimeout(timer);
    },
  };
}
