// The cell ↔ editorState round-trip — the contract that makes add≡edit impossible to break (viz
// panel-editor scope, Testing plan: "editorStateToCell(cellToEditorState(c)) ≡ c for v1/v2/v3"). If
// this passes, EDIT reconstructs exactly what was saved and ADD (a default cell) serializes losslessly
// — the user's "edit loses my SQL options" bug cannot recur silently. Pure, no gateway.

import { describe, it, expect } from "vitest";

import type { Cell } from "@/lib/dashboard";
import { cellToEditorState, editorStateToCell } from "./cellEditorState";
import { defaultCell } from "./defaultCell";
import { emptySqlSource } from "../builder/sql/SqlQueryEditor";

/** The identity the editor relies on: serialize(deserialize(c)) preserves the cell. */
function roundTrip(c: Cell): Cell {
  return editorStateToCell(cellToEditorState(c), c);
}

describe("cell ↔ editorState round-trip", () => {
  it("a v1 series cell round-trips byte-identical (no v2/v3 fields injected)", () => {
    const v1: Cell = {
      i: "w1",
      x: 0,
      y: 0,
      w: 4,
      h: 3,
      widget_type: "chart",
      binding: { series: "cooler.temp" },
    };
    const back = roundTrip(v1);
    expect(back).toEqual(v1);
    expect(back.sources).toBeUndefined();
    expect(back.fieldConfig).toBeUndefined();
    expect(back.v).toBeUndefined();
  });

  it("a v2 chart + store.query cell round-trips (single source stays single source, not promoted)", () => {
    const v2: Cell = {
      i: "w2",
      x: 1,
      y: 2,
      w: 6,
      h: 4,
      v: 2,
      widget_type: "chart",
      view: "chart",
      title: "Temps",
      binding: { series: "" },
      source: { tool: "store.query", args: { sql: "SELECT value FROM reading" } },
      options: { sql: { ...emptySqlSource(), rawSql: "SELECT value FROM reading" } },
    };
    const back = roundTrip(v2);
    expect(back).toEqual(v2);
    // The v2 cell stays single-`source` — NOT promoted to `sources[]` (byte-identical round-trip).
    expect(back.sources).toBeUndefined();
    expect(back.source?.tool).toBe("store.query");
    // The SQL builder state survives in options.sql (the exact thing edit used to drop).
    expect((back.options?.sql as { rawSql: string }).rawSql).toBe("SELECT value FROM reading");
  });

  it("a full v3 timeseries cell round-trips (sources[]/fieldConfig/transformations/overrides)", () => {
    const v3: Cell = {
      i: "p1",
      x: 0,
      y: 0,
      w: 8,
      h: 4,
      v: 3,
      widget_type: "chart",
      view: "timeseries",
      title: "Cooler °C",
      description: "desc",
      binding: { series: "" },
      sources: [{ refId: "A", tool: "store.query", args: { sql: "SELECT v FROM r" }, datasource: { type: "surreal" } }],
      transformations: [{ id: "reduce", options: { reducers: ["last"] } }],
      options: { legend: { showLegend: true, displayMode: "table", placement: "bottom", calcs: ["mean"] }, tooltip: { mode: "single", sort: "none" } },
      fieldConfig: {
        defaults: { unit: "celsius", decimals: 1, min: 0, max: 50, thresholds: { mode: "absolute", steps: [{ value: null, color: "green" }, { value: 5, color: "red" }] } },
        overrides: [{ matcher: { id: "byName", options: "value" }, properties: [{ id: "custom.lineWidth", value: 3 }] }],
      },
      pluginVersion: "lb-viz@1",
    };
    const back = roundTrip(v3);
    expect(back).toEqual(v3);
  });

  it("full Phase-2 cells (stat/gauge/bargauge/table/barchart/piechart) round-trip with typed options", () => {
    // Each carries a fully-populated typed `options` for its view — proving the Phase-2 option keys are
    // owned by the editor (typed groups), not dropped into `extraOptions`, and round-trip identically.
    const base = (i: string, view: string, options: Record<string, unknown>): Cell => ({
      i,
      x: 0,
      y: 0,
      w: 6,
      h: 4,
      v: 3,
      widget_type: "stat",
      view: view as Cell["view"],
      binding: { series: "" },
      sources: [{ refId: "A", tool: "store.query", args: { sql: "SELECT value FROM r" }, datasource: { type: "surreal" } }],
      fieldConfig: { defaults: { unit: "celsius", decimals: 1, thresholds: { mode: "absolute", steps: [{ value: null, color: "green" }, { value: 30, color: "red" }] } }, overrides: [] },
      options,
    });
    const cells: Cell[] = [
      base("s1", "stat", { reduceOptions: { calcs: ["lastNotNull"] }, orientation: "auto", graphMode: "area", colorMode: "value", justifyMode: "auto", textMode: "auto", showPercentChange: false }),
      base("g1", "gauge", { reduceOptions: { calcs: ["mean"] }, orientation: "auto", showThresholdLabels: true, showThresholdMarkers: true, sizing: "auto", minVizWidth: 75, minVizHeight: 75 }),
      base("bg1", "bargauge", { reduceOptions: { calcs: [], values: true, limit: 5 }, orientation: "horizontal", displayMode: "gradient", valueMode: "color", showUnfilled: true, minVizWidth: 8, minVizHeight: 16 }),
      base("t1", "table", { showHeader: true, cellHeight: "md", enablePagination: false, sortBy: [{ displayName: "value", desc: true }] }),
      base("bc1", "barchart", { legend: { showLegend: true, displayMode: "list", placement: "bottom", calcs: [] }, tooltip: { mode: "single", sort: "none" }, orientation: "horizontal", stacking: "none", showValue: "auto", barWidth: 0.97, groupWidth: 0.7, xTickLabelRotation: 0 }),
      base("p1", "piechart", { reduceOptions: { calcs: [] }, pieType: "donut", displayLabels: ["name", "percent"], legend: { showLegend: true, displayMode: "list", placement: "right", calcs: [] }, tooltip: { mode: "single", sort: "none" } }),
    ];
    for (const c of cells) {
      const back = roundTrip(c);
      expect(back).toEqual(c);
      // The typed option keys round-tripped under `options`, not lost.
      expect(back.options).toEqual(c.options);
    }
  });

  it("a fresh default cell (ADD) serializes losslessly through the editor", () => {
    const fresh = defaultCell("timeseries", "w3");
    const back = roundTrip(fresh);
    expect(back).toEqual(fresh);
    // ADD seeds the full v3 surface: a target, default options, an empty field-config.
    expect(back.view).toBe("timeseries");
    expect(back.sources?.length).toBe(1);
    expect(back.fieldConfig).toEqual({ defaults: {}, overrides: [] });
  });

  it("the cell key + geometry is preserved on serialize (the edit invariant)", () => {
    const base = defaultCell("timeseries", "keep-me", { x: 3, y: 7, w: 5, h: 2 });
    const state = cellToEditorState(base);
    const out = editorStateToCell({ ...state, title: "changed" }, base);
    expect(out.i).toBe("keep-me");
    expect([out.x, out.y, out.w, out.h]).toEqual([3, 7, 5, 2]);
    expect(out.title).toBe("changed");
  });
});
