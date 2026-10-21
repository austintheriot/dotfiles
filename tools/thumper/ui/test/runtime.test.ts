import { expect, test } from "bun:test";
import "../src/components/toast-stack";
import type { ToastStack } from "../src/components/toast-stack";
import { LEAVE_MS, TICK_MS } from "../src/model";
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

test("the runtime holds one tick subscription however many ticks pass", async () => {
  const { services, runtime } = await boot(true);
  for (let i = 0; i < 200; i++) services.advance(TICK_MS);
  expect(services.liveTimers()).toBe(1);
  runtime.stop();
  expect(services.timerCancels()).toBe(1);
  expect(services.liveTimers()).toBe(0);
});

test("native hover from the shell reaches the model", async () => {
  const { services, runtime } = await boot(true);
  services.pushHover(true);
  expect(runtime.current().hovered).toBe(true);
  runtime.stop();
});

function withFakeResizeObserver<Result>(body: (resize: () => void) => Promise<Result>): Promise<Result> {
  const original = globalThis.ResizeObserver;
  const callbacks: Array<() => void> = [];
  globalThis.ResizeObserver = class {
    public constructor(callback: () => void) {
      callbacks.push(callback);
    }
    public observe(): void {}
    public unobserve(): void {}
    public disconnect(): void {}
  } as unknown as typeof ResizeObserver;
  return body(() => callbacks.forEach((callback) => callback())).finally(() => {
    globalThis.ResizeObserver = original;
  });
}

test("the stack reports its measured height once per change", async () => {
  await withFakeResizeObserver(async (resize) => {
    const { services, root, runtime } = await boot(true);
    let height = 212.4;
    root.getBoundingClientRect = () => ({ height }) as DOMRect;
    resize();
    resize();
    height = 300;
    resize();
    const layouts = services.sent.filter((message) => message.type === "layout");
    expect(layouts).toEqual([
      { type: "layout", content_height_px: 212 },
      { type: "layout", content_height_px: 300 },
    ]);
    runtime.stop();
  });
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
