// Unit tests for the per-datasource run-history fold (localStorage-backed; jsdom provides a real
// localStorage — nothing platform-side is faked, there is nothing platform-side here).

import { beforeEach, describe, expect, it } from "vitest";

import { HISTORY_CAP, loadHistory, recordRun } from "./runHistory";

beforeEach(() => localStorage.clear());

describe("runHistory", () => {
  it("records runs most-recent first and persists across loads", () => {
    recordRun("acme", "timescale", "SELECT 1", 100);
    recordRun("acme", "timescale", "SELECT 2", 200);
    expect(loadHistory("acme", "timescale").map((e) => e.sql)).toEqual(["SELECT 2", "SELECT 1"]);
  });

  it("dedupes by exact SQL — a re-run moves to the front with the fresh ts", () => {
    recordRun("acme", "ts", "SELECT 1", 100);
    recordRun("acme", "ts", "SELECT 2", 200);
    const after = recordRun("acme", "ts", "SELECT 1", 300);
    expect(after.map((e) => e.sql)).toEqual(["SELECT 1", "SELECT 2"]);
    expect(after[0].ts).toBe(300);
    expect(after).toHaveLength(2);
  });

  it("caps at 10 unique entries (oldest drops)", () => {
    for (let i = 0; i < 15; i++) recordRun("acme", "ts", `SELECT ${i}`, i);
    const list = loadHistory("acme", "ts");
    expect(list).toHaveLength(HISTORY_CAP);
    expect(list[0].sql).toBe("SELECT 14");
    expect(list[9].sql).toBe("SELECT 5");
  });

  it("is keyed by (workspace, source) — no bleed", () => {
    recordRun("acme", "a", "SELECT a", 1);
    recordRun("acme", "b", "SELECT b", 1);
    recordRun("other", "a", "SELECT o", 1);
    expect(loadHistory("acme", "a").map((e) => e.sql)).toEqual(["SELECT a"]);
    expect(loadHistory("acme", "b").map((e) => e.sql)).toEqual(["SELECT b"]);
    expect(loadHistory("other", "a").map((e) => e.sql)).toEqual(["SELECT o"]);
  });

  it("ignores blank SQL and survives corrupt storage", () => {
    expect(recordRun("acme", "ts", "   ", 1)).toEqual([]);
    localStorage.setItem("lb.query-history.acme.ts", "{not json");
    expect(loadHistory("acme", "ts")).toEqual([]);
    localStorage.setItem("lb.query-history.acme.ts", JSON.stringify([{ bad: true }, { sql: "SELECT 1", ts: 5 }]));
    expect(loadHistory("acme", "ts")).toEqual([{ sql: "SELECT 1", ts: 5 }]);
  });
});
