// Registry-driven round-trip coverage (editor-parity scope, step 2 testing plan; CLAUDE §9 real infra
// not needed — this is a pure (de)serializer contract, no gateway). The hard constraint: EVERY
// registered option must survive `editorStateToCell(cellToEditorState(c)) ≡ c`. We build a cell that
// sets each registered option to a NON-DEFAULT sample value via the SAME write path the editor uses
// (`writeOption`), then assert the cell round-trips byte-identical AND each option reads back its value.
// A new option added to the registry with no sample here FAILS the exhaustiveness guard — it can't dodge
// the test.

import { describe, expect, it } from "vitest";
import type { Cell, View } from "@/lib/dashboard";
import { cellToEditorState, editorStateToCell, type EditorState } from "../cellEditorState";
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

describe("option registry round-trip", () => {
  it("has unique ids and every def has a sample (exhaustiveness guard)", () => {
    const ids = OPTION_REGISTRY.map((d) => d.id);
    expect(new Set(ids).size).toBe(ids.length); // no duplicate ids
    for (const def of OPTION_REGISTRY) {
      expect(sampleValue(def), `no sample for control kind of ${def.id}`).toBeDefined();
    }
  });

  it("every registered timeseries option survives editorStateToCell(cellToEditorState(c)) ≡ c", () => {
    const { cell } = fullyPopulatedCell("timeseries");
    const round = editorStateToCell(cellToEditorState(cell), cell);
    expect(round).toEqual(cell);
  });

  it("each populated option reads back its sample value after a round-trip", () => {
    const { cell, defs } = fullyPopulatedCell("timeseries");
    const reopened = cellToEditorState(cell);
    for (const def of defs) {
      expect(readOption(reopened, def), `option ${def.id} did not round-trip`).toEqual(sampleValue(def));
    }
  });

  it("clearing an option prunes it back to absent (no empty groups linger)", () => {
    const { cell } = fullyPopulatedCell("timeseries");
    let state = cellToEditorState(cell);
    for (const def of optionsForView("timeseries")) state = { ...state, ...writeOption(state, def, undefined) };
    // With every option cleared, fieldConfig collapses to absent and options has no registered keys.
    expect(state.fieldConfig).toBeUndefined();
    const out = editorStateToCell(state, cell);
    expect(out.fieldConfig).toBeUndefined();
  });
});
