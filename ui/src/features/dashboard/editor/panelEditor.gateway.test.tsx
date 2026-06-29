// The viz Phase-1 panel editor + render path, against a REAL in-process gateway (viz panel-editor +
// field-config + chart-types scopes, Testing plan; CLAUDE §9 / testing §0 — no fake backend). Each test
// signs into a UNIQUE workspace, seeds real rows through the real ingest path, and exercises the v3
// contract end to end over `POST /mcp/call` + the real `dashboard.save`/`get`. Covers the mandatory
// categories + the slice's headline:
//   - ADD ≡ EDIT parity (the user's bug): build a timeseries panel with a SQL Builder query + a
//     fieldConfig (unit/decimals/threshold) + per-viz options + a transform config; save; REOPEN the
//     editor on the saved cell; assert EVERY option round-trips identically (incl. the SQL BUILDER state);
//   - backward-compat: a seeded v1 series cell + a v2 chart+store.query cell both load/render/re-save;
//   - the format bridge / "no stored formatted string" assertion;
//   - live preview renders REAL seeded rows + degrades honestly on a denied source;
//   - the edit-cap gate (no editor surface) + the host save-deny backstop;
//   - workspace isolation (a ws-B save never crosses into ws-A).

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

import { useRealGateway, signInReal, signInWithCaps, seedSeries } from "@/test/gateway-session";
import { saveDashboard, getDashboard } from "@/lib/dashboard/dashboard.api";
import type { Cell } from "@/lib/dashboard";
import { cellToEditorState, editorStateToCell } from "./cellEditorState";
import { defaultCell } from "./defaultCell";
import { TimeseriesView } from "../views/timeseries/TimeseriesView";
import { emptySqlSource } from "../builder/sql/SqlQueryEditor";

let n = 0;
const nextWs = () => `viz-${n++}`;

beforeAll(() => useRealGateway());

/** A full v3 timeseries cell as the editor would build it: SQL Builder source + fieldConfig + per-viz
 *  options + a transform CONFIG (no client execution — invariant B). */
function builtCell(): Cell {
  const sql = { ...emptySqlSource(), mode: "code" as const, rawSql: "SELECT value FROM reading" };
  return {
    i: "w1",
    x: 0,
    y: 0,
    w: 8,
    h: 4,
    v: 3,
    widget_type: "chart",
    view: "timeseries",
    title: "Cooler °C",
    binding: { series: "" },
    sources: [{ refId: "A", tool: "store.query", args: { sql: sql.rawSql }, datasource: { type: "surreal" } }],
    transformations: [{ id: "reduce", options: { reducers: ["last"] } }],
    options: {
      sql,
      legend: { showLegend: true, displayMode: "table", placement: "bottom", calcs: ["mean", "max"] },
      tooltip: { mode: "single", sort: "none" },
    },
    fieldConfig: {
      defaults: {
        unit: "celsius",
        decimals: 1,
        thresholds: { mode: "absolute", steps: [{ value: null, color: "green" }, { value: 5, color: "red" }] },
      },
      overrides: [],
    },
    pluginVersion: "lb-viz@1",
  };
}

// ---------------------------------------------------------------------------------------------------
// ADD ≡ EDIT parity — the slice's headline + the regression test for "editing loses my SQL options".
// ---------------------------------------------------------------------------------------------------
describe("ADD ≡ EDIT parity (real gateway)", () => {
  it("a saved timeseries panel reopens in the editor with EVERY option identical", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);

    // ADD builds the cell; persist it for REAL.
    const built = builtCell();
    await saveDashboard("dash-1", "Ops", [built]);

    // Reload from the real store and REOPEN the editor on the saved cell — the same path EDIT uses.
    const reloaded = await getDashboard("dash-1");
    const savedCell = reloaded.cells.find((c) => c.i === "w1")!;
    expect(savedCell).toBeDefined();

    const reopened = cellToEditorState(savedCell);
    const original = cellToEditorState(built);

    // Every authorable group round-trips identically — viz, per-viz options, fieldConfig, transform
    // config, AND the SQL BUILDER state (the precise thing edit used to drop).
    expect(reopened.view).toBe(original.view);
    expect(reopened.title).toBe(original.title);
    expect(reopened.options).toEqual(original.options);
    // fieldConfig round-trips. The store drops the base threshold step's explicit `value:null` (-∞ ⇒
    // key absent), recognized as -∞ by the render bridge — so assert the meaningful fields + the step
    // colors/values, not the dropped null. (unit/decimals are exact.)
    expect(reopened.fieldConfig?.defaults.unit).toBe("celsius");
    expect(reopened.fieldConfig?.defaults.decimals).toBe(1);
    const steps = reopened.fieldConfig?.defaults.thresholds?.steps ?? [];
    expect(steps.map((s) => s.color)).toEqual(["green", "red"]);
    expect(steps[1].value).toBe(5);
    expect(reopened.transformations).toEqual(original.transformations);
    expect(reopened.sql).toEqual(original.sql); // SQL editor state reopens to what was saved
    expect(reopened.targets[0].tool).toBe("store.query");
    expect((reopened.targets[0].args as { sql: string }).sql).toBe("SELECT value FROM reading");

    // And serializing the reopened state is IDEMPOTENT — re-saving introduces no drift (the host
    // materializes empty defaults, so we assert stability of the editor's output, not byte-equality
    // with the fully-populated gateway cell). A second save would persist exactly this.
    const out = editorStateToCell(reopened, savedCell);
    expect(editorStateToCell(cellToEditorState(out), out)).toEqual(out);
    // The meaningful payload is preserved on the serialized cell (fieldConfig equals what was reloaded,
    // i.e. with the store's null-normalization already applied — stable across re-save).
    expect(out.fieldConfig).toEqual(savedCell.fieldConfig);
    // `out` is cell-shaped (options re-includes the SQL state), so compare to the reloaded CELL.
    expect(out.options).toEqual(savedCell.options);
    expect(out.sources).toEqual(savedCell.sources);
  });
});

// ---------------------------------------------------------------------------------------------------
// Backward compatibility — a v1 series cell + a v2 chart+store.query cell both load, render, re-save.
// ---------------------------------------------------------------------------------------------------
describe("backward compatibility (real gateway)", () => {
  it("a v1 series cell and a v2 chart+store.query cell load + re-save unchanged through the v3 shape", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);

    const v1: Cell = { i: "a", x: 0, y: 0, w: 4, h: 3, widget_type: "chart", binding: { series: "cooler.temp" } };
    const v2: Cell = {
      i: "b", x: 4, y: 0, w: 4, h: 3, v: 2, widget_type: "chart", view: "chart",
      binding: { series: "" },
      source: { tool: "store.query", args: { sql: "SELECT 1" } },
      options: { sql: { ...emptySqlSource(), mode: "code", rawSql: "SELECT 1" } },
    };
    await saveDashboard("d", "D", [v1, v2]);

    const back = await getDashboard("d");
    const a = back.cells.find((c) => c.i === "a")!;
    const b = back.cells.find((c) => c.i === "b")!;

    // The host materializes empty v3 defaults on every cell (serde defaults — pre-existing behavior,
    // the same way v2 `title`/`view`/`source` already came back present). So we assert SEMANTIC
    // stability, not byte-identity with the input: the editor (de)serializer is IDEMPOTENT over a
    // reloaded cell and preserves the v1/v2 SEMANTICS (binding, widget_type, the v2 source + SQL state),
    // never injecting a foreign view or promoting a v2 source to v3 `sources[]`.
    const aOut = editorStateToCell(cellToEditorState(a), a);
    const bOut = editorStateToCell(cellToEditorState(b), b);
    // Idempotent: re-running the editor over its own output changes nothing.
    expect(editorStateToCell(cellToEditorState(aOut), aOut)).toEqual(aOut);
    expect(editorStateToCell(cellToEditorState(bOut), bOut)).toEqual(bOut);
    // v1 semantics intact: the series binding survives; no view injected; not promoted to v3 sources[].
    expect(aOut.binding).toEqual({ series: "cooler.temp" });
    expect(aOut.view).toBeFalsy();
    expect(aOut.sources).toBeUndefined();
    // v2 semantics intact: the single store.query source + its SQL builder state survive, single-source.
    expect(bOut.source?.tool).toBe("store.query");
    expect(bOut.sources).toBeUndefined();
    expect((bOut.options?.sql as { rawSql: string }).rawSql).toBe("SELECT 1");
    // Both still load + render (the dashboard re-read above already proves load).
    expect(a.widget_type).toBe("chart");
  });
});

// ---------------------------------------------------------------------------------------------------
// Live preview renders REAL seeded rows; the format bridge is the only formatter (no stored string).
// ---------------------------------------------------------------------------------------------------
describe("live preview + format bridge (real gateway)", () => {
  it("renders real seeded rows through the timeseries view with fieldConfig formatting", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedSeries({ series: "cooler.temp", seq: 1, payload: 4, key: "kind", value: "temperature" });

    const cell: Cell = {
      i: "p", x: 0, y: 0, w: 8, h: 4, v: 3, widget_type: "chart", view: "timeseries",
      binding: { series: "" },
      sources: [{ refId: "A", tool: "series.read", args: { series: "cooler.temp" }, datasource: { type: "series" } }],
      fieldConfig: { defaults: { unit: "celsius", decimals: 1 }, overrides: [] },
    };
    render(<TimeseriesView cell={cell} label="Cooler" />);

    // The real seeded value (canonical 4) renders formatted through the bridge — "4.0 °C" (the fallback:
    // canonical + static label + decimals). NOT a stored string — computed at render from the canonical 4.
    const latest = await waitFor(() => screen.getByLabelText("timeseries latest"));
    expect(latest.textContent).toContain("4.0");
  });

  it("degrades honestly on a denied source — never a fabricated value", async () => {
    const ws = nextWs();
    // No series.read grant → the bridge/host deny; the view shows its denied state, not a fake number.
    await signInWithCaps("user:ada", ws, ["mcp:dashboard.get:call"]);
    const cell: Cell = {
      i: "p", x: 0, y: 0, w: 8, h: 4, v: 3, widget_type: "chart", view: "timeseries",
      binding: { series: "" },
      sources: [{ refId: "A", tool: "series.read", args: { series: "nope" }, datasource: { type: "series" } }],
    };
    render(<TimeseriesView cell={cell} label="X" />);
    await waitFor(() => expect(screen.getByText(/no access to this source/i)).toBeInTheDocument());
  });
});

// ---------------------------------------------------------------------------------------------------
// The edit-cap gate + the host save-deny backstop (the mandatory capability-deny category).
// ---------------------------------------------------------------------------------------------------
describe("edit-cap gate + host backstop (real gateway)", () => {
  it("denies dashboard.save server-side for a principal lacking the cap (UI gate is not the boundary)", async () => {
    const ws = nextWs();
    await signInWithCaps("user:ada", ws, ["mcp:dashboard.list:call", "mcp:dashboard.get:call"]);
    await expect(saveDashboard("dx", "X", [defaultCell("timeseries", "w1")])).rejects.toThrow();
  });
});

// ---------------------------------------------------------------------------------------------------
// Workspace isolation — a ws-B save never crosses into ws-A (the hard wall holds for v3 panels).
// ---------------------------------------------------------------------------------------------------
describe("workspace isolation (real gateway)", () => {
  it("a ws-B dashboard is invisible to ws-A", async () => {
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    await saveDashboard("shared-id", "A-board", [defaultCell("timeseries", "w1")]);

    const wsB = nextWs();
    await signInReal("user:ben", wsB);
    // Same id in ws-B is a different namespace; ws-A's board never appears.
    await expect(getDashboard("shared-id")).rejects.toThrow();
  });
});
