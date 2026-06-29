// The Timeseries verb family — `rollup`/`lag`/`delta`/`rate`/`interpolate`/`gapfill`/`resample`. Mirrors
// `rust/crates/rules/src/verbs/timeseries.rs` exactly (rules-editor-ux scope). A first-class category
// (the user asked for it): each wraps a (ts, value) grid in a new outer query — nothing scans until a
// terminal. `resample` is `rollup`'s alias; `interpolate`/`gapfill` validate args (identity plan in v1).

import type { CatalogGroup } from "./catalog.types";

export const TIMESERIES_GROUP: CatalogGroup = {
  category: "timeseries",
  label: "Timeseries",
  blurb: "Bucket, aggregate, and shift a (ts, value) series.",
  entries: [
    { name: "rollup", signature: "rollup(every, agg)", summary: "Time-bucket + aggregate (avg/min/max/sum/count/first/last).", snippet: '.rollup("1h", "avg")', category: "timeseries" },
    { name: "lag", signature: "lag(col, n)", summary: "The value n rows back (a {col}_lag column).", snippet: '.lag("value", 1)', category: "timeseries" },
    { name: "delta", signature: "delta(col)", summary: "The change from the previous row (a {col}_delta column).", snippet: '.delta("value")', category: "timeseries" },
    { name: "rate", signature: "rate(col)", summary: "The per-step rate of change (a {col}_rate column).", snippet: '.rate("value")', category: "timeseries" },
    { name: "interpolate", signature: "interpolate(method)", summary: 'Fill gaps by method ("locf" | "none"); identity plan in v1.', snippet: '.interpolate("locf")', category: "timeseries" },
    { name: "gapfill", signature: "gapfill(every)", summary: "Validate a regular cadence; identity plan in v1.", snippet: '.gapfill("1h")', category: "timeseries" },
    { name: "resample", signature: "resample(every, agg)", summary: "Resample to a cadence (alias of rollup).", snippet: '.resample("1h", "avg")', category: "timeseries" },
  ],
};
