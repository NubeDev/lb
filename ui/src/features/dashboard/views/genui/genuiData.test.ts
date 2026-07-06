// Pure unit coverage of the genui data-model helpers (no gateway — the data-bound render is proven in
// genuiAuthor.gateway.test.tsx against a real node). Covers the `/data/{refId}` shaping, the v3
// empty-source guard (the known "binding broken" trap), and the all-denied roll-up.
import { describe, it, expect } from "vitest";
import type { Cell } from "@/lib/dashboard";
import { genuiTargets, refDataOf, singleTargetCell, allDenied } from "./genuiData";
import type { SourceState } from "../../builder/useSource";

const baseCell: Cell = {
  i: "g1", x: 0, y: 0, w: 6, h: 4, v: 3, widget_type: "chart",
  view: "genui", binding: { series: "" },
};

describe("genuiTargets", () => {
  it("uses v3 sources[] when present, skipping hidden", () => {
    const cell: Cell = {
      ...baseCell,
      sources: [
        { refId: "A", tool: "series.watch", args: { series: "office/temp" } },
        { refId: "B", tool: "flows.node_state", args: { id: "f1" }, hide: true },
      ],
    };
    expect(genuiTargets(cell).map((t) => t.refId)).toEqual(["A"]);
  });

  it("empty-source v3 trap: an empty placeholder source[] does NOT shadow a real sources[]", () => {
    // The gateway round-trips a v3 cell with an empty `source:{tool:"",args:null}` beside real sources[].
    const cell: Cell = {
      ...baseCell,
      source: { tool: "", args: undefined },
      sources: [{ refId: "A", tool: "series.watch", args: { series: "x" } }],
    };
    expect(genuiTargets(cell)).toEqual([{ refId: "A", tool: "series.watch", args: { series: "x" } }]);
  });

  it("promotes a real v2 single source to refId A", () => {
    const cell: Cell = { ...baseCell, source: { tool: "flows.node_state", args: { id: "f1" } } };
    expect(genuiTargets(cell)).toEqual([{ refId: "A", tool: "flows.node_state", args: { id: "f1" } }]);
  });

  it("returns [] when there is no real source (empty placeholder only)", () => {
    const cell: Cell = { ...baseCell, source: { tool: "", args: undefined } };
    expect(genuiTargets(cell)).toEqual([]);
  });

  it("hidden-only sources[] (a rich_result cell's leash extras) do NOT shadow the real v2 source", () => {
    // ResponseView.buildCell folds extra declared tools as hidden targets beside the envelope's single
    // `source` — the dock genui preview must still resolve refId A from that source (channel-widgets).
    const cell: Cell = {
      ...baseCell,
      source: { tool: "federation.query", args: { source: "db", sql: "SELECT 1" } },
      sources: [{ refId: "T0", tool: "reminder.update", args: undefined, hide: true }],
    };
    expect(genuiTargets(cell)).toEqual([
      { refId: "A", tool: "federation.query", args: { source: "db", sql: "SELECT 1" } },
    ]);
  });
});

describe("refDataOf", () => {
  it("exposes rows/latest/value for a stat binding", () => {
    const state: SourceState = { rows: [{ value: 42 }], latest: 42, loading: false, denied: false };
    expect(refDataOf(state)).toEqual({ rows: [{ value: 42 }], latest: 42, value: 42, loading: false, denied: false });
  });
  it("derives a scalar `value` from a single row when latest is null", () => {
    const state: SourceState = { rows: [{ count: 7 }], latest: null, loading: false, denied: false };
    expect(refDataOf(state).value).toBe(7);
  });
  it("carries the denied flag", () => {
    const state: SourceState = { rows: [], latest: null, loading: false, denied: true };
    expect(refDataOf(state).denied).toBe(true);
  });
});

describe("singleTargetCell", () => {
  it("isolates one target and clears the empty-source placeholder + parent transforms", () => {
    const parent: Cell = {
      ...baseCell,
      sources: [
        { refId: "A", tool: "series.watch", args: { series: "x" } },
        { refId: "B", tool: "flows.node_state", args: { id: "f1" } },
      ],
      transformations: [{ id: "reduce", options: {} }],
    };
    const only = singleTargetCell(parent, parent.sources![1]);
    expect(only.sources).toEqual([{ refId: "B", tool: "flows.node_state", args: { id: "f1" } }]);
    expect(only.source).toEqual({ tool: "", args: undefined });
    expect(only.transformations).toEqual([]);
  });
});

describe("allDenied", () => {
  it("true only when every resolved target is denied", () => {
    expect(allDenied({ data: { A: deny(true), B: deny(true) } }, 2)).toBe(true);
    expect(allDenied({ data: { A: deny(true), B: deny(false) } }, 2)).toBe(false);
    expect(allDenied({ data: {} }, 0)).toBe(false);
  });
});

function deny(d: boolean) {
  return { rows: [], latest: null, value: null, loading: false, denied: d };
}
