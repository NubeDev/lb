import { describe, it, expect } from "vitest";
import { cronNextRuns } from "./CronPanel";

const BASE = Date.UTC(2026, 0, 1, 0, 0, 30); // fixed instant (ms)

describe("cronNextRuns", () => {
  it("rejects malformed expressions", () => {
    expect(cronNextRuns("* * * *", BASE, 3)).toBeNull(); // 4 fields
    expect(cronNextRuns("nope", BASE, 3)).toBeNull();
    expect(cronNextRuns("99 * * * *", BASE, 3)).toBeNull(); // out of range
  });

  it("every minute → consecutive ascending minutes after `from`", () => {
    const r = cronNextRuns("* * * * *", BASE, 5)!;
    expect(r).toHaveLength(5);
    expect(r[0]).toBeGreaterThan(BASE);
    for (let i = 1; i < r.length; i++) expect(r[i] - r[i - 1]).toBe(60_000);
  });

  it("step fields fire on the interval", () => {
    const r = cronNextRuns("*/15 * * * *", BASE, 4)!;
    for (let i = 1; i < r.length; i++) expect(r[i] - r[i - 1]).toBe(15 * 60_000);
  });
});
