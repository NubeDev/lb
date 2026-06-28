import { describe, expect, it } from "vitest";

import {
  DEFAULT_CHANNEL,
  defaultDashboardSearch,
  validateChannelSearch,
  validateDashboardSearch,
  varsFromSearch,
  withVar,
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

describe("dashboard variable + refresh URL round-trip (widget-config-vars Slices 2/4)", () => {
  it("parses ?var-host=web01&var-host=web02&refresh=30s (multi repeats + refresh)", () => {
    const parsed = validateDashboardSearch({
      from: "2026-01-01",
      to: "2026-03-31",
      "var-host": ["web01", "web02"],
      refresh: "30s",
    });
    expect(parsed.refresh).toBe("30s");
    expect(varsFromSearch(parsed)).toEqual({ host: ["web01", "web02"] });
  });

  it("a single var param is a string, a repeated one is an array", () => {
    expect(varsFromSearch(validateDashboardSearch({ from: "2026-01-01", to: "2026-01-02", "var-env": "prod" }))).toEqual({
      env: "prod",
    });
  });

  it("malformed degrades to defaults (unknown refresh dropped, bad var ignored) — never throws", () => {
    const parsed = validateDashboardSearch({
      from: "2026-01-01",
      to: "2026-01-02",
      refresh: "13s", // not an allowed option → dropped (off)
      "var-x": 42 as unknown as string, // non-string/array → ignored
    });
    expect(parsed.refresh).toBeUndefined();
    expect(varsFromSearch(parsed)).toEqual({});
  });

  it("withVar sets, clears (empty/[]), and round-trips through varsFromSearch", () => {
    const base = validateDashboardSearch({ from: "2026-01-01", to: "2026-01-02" });
    const set = withVar(base, "host", ["web01", "web02"]);
    expect(varsFromSearch(set)).toEqual({ host: ["web01", "web02"] });
    const cleared = withVar(set, "host", []);
    expect(varsFromSearch(cleared)).toEqual({});
    const single = withVar(base, "env", "prod");
    expect(single["var-env"]).toBe("prod");
  });
});
