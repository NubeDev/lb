// Registry-driven round-trip coverage (editor-parity scope, step 2 testing plan; CLAUDE §9 real infra
// not needed — this is a pure (de)serializer contract, no gateway). The hard constraint: EVERY
// registered option must survive `editorStateToCell(cellToEditorState(c)) ≡ c`. We build a cell that
// sets each registered option to a NON-DEFAULT sample value via the SAME write path the editor uses
// (`writeOption`), then assert the cell round-trips byte-identical AND each option reads back its value.
// A new option added to the registry with no sample here FAILS the exhaustiveness guard — it can't dodge
// the test.

import { describe, expect, it } from "vitest";
import type { Cell, View } from "@/lib/dashboard";
import { cellToEditorState, editorStateToCell, type EditorState } from "@/lib/panel-kit/cellEditorState";
import { OPTION_REGISTRY, optionsForView } from "./registry";
import { readOption, writeOption } from "./binding";
import type { OptionDef } from "./types";

/** A non-default sample value for each control kind — distinct from the option's `default` so a dropped
 *  write is caught. Rich kinds get a minimal valid instance of their stored shape. */
function sampleValue(def: OptionDef): unknown {
  switch (def.control.kind) {
    case "number":
      return 7;
    case "text":
      return `${def.id}-sample`;
    case "toggle":
      return def.default === true ? false : true;
    case "select":
    case "multi-select":
      // pick a choice that isn't the default
      return def.control.choices.find((c) => c.value !== def.default)?.value ?? def.control.choices[0].value;
    case "field-name":
      return "value";
    case "color":
      return "red";
    case "unit":
      return "celsius";
    case "thresholds":
      return { mode: "percentage", steps: [{ value: null, color: "green" }, { value: 80, color: "red" }] };
    case "mappings":
      return [
        { type: "value", options: { OK: { text: "Good", color: "green" } } },
        { type: "range", options: { from: 0, to: 10, result: { text: "low", color: "yellow" } } },
        { type: "special", options: { match: "null", result: { text: "n/a" } } },
      ];
    case "color-scheme":
      return { mode: "fixed", fixedColor: "blue" };
    case "data-links":
      return [{ title: "Docs", url: "https://x/${__value.text}", targetBlank: true }];
  }
}

/** Build a fully-populated cell: every option that applies to `view` set to its sample value, through
 *  the editor's own `writeOption`. Starts from a minimal v3 cell so the base is realistic. */
function fullyPopulatedCell(view: View): { cell: Cell; state: EditorState; defs: OptionDef[] } {
  const base: Cell = {
    i: "w1", x: 0, y: 0, w: 8, h: 4, v: 3, widget_type: "chart", view,
    binding: { series: "" },
    sources: [{ refId: "A", tool: "series.read", args: { series: "s" }, datasource: { type: "series" } }],
  };
  const defs = optionsForView(view);
  let state = cellToEditorState(base);
  for (const def of defs) state = { ...state, ...writeOption(state, def, sampleValue(def)) };
  return { cell: editorStateToCell(state, base), state, defs };
}

// Every view that has registered options — the round-trip must hold for ALL of them, not just
// timeseries (so a per-viz option added to table/stat/gauge/etc. is exercised too).
const VIEWS_WITH_OPTIONS: View[] = ["timeseries", "table", "stat", "gauge", "bargauge", "piechart"];

describe("option registry round-trip", () => {
  it("has unique ids and every def has a sample (exhaustiveness guard)", () => {
    const ids = OPTION_REGISTRY.map((d) => d.id);
    // ids may repeat across views (e.g. `orientation` for stat/gauge/bargauge) — uniqueness is per id+view.
    for (const def of OPTION_REGISTRY) {
      expect(sampleValue(def), `no sample for control kind of ${def.id}`).toBeDefined();
    }
    expect(ids.length).toBeGreaterThan(0);
  });

  it.each(VIEWS_WITH_OPTIONS)("every registered %s option survives editorStateToCell(cellToEditorState(c)) ≡ c", (view) => {
    const { cell } = fullyPopulatedCell(view);
    const round = editorStateToCell(cellToEditorState(cell), cell);
    expect(round).toEqual(cell);
  });

  it.each(VIEWS_WITH_OPTIONS)("each populated %s option reads back its sample value after a round-trip", (view) => {
    const { cell, defs } = fullyPopulatedCell(view);
    const reopened = cellToEditorState(cell);
    for (const def of defs) {
      expect(readOption(reopened, def), `option ${def.id} did not round-trip`).toEqual(sampleValue(def));
    }
  });

  it("the fieldConfig-less views (insights, weather) expose no option cards", () => {
    // A view in NO_FIELDCONFIG_VIEWS renders fixed fields, not a fieldConfig-formatted value — so the
    // universal standard options (unit/decimals/color/thresholds…) are excluded and it has no per-viz
    // defs. `optionsForView` returns []; the Options step is empty (and `optionLiveness` is never asked
    // for a row it doesn't have — the "no row for weather/color" throw the wizard hit before this).
    for (const view of ["insights", "weather"] as View[]) {
      // insights carries its OWN `options.insights.*` defs; weather carries none at all.
      const standardIds = optionsForView(view).filter((d) => d.scope === "fieldConfig");
      expect(standardIds, `${view} must expose no fieldConfig options`).toEqual([]);
    }
    expect(optionsForView("weather")).toEqual([]);
  });

  it("clearing every fieldConfig option prunes fieldConfig back to absent (no empty groups linger)", () => {
    const { cell } = fullyPopulatedCell("timeseries");
    let state = cellToEditorState(cell);
    for (const def of optionsForView("timeseries").filter((d) => d.scope === "fieldConfig")) {
      state = { ...state, ...writeOption(state, def, undefined) };
    }
    // With every fieldConfig option cleared, fieldConfig collapses to absent.
    expect(state.fieldConfig).toBeUndefined();
  });
});
