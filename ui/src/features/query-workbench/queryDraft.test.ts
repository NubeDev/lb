// Pure unit tests for the query-draft streaming contract (query-draft-streaming scope): the
// subject convention and the defensive frame parse. The real-transport proof (publish over the
// real bus → the mounted workbench follows) is `QueryDraftFollow.gateway.test.tsx`'s job.

import { describe, expect, it } from "vitest";

import type { SqlSourceState } from "@/lib/panel-kit/sql/query";
import { draftSubject, parseDraftFrame } from "./queryDraft";

describe("draftSubject", () => {
  it("is the per-source workspace-relative subject (the host adds the ws/ext wall)", () => {
    expect(draftSubject("timescale")).toBe("querybuilder/timescale/draft");
    expect(draftSubject("surreal-local")).toBe("querybuilder/surreal-local/draft");
  });
});

describe("parseDraftFrame", () => {
  const full: SqlSourceState = {
    mode: "builder",
    rawSql: 'SELECT * FROM "site"',
    builder: {
      table: "site",
      joins: [{ table: "site_tag", type: "inner", on: [{ leftColumn: "id", rightColumn: "site_id" }] }],
      columns: [{ name: "name", table: "site" }],
      filters: [],
      groupBy: [],
    },
    format: "table",
    builderLayout: { site: { x: 0, y: 40 } },
  };

  it("accepts a full SqlSourceState frame (round-trips builder, layout, format)", () => {
    // JSON round-trip = what actually crosses the bus.
    const parsed = parseDraftFrame(JSON.parse(JSON.stringify(full)));
    expect(parsed).toEqual(full);
  });

  it("accepts a minimal code-mode frame and defaults format to table", () => {
    const parsed = parseDraftFrame({ mode: "code", rawSql: "SELECT 1" });
    expect(parsed).toEqual({ mode: "code", rawSql: "SELECT 1", format: "table" });
  });

  it("drops junk: non-objects, bad mode, missing rawSql, malformed builder", () => {
    expect(parseDraftFrame(null)).toBeNull();
    expect(parseDraftFrame("SELECT 1")).toBeNull();
    expect(parseDraftFrame(42)).toBeNull();
    expect(parseDraftFrame({ mode: "yolo", rawSql: "x" })).toBeNull();
    expect(parseDraftFrame({ mode: "code" })).toBeNull();
    expect(parseDraftFrame({ mode: "builder", rawSql: "", builder: "site" })).toBeNull();
    expect(parseDraftFrame({ mode: "builder", rawSql: "", builder: { table: 1, columns: [], filters: [] } })).toBeNull();
    expect(parseDraftFrame({ mode: "builder", rawSql: "", builder: { table: "t", columns: {}, filters: [] } })).toBeNull();
  });

  it("normalizes an unknown format to table", () => {
    expect(parseDraftFrame({ mode: "code", rawSql: "x", format: "csv" })?.format).toBe("table");
    expect(parseDraftFrame({ mode: "code", rawSql: "x", format: "time-series" })?.format).toBe("time-series");
  });
});
