import { expect, test } from "bun:test";
import { createErrorReporter, ERROR_INTERVAL_MS } from "../src/error-reporter";
import { empty } from "./fixtures/view-models";
import { recordingServices } from "./recording-services";

const harness = () => {
  const services = recordingServices({ view_model: empty, native_hover: true });
  return { services, report: createErrorReporter(services) };
};

test("an error is sent to the shell as an error message", () => {
  const { services, report } = harness();
  report("boom", "at a:1");
  expect(services.sent).toEqual([{ type: "error", message: "boom", stack: "at a:1" }]);
});

test("a flood of errors sends one per interval and drops the rest", () => {
  const { services, report } = harness();
  report("first", null);
  report("second", null);
  report("third", null);
  expect(services.sent).toEqual([{ type: "error", message: "first", stack: null }]);
  services.advance(ERROR_INTERVAL_MS - 1);
  report("still too soon", null);
  expect(services.sent.length).toBe(1);
  services.advance(1);
  report("after the interval", null);
  expect(services.sent.length).toBe(2);
  expect(services.sent[1]).toEqual({ type: "error", message: "after the interval", stack: null });
});
