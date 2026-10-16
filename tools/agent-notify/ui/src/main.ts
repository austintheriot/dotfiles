import "./components/toast-stack";
import type { ToastStack } from "./components/toast-stack";
import { createErrorReporter } from "./error-reporter";
import { start } from "./runtime";
import { tauriServices } from "./tauri-services";

const services = tauriServices();
const report = createErrorReporter(services);

window.addEventListener("error", (event) => report(String(event.message), event.error instanceof Error ? (event.error.stack ?? null) : null));
window.addEventListener("unhandledrejection", (event) => {
  const reason: unknown = event.reason;
  report(reason instanceof Error ? reason.message : String(reason), reason instanceof Error ? (reason.stack ?? null) : null);
});

const root = document.createElement("toast-stack") as ToastStack;
document.body.append(root);
start(root, services).catch((error: unknown) => report(`start failed: ${String(error)}`, null));
