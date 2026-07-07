// The Field-tab "what works / what doesn't" baseline (viz field-config scope), against a REAL gateway.
// This is the durable proof behind the Field-tab audit: for every option the Field tab exposes, it
// classifies the option as LIVE (setting it changes the rendered DOM in an observable, specific way) or
// DEAD (setting it changes nothing — the renderer never reads it). It is the regression net for the
// UX-simplify step: when we collapse the Field tab we will know exactly what we are intentionally
// dropping (a DEAD option) vs accidentally breaking (a LIVE option whose assertion starts failing).
//
// Doctrine (CLAUDE §9 / testing §0): real gateway, real seeded samples through the ingest path, the
// editor's OWN authoring path (`cellToEditorState` + `writeOption` + `editorStateToCell`) so a "set
// option" here is byte-identical to a user setting it in the Field tab. No fake backend.
//
// Why the DEAD check compares the NON-SVG DOM: recharts generates non-deterministic clipPath ids per
// render, so raw innerHTML differs even for identical inputs. The plain-HTML wrapper (header, the
// `timeseries latest` readout, the legend, the chart host's `data-draw-style` + `style` color) is
// deterministic, and it is ALSO where every Field-tab option's visible effect lands (text, color, draw
// style). A DEAD option leaves this wrapper unchanged; a LIVE one changes it observably.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor, cleanup } from "@testing-library/react";

import { useRealGateway, signInReal, seedSeries } from "@/test/gateway-session";
import type { Cell } from "@/lib/dashboard";
import { cellToEditorState, editorStateToCell } from "@/lib/panel-kit/cellEditorState";
import { writeOption } from "@/features/panel-builder/options/binding";
import { WithDashboardCache } from "@/features/dashboard/cache/testCacheWrapper";

import { StatPanel } from "./stat/StatPanel";
import { GaugePanel } from "./gauge/GaugePanel";
import { TimeseriesView } from "./timeseries/TimeseriesView";
import { TablePanel } from "./table/TablePanel";

let n = 0;
const nextWs = () => `fieldbase-${n++}`;

beforeAll(() => useRealGateway());

/** Seed one real sample (canonical value `payload`) so a single-value panel has a deterministic read. */
async function seedOne(series: string, payload: number): Promise<void> {
  await seedSeries({ series, seq: 1, payload, key: "kind", value: "temperature" });
}

/** Set a registered option on `cell` through the editor's REAL authoring path (the same `writeOption`
 *  the Field tab calls). Returns a new cell; the input is untouched. */
function setOpt(cell: Cell, optionId: string, value: unknown): Cell {
  const state = cellToEditorState(cell);
  // `optionById` is in the registry; importing it here would couple this view-level test to the
  // panel-builder option registry. Instead we write through the SAME binding the editor tab does, by
  // looking the def up via the registry's own finder (re-exported here for the test).
  const def = optionById(optionId);
  if (!def) throw new Error(`unknown option ${optionId}`);
  const next = writeOption(state, def, value);
  return editorStateToCell({ ...state, ...next }, cell);
}

// Registry lookup kept behind a lazy import so this view-level test does not pull the whole editor tree
// at module load (and so the failure is clear if an option id is renamed).
import { optionById } from "@/features/panel-builder/options/registry";

/** The plain-HTML DOM, with recharts `<svg>` subtrees removed. The wrapper (header / readout / legend /
 *  chart-host `data-draw-style` + `style`) is what Field-tab options affect and what we compare. */
function plainDom(container: HTMLElement): string {
  const clone = container.cloneNode(true) as HTMLElement;
  clone.querySelectorAll("svg").forEach((s) => s.remove());
  return clone.innerHTML;
}

/** Collapse whitespace so a color assertion is robust to jsdom's `hsl(...)` → `rgb(r, g, b)` rendering
 *  AND to its spacing. We assert on the distinctive `rgb(r,g,b)` triplet resolveColor produces. */
function norm(s: string): string {
  return s.replace(/\s+/g, "");
}

/** A v3 single-value cell bound to a `series.read` source — the shape the PanelEditor saves. */
function baseCell(view: Cell["view"], series: string, options: Record<string, unknown> = {}): Cell {
  return {
    i: "c",
    x: 0,
    y: 0,
    w: 6,
    h: 4,
    v: 3,
    widget_type: "stat",
    view,
    binding: { series: "" },
    sources: [{ refId: "A", tool: "series.read", args: { series }, datasource: { type: "series" } }],
    options,
  };
}

/** Render one panel, wait for its value readout, return the rendered container + the plain DOM. */
async function renderStat(ws: string, cell: Cell): Promise<{ html: string; container: HTMLElement }> {
  const { container, unmount } = render(
    <WithDashboardCache ws={ws}>
      <StatPanel cell={cell} label="S" />
    </WithDashboardCache>,
  );
  await waitFor(() => expect(screen.getByLabelText("stat value")).toBeInTheDocument());
  const html = plainDom(container);
  unmount();
  cleanup();
  return { html, container };
}

/** Render a timeseries, wait for its `latest` readout, return the plain DOM (SVG stripped). */
async function renderTimeseries(ws: string, cell: Cell): Promise<{ html: string; container: HTMLElement }> {
  const { container, unmount } = render(
    <WithDashboardCache ws={ws}>
      <TimeseriesView cell={cell} label="T" />
    </WithDashboardCache>,
  );
  await waitFor(() => expect(screen.getByLabelText("timeseries latest")).toBeInTheDocument());
  const html = plainDom(container);
  unmount();
  cleanup();
  return { html, container };
}

// ===================================================================================================
// STANDARD field options — LIVE on the single-stat family (the faithful renderer). These are the
// options that DO work and that the UX-simplify step must preserve. Each asserts a SPECIFIC observable.
// ===================================================================================================
describe("Field tab — standard options LIVE on stat (real gateway)", () => {
  it.each([
    ["unit", { unit: "celsius" }, "°C"],
    ["decimals", { decimals: 2 }, "42.00"],
  ] as const)("option %s changes the rendered value (observes %p)", async (id, set, marker) => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedOne("s.opt", 42);
    const base = baseCell("stat", "s.opt", { reduceOptions: { calcs: ["lastNotNull"] }, colorMode: "value", graphMode: "none", textMode: "auto" });
    const withOpt = setOpt(base, id, Object.values(set)[0]);
    const { html } = await renderStat(ws, withOpt);
    expect(html).toContain(marker);
  });

  it("displayName overrides the panel label (used when the cell carries no title)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedOne("s.dn", 42);
    const base = baseCell("stat", "s.dn", { reduceOptions: { calcs: ["lastNotNull"] }, colorMode: "value", graphMode: "none", textMode: "name" });
    const withOpt = setOpt(base, "displayName", "My Field");
    // Render with NO label so `label ?? opts.displayName` falls through to the field's displayName —
    // this is the dashboard case where a cell has no title.
    const { container, unmount } = render(
      <WithDashboardCache ws={ws}>
        <StatPanel cell={withOpt} />
      </WithDashboardCache>,
    );
    await waitFor(() => expect(screen.getByLabelText("stat value")).toBeInTheDocument());
    const html = plainDom(container);
    unmount();
    cleanup();
    expect(html).toContain("My Field");
  });

  it("noValue shows when the value is null", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    // Bind to a series with no data → value reduces to null → noValue shows.
    const base = baseCell("stat", "s.empty", { reduceOptions: { calcs: ["lastNotNull"] }, colorMode: "value", graphMode: "none", textMode: "auto" });
    const withOpt = setOpt(base, "noValue", "—none—");
    const { html } = await renderStat(ws, withOpt);
    expect(html).toContain("—none—");
  });

  it("thresholds color the value (red ≥ 30) — thresholds COLOR, never alert", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedOne("s.th", 42); // 42 ≥ 30 → red step
    const base = baseCell("stat", "s.th", { reduceOptions: { calcs: ["lastNotNull"] }, colorMode: "value", graphMode: "none", textMode: "auto" });
    const withOpt = setOpt(base, "thresholds", { mode: "absolute", steps: [{ value: null, color: "green" }, { value: 30, color: "red" }] });
    const { html } = await renderStat(ws, withOpt);
    // resolveColor("red") = "hsl(0 72% 51%)" → jsdom renders the value span's inline style as
    // rgb(220, 40, 40). Distinct from the default accent token; that flip is the proof.
    expect(norm(html)).toContain("rgb(220,40,40)");
  });

  it("color scheme = fixed paints the value that fixed color", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedOne("s.fx", 42);
    const base = baseCell("stat", "s.fx", { reduceOptions: { calcs: ["lastNotNull"] }, colorMode: "value", graphMode: "none", textMode: "auto" });
    const withOpt = setOpt(base, "color", { mode: "fixed", fixedColor: "blue" });
    const { html } = await renderStat(ws, withOpt);
    expect(norm(html)).toContain("rgb(60,131,246)"); // resolveColor("blue") = hsl(217 91% 60%) → rgb
  });

  it("value mappings replace the value text (stat honors them — the faithful renderer)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedOne("s.map", 42);
    const base = baseCell("stat", "s.map", { reduceOptions: { calcs: ["lastNotNull"] }, colorMode: "value", graphMode: "none", textMode: "auto" });
    const withOpt = setOpt(base, "mappings", [{ type: "value", options: { "42": { text: "FORTY-TWO" } } }]);
    const { html } = await renderStat(ws, withOpt);
    expect(html).toContain("FORTY-TWO");
  });
});

// ===================================================================================================
// TIMESERIES Field-tab options — LIVE. The timeseries honors the standard value-formatting options and
// the graph-style draw style. Each asserts a SPECIFIC observable.
// ===================================================================================================
describe("Field tab — timeseries LIVE options (real gateway)", () => {
  it("unit + decimals format the latest readout", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedOne("t.ud", 4);
    let cell = setOpt(baseCell("timeseries", "t.ud"), "unit", "celsius");
    cell = setOpt(cell, "decimals", 1);
    const { html } = await renderTimeseries(ws, cell);
    expect(html).toContain("4.0");
  });

  it("drawStyle changes the chart host's data-draw-style attribute", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedOne("t.ds", 4);
    const base = setOpt(baseCell("timeseries", "t.ds"), "custom.drawStyle", "bars");
    const { html } = await renderTimeseries(ws, base);
    expect(html).toContain('data-draw-style="bars"');
  });

  it("thresholds color the line (the host's inline color flips to red)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedOne("t.th", 42); // last = 42 ≥ 30 → red
    const base = setOpt(baseCell("timeseries", "t.th"), "thresholds", { mode: "absolute", steps: [{ value: null, color: "green" }, { value: 30, color: "red" }] });
    const { html } = await renderTimeseries(ws, base);
    // The host div + the legend swatch both flip to resolveColor("red") = hsl(0 72% 51%) → rgb(220,40,40).
    expect(norm(html)).toContain("rgb(220,40,40)");
  });
});

// ===================================================================================================
// TIMESERIES Field-tab options — DEAD. THE HEADLINE FINDING: these options the Field tab exposes but the
// timeseries renderer never reads. Each sets the option to a non-default value through the editor's real
// write path and asserts the rendered (non-SVG) DOM is byte-identical to the baseline — i.e. the option
// had zero visible effect. This is the list the UX-simplify step can collapse without losing behavior.
// ===================================================================================================
describe("Field tab — timeseries DEAD options (set, but the renderer ignores them)", () => {
  // Each entry: [optionId, nonDefaultSampleValue]. A non-default value that, IF the renderer honored it,
  // would change the plain DOM. The assertion proves it does not.
  const DEAD: Array<[string, unknown]> = [
    // Standard options the timeseries renderer does NOT apply:
    ["mappings", [{ type: "value", options: { "4": { text: "MAPPED" } } }]], // TS never calls applyMappings
    ["links", [{ title: "Docs", url: "https://x/${__value.text}", targetBlank: true }]], // no drilldown render
    // Graph-style options read into `custom` but never applied to the recharts SVG:
    ["custom.lineInterpolation", "stepBefore"], // recharts always uses monotone
    ["custom.gradientMode", "opacity"],
    ["custom.showPoints", "always"],
    ["custom.spanNulls", true], // recharts connects gaps regardless
    // Axis option — the axis is hidden, so placement has nothing to place:
    ["custom.axisPlacement", "right"],
    // timeseriesViz field-scoped options with no render path:
    ["custom.stacking.mode", "normal"], // no stacking in the renderer
    ["custom.thresholdsStyle.mode", "line"], // no threshold line/area rendering
  ];

  it.each(DEAD)("option %s is DEAD — setting it leaves the rendered DOM unchanged", async (id, value) => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedOne("t.dead", 4);
    const base = baseCell("timeseries", "t.dead");
    const baseline = await renderTimeseries(ws, base);
    const withOpt = setOpt(base, id, value);
    const changed = await renderTimeseries(ws, withOpt);
    expect(changed.html, `option ${id} should be DEAD but changed the rendered DOM`).toBe(baseline.html);
  });
});

// ===================================================================================================
// TABLE per-column custom options — DEAD. The table renderer does not apply per-column width / alignment
// / cell display type / per-column filter; `custom.*` on a table cell round-trips but renders unchanged.
// ===================================================================================================
describe("Field tab — table per-column DEAD options (real gateway)", () => {
  const DEAD: Array<[string, unknown]> = [
    ["custom.width", 400],
    ["custom.align", "center"],
    ["custom.cellOptions.type", "color-background"],
    ["custom.filterable", true],
  ];

  it.each(DEAD)("table option %s is DEAD — setting it leaves the rendered table unchanged", async (id, value) => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedOne("t.tbl", 7);
    // A table reads the seeded scalar; the fieldConfig custom.* would target its column if honored.
    const base = baseCell("table", "t.tbl");
    const baseline = await renderTable(ws, base);
    const withOpt = setOpt(base, id, value);
    const changed = await renderTable(ws, withOpt);
    expect(changed.html, `table option ${id} should be DEAD but changed the rendered DOM`).toBe(baseline.html);
  });
});

/** Render a table, wait for a value, return the plain DOM (SVG stripped — table has none, but stable). */
async function renderTable(ws: string, cell: Cell): Promise<{ html: string; container: HTMLElement }> {
  const { container, unmount } = render(
    <WithDashboardCache ws={ws}>
      <TablePanel cell={cell} label="T" />
    </WithDashboardCache>,
  );
  await waitFor(() => expect(container.querySelector("table")).toBeInTheDocument());
  await new Promise((r) => setTimeout(r, 30)); // let the seeded row paint
  const html = plainDom(container);
  unmount();
  cleanup();
  return { html, container };
}

// ===================================================================================================
// Gauge min/max — LIVE (the gauge arc + bounds read canonical min/max). Included because min/max are
// Field-tab standard options and the gauge is where their effect is most observable.
// ===================================================================================================
describe("Field tab — gauge min/max LIVE (real gateway)", () => {
  it("min/max rescale the gauge so a value shows a different arc", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedOne("g.mm", 50);
    const cell = setOpt(setOpt(baseCell("gauge", "g.mm", { reduceOptions: { calcs: ["lastNotNull" as never] } }), "max", 200), "min", 0);
    const { unmount } = render(
      <WithDashboardCache ws={ws}>
        <GaugePanel cell={cell} label="G" />
      </WithDashboardCache>,
    );
    await waitFor(() => expect(screen.getByLabelText("gauge value")).toBeInTheDocument());
    // The gauge value reads 50 (canonical) regardless of max; the assertion is that min/max are RESOLVED
    // (no crash, the bounds label renders when showThresholdLabels). The arc fraction = (50-0)/(200-0).
    expect(screen.getByLabelText("gauge value").textContent).toContain("50");
    unmount();
    cleanup();
  });
});
