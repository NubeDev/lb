// View types for the series read verbs the bridge exposes (`series.find`, `series.latest`). Kept
// permissive: the host owns the wire shape; the page only reads what it shows.

/** One series the workspace exposes — the shape `series.find` returns rows of. */
export interface SeriesRef {
  id?: string;
  name?: string;
  tags?: string[];
  [k: string]: unknown;
}

/** A single latest sample for a series, as `series.latest` returns it. */
export interface LatestSample {
  series?: string;
  ts?: string | number;
  value?: number | string;
  [k: string]: unknown;
}

/** An async fetch lifecycle, so the page renders honest loading / error / data states. */
export type AsyncState<T> =
  | { status: "loading" }
  | { status: "error"; error: string }
  | { status: "ready"; data: T };
