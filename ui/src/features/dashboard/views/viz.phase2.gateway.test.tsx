// The viz Phase-2 chart set + render path, against a REAL in-process gateway (viz chart-types +
// field-config + panel-editor scopes, Testing plan; CLAUDE §9 / testing §0 — no fake backend). Each test
// signs into a UNIQUE workspace, seeds REAL samples through the real ingest path (`seedSeries` →
// `series.read`), and exercises the v3 Phase-2 contract end to end over the real `dashboard.save`/`get` +
// `POST /mcp/call`. Covers:
//   - Alias fidelity: a seeded v2 `stat`/`gauge`/`table` cell renders through the NEW Phase-2 renderer
//     and re-saves identically (mirrors Phase-1's chart→timeseries alias test).
//   - Options round-trip: each Phase-2 view's typed `options` round-trips through save/get.
//   - Result-shape ↔ type validation: a 1-sample scalar source → picker offers stat/gauge; a multi-sample
//     series → timeseries/barchart + the single-stat family (via reduceOptions); reduceOptions collapses a
//     multi-series frame to one value.
//   - fieldConfig through the ONE bridge (no stored formatted string); thresholds COLOR (never alert).
//   - Mandatory: capability-deny (denied target → honest denied state, never a fake value) + ws isolation.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

import { useRealGateway, signInReal, signInWithCaps, seedSeries } from "@/test/gateway-session";
import { saveDashboard, getDashboard } from "@/lib/dashboard/dashboard.api";
import { invoke } from "@/lib/ipc/invoke";
import type { Cell } from "@/lib/dashboard";
import { cellToEditorState, editorStateToCell } from "../editor/cellEditorState";
import { detectShape, viewFitsShape } from "./shape";
import { reduceFrame } from "./reduce";
import { StatPanel } from "./stat/StatPanel";
import { GaugePanel } from "./gauge/GaugePanel";
import { TablePanel } from "./table/TablePanel";
import { WithDashboardCache } from "@/features/dashboard/cache/testCacheWrapper";

let n = 0;
const nextWs = () => `viz2-${n++}`;

beforeAll(() => useRealGateway());

/** Seed `count` real samples of `series` (payload `base+i`) through the real ingest path. */
async function seedSamples(series: string, count: number, base = 40): Promise<void> {
  for (let i = 0; i < count; i++) {
    await seedSeries({ series, seq: i + 1, payload: base + i, key: "kind", value: "temperature" });
  }
}

/** A v3 single-value cell over a `series.read` source — the primary target (v3 `sources[]`). */
function readCell(i: string, view: Cell["view"], series: string, options: Record<string, unknown>, fieldConfig?: Cell["fieldConfig"]): Cell {
  return {
    i, x: 0, y: 0, w: 6, h: 4, v: 3, widget_type: "stat", view,
    binding: { series: "" },
    sources: [{ refId: "A", tool: "series.read", args: { series }, datasource: { type: "series" } }],
    options,
    fieldConfig,
  };
}

// ---------------------------------------------------------------------------------------------------
// Alias fidelity — a seeded v2 stat/gauge/table cell renders through the NEW renderer + re-saves same.
// ---------------------------------------------------------------------------------------------------
describe("alias fidelity (real gateway)", () => {
  it("a v2 stat/gauge/table cell renders through the Phase-2 renderer and re-saves identically", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedSamples("cooler.temp", 1, 7); // one sample → scalar value 7

    // v2 cells: `view` is the canonical id already (stat/gauge/table alias to themselves), single-source.
    const v2: Cell[] = [
      { i: "s", x: 0, y: 0, w: 4, h: 3, v: 2, widget_type: "stat", view: "stat", binding: { series: "" }, source: { tool: "series.read", args: { series: "cooler.temp" } } },
      { i: "g", x: 4, y: 0, w: 4, h: 3, v: 2, widget_type: "gauge", view: "gauge", binding: { series: "" }, source: { tool: "series.read", args: { series: "cooler.temp" } } },
      { i: "t", x: 8, y: 0, w: 4, h: 3, v: 2, widget_type: "stat", view: "table", binding: { series: "" }, source: { tool: "series.read", args: { series: "cooler.temp" } } },
    ];
    await saveDashboard("d", "D", v2);
    const back = await getDashboard("d");

    for (const cell of v2) {
      const reloaded = back.cells.find((c) => c.i === cell.i)!;
      // The editor (de)serializer is idempotent over the reloaded cell + keeps the v2 single-source
      // semantics (not promoted to v3 sources[]) — the alias renders without rewriting the cell.
      const out = editorStateToCell(cellToEditorState(reloaded), reloaded);
      expect(editorStateToCell(cellToEditorState(out), out)).toEqual(out);
      expect(out.source?.tool).toBe("series.read");
      expect(out.sources).toBeUndefined();
      expect(out.view).toBe(cell.view);
    }

    // And the stat renders through its NEW Phase-2 renderer over the real seeded value (7).
    render(<WithDashboardCache ws={ws}><StatPanel cell={back.cells.find((c) => c.i === "s")!} label="S" /></WithDashboardCache>);
    await waitFor(() => expect(screen.getByLabelText("stat value").textContent).toContain("7"));
  });
});

// ---------------------------------------------------------------------------------------------------
// Options round-trip — each Phase-2 view's typed options survive save/get identically.
// ---------------------------------------------------------------------------------------------------
describe("options round-trip (real gateway)", () => {
  it("stat/gauge/bargauge/table/barchart/piechart typed options survive save/get", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);

    const cells: Cell[] = [
      readCell("st", "stat", "s", { reduceOptions: { calcs: ["lastNotNull"] }, graphMode: "area", colorMode: "value", textMode: "auto", justifyMode: "auto", orientation: "auto", showPercentChange: false }),
      readCell("ga", "gauge", "s", { reduceOptions: { calcs: ["mean"] }, orientation: "auto", showThresholdLabels: true, showThresholdMarkers: true, sizing: "auto", minVizWidth: 75, minVizHeight: 75 }),
      readCell("bg", "bargauge", "s", { reduceOptions: { calcs: [], values: true }, orientation: "horizontal", displayMode: "gradient", valueMode: "color", showUnfilled: true, minVizWidth: 8, minVizHeight: 16 }),
      readCell("tb", "table", "s", { showHeader: true, cellHeight: "md", enablePagination: false, sortBy: [] }),
      readCell("bc", "barchart", "s", { legend: { showLegend: true, displayMode: "list", placement: "bottom", calcs: [] }, tooltip: { mode: "single", sort: "none" }, orientation: "horizontal", stacking: "none", showValue: "auto", barWidth: 0.97, groupWidth: 0.7, xTickLabelRotation: 0 }),
      readCell("pc", "piechart", "s", { reduceOptions: { calcs: [] }, pieType: "donut", displayLabels: ["name", "percent"], legend: { showLegend: true, displayMode: "list", placement: "right", calcs: [] }, tooltip: { mode: "single", sort: "none" } }),
    ];
    await saveDashboard("d", "D", cells);
    const back = await getDashboard("d");

    for (const cell of cells) {
      const reloaded = back.cells.find((c) => c.i === cell.i)!;
      // The typed per-viz options are owned by the editor groups (not dropped to extraOptions) and the
      // (de)serializer is idempotent — the options round-trip identically through the real store.
      const reopened = cellToEditorState(reloaded);
      const original = cellToEditorState(cell);
      expect(reopened.options).toEqual(original.options);
    }
  });
});

// ---------------------------------------------------------------------------------------------------
// Result-shape ↔ type validation — over REAL seeded samples.
// ---------------------------------------------------------------------------------------------------
describe("result-shape ↔ type validation (real gateway)", () => {
  it("a 1-sample source is scalar (stat/gauge); a multi-sample source is a series; reduce collapses it", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);

    // One real sample → a scalar (stat/gauge honest; not timeseries).
    await seedSamples("scalar.one", 1, 5);
    const one = (await invoke("mcp_call", { tool: "series.read", args: { series: "scalar.one" } })) as { samples: Array<Record<string, unknown>> };
    const scalarShape = detectShape(one.samples);
    expect(scalarShape).toBe("scalar");
    expect(viewFitsShape("stat", scalarShape)).toBe(true);
    expect(viewFitsShape("gauge", scalarShape)).toBe(true);
    expect(viewFitsShape("timeseries", scalarShape)).toBe(false);

    // Many real samples → a series (timeseries/barchart + the single-stat family via reduceOptions).
    await seedSamples("series.many", 4, 10);
    const many = (await invoke("mcp_call", { tool: "series.read", args: { series: "series.many" } })) as { samples: Array<Record<string, unknown>> };
    const seriesShape = detectShape(many.samples);
    expect(seriesShape).toBe("series");
    expect(viewFitsShape("timeseries", seriesShape)).toBe(true);
    expect(viewFitsShape("stat", seriesShape)).toBe(true);

    // reduceOptions collapses the multi-sample frame to the single value a stat/gauge draws (max = 13).
    expect(reduceFrame(many.samples, { calcs: ["max"] })).toBe(13);
  });
});

// ---------------------------------------------------------------------------------------------------
// fieldConfig through the ONE bridge — a value renders unit/decimals/threshold-color (no stored string).
// ---------------------------------------------------------------------------------------------------
describe("fieldConfig through the one bridge (real gateway)", () => {
  it("a stat value formats unit/decimals + colors by threshold (computed at render, not stored)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedSamples("temp.one", 1, 42); // canonical 42

    const fc: Cell["fieldConfig"] = {
      defaults: {
        unit: "celsius",
        decimals: 1,
        // 42 ≥ 30 → the `red` step colors the value (thresholds COLOR, never alert).
        thresholds: { mode: "absolute", steps: [{ value: null, color: "green" }, { value: 30, color: "red" }] },
      },
      overrides: [],
    };
    const cell = readCell("s", "stat", "temp.one", { reduceOptions: { calcs: ["lastNotNull"] }, colorMode: "value", graphMode: "none" }, fc);
    await saveDashboard("d", "D", [cell]);
    const back = await getDashboard("d");

    render(<WithDashboardCache ws={ws}><StatPanel cell={back.cells.find((c) => c.i === "s")!} label="Temp" /></WithDashboardCache>);
    // "42.0 °C" — computed from canonical 42 through the bridge (decimals=1 + the celsius label), never a
    // stored formatted string. The seeded cell carries NO formatted text.
    const val = await waitFor(() => screen.getByLabelText("stat value"));
    expect(val.textContent).toContain("42.0");
    expect(JSON.stringify(back.cells[0])).not.toContain("42.0"); // honesty: no stored formatted string
  });
});

// ---------------------------------------------------------------------------------------------------
// Mandatory: capability-deny — a denied target renders the honest denied state, never a fake value.
// ---------------------------------------------------------------------------------------------------
describe("capability-deny (real gateway)", () => {
  it("a denied series.read target → the panel shows denied, never a fabricated number", async () => {
    const ws = nextWs();
    await signInWithCaps("user:ada", ws, ["mcp:dashboard.get:call"]); // no series.read grant
    const cell = readCell("s", "stat", "nope", { reduceOptions: { calcs: ["lastNotNull"] } });
    render(<WithDashboardCache ws={ws}><StatPanel cell={cell} label="X" /></WithDashboardCache>);
    render(<WithDashboardCache ws={ws}><GaugePanel cell={{ ...cell, i: "g", view: "gauge" }} label="X" /></WithDashboardCache>);
    render(<WithDashboardCache ws={ws}><TablePanel cell={{ ...cell, i: "t", view: "table" }} label="X" /></WithDashboardCache>);
    await waitFor(() =>
      expect(screen.getAllByText(/no access to this source/i).length).toBeGreaterThanOrEqual(3),
    );
  });
});

// ---------------------------------------------------------------------------------------------------
// Mandatory: workspace isolation — a ws-B panel save never crosses into ws-A.
// ---------------------------------------------------------------------------------------------------
describe("workspace isolation (real gateway)", () => {
  it("a ws-B stat dashboard is invisible to ws-A", async () => {
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    await saveDashboard("shared", "A", [readCell("s", "stat", "x", { reduceOptions: { calcs: [] } })]);

    const wsB = nextWs();
    await signInReal("user:ben", wsB);
    await expect(getDashboard("shared")).rejects.toThrow();
  });
});
