// The auto-suggester — pick a sensible default `PlotSpec` from a query frame so a chart appears the
// instant a query returns, before the user touches the builder. It is the TS twin of the host's
// `pick_chart` (rust/crates/host/src/channel/chart.rs): temporal-x → line, categorical-x → bar, a lone
// numeric column → histogram. Keeping the rule here (over the shared field typing) means the dashboard
// and the channel result open on the SAME default, and the user only reaches for the builder to change
// it, never to make the first chart appear.
//
// One responsibility: fields (+ row count) → a starting PlotSpec (or null when nothing is plottable).

import type { PlotSpec } from "./plotSpec";
import { inferFields, numericFields, type FieldInfo } from "./fieldKind";

/** Suggest a starting spec from already-typed fields. Returns null when there's no numeric column to
 *  plot (the caller shows the table only — never a fabricated chart). */
export function suggestFromFields(fields: FieldInfo[], rowCount: number): PlotSpec | null {
  const nums = numericFields(fields);
  if (nums.length === 0) return null;

  const time = fields.find((f) => f.kind === "time");
  const category = fields.find((f) => f.kind === "category");

  // A single numeric column with no axis and enough rows → a histogram of its distribution.
  if (nums.length === 1 && !time && !category && rowCount >= 4) {
    const bins = Math.max(5, Math.min(20, Math.round(Math.sqrt(rowCount))));
    return { type: "histogram", xField: "", yFields: nums, bins };
  }

  // Temporal x → a line over time; the numeric columns (minus the x) are the series.
  if (time) {
    const ys = nums.filter((n) => n !== time.name);
    if (ys.length > 0) return { type: "line", xField: time.name, yFields: ys, smooth: true };
  }

  // Categorical x → a bar per category.
  if (category) {
    const ys = nums.filter((n) => n !== category.name);
    if (ys.length > 0) return { type: "bar", xField: category.name, yFields: ys };
  }

  // Numeric-only frame: plot the rest against the first numeric column (a scatter-ish line).
  const [x, ...ys] = nums;
  if (ys.length > 0) return { type: "line", xField: x, yFields: ys, smooth: true };
  return null;
}

/** Convenience: infer fields from rows, then suggest. */
export function suggestPlot(rows: Array<Record<string, unknown>>): PlotSpec | null {
  return suggestFromFields(inferFields(rows), rows.length);
}
