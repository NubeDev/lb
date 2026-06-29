// Result-shape ↔ panel-type validation (viz chart-types scope, "Result-shape ↔ type validation"). The
// editor's viz picker offers only the views a target's data shape can HONESTLY fill — a scalar can be a
// stat/gauge/bargauge/piechart (collapsed by the visible `reduceOptions` bridge), rows-of-series can be
// a timeseries/barchart, tabular rows can be a table. The detector is CONSERVATIVE (chart-types scope,
// Risks: "offer a view only when the shape clearly supports it") — it never tempts a fabricated value.
//
// One responsibility: classify a frame's shape + map a shape to its honest view set. The numeric
// coercion is `num.ts`'s job; the picker reads `viewsForShape` to disable the rest.

import type { View } from "@/lib/dashboard";
import { rowNumber } from "./num";

/** The three shapes the picker discriminates (chart-types scope, Example flow). `unknown` = no data yet
 *  (offer nothing shape-specific; the picker stays permissive until data arrives). */
export type ResultShape = "scalar" | "series" | "table" | "unknown";

/** Classify a panel's rows. CONSERVATIVE:
 *  - empty → `unknown` (don't constrain the picker before data loads).
 *  - a single numeric row → `scalar` (one value: stat/gauge/bargauge/piechart honest).
 *  - many rows, all reducible to a number → `series` (a line/bars over time: timeseries/barchart).
 *  - rows with multiple columns (not a bare numeric value) → `table` (tabular: only a table is honest).
 *  A multi-column frame is NOT offered a gauge (you'd have to guess which number); a bare-number frame is
 *  NOT offered a table (a single value isn't a grid). reduceOptions is the explicit bridge for the
 *  scalar/series → single-value collapse, never an implicit guess. */
export function detectShape(rows: Array<Record<string, unknown>>): ResultShape {
  if (rows.length === 0) return "unknown";

  const numericRows = rows.filter((r) => rowNumber(r) !== null).length;
  const allNumeric = numericRows === rows.length;

  // Tabular: any row carries more than a single bare value column (multiple keys none of which is the
  // canonical numeric `value`/`payload`), OR the rows aren't uniformly reducible to a number.
  const multiColumn = rows.some((r) => {
    const keys = Object.keys(r);
    return keys.length > 1 && r.value === undefined && r.payload === undefined;
  });
  if (multiColumn || !allNumeric) return "table";

  return rows.length === 1 ? "scalar" : "series";
}

/** The canonical views a shape can honestly fill. A `scalar`/`series` also lists the single-stat panels
 *  because `reduceOptions` collapses them to the value those panels draw (the visible bridge). `table`
 *  is offered for any shape (a frame is always renderable as a grid) but a scalar/series-only view is
 *  withheld from a tabular frame. `unknown` offers everything (pre-data; the user picks, data validates
 *  on save). */
export function viewsForShape(shape: ResultShape): View[] {
  switch (shape) {
    case "scalar":
      // One value: the single-stat family + a table (a 1-row grid is honest). Not timeseries/barchart
      // (a single point isn't a series to draw).
      return ["stat", "gauge", "bargauge", "piechart", "table"];
    case "series":
      // Many reducible points: the series charts + the single-stat family (reduceOptions collapses) + table.
      return ["timeseries", "barchart", "stat", "gauge", "bargauge", "piechart", "table"];
    case "table":
      // Multi-column tabular: only a table is honest — a gauge/stat would have to guess a number.
      return ["table"];
    case "unknown":
    default:
      return ["timeseries", "barchart", "stat", "gauge", "bargauge", "piechart", "table"];
  }
}

/** Whether `view` is honestly fillable by `shape` (the picker disables the rest). Non-Grafana views
 *  (scripted/control/ext) are always allowed — they aren't shape-validated standard panels. */
export function viewFitsShape(view: View, shape: ResultShape): boolean {
  const standard: View[] = ["timeseries", "barchart", "stat", "gauge", "bargauge", "piechart", "table"];
  if (!standard.includes(view)) return true;
  return viewsForShape(shape).includes(view);
}
