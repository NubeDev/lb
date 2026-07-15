//! The `Frame` verb surface — polars in the cage (data-stdlib-scope Phase 2). `g.frame()`
//! materializes a Grid through the EXISTING gated seam (`Grid::collect_json` → `DataSeam` — the
//! host re-runs workspace + caps checks there, so a Frame adds no new authority); `frame(records)`
//! builds one from an array of maps. Local compute only: a Frame never opens a connection or reads
//! a file (the cage stays zero-I/O; `f.sql` registers ONLY `self`). Bounded by
//! `RuleLimits::max_frame_rows`/`max_frame_cells` — the deadline cannot interrupt a native polars
//! call, so the bound moves to the inputs (enforced on construction AND on every frame-producing
//! output: join/vstack/pivot/sql included). The method bodies live in `lb-frame` (folder-of-verbs);
//! this file owns the seam wiring + the catalog rows.

use rhai::{Engine, EvalAltResult};

use crate::grid::{Grid, GridCtx};
use crate::sandbox::RuleLimits;
use lb_frame::FrameLimits;
use std::sync::Arc;

/// Register the `Frame` type, constructors (`frame(records)`, `g.frame()`), and methods. `_ctx`
/// stays in the signature for wiring symmetry, but the Grid carries its own collect context —
/// `g.frame()` reaches the seam through the grid itself, exactly like `g.records()` does.
pub fn register(engine: &mut Engine, _ctx: Arc<GridCtx>, limits: &RuleLimits) {
    let fl = FrameLimits {
        max_frame_rows: limits.max_frame_rows,
        max_frame_cells: limits.max_frame_cells,
        max_string_bytes: limits.max_string_bytes,
    };
    lb_frame::register(engine, &fl);
    engine.register_fn(
        "frame",
        move |g: &mut Grid| -> Result<lb_frame::Frame, Box<EvalAltResult>> {
            let grid = g.collect_json()?;
            lb_frame::frame_from_grid(&grid.columns, &grid.rows, &fl)
        },
    );
}

/// One catalog row in the `frame` family (compact form — the family never varies).
const fn e(
    name: &'static str,
    signature: &'static str,
    description: &'static str,
) -> crate::catalog::FnEntry {
    crate::catalog::FnEntry {
        name,
        family: "frame",
        signature,
        description,
    }
}

/// Catalog rows for the `frame` family — one per `register_fn` site (here + `lb-frame`'s
/// `register`, which this file wires in). Append-only within the family.
#[rustfmt::skip]
pub(crate) const CATALOG: &[crate::catalog::FnEntry] = &[
    // ---- construction ----
    e("frame", "frame(records: Array<Map>) -> Frame", "Build a Frame from an array of row maps (capped by max_frame_rows/max_frame_cells)."),
    e("g.frame", "frame(grid) -> Frame", "Materialize a Grid through the gated seam into an in-memory Frame (capped at max_frame_rows)."),
    // ---- inspect ----
    e("f.shape", "shape(f: Frame) -> Array", "The [rows, cols] shape of the frame."),
    e("f.height", "height(f: Frame) -> i64", "Row count."),
    e("f.width", "width(f: Frame) -> i64", "Column count."),
    e("f.columns", "columns(f: Frame) -> Array<String>", "The column names, in order."),
    e("f.dtypes", "dtypes(f: Frame) -> Map", "Column name -> dtype name."),
    e("f.head", "head(f: Frame, n: i64) -> Frame", "The first n rows."),
    e("f.tail", "tail(f: Frame, n: i64) -> Frame", "The last n rows."),
    e("f.slice", "slice(f: Frame, offset: i64, n: i64) -> Frame", "n rows starting at offset (negative offset counts from the end)."),
    e("f.describe", "describe(f: Frame) -> Frame", "Summary statistics (count/null_count/mean/std/min/max/median) per numeric column."),
    e("f.null_count", "null_count(f: Frame) -> Map", "Column name -> number of nulls."),
    e("f.is_empty", "is_empty(f: Frame) -> bool", "True when the frame has no rows."),
    // ---- shape ----
    e("f.select", "select(f: Frame, cols: Array<String>) -> Frame", "Project a subset of columns."),
    e("f.drop", "drop(f: Frame, cols: Array<String>) -> Frame", "Remove columns."),
    e("f.rename", "rename(f: Frame, from: String, to: String) -> Frame", "Rename a column."),
    e("f.with_col_from", "with_col_from(f: Frame, name: String, values: Array) -> Frame", "Add a column from a plain array (length must match the height)."),
    e("f.sort", "sort(f: Frame, col: String) -> Frame", "Sort ascending by a column (stable; nulls last)."),
    e("f.sort_desc", "sort(f: Frame, col: String, desc: bool) -> Frame", "Sort by a column, descending when desc is true (stable; nulls last)."),
    e("f.unique", "unique(f: Frame) -> Frame", "Drop duplicate rows (keeps the first occurrence)."),
    e("f.unique_by", "unique_by(f: Frame, cols: Array<String>) -> Frame", "Drop rows duplicated on the given columns (keeps the first)."),
    e("f.reverse", "reverse(f: Frame) -> Frame", "Reverse the row order."),
    // ---- filter ----
    e("f.filter_eq", "filter_eq(f: Frame, col: String, v) -> Frame", "Keep rows where col == v."),
    e("f.filter_ne", "filter_ne(f: Frame, col: String, v) -> Frame", "Keep rows where col != v."),
    e("f.filter_gt", "filter_gt(f: Frame, col: String, v) -> Frame", "Keep rows where col > v."),
    e("f.filter_ge", "filter_ge(f: Frame, col: String, v) -> Frame", "Keep rows where col >= v."),
    e("f.filter_lt", "filter_lt(f: Frame, col: String, v) -> Frame", "Keep rows where col < v."),
    e("f.filter_le", "filter_le(f: Frame, col: String, v) -> Frame", "Keep rows where col <= v."),
    e("f.filter_in", "filter_in(f: Frame, col: String, vs: Array) -> Frame", "Keep rows where col is one of vs."),
    e("f.filter_between", "filter_between(f: Frame, col: String, lo, hi) -> Frame", "Keep rows where lo <= col <= hi."),
    e("f.filter_null", "filter_null(f: Frame, col: String) -> Frame", "Keep rows where col is null."),
    e("f.filter_not_null", "filter_not_null(f: Frame, col: String) -> Frame", "Keep rows where col is not null."),
    e("f.sample", "sample(f: Frame, n: i64, seed: i64) -> Frame", "Deterministic sample of n rows without replacement (seed is mandatory)."),
    // ---- missing ----
    e("f.drop_nulls", "drop_nulls(f: Frame) -> Frame", "Drop rows containing any null."),
    e("f.drop_nulls_in", "drop_nulls(f: Frame, cols: Array<String>) -> Frame", "Drop rows with a null in the given columns."),
    e("f.fill_null", "fill_null(f: Frame, v) -> Frame", "Replace nulls in every column with v."),
    e("f.fill_null_strategy", "fill_null_strategy(f: Frame, col: String, strategy: String) -> Frame", "Fill nulls in a column by strategy: forward|backward|mean|zero."),
    // ---- aggregate ----
    e("f.mean", "mean(f: Frame, col: String) -> Dynamic", "Mean of a column (nulls skipped)."),
    e("f.median", "median(f: Frame, col: String) -> Dynamic", "Median of a column (nulls skipped)."),
    e("f.sum", "sum(f: Frame, col: String) -> Dynamic", "Sum of a column (nulls skipped)."),
    e("f.min", "min(f: Frame, col: String) -> Dynamic", "Minimum of a column (nulls skipped)."),
    e("f.max", "max(f: Frame, col: String) -> Dynamic", "Maximum of a column (nulls skipped)."),
    e("f.std", "std(f: Frame, col: String) -> Dynamic", "Sample standard deviation of a column (ddof 1; nulls skipped)."),
    e("f.variance", "variance(f: Frame, col: String) -> Dynamic", "Sample variance of a column (ddof 1; nulls skipped; named variance because var is a rhai keyword)."),
    e("f.quantile", "quantile(f: Frame, col: String, q: f64) -> Dynamic", "Linear-interpolated quantile of a column, q in 0.0..=1.0."),
    e("f.count", "count(f: Frame) -> i64", "Row count (same as height)."),
    e("f.n_unique", "n_unique(f: Frame, col: String) -> i64", "Number of distinct values in a column."),
    e("f.value_counts", "value_counts(f: Frame, col: String) -> Frame", "Distinct values of a column with their counts (most frequent first)."),
    // ---- group / combine ----
    e("f.group_agg", "group_agg(f: Frame, keys: Array<String>, aggs: Map) -> Frame", "Group by keys and aggregate: #{col: \"mean\"|\"sum\"|\"min\"|\"max\"|\"median\"|\"std\"|\"var\"|\"count\"|\"n_unique\"|\"first\"|\"last\"}."),
    e("f.join", "join(f: Frame, other: Frame, on: String, how: String) -> Frame", "Join two frames on a column: inner|left|outer|anti (output is cap-checked)."),
    e("f.vstack", "vstack(f: Frame, other: Frame) -> Frame", "Stack another frame's rows below (same columns; output is cap-checked)."),
    e("f.pivot", "pivot(f: Frame, idx: String, cols: String, vals: String, agg: String) -> Frame", "Wide reshape: one row per idx, one column per distinct cols value, cells aggregated from vals."),
    e("f.melt", "melt(f: Frame, ids: Array<String>, vals: Array<String>) -> Frame", "Long reshape: keep ids, fold vals into variable/value rows."),
    // ---- series ops (derivations add <col>_<op>; clip replaces in place) ----
    e("f.rolling_mean", "rolling_mean(f: Frame, col: String, w: i64) -> Frame", "Add <col>_rolling_mean over a w-row window (nulls until the window fills)."),
    e("f.rolling_sum", "rolling_sum(f: Frame, col: String, w: i64) -> Frame", "Add <col>_rolling_sum over a w-row window."),
    e("f.rolling_min", "rolling_min(f: Frame, col: String, w: i64) -> Frame", "Add <col>_rolling_min over a w-row window."),
    e("f.rolling_max", "rolling_max(f: Frame, col: String, w: i64) -> Frame", "Add <col>_rolling_max over a w-row window."),
    e("f.rolling_std", "rolling_std(f: Frame, col: String, w: i64) -> Frame", "Add <col>_rolling_std over a w-row window (ddof 1)."),
    e("f.ewm_mean", "ewm_mean(f: Frame, col: String, alpha: f64) -> Frame", "Add <col>_ewm_mean, exponentially weighted with alpha in (0, 1]."),
    e("f.diff", "diff(f: Frame, col: String) -> Frame", "Add <col>_diff: the row-over-row difference."),
    e("f.pct_change", "pct_change(f: Frame, col: String) -> Frame", "Add <col>_pct_change: the row-over-row fractional change."),
    e("f.cumsum", "cumsum(f: Frame, col: String) -> Frame", "Add <col>_cumsum: the running sum."),
    e("f.shift", "shift(f: Frame, col: String, n: i64) -> Frame", "Add <col>_shift: values moved down n rows (negative shifts up)."),
    e("f.rank", "rank(f: Frame, col: String) -> Frame", "Add <col>_rank: ascending average rank."),
    e("f.zscore", "zscore(f: Frame, col: String) -> Frame", "Add <col>_zscore: (x - mean) / std (ddof 1)."),
    e("f.clip", "clip(f: Frame, col: String, lo, hi) -> Frame", "Bound a column into [lo, hi] in place."),
    // ---- time ----
    e("f.bucket", "bucket(f: Frame, ts_col: String, dur: String) -> Frame", "Truncate an epoch timestamp column to a duration boundary (s/m/h/d/w; secs or ms per value)."),
    // ---- sql ----
    e("f.sql", "sql(f: Frame, query: String) -> Frame", "Run SQL over this frame only (registered as table 'self'; in-memory, no I/O)."),
    // ---- export ----
    e("f.records", "records(f: Frame) -> Array<Map>", "The rows as an array of maps (NaN/Inf come back as ())."),
    e("f.col", "col(f: Frame, name: String) -> Array", "One column as a plain array — feeds the stats functions."),
    e("f.to_grid_json", "to_grid_json(f: Frame) -> Map", "#{columns, rows} — the chart-ready grid shape."),
    e("f.to_csv_string", "to_csv_string(f: Frame) -> String", "The frame as CSV text (bounded by max_string_bytes)."),
    e("f.to_json_string", "to_json_string(f: Frame) -> String", "The frame as a JSON array of row objects (bounded by max_string_bytes)."),
];
