import { expect, test } from "bun:test";
import { LEAVE_MS, closeVisible, initialModel, update, type Cmd, type Model, type Msg } from "../src/model";
import { mixed, revisionNine, revisionTen } from "./fixtures/view-models";

function run(model: Model, ...messages: Msg[]): [Model, Cmd[]] {
  return messages.reduce<[Model, Cmd[]]>(
    ([current, commands], message) => {
      const [next, more] = update(current, message);
      return [next, [...commands, ...more]];
    },
    [model, []],
  );
}

const booted = (nativeHover: boolean): Model =>
  update(initialModel(0), { type: "booted", bootstrap: { view_model: revisionTen, native_hover: nativeHover } })[0];

test("an_older_revision_is_dropped", () => {
  const [model] = run(booted(true), { type: "view_model", view: revisionNine });
  expect(model.view).toEqual(revisionTen);
});

test("a newer revision replaces the view", () => {
  const [model] = run(booted(true), { type: "view_model", view: { ...mixed, revision: 11 } });
  expect(model.view?.revision).toBe(11);
});

test("a push before the bootstrap answer is kept when it is newer", () => {
  const [early] = update(initialModel(0), { type: "view_model", view: { ...mixed, revision: 12 } });
  const [model] = update(early, { type: "booted", bootstrap: { view_model: revisionTen, native_hover: true } });
  expect(model.view?.revision).toBe(12);
  expect(model.nativeHover).toBe(true);
});

test("clicks and closes become messages for the shell", () => {
  const cardId = mixed.cards[0]?.card_id ?? "";
  const [, commands] = run(booted(true), { type: "clicked", cardId }, { type: "closed", cardId });
  expect(commands).toEqual([
    { type: "send", message: { type: "clicked", card_id: cardId } },
    { type: "send", message: { type: "closed", card_id: cardId } },
  ]);
});

test("native hover shows the close buttons and ignores DOM pointer events", () => {
  const [hovered, commands] = run(booted(true), { type: "native_hover", hovered: true }, { type: "pointer", inside: false });
  expect(closeVisible(hovered)).toBe(true);
  expect(commands).toEqual([]);
});

test("DOM hover reports enter at once and leave after a delay", () => {
  const [entered, enterCommands] = run(booted(false), { type: "pointer", inside: true });
  expect(enterCommands).toEqual([{ type: "cancel_leave" }, { type: "send", message: { type: "hover", hovered: true } }]);
  const [leaving, leaveCommands] = run(entered, { type: "pointer", inside: false });
  expect(leaveCommands).toEqual([{ type: "schedule_leave", delayMs: LEAVE_MS }]);
  expect(leaving.hovered).toBe(true);
  const [left, leftCommands] = run(leaving, { type: "left" });
  expect(left.hovered).toBe(false);
  expect(leftCommands).toEqual([{ type: "send", message: { type: "hover", hovered: false } }]);
});

test("re-entering before the delay cancels the leave", () => {
  const [model, commands] = run(booted(false), { type: "pointer", inside: true }, { type: "pointer", inside: false }, { type: "pointer", inside: true });
  expect(model.hovered).toBe(true);
  expect(commands.at(-1)).toEqual({ type: "cancel_leave" });
});

test("layout is reported once per height", () => {
  const [, commands] = run(booted(true), { type: "layout", heightPx: 212.4 }, { type: "layout", heightPx: 212 }, { type: "layout", heightPx: 300 });
  expect(commands).toEqual([
    { type: "send", message: { type: "layout", content_height_px: 212 } },
    { type: "send", message: { type: "layout", content_height_px: 300 } },
  ]);
});

test("a non-finite height is not reported", () => {
  const [, commands] = run(booted(true), { type: "layout", heightPx: Number.NaN }, { type: "layout", heightPx: Number.POSITIVE_INFINITY });
  expect(commands).toEqual([]);
});

test("ticks move the clock", () => {
  const [model] = run(booted(true), { type: "tick", now: 99 });
  expect(model.now).toBe(99);
});
