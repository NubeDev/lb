// THE canonical plot specification — the ONE model that both chart surfaces share (the dashboard
// panels and the in-channel query results). A `PlotSpec` says *how* to draw a set of rows: which
// column is the x/category axis, which numeric columns are the y series, an optional column to split
// one y series into many, and the chart type. Both surfaces persist THIS shape (a dashboard cell in
// `options.plot`, a channel result in a per-user `chart_pref` record) so the picker, the renderer, and
// the auto-suggester never drift between contexts.
//
// One responsibility: the plot-spec type + its defaults/normaliser. No rows, no React, no I/O — the
// transform (`buildPlot`) and the field typing (`fieldKind`) live in their own files.

/** The chart shapes the shared renderer draws. `line`/`area`/`scatter` plot y-vs-x over an ordered
 *  (usually temporal or numeric) x; `bar` plots one group per distinct x category; `pie` draws one
 *  slice per category from a single y field; `histogram` bins one numeric field. */
export type PlotType = "line" | "area" | "bar" | "scatter" | "pie" | "histogram";

/** The full plot spec. `xField`/`yFields` reference column names in the rows. `seriesField` (optional)
 *  pivots a long frame into wide series — e.g. rows `{t, host, cpu}` with `seriesField:"host"` draw one
 *  line per host. `bins` is histogram-only; `horizontal`/`stacked`/`smooth` are draw modifiers. */
export interface PlotSpec {
  type: PlotType;
  xField: string;
  yFields: string[];
  seriesField?: string;
  stacked?: boolean;
  /** Histogram bucket count (ignored by other types). */
  bins?: number;
  /** Bar orientation — horizontal bars when true. */
  horizontal?: boolean;
  /** Curve the line/area (monotone) vs straight segments. */
  smooth?: boolean;
}

/** The plot types that need a categorical (non-continuous) x axis. */
export const CATEGORICAL_TYPES: ReadonlySet<PlotType> = new Set<PlotType>(["bar", "pie"]);

/** The plot types that plot y against a continuous, ordered x. */
export const CONTINUOUS_TYPES: ReadonlySet<PlotType> = new Set<PlotType>(["line", "area", "scatter"]);

/** A safe empty spec — a table-only fallback the renderer treats as "nothing plottable". */
export function emptyPlotSpec(): PlotSpec {
  return { type: "line", xField: "", yFields: [] };
}

/** True when a spec has enough to draw: a histogram needs one y field; a pie needs x + one y; every
 *  other type needs an x and at least one y. Guards the renderer against a half-built spec. */
export function isPlottable(spec: PlotSpec): boolean {
  if (spec.type === "histogram") return spec.yFields.length > 0;
  if (spec.type === "pie") return !!spec.xField && spec.yFields.length > 0;
  return !!spec.xField && spec.yFields.length > 0;
}

/** Coerce an unknown persisted blob back into a well-formed `PlotSpec`, dropping anything malformed.
 *  Used when reading `cell.options.plot` or a `chart_pref` record — never trust the stored shape. */
export function readPlotSpec(raw: unknown): PlotSpec | null {
  if (!raw || typeof raw !== "object") return null;
  const o = raw as Record<string, unknown>;
  const type = o.type;
  const valid: PlotType[] = ["line", "area", "bar", "scatter", "pie", "histogram"];
  if (typeof type !== "string" || !valid.includes(type as PlotType)) return null;
  const yFields = Array.isArray(o.yFields) ? o.yFields.filter((f): f is string => typeof f === "string") : [];
  return {
    type: type as PlotType,
    xField: typeof o.xField === "string" ? o.xField : "",
    yFields,
    seriesField: typeof o.seriesField === "string" && o.seriesField ? o.seriesField : undefined,
    stacked: typeof o.stacked === "boolean" ? o.stacked : undefined,
    bins: typeof o.bins === "number" && o.bins > 0 ? o.bins : undefined,
    horizontal: typeof o.horizontal === "boolean" ? o.horizontal : undefined,
    smooth: typeof o.smooth === "boolean" ? o.smooth : undefined,
  };
}
