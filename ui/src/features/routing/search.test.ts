import { describe, expect, it } from "vitest";

import {
  DEFAULT_CHANNEL,
  defaultDashboardSearch,
  validateChannelSearch,
  validateDashboardSearch,
} from "./search";

describe("route search validation", () => {
  it("defaults the channel when c is missing or empty", () => {
    expect(validateChannelSearch({})).toEqual({ c: DEFAULT_CHANNEL });
    expect(validateChannelSearch({ c: "  " })).toEqual({ c: DEFAULT_CHANNEL });
  });

  it("keeps a valid dashboard range", () => {
    expect(validateDashboardSearch({ from: "2026-01-01", to: "2026-03-31" })).toEqual({
      from: "2026-01-01",
      to: "2026-03-31",
    });
  });

  it("degrades malformed dashboard dates to defaults", () => {
    const today = new Date("2026-06-27T10:00:00.000Z");
    expect(defaultDashboardSearch(today)).toEqual({ from: "2026-05-28", to: "2026-06-27" });

    const parsed = validateDashboardSearch({ from: "garbage", to: "2026-03-31" });
    expect(parsed.from).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    expect(parsed.to).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    expect(parsed.from <= parsed.to).toBe(true);
  });

  it("defaults an inverted dashboard range", () => {
    const parsed = validateDashboardSearch({ from: "2026-03-31", to: "2026-01-01" });
    expect(parsed.from <= parsed.to).toBe(true);
  });
});
