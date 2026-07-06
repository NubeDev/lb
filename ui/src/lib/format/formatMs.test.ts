// Pure-logic tests for the shared `formatMs` duration formatter. One behaviour per case so a
// regression names the exact boundary that broke.

import { describe, expect, it } from "vitest";

import { formatMs } from "./formatMs";

describe("formatMs", () => {
  it("returns null for absent / non-finite input (the field was missing, not '0')", () => {
    expect(formatMs(undefined)).toBeNull();
    expect(formatMs(null)).toBeNull();
    expect(formatMs(NaN)).toBeNull();
    expect(formatMs(Infinity)).toBeNull();
  });

  it("renders sub-millisecond as '<1 ms' (a '0 ms' reads as a bug, not a duration)", () => {
    expect(formatMs(0)).toBe("<1 ms");
    expect(formatMs(0.4)).toBe("<1 ms");
    expect(formatMs(0.99)).toBe("<1 ms");
  });

  it("renders sub-second durations as rounded milliseconds", () => {
    expect(formatMs(1)).toBe("1 ms");
    expect(formatMs(42)).toBe("42 ms");
    expect(formatMs(42.4)).toBe("42 ms");
    expect(formatMs(42.5)).toBe("43 ms"); // Math.round → half rounds up
    expect(formatMs(999)).toBe("999 ms");
  });

  it("renders ≥1 s with two decimals (enough resolution to spot a 1.05 → 1.20 s regression)", () => {
    expect(formatMs(1000)).toBe("1.00 s");
    expect(formatMs(1050)).toBe("1.05 s");
    expect(formatMs(12345)).toBe("12.35 s");
  });
});
