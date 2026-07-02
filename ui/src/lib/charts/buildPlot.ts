// THE rows + PlotSpec → render-ready data transform, shared by both chart surfaces. It turns a raw
// query frame into exactly what Recharts consumes: a flat `data` array, the x key, and the list of
// series (each a data key + display name). It owns the three shape changes a real x/y chart needs and
// today's `rowNumber` collapse cannot express:
//   - multi-series wide frames (several y columns → several lines/bars),
//   - long→wide pivot (`seriesField` splits one y column into one series per distinct value),
//   - histogram binning and pie aggregation.
// Pure and datasource-agnostic; the numeric coercion is local so the charts lib has no dashboard dep.
//
// One responsibility: (rows, spec) → PlotFrame. Field typing is `fieldKind`; the SVG is `PlotChart`.

import type { PlotSpec } from "./plotSpec";

export interface PlotSeries {
  /** The data-array key holding this series' numeric value. */
  key: string;
  /** The human label (legend/tooltip) — the column name, or the pivoted category value. */
  name: string;
}

export interface PlotFrame {
  data: Array<Record<string, unknown>>;
  /** The data key for the x axis (`"__x"` for the synthesized category/bucket axis). */
  xKey: string;
  series: PlotSeries[];
}

const X_KEY = "__x";

function asNumber(value: unknown): number | null {
  if (typeof value === "number") return Number.isFinite(value) ? value : null;
  if (typeof value === "string" && value.trim() !== "") {
    const n = Number(value);
    return Number.isFinite(n) ? n : null;
  }
  if (typeof value === "boolean") return value ? 1 : 0;
  return null;
}

function xLabel(value: unknown): string {
  if (value == null) return "";
  if (value instanceof Date) return value.toISOString();
  return String(value);
}

/** Bin one numeric field into evenly-spaced buckets (histogram). The bucket label is the left edge. */
function histogram(rows: Array<Record<string, unknown>>, field: string, bins: number): PlotFrame {
  const nums = rows.map((r) => asNumber(r[field])).filter((n): n is number => n !== null);
  if (nums.length === 0) return { data: [], xKey: X_KEY, series: [{ key: "count", name: "count" }] };
  const min = Math.min(...nums);
  const max = Math.max(...nums);
  const width = (max - min) / bins || 1;
  const buckets = Array.from({ length: bins }, (_, i) => ({
    [X_KEY]: (min + i * width).toPrecision(3),
    count: 0,
  }));
  for (const n of nums) {
    const idx = Math.min(bins - 1, Math.max(0, Math.floor((n - min) / width)));
    (buckets[idx].count as number) += 1;
  }
  return { data: buckets, xKey: X_KEY, series: [{ key: "count", name: "count" }] };
}

/** Aggregate one y field per distinct x category (pie / bar-of-categories) — sum collides. */
function aggregate(rows: Array<Record<string, unknown>>, xField: string, yField: string): PlotFrame {
  const totals = new Map<string, number>();
  for (const row of rows) {
    const y = asNumber(row[yField]);
    if (y === null) continue;
    const name = xLabel(row[xField]);
    totals.set(name, (totals.get(name) ?? 0) + y);
  }
  const data = [...totals].map(([name, value]) => ({ [X_KEY]: name, value }));
  return { data, xKey: X_KEY, series: [{ key: "value", name: yField }] };
}

/** Pivot a long frame into wide series: one series per distinct `seriesField` value, carrying the first
 *  y field. Rows sharing an x value merge into one datum. */
function pivot(rows: Array<Record<string, unknown>>, spec: PlotSpec): PlotFrame {
  const yField = spec.yFields[0];
  const byX = new Map<string, Record<string, unknown>>();
  const seriesKeys = new Set<string>();
  for (const row of rows) {
    const y = asNumber(row[yField]);
    if (y === null) continue;
    const x = xLabel(row[spec.xField]);
    const seriesName = xLabel(row[spec.seriesField!]);
    seriesKeys.add(seriesName);
    let datum = byX.get(x);
    if (!datum) {
      datum = { [X_KEY]: row[spec.xField] };
      byX.set(x, datum);
    }
    datum[seriesName] = y;
  }
  return {
    data: [...byX.values()],
    xKey: X_KEY,
    series: [...seriesKeys].map((name) => ({ key: name, name })),
  };
}

/** Straight multi-series: each y field is its own series, keyed by column name, x carried verbatim. */
function wide(rows: Array<Record<string, unknown>>, spec: PlotSpec): PlotFrame {
  const data = rows.map((row) => {
    const datum: Record<string, unknown> = { [X_KEY]: row[spec.xField] };
    for (const y of spec.yFields) datum[y] = asNumber(row[y]);
    return datum;
  });
  return { data, xKey: X_KEY, series: spec.yFields.map((name) => ({ key: name, name })) };
}

/** Build the render-ready frame for `spec` from `rows`. Empty when the spec references missing fields. */
export function buildPlot(rows: Array<Record<string, unknown>>, spec: PlotSpec): PlotFrame {
  if (spec.type === "histogram") return histogram(rows, spec.yFields[0] ?? "", spec.bins ?? 12);
  if (spec.type === "pie") return aggregate(rows, spec.xField, spec.yFields[0] ?? "");
  if (spec.seriesField && spec.yFields.length > 0) return pivot(rows, spec);
  return wide(rows, spec);
}
