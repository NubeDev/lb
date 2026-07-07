// Pure unit tests for the Phase-2 frame→value bridge + result-shape detector (viz chart-types scope).
// These prove the two load-bearing invariants of the single-stat family: reduceOptions is the EXPLICIT
// frame→value collapse (never a fabricated value), and the picker's shape detector is conservative.

import { describe, it, expect } from "vitest";
import { reduceFrame, reduceFrameValues, frameCategories } from "./reduce";
import { detectShape, viewsForShape, viewFitsShape } from "./shape";

describe("reduceOptions frame→value bridge", () => {
  it("collapses a multi-series frame to the single value a stat/gauge draws (default lastNotNull)", () => {
    const rows = [{ value: 1 }, { value: 2 }, { value: 3 }];
    expect(reduceFrame(rows, { calcs: [] })).toBe(3); // lastNotNull
    expect(reduceFrame(rows, { calcs: ["mean"] })).toBe(2);
    expect(reduceFrame(rows, { calcs: ["max"] })).toBe(3);
    expect(reduceFrame(rows, { calcs: ["min"] })).toBe(1);
    expect(reduceFrame(rows, { calcs: ["sum"] })).toBe(6);
  });

  it("returns null for an empty/non-numeric frame — never a fabricated 0", () => {
    expect(reduceFrame([], { calcs: ["mean"] })).toBeNull();
    expect(reduceFrame([{ value: "n/a" }], { calcs: ["mean"] })).toBeNull();
  });

  it("values:true yields every value, capped by limit (the bargauge per-value path)", () => {
    const rows = [{ value: 10 }, { value: 20 }, { value: 30 }];
    expect(reduceFrameValues(rows, { calcs: [], values: true })).toEqual([10, 20, 30]);
    expect(reduceFrameValues(rows, { calcs: [], values: true, limit: 2 })).toEqual([10, 20]);
    // Without values:true it reduces to the single value.
    expect(reduceFrameValues(rows, { calcs: ["mean"] })).toEqual([20]);
  });

  it("frameCategories derives one {name,value} per row (label column or index)", () => {
    expect(frameCategories([{ name: "a", value: 1 }, { name: "b", value: 2 }], { calcs: [] })).toEqual([
      { name: "a", value: 1 },
      { name: "b", value: 2 },
    ]);
    // No label column → the row index is the honest name.
    expect(frameCategories([{ value: 5 }, { value: 6 }], { calcs: [] })).toEqual([
      { name: "0", value: 5 },
      { name: "1", value: 6 },
    ]);
  });

  it("SQL frames (no value/payload column): the FIRST numeric column charts; the string column names", () => {
    // The federation.query shape — a summary row has no `value` column. The deterministic contract:
    // first numeric column = the point; first string column = the category name.
    const rows = [
      { point_id: "meter-001-kwh", reading_count: 2880, avg_value: 4.87 },
      { point_id: "meter-002-kwh", reading_count: 2880, avg_value: 1.1 },
    ];
    expect(frameCategories(rows, { calcs: [] })).toEqual([
      { name: "meter-001-kwh", value: 2880 },
      { name: "meter-002-kwh", value: 2880 },
    ]);
    expect(reduceFrame(rows, { calcs: ["last"] })).toBe(2880);
    // Numeric-looking STRINGS never chart (an id/timestamp column must not become a point) — a row
    // with only string columns yields no categories, never a fabricated 0.
    expect(frameCategories([{ id: "123", when: "2026-06-05T10:45:00+00:00" }], { calcs: [] })).toEqual([]);
    // A series row with a NULL payload stays null — the key exists, so the column scan never kicks in
    // (seq/ts must not silently become the point).
    expect(reduceFrame([{ series: "cpu", seq: 7, payload: null, ts: 99 }], { calcs: ["last"] })).toBe(null);
  });
});

describe("result-shape ↔ type validation", () => {
  it("a single numeric row is scalar → stat/gauge offered, table too, NOT timeseries", () => {
    expect(detectShape([{ value: 42 }])).toBe("scalar");
    expect(viewFitsShape("stat", "scalar")).toBe(true);
    expect(viewFitsShape("gauge", "scalar")).toBe(true);
    expect(viewFitsShape("timeseries", "scalar")).toBe(false);
  });

  it("many reducible rows are a series → timeseries/barchart AND the single-stat family (via reduce)", () => {
    expect(detectShape([{ value: 1 }, { value: 2 }, { value: 3 }])).toBe("series");
    expect(viewFitsShape("timeseries", "series")).toBe(true);
    expect(viewFitsShape("stat", "series")).toBe(true);
  });

  it("multi-column tabular rows are a table → gauge NOT offered (no honest single number)", () => {
    expect(detectShape([{ city: "NYC", pop: 8 }, { city: "LA", pop: 4 }])).toBe("table");
    expect(viewsForShape("table")).toEqual(["table"]);
    expect(viewFitsShape("gauge", "table")).toBe(false);
    expect(viewFitsShape("table", "table")).toBe(true);
  });

  it("no data is unknown → the picker stays permissive (every standard view allowed)", () => {
    expect(detectShape([])).toBe("unknown");
    expect(viewFitsShape("gauge", "unknown")).toBe(true);
  });
});
