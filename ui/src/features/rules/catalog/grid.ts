// The Grid verb family — the chainable grid plan-builders + the `Col` reductions. Mirrors
// `rust/crates/rules/src/verbs/mod.rs` (`register_grid_methods`) exactly (rules-editor-ux scope). The
// builders compose lazily over a grid (nothing scans until a terminal); the reductions collapse a single
// column to a scalar.

import type { CatalogGroup } from "./catalog.types";

export const GRID_GROUP: CatalogGroup = {
  category: "grid",
  label: "Grid",
  blurb: "Chainable grid ops + single-column reductions.",
  entries: [
    { name: "filter", signature: "filter(expr)", summary: "Keep rows matching a boolean expression.", snippet: '.filter("value > 5.0")', category: "grid" },
    { name: "select", signature: "select([cols])", summary: "Project a subset of columns.", snippet: '.select(["ts", "value"])', category: "grid" },
    { name: "add_col", signature: "add_col(name, expr)", summary: "Add a computed column.", snippet: '.add_col("f", "value * 1.8 + 32")', category: "grid" },
    { name: "rename", signature: "rename(from, to)", summary: "Rename a column.", snippet: '.rename("value", "temp")', category: "grid" },
    { name: "group_by", signature: "group_by([keys])", summary: "Group rows by key columns (then agg).", snippet: '.group_by(["point"])', category: "grid" },
    { name: "join", signature: "join(other, on, how)", summary: "Join another grid on a column.", snippet: '.join(other, "ts", "inner")', category: "grid" },
    { name: "col", signature: "col(name)", summary: "Pick a single column for a reduction.", snippet: '.col("value")', category: "grid" },
    { name: "head", signature: "head(n)", summary: "Take the first n rows.", snippet: ".head(20)", category: "grid" },
    { name: "size", signature: "size()", summary: "The row count of the grid.", snippet: ".size()", category: "grid" },
    { name: "columns", signature: "columns()", summary: "The grid's column names.", snippet: ".columns()", category: "grid" },
    { name: "records", signature: "records()", summary: "Materialize the grid as an array of row maps.", snippet: ".records()", category: "grid" },
    { name: "agg", signature: "agg([exprs])", summary: "Aggregate a grouped grid.", snippet: '.agg(["avg(value)"])', category: "grid" },
    { name: "max", signature: "max()", summary: "Maximum of the selected column.", snippet: ".max()", category: "grid" },
    { name: "min", signature: "min()", summary: "Minimum of the selected column.", snippet: ".min()", category: "grid" },
    { name: "avg", signature: "avg()", summary: "Mean of the selected column.", snippet: ".avg()", category: "grid" },
    { name: "mean", signature: "mean()", summary: "Mean of the selected column (alias of avg).", snippet: ".mean()", category: "grid" },
    { name: "sum", signature: "sum()", summary: "Sum of the selected column.", snippet: ".sum()", category: "grid" },
    { name: "count", signature: "count()", summary: "Count of the selected column.", snippet: ".count()", category: "grid" },
    { name: "std", signature: "std()", summary: "Standard deviation of the selected column.", snippet: ".std()", category: "grid" },
    { name: "first", signature: "first()", summary: "First value of the selected column.", snippet: ".first()", category: "grid" },
    { name: "last", signature: "last()", summary: "Last value of the selected column.", snippet: ".last()", category: "grid" },
    { name: "p", signature: "p(pct)", summary: "The pct-th percentile of the selected column.", snippet: ".p(95)", category: "grid" },
  ],
};
