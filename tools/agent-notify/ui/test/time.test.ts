import { expect, test } from "bun:test";
import { relativeTime } from "../src/time";

const base = 1_760_500_000_000;

test("recent cards read as now", () => {
  expect(relativeTime(base, base)).toBe("now");
  expect(relativeTime(base, base + 44_000)).toBe("now");
});

test("minutes, hours and days", () => {
  expect(relativeTime(base, base + 45_000)).toBe("1m");
  expect(relativeTime(base, base + 59 * 60_000)).toBe("59m");
  expect(relativeTime(base, base + 60 * 60_000)).toBe("1h");
  expect(relativeTime(base, base + 47 * 3_600_000)).toBe("1d");
});

test("a clock behind the card reads as now", () => {
  expect(relativeTime(base, base - 5_000)).toBe("now");
});
