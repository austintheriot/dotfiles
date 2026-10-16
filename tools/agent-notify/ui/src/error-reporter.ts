import type { UiServices } from "./services";

export const ERROR_INTERVAL_MS = 1_000;

export function createErrorReporter(services: Pick<UiServices, "send" | "now">): (message: string, stack: string | null) => void {
  let lastSentMs: number | null = null;
  return (message, stack) => {
    const nowMs = services.now();
    if (lastSentMs !== null && nowMs - lastSentMs < ERROR_INTERVAL_MS) return;
    lastSentMs = nowMs;
    services.send({ type: "error", message, stack });
  };
}
