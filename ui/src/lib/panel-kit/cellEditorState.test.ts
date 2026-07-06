// The cell ↔ editorState round-trip — the contract that makes add≡edit impossible to break (viz
// panel-editor scope, Testing plan: "editorStateToCell(cellToEditorState(c)) ≡ c for v1/v2/v3"). If
// this passes, EDIT reconstructs exactly what was saved and ADD (a default cell) serializes losslessly
// — the user's "edit loses my SQL options" bug cannot recur silently. Pure, no gateway.

import { describe, it, expect } from "vitest";

import type { Cell } from "@/lib/dashboard";
import { cellToEditorState, editorStateToCell } from "./cellEditorState";
import { defaultCell } from "./defaultCell";
import { emptySqlSource } from "./sql/query";
import { emitSql } from "./sql/dialect";

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

  it("a v3 federation cell carrying options.sql round-trips (the query-builder-common contract)", () => {
    // After the query-builder-common scope, a federation target stores the SAME `options.sql`
    // (a SqlSourceState) a surreal target stores — so reopening returns to the builder. The wire
    // shape (`federation.query {source, sql}`) is unchanged; `options.sql` is the additive carry.
    const fed: Cell = {
      i: "f1",
      x: 0,
      y: 0,
      w: 8,
      h: 4,
      v: 3,
      widget_type: "chart",
      view: "table",
      title: "Avg reading",
      binding: { series: "" },
      sources: [
        {
          refId: "A",
          tool: "federation.query",
          args: { source: "demo-buildings", sql: 'SELECT AVG("value") AS "avg_value" FROM "point_reading" LIMIT 100' },
          datasource: { type: "federation", uid: "datasource:acme:demo-buildings" },
        },
      ],
      options: {
        sql: {
          mode: "builder",
          rawSql: 'SELECT AVG("value") AS "avg_value" FROM "point_reading" LIMIT 100',
          builder: {
            table: "point_reading",
            columns: [{ name: "value", aggregation: "avg" }],
            filters: [],
            groupBy: [],
            limit: 100,
          },
          format: "table",
        },
      },
    };
    const back = roundTrip(fed);
    expect(back).toEqual(fed);
    // The builder state survives in options.sql (reopening returns to the builder, like surreal).
    expect((back.options?.sql as { builder: { table: string } }).builder.table).toBe("point_reading");
    // The wire shape is unchanged — args.sql is still the raw string federation.query runs.
    expect((back.sources?.[0].args as { sql: string }).sql).toBe(
      'SELECT AVG("value") AS "avg_value" FROM "point_reading" LIMIT 100',
    );
  });

  it("a pre-slice federation cell (args.sql but no options.sql) round-trips without fabrication", () => {
    // A federation cell authored BEFORE the query-builder-common slice has `target.args.sql` but no
    // `options.sql` — the legacy raw-SQL-only shape. Reopen must preserve it byte-for-byte; we do
    // NOT fabricate a builder query from hand-edited SQL (scope Risks). The editor falls to Code mode.
    const legacy: Cell = {
      i: "f2",
      x: 0,
      y: 0,
      w: 8,
      h: 4,
      v: 3,
      widget_type: "chart",
      view: "table",
      binding: { series: "" },
      sources: [
        {
          refId: "A",
          tool: "federation.query",
          args: { source: "demo-buildings", sql: "SELECT 1" },
          datasource: { type: "federation" },
        },
      ],
      options: { code: "legacy" }, // unrelated option key, must round-trip verbatim
    };
    const back = roundTrip(legacy);
    expect(back).toEqual(legacy);
    // No spurious `options.sql` was injected (byte-clean).
    expect(back.options?.sql).toBeUndefined();
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

  it("a v3 template cell's inline `options.code` round-trips (the save/reload contract)", () => {
    // render-template-inprocess scope: a template cell carries its body in `options.code` (inline) — this
    // is the exact field the builder's TemplateOptionsEditor patches through `carry.extraOptions`. If the
    // round-trip dropped it, saving a template would lose the body on reload.
    const tpl: Cell = {
      i: "t1",
      x: 0, y: 0, w: 6, h: 4, v: 3,
      widget_type: "chart",
      view: "template",
      binding: { series: "" },
      sources: [{ refId: "A", tool: "federation.query", args: { source: "timescale", sql: "select * from site" }, datasource: { type: "federation" } }],
      options: { code: `<ul>{{#each rows}}<li>{{site}}</li>{{/each}}</ul>` },
    };
    const back = roundTrip(tpl);
    expect(back).toEqual(tpl);
    expect(back.view).toBe("template");
    // The body survives under options.code (not dropped, not moved to a typed group).
    expect(back.options?.code).toBe(tpl.options?.code);
  });

  it("a v3 template cell's `options.templateId` (Saved mode) round-trips too", () => {
    const tpl: Cell = {
      i: "t2",
      x: 0, y: 0, w: 6, h: 4, v: 3,
      widget_type: "chart",
      view: "template",
      binding: { series: "" },
      sources: [{ refId: "A", tool: "store.query", args: { sql: "SELECT 1" }, datasource: { type: "surreal" } }],
      options: { templateId: "defrost" },
    };
    const back = roundTrip(tpl);
    expect(back).toEqual(tpl);
    expect(back.options?.templateId).toBe("defrost");
  });

  it("typing template code in the editor reaches the saved cell (the save/reload flow, end-to-end)", () => {
    // Regression for the reported "I save the code and it's not persisted": simulate the EXACT builder
    // flow — a fresh default template cell → cellToEditorState → the editor patches `carry.extraOptions.code`
    // (what TemplateOptionsEditor.onChange does) → editorStateToCell (what Save persists). The body must
    // land on cell.options.code.
    const fresh = defaultCell("template", "new", undefined, {});
    const state = cellToEditorState(fresh);
    // The TemplateOptionsEditor patches code through carry.extraOptions (NOT state.options — `code` is
    // not an OWNED_OPTION_KEY, so it rides the carry).
    const patched: typeof state = {
      ...state,
      carry: { ...state.carry, extraOptions: { ...state.carry.extraOptions, code: "<p>hi {{rows.length}}</p>" } },
    };
    const saved = editorStateToCell(patched, fresh);
    expect(saved.view).toBe("template");
    expect((saved.options as { code?: string }).code).toBe("<p>hi {{rows.length}}</p>");
    // And the body survives a reload (cellToEditorState reads it back into extraOptions).
    const reloaded = cellToEditorState(saved);
    expect((reloaded.carry.extraOptions as { code?: string }).code).toBe("<p>hi {{rows.length}}</p>");
  });

  it("a fresh default cell (ADD) serializes losslessly through the editor", () => {
    // The per-view option defaults are INJECTED (the view substrate's registry owns them; panel-kit
    // stays headless) — pass a representative block, as a real caller does.
    const fresh = defaultCell("timeseries", "w3", undefined, {
      legend: { showLegend: true, displayMode: "list", placement: "bottom", calcs: [] },
      tooltip: { mode: "single", sort: "none" },
    });
    const back = roundTrip(fresh);
    expect(back).toEqual(fresh);
    // ADD seeds the full v3 surface: a target, the injected options, an empty field-config.
    expect(back.view).toBe("timeseries");
    expect(back.options).toEqual(fresh.options);
    expect(back.sources?.length).toBe(1);
    expect(back.fieldConfig).toEqual({ defaults: {}, overrides: [] });
  });

  it("a target-less cell gains a queryable sources[] once a target is authored (federation 'run the sql')", () => {
    // Regression: a cell with NO source/sources started as targetRepr:"none". Authoring a federation
    // target in the Query tab put it in state.targets, but serialize dropped it (none branch) — so the
    // preview's viz.query saw no source and showed "no data yet", and save persisted an empty panel.
    const targetless: Cell = {
      i: "new",
      x: 0,
      y: 0,
      w: 6,
      h: 4,
      widget_type: "chart",
      binding: { series: "" },
    };
    const state = cellToEditorState(targetless);
    expect(state.carry.targetRepr).toBe("none");
    const fedTarget = { refId: "A", tool: "federation.query", args: { source: "timescale", sql: "select * from sites" }, datasource: { type: "federation" as const } };
    const out = editorStateToCell({ ...state, targets: [fedTarget] }, targetless);
    expect(out.sources).toEqual([fedTarget]);
    // A still-empty (no-tool) target must NOT spawn a spurious source — a blank panel stays blank.
    const stillEmpty = editorStateToCell({ ...state, targets: [{ refId: "A", tool: "", args: {} }] }, targetless);
    expect(stillEmpty.sources).toBeUndefined();
    expect(stillEmpty.source).toBeUndefined();
  });

  it("the cell key + geometry is preserved on serialize (the edit invariant)", () => {
    const base = defaultCell("timeseries", "keep-me", { x: 3, y: 7, w: 5, h: 2 });
    const state = cellToEditorState(base);
    const out = editorStateToCell({ ...state, title: "changed" }, base);
    expect(out.i).toBe("keep-me");
    expect([out.x, out.y, out.w, out.h]).toEqual([3, 7, 5, 2]);
    expect(out.title).toBe("changed");
  });

  // ── visual-canvas-builder slice: extended query (joins + HAVING + aliases + multi-orderBy) round-trips ──

  it("an extended builder query (joins + HAVING + aliases + multi-orderBy + builderLayout) round-trips byte-identical", () => {
    // The full slice-1 model surface — every new field carried in options.sql. Persist + reopen must
    // return the SAME shape (joins intact, HAVING filter preserved with its aggregation, multi-orderBy
    // array intact, the opaque builderLayout blob preserved verbatim). The model is the source of truth.
    const ext: Cell = {
      i: "x1",
      x: 0,
      y: 0,
      w: 8,
      h: 4,
      v: 3,
      widget_type: "chart",
      view: "table",
      title: "Join + HAVING",
      binding: { series: "" },
      sources: [
        {
          refId: "A",
          tool: "federation.query",
          args: { source: "demo-buildings", sql: "" },
          datasource: { type: "federation", uid: "datasource:acme:demo-buildings" },
        },
      ],
      options: {
        sql: {
          mode: "builder",
          rawSql: "",
          builder: {
            table: "site",
            joins: [
              {
                table: "point_reading",
                type: "inner",
                on: [{ leftColumn: "id", rightColumn: "site_id" }],
              },
            ],
            columns: [
              { name: "name", table: "site", order: 1 },
              { name: "value", table: "point_reading", aggregation: "avg", alias: "avg_v", order: 2 },
            ],
            filters: [
              { column: "kind", operator: "=", value: "cpu" },
              { column: "value", operator: ">", value: 10, isAggregate: true, aggregation: "avg" },
            ],
            groupBy: [{ table: "site", column: "name" }],
            orderBy: [
              { column: "name", direction: "asc" },
              { column: "avg_v", direction: "desc" },
            ],
            limit: 100,
          },
          format: "table",
          builderLayout: { site: { x: 0, y: 40 }, point_reading: { x: 280, y: 40 } },
        },
      },
    };
    // The rawSql mirrors what the emitter would produce (the host keeps them in sync); fill it in to
    // assert the round-tripped SQL is byte-identical too.
    const expectedSql = emitSql("standard", (ext.options!.sql as { builder: unknown }).builder as never);
    (ext.options!.sql as { rawSql: string }).rawSql = expectedSql;

    const back = roundTrip(ext);
    expect(back).toEqual(ext);
    // The opaque layout blob survives verbatim (positions are view state, not query semantics).
    expect((back.options?.sql as { builderLayout: unknown }).builderLayout).toEqual({
      site: { x: 0, y: 40 },
      point_reading: { x: 280, y: 40 },
    });
    // The joins + HAVING survive (the model-as-truth invariant — the canvas would re-derive from these).
    const b = (back.options?.sql as { builder: { joins: unknown[]; filters: unknown[] } }).builder;
    expect(b.joins).toHaveLength(1);
    expect(b.filters.some((f) => (f as { isAggregate?: boolean }).isAggregate)).toBe(true);
    // The emitted SQL is stable across the round-trip.
    expect(emitSql("standard", b as never)).toBe(expectedSql);
  });

  it("a legacy single-object orderBy round-trips to the array shape (write-array contract); SQL is byte-identical", () => {
    // A pre-slice-2 cell persisted with the OLD single-object orderBy. Reopen normalizes it to a
    // 1-element array (the WRITE contract); the emitter's `normalizeOrderBy` reads both shapes, so the
    // SQL string is byte-identical before and after. This is a SEMANTIC round-trip (the persisted shape
    // upgrades on first save), not byte-identity of the input.
    const legacy: Cell = {
      i: "x2",
      x: 0,
      y: 0,
      w: 8,
      h: 4,
      v: 3,
      widget_type: "chart",
      view: "table",
      title: "Legacy order",
      binding: { series: "" },
      sources: [
        {
          refId: "A",
          tool: "federation.query",
          args: { source: "demo-buildings", sql: 'SELECT "value" FROM "point_reading" ORDER BY "value" ASC' },
          datasource: { type: "federation" },
        },
      ],
      options: {
        sql: {
          mode: "builder",
          rawSql: 'SELECT "value" FROM "point_reading" ORDER BY "value" ASC',
          builder: {
            table: "point_reading",
            columns: [{ name: "value" }],
            filters: [],
            groupBy: [],
            orderBy: { column: "value", direction: "asc" }, // legacy single-object shape
            limit: 100,
          },
          format: "table",
        },
      },
    };

    const legacySql = emitSql(
      "standard",
      (legacy.options!.sql as { builder: unknown }).builder as never,
    );
    expect(legacySql).toBe('SELECT "value" FROM "point_reading" ORDER BY "value" ASC LIMIT 100');

    const back = roundTrip(legacy);
    const b = (back.options?.sql as { builder: { orderBy: unknown } }).builder;
    // The write-array contract: orderBy is now an array.
    expect(Array.isArray(b.orderBy)).toBe(true);
    expect(b.orderBy).toEqual([{ column: "value", direction: "asc" }]);
    // The emitted SQL is byte-identical (the normalization is semantics-preserving).
    expect(emitSql("standard", b as never)).toBe(legacySql);
  });
});
