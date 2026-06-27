// View types for the series read verbs the bridge exposes (`series.find`, `series.latest`). The host
// owns the wire shape; the page reads only what it shows. These mirror the REAL verb results the
// host-mediated bridge returns (see crates/host/src/ingest): `series.find` → `{ series: string[] }`,
// `series.latest` → `{ sample: Sample | null }`.

/** One facet of a `series.find` query — an exact `key:value`, or key-only when `value` is omitted.
 *  The host's `series.find` takes a `facets` array and intersects them over the tag graph. An EMPTY
 *  facets list returns nothing (a query must constrain something) — so the page searches by facet. */
export interface Facet {
  key: string;
  value?: unknown;
}

/** The raw `series.find` result the bridge forwards: the matching series NAMES in the workspace. */
export interface SeriesFindResult {
  series: string[];
}

/** One sample of a series, as `series.latest` returns it inside `{ sample }`. Permissive: the payload
 *  is arbitrary JSON (the platform carries heterogeneous series). */
export interface Sample {
  series?: string;
  producer?: string;
  ts?: number;
  seq?: number;
  payload?: unknown;
  [k: string]: unknown;
}

/** The raw `series.latest` result the bridge forwards: the newest committed sample, or `null`. */
export interface SeriesLatestResult {
  sample: Sample | null;
}

/** An async fetch lifecycle, so the page renders honest loading / error / data states. */
export type AsyncState<T> =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "error"; error: string }
  | { status: "ready"; data: T };
