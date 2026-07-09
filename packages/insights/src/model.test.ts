// Model helpers are pure — assert the tone mapping, the severity floor ordering, and the time-ago
// formatter against a fixed `now` (no wall-clock leak).

import { describe, expect, it } from "vitest";

import {
  SEVERITY_ORDER,
  originLine,
  severityRank,
  severityTone,
  statusTone,
  timeAgo,
} from "./model";

describe("severity", () => {
  it("orders info < warning < critical", () => {
    expect(SEVERITY_ORDER).toEqual(["info", "warning", "critical"]);
    expect(severityRank("info")).toBeLessThan(severityRank("warning"));
    expect(severityRank("warning")).toBeLessThan(severityRank("critical"));
  });

  it("maps to a tone key", () => {
    expect(severityTone("critical")).toBe("destructive");
    expect(severityTone("warning")).toBe("warning");
    expect(severityTone("info")).toBe("accent-2");
  });
});

describe("status", () => {
  it("maps to a tone key", () => {
    expect(statusTone("open")).toBe("default");
    expect(statusTone("acked")).toBe("warning");
    expect(statusTone("resolved")).toBe("success");
  });
});

describe("timeAgo", () => {
  const now = 1_000_000_000_000;
  it("formats seconds / minutes / hours / days", () => {
    expect(timeAgo(now - 5_000, now)).toBe("5s ago");
    expect(timeAgo(now - 2 * 60_000, now)).toBe("2m ago");
    expect(timeAgo(now - (2 * 60 + 30) * 1000, now)).toBe("2m 30s ago");
    expect(timeAgo(now - 3 * 3_600_000, now)).toBe("3h ago");
    expect(timeAgo(now - 2 * 86_400_000, now)).toBe("2d ago");
  });
});

describe("originLine", () => {
  it("joins kind:ref and appends the run when present", () => {
    expect(originLine({ kind: "rule", ref: "cpu-hot" })).toBe("rule:cpu-hot");
    expect(originLine({ kind: "flow", ref: "f1", run: "r9" })).toBe("flow:f1 · run:r9");
  });
});
