// THE shared frame→value reducer (viz chart-types scope: "reduceOptions {calcs,fields,values,limit} is
// the shared frame→value reducer for the single-stat-style panels — stat/gauge/bargauge/piechart").
// It collapses a panel's rows (a frame) to the single value(s) those panels draw. This is the explicit,
// VISIBLE bridge from frames to single values — never an implicit "guess a number" (chart-types scope,
// Risks: "reduceOptions must be the explicit bridge"). It is NOT the transform pipeline (invariant B):
// no joins/filters/groupBy — just the per-field reduce a single-stat panel needs.
//
// One responsibility: rows + a `ReduceDataOptions` → reduced value(s). The numeric coercion is `num.ts`'s
// job; the calc set is shared with the timeseries legend (`Legend.tsx` reuses `reduceCalc`).

import { rowNumber } from "./num";

/** Grafana's `ReduceDataOptions` (common.gen.ts) — names + default verbatim. `calcs` picks the reducer(s);
 *  `values` shows every value instead of reducing; `limit` caps the value count; `fields` is a field
 *  matcher (Phase 1 single-field panels ignore it — the bridge reduces the numeric `value` field). */
export interface ReduceDataOptions {
  /** When !values, pick one value for the whole field. */
  calcs: string[];
  /** Which fields to show (Grafana matcher string). Single-field Phase-2 panels reduce the value field. */
  fields?: string;
  /** Show every value instead of reducing to one. */
  values?: boolean;
  /** If showing all values, cap the count. */
  limit?: number;
}

/** Grafana's default — `calcs:[]` (the renderer falls back to `lastNotNull` when empty, Grafana's
 *  effective default for a single-stat panel). */
export function defaultReduceOptions(): ReduceDataOptions {
  return { calcs: [] };
}

/** Read an opaque blob as `ReduceDataOptions`, filling the Grafana default. */
export function readReduceOptions(raw: unknown): ReduceDataOptions {
  const o = (raw ?? {}) as Partial<ReduceDataOptions>;
  return { calcs: Array.isArray(o.calcs) ? o.calcs : [], fields: o.fields, values: o.values, limit: o.limit };
}

/** Apply one reducer calc over numeric points. Pure; formatting happens after, through the one bridge.
 *  Shared with the timeseries legend so the calc set never drifts between the legend and reduceOptions. */
export function reduceCalc(calc: string, points: number[]): number | null {
  if (points.length === 0) return null;
  switch (calc) {
    case "mean":
      return points.reduce((a, b) => a + b, 0) / points.length;
    case "max":
      return Math.max(...points);
    case "min":
      return Math.min(...points);
    case "sum":
      return points.reduce((a, b) => a + b, 0);
    case "count":
      return points.length;
    case "first":
    case "firstNotNull":
      return points[0];
    case "last":
    case "lastNotNull":
      return points[points.length - 1];
    default:
      return null;
  }
}

/** One reduced value of a frame (the single-stat value). The reducer = the first `calcs` entry, else
 *  Grafana's effective default `lastNotNull`. Returns `null` for an empty/non-numeric frame so the panel
 *  shows an honest "no value" — never a fabricated 0 (the no-mock rule). */
export function reduceFrame(rows: Array<Record<string, unknown>>, opts: ReduceDataOptions): number | null {
  const points = rows.map(rowNumber).filter((n): n is number => n !== null);
  if (points.length === 0) return null;
  const calc = opts.calcs[0] ?? "lastNotNull";
  return reduceCalc(calc, points);
}

/** Named categories of a frame (for piechart/barchart) — one `{name, value}` per row. The `name` is a
 *  string/label column when the row has one (a `name`/`label`/`category`/`metric` key, else the first
 *  non-numeric key), else the row index. The `value` is the row's numeric value (skipped when non-numeric
 *  — never a fabricated 0). Capped by `limit`. This is the explicit per-row collapse a categorical panel
 *  draws; like `reduceFrame`, it never guesses a value. */
export interface FrameCategory {
  name: string;
  value: number;
}
const NAME_KEYS = ["name", "label", "category", "metric", "key", "series"];
export function frameCategories(
  rows: Array<Record<string, unknown>>,
  opts: ReduceDataOptions,
): FrameCategory[] {
  const out: FrameCategory[] = [];
  for (let i = 0; i < rows.length; i++) {
    const v = rowNumber(rows[i]);
    if (v === null) continue;
    out.push({ name: categoryName(rows[i], i), value: v });
  }
  const limit = typeof opts.limit === "number" && opts.limit >= 0 ? opts.limit : undefined;
  return limit === undefined ? out : out.slice(0, limit);
}

function categoryName(row: Record<string, unknown>, index: number): string {
  for (const k of NAME_KEYS) {
    if (typeof row[k] === "string" && (row[k] as string).length > 0) return row[k] as string;
  }
  for (const [k, val] of Object.entries(row)) {
    if (k === "value" || k === "payload") continue;
    if (typeof val === "string" && val.length > 0) return val;
  }
  return String(index);
}

/** Every value of a frame, in row order, capped by `limit` — for `values:true` single-stat panels
 *  (a bargauge/piechart that draws one bar/slice per value rather than one reduced value). */
export function reduceFrameValues(rows: Array<Record<string, unknown>>, opts: ReduceDataOptions): number[] {
  const points = rows.map(rowNumber).filter((n): n is number => n !== null);
  if (!opts.values) {
    const one = reduceFrame(rows, opts);
    return one == null ? [] : [one];
  }
  return typeof opts.limit === "number" && opts.limit >= 0 ? points.slice(0, opts.limit) : points;
}
