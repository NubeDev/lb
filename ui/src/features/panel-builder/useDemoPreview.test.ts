// The demo-data preview hook (data-studio-10x scope, phase 3 "Demo data, honestly seeded") — when
// the user's query returns ZERO rows, the empty preview offers "Preview with demo data". This unit
// test covers the state machine the gateway suite can't reach cleanly (driving a real 0-row query
// through the CodeMirror SQL editor under jsdom is the pre-existing getClientRects gap). The hook's
// `available` reads the datasource roster through the shared `datasource.list` cache — we mock ONLY
// that onepure-function seam (`useDatasourceList` — itself a documented fake-LOADER pattern, NOT a fake
// backend; rule 9 / the system-catalog precedent), so the test asserts the hook's logic, not gateway
// behaviour. The real gateway path is proven by `DataStudioBuilderFlow.gateway.test.tsx` (the demo
// badge is OFF when the user query has rows; the demo datasource is real through `addDatasource`).

import { describe, expect, it, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";

import { useDemoPreview, DEMO_DATASOURCE, DEMO_SQL, demoSwappedCell } from "./useDemoPreview";
import type { Cell } from "@/lib/dashboard";

// Mock ONLY `useDatasourceList` (a pure data seam — the list of federation datasources). The hook's
// contract is what matters here; the real `datasource.list` round-trip is exercised by the gateway suite.
vi.mock("./tabs/useDatasourceList", () => ({
  useDatasourceList: () => ({ options: mockOptions, loading: false }),
}));

let mockOptions: { type: string; name?: string; label: string }[] = [];

function renderDemo(state: { hasTarget: boolean; loading: boolean; rowCount: number }) {
  return renderHook(() => useDemoPreview("acme", state));
}

const LOADED = { hasTarget: true, loading: false, rowCount: 0 };

describe("useDemoPreview — the demo-data state machine", () => {
  it("DEMO_DATASOURCE is the seeded 'demo-buildings' name (the make-seed-demo-sqlite convention)", () => {
    expect(DEMO_DATASOURCE).toBe("demo-buildings");
  });

  it("is NOT available when the demo datasource is absent from the workspace roster", () => {
    mockOptions = [{ type: "federation", name: "timescale", label: "timescale (postgres)" }];
    const { result } = renderDemo(LOADED);
    expect(result.current.available).toBe(false);
    expect(result.current.active).toBe(false);
  });

  it("is available when the demo datasource exists, a target is staged, and the query returned 0 rows", () => {
    mockOptions = [
      { type: "federation", name: "timescale", label: "timescale (postgres)" },
      { type: "federation", name: "demo-buildings", label: "demo-buildings (sqlite)" },
    ];
    const { result } = renderDemo(LOADED);
    expect(result.current.available).toBe(true);
    expect(result.current.active).toBe(false);
  });

  it("is NOT available while the query is loading or has rows (the offer is for the zero-row case only)", () => {
    mockOptions = [{ type: "federation", name: "demo-buildings", label: "demo-buildings (sqlite)" }];
    expect(renderDemo({ hasTarget: true, loading: true, rowCount: 0 }).result.current.available).toBe(false);
    expect(renderDemo({ hasTarget: true, loading: false, rowCount: 500 }).result.current.available).toBe(
      false,
    );
    // No target staged → no query to mirror with demo data.
    expect(renderDemo({ hasTarget: false, loading: false, rowCount: 0 }).result.current.available).toBe(
      false,
    );
  });

  it("enable() turns demo mode on; the badge tracks `active`", () => {
    mockOptions = [{ type: "federation", name: "demo-buildings", label: "demo-buildings (sqlite)" }];
    const { result } = renderDemo(LOADED);
    act(() => result.current.enable());
    expect(result.current.active).toBe(true);
    // `available` reflects the data state (zero rows + target + demo source exists) — it stays true
    // while demo is on; the BUILDER UI gates the offer with `!demo.active` (the badge replaces it).
    expect(result.current.available).toBe(true);
  });

  it("AUTO-YIELDS: the moment the user's query has rows, demo mode turns off (an unbadged demo frame is a lie)", () => {
    mockOptions = [{ type: "federation", name: "demo-buildings", label: "demo-buildings (sqlite)" }];
    const { result, rerender } = renderDemo(LOADED);
    act(() => result.current.enable());
    expect(result.current.active).toBe(true);

    // User re-runs and gets rows — demo auto-yields (correctness requirement, not polish).
    rerender();
    const next = renderHook(() =>
      useDemoPreview("acme", { hasTarget: true, loading: false, rowCount: 500 }),
    );
    expect(next.result.current.active).toBe(false);
  });

  it("disable() turns demo mode off (the user dismisses the demo)", () => {
    mockOptions = [{ type: "federation", name: "demo-buildings", label: "demo-buildings (sqlite)" }];
    const { result } = renderDemo(LOADED);
    act(() => result.current.enable());
    expect(result.current.active).toBe(true);
    act(() => result.current.disable());
    expect(result.current.active).toBe(false);
  });
});

describe("demoSwappedCell — the display-only source swap", () => {
  it("rewrites the draft's source to federation.query over the demo dataset; view/options/fieldConfig stay the user's own", () => {
    const draft: Cell = {
      i: "p",
      x: 0,
      y: 0,
      w: 8,
      h: 4,
      v: 3,
      widget_type: "chart",
      view: "timeseries",
      title: "My chart",
      binding: { series: "" },
      sources: [{ refId: "A", tool: "series.read", args: { series: "nonexistent" }, datasource: { type: "series" } }],
      fieldConfig: { defaults: { unit: "celsius" }, overrides: [] },
    };

    const swapped = demoSwappedCell(draft);
    // The PRIMARY source is now the demo dataset, behind the real `federation.query` tool.
    expect(swapped.sources?.[0]?.tool).toBe("federation.query");
    expect(swapped.sources?.[0]?.args).toEqual({ source: "demo-buildings", sql: DEMO_SQL });
    expect((swapped.sources?.[0]?.args as { source: string }).source).toBe(DEMO_DATASOURCE);
    // The user's view, title, and fieldConfig survive — only the DATA binding swaps.
    expect(swapped.view).toBe("timeseries");
    expect(swapped.title).toBe("My chart");
    expect(swapped.fieldConfig?.defaults.unit).toBe("celsius");
  });
});
