import type { PanelViewModel } from "../src/bindings/PanelViewModel";
import type { UiBootstrap } from "../src/bindings/UiBootstrap";
import type { UiMessage } from "../src/bindings/UiMessage";
import type { UiServices, Unsubscribe } from "../src/services";

export type RecordingServices = UiServices & {
  readonly sent: UiMessage[];
  pushViewModel(view: PanelViewModel): void;
  pushHover(hovered: boolean): void;
  advance(milliseconds: number): void;
};

export function recordingServices(bootstrap: UiBootstrap, startMs = 1_760_500_000_000): RecordingServices {
  const sent: UiMessage[] = [];
  const viewListeners: Array<(view: PanelViewModel) => void> = [];
  const hoverListeners: Array<(hovered: boolean) => void> = [];
  const timers: Array<{ at: number; callback: () => void; live: boolean }> = [];
  let clock = startMs;
  return {
    sent,
    bootstrap: async () => bootstrap,
    send: (message) => {
      sent.push(message);
    },
    onViewModel: async (listener) => {
      viewListeners.push(listener);
      return () => {};
    },
    onHover: async (listener) => {
      hoverListeners.push(listener);
      return () => {};
    },
    now: () => clock,
    setTimer: (callback, delayMs): Unsubscribe => {
      const timer = { at: clock + delayMs, callback, live: true };
      timers.push(timer);
      return () => {
        timer.live = false;
      };
    },
    pushViewModel: (view) => viewListeners.forEach((listener) => listener(view)),
    pushHover: (hovered) => hoverListeners.forEach((listener) => listener(hovered)),
    advance: (milliseconds) => {
      clock += milliseconds;
      for (const timer of timers.filter((entry) => entry.live && entry.at <= clock)) {
        timer.live = false;
        timer.callback();
      }
    },
  };
}
