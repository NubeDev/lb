// `observedLocal` renders a UTC epoch (seconds) in the viewer's LOCAL timezone. Pure; we pin the
// process TZ per case (vitest runs node, which honors `process.env.TZ` via the Intl/Date stack) to prove
// the same instant renders as different wall-clocks in different zones. One responsibility: the contract.

import { afterEach, describe, expect, it } from "vitest";
import { observedLocal } from "./observedLocal";

const REAL_TZ = process.env.TZ;
afterEach(() => {
  process.env.TZ = REAL_TZ;
});

// 1783598400 = 2026-07-09T12:00:00Z.
const EPOCH = 1783598400;

describe("observedLocal", () => {
  it("renders the UTC instant as UTC wall-clock when the viewer is in UTC", () => {
    process.env.TZ = "UTC";
    expect(observedLocal(EPOCH)).toBe("2026-07-09 12:00");
  });

  it("renders the SAME instant in the viewer's local wall-clock (Da Nang, UTC+7 → 19:00)", () => {
    process.env.TZ = "Asia/Ho_Chi_Minh";
    expect(observedLocal(EPOCH)).toBe("2026-07-09 19:00");
  });

  it("crosses the date boundary for a west-of-UTC viewer (New York, UTC-4 → 08:00)", () => {
    process.env.TZ = "America/New_York";
    expect(observedLocal(EPOCH)).toBe("2026-07-09 08:00");
  });

  it("returns empty for an absent/garbled timestamp (no 'Invalid Date')", () => {
    expect(observedLocal(null)).toBe("");
    expect(observedLocal(NaN)).toBe("");
  });
});
