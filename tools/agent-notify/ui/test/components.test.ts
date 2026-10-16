import { expect, test } from "bun:test";
import "../src/components/toast-stack";
import type { ToastStack } from "../src/components/toast-stack";
import { initialModel, update, type Model, type Msg } from "../src/model";
import { dndOn, mixed } from "./fixtures/view-models";

async function mount(model: Model): Promise<{ stack: ToastStack; messages: Msg[] }> {
  const messages: Msg[] = [];
  const stack = document.createElement("toast-stack") as ToastStack;
  stack.dispatch = (message) => {
    if (message.type !== "layout") messages.push(message);
  };
  stack.model = model;
  document.body.replaceChildren(stack);
  await stack.updateComplete;
  for (const child of stack.renderRoot.querySelectorAll("toast-card, finished-row")) {
    await (child as HTMLElement & { updateComplete: Promise<unknown> }).updateComplete;
  }
  return { stack, messages };
}

const withView = (nativeHover: boolean, hovered = false): Model => {
  const [booted] = update(initialModel(1_760_500_400_000), { type: "booted", bootstrap: { view_model: mixed, native_hover: nativeHover } });
  return { ...booted, hovered };
};

function cards(stack: ToastStack): HTMLElement[] {
  return [...stack.renderRoot.querySelectorAll<HTMLElement>("toast-card")];
}

function inner(element: Element): ShadowRoot {
  const root = element.shadowRoot;
  if (root === null) throw new Error(`${element.tagName} has no shadow root`);
  return root;
}

test("every card renders with its kind and location", async () => {
  const { stack } = await mount(withView(true));
  const rendered = cards(stack);
  expect(rendered.length).toBe(3);
  expect(inner(rendered[0] as Element).querySelector(".card")?.classList.contains("attention")).toBe(true);
  expect(inner(rendered[1] as Element).textContent).toContain("2 · notes/main");
});

test("card text is text, never markup", async () => {
  const { stack } = await mount(withView(true));
  const info = inner(cards(stack)[1] as Element);
  expect(info.textContent).toContain("<b>not bold</b>");
  expect(info.querySelector("b")).toBeNull();
});

test("a note replaces nothing and shows under the body", async () => {
  const { stack } = await mount(withView(true));
  expect(inner(cards(stack)[2] as Element).querySelector(".note")?.textContent).toBe("Session ended");
});

test("a card without a title renders no title row", async () => {
  const { stack } = await mount(withView(true));
  expect(inner(cards(stack)[2] as Element).querySelector(".title")).toBeNull();
  expect(inner(cards(stack)[0] as Element).querySelector(".title")?.textContent).toBe("Fix sync retry");
});

test("clicking a card sends its id; the close button sends close only", async () => {
  const { stack, messages } = await mount(withView(true, true));
  const first = inner(cards(stack)[0] as Element);
  (first.querySelector(".card") as HTMLElement).click();
  (first.querySelector("button.close") as HTMLElement).click();
  const cardId = mixed.cards[0]?.card_id ?? "";
  expect(messages).toEqual([
    { type: "clicked", cardId },
    { type: "closed", cardId },
  ]);
});

test("close buttons are visible only while native hover is on", async () => {
  const { stack } = await mount(withView(true, true));
  expect(cards(stack).every((card) => card.hasAttribute("close-visible"))).toBe(true);
  const { stack: idle } = await mount(withView(true, false));
  expect(cards(idle).some((card) => card.hasAttribute("close-visible"))).toBe(false);
});

test("the finished row lists folded cards as buttons", async () => {
  const { stack, messages } = await mount(withView(true));
  const row = stack.renderRoot.querySelector("finished-row");
  const button = row === null ? null : inner(row).querySelector("button");
  expect(button?.textContent).toBe("6");
  button?.click();
  expect(messages).toEqual([{ type: "clicked", cardId: mixed.finished_row[0]?.card_id ?? "" }]);
});

test("do-not-disturb and an empty view render nothing", async () => {
  const [model] = update(initialModel(0), { type: "booted", bootstrap: { view_model: dndOn, native_hover: true } });
  const { stack } = await mount(model);
  expect(cards(stack).length).toBe(0);
  expect(stack.renderRoot.querySelector("finished-row")).toBeNull();
});
