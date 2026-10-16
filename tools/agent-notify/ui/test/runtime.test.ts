import { expect, test } from "bun:test";
import "../src/components/toast-stack";
import type { ToastStack } from "../src/components/toast-stack";
import { LEAVE_MS } from "../src/model";
import { start } from "../src/runtime";
import { revisionNine, revisionTen } from "./fixtures/view-models";
import { recordingServices } from "./recording-services";

async function boot(nativeHover: boolean) {
  const services = recordingServices({ view_model: revisionTen, native_hover: nativeHover });
  const root = document.createElement("toast-stack") as ToastStack;
  document.body.replaceChildren(root);
  const runtime = await start(root, services);
  return { services, root, runtime };
}

test("the runtime renders the bootstrap view", async () => {
  const { root, runtime } = await boot(true);
  await root.updateComplete;
  expect(root.model?.view?.revision).toBe(10);
  runtime.stop();
});

test("a stale push does not replace the view", async () => {
  const { services, runtime } = await boot(true);
  services.pushViewModel(revisionNine);
  expect(runtime.current().view?.revision).toBe(10);
  runtime.stop();
});

test("native hover from the shell reaches the model", async () => {
  const { services, runtime } = await boot(true);
  services.pushHover(true);
  expect(runtime.current().hovered).toBe(true);
  runtime.stop();
});

test("a DOM leave is reported after the delay, through the timer service", async () => {
  const { services, runtime } = await boot(false);
  const hoverMessages = () => services.sent.filter((message) => message.type === "hover");
  runtime.dispatch({ type: "pointer", inside: true });
  runtime.dispatch({ type: "pointer", inside: false });
  expect(hoverMessages()).toEqual([{ type: "hover", hovered: true }]);
  services.advance(LEAVE_MS);
  expect(hoverMessages()).toEqual([
    { type: "hover", hovered: true },
    { type: "hover", hovered: false },
  ]);
  runtime.stop();
});
