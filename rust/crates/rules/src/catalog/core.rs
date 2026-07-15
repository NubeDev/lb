//! The **core verb families**' catalog rows — `data`, `grid`, `timeseries`, `chart`, `emit`, `ai`,
//! `messaging` (inbox+outbox+channel), and `insight`: the pre-stdlib surface ported from
//! rubix-cube, whose `register_fn` sites live across `verbs/mod.rs` + `verbs/*.rs` rather than in
//! one family file. The data-stdlib families (`time`/`json`/`stats`/`mathx`/`job`/`frame`) each
//! keep their rows beside their own registrations; [`CATALOG`](super::CATALOG) chains them all.
//!
//! Append-only — never reorder existing rows (callers may index by position in a future cache).

use super::FnEntry;

/// The pre-stdlib core families (data/grid/timeseries/chart/emit/ai/messaging/insight), grouped by
/// family. Append-only — never reorder existing rows (callers may index by position in a future
/// cache).
pub(super) const CORE: &[FnEntry] = &[
    // ---- data (verbs/data.rs): how rows ENTER the cage (gated by the allowlist + seam) ----
    FnEntry {
        name: "source",
        family: "data",
        signature: "source(name: String) -> Grid",
        description: "Open a granted source as a lazy Grid (uniform entry; the seam picks platform vs federation).",
    },
    FnEntry {
        name: "query",
        family: "data",
        signature: "query(source: String, sql: String) -> Grid",
        description: "Hand-written SQL against a named granted source (re-validated at collect).",
    },
    FnEntry {
        name: "history",
        family: "data",
        signature: "history(source, point, span) -> Grid  |  history(source, point, \"24h\") -> Grid",
        description: "Timeseries sugar: the (ts, value) rows of a point within a window.",
    },
    FnEntry {
        name: "span",
        family: "data",
        signature: "span(s: String) -> Span",
        description: "Typed duration window (validated s/m/h/d/w) for history().",
    },
    FnEntry {
        name: "last",
        family: "data",
        signature: "last(s: String) -> Span",
        description: "Alias of span() — a typed window (\"last 7 days\" readability).",
    },
    FnEntry {
        name: "param",
        family: "data",
        signature: "param(name: String) -> Dynamic",
        description: "Read a bound rule parameter by name (also pushed as a scope var).",
    },
    // ---- grid (verbs/mod.rs): the lazy plan-builders + Col reductions (chainable) ----
    FnEntry {
        name: "g.filter",
        family: "grid",
        signature: "filter(grid, expr: String) -> Grid",
        description: "Append a WHERE to the plan (lazy; nothing scans until records/return).",
    },
    FnEntry {
        name: "g.select",
        family: "grid",
        signature: "select(grid, cols: Array) -> Grid",
        description: "Project a subset of columns.",
    },
    FnEntry {
        name: "g.add_col",
        family: "grid",
        signature: "add_col(grid, name: String, expr: String) -> Grid",
        description: "Add a computed column from a SQL expression.",
    },
    FnEntry {
        name: "g.rename",
        family: "grid",
        signature: "rename(grid, from: String, to: String) -> Grid",
        description: "Rename a column.",
    },
    FnEntry {
        name: "g.group_by",
        family: "grid",
        signature: "group_by(grid, keys: Array) -> GroupedGrid",
        description: "Start a GROUP BY (pair with agg()).",
    },
    FnEntry {
        name: "g.join",
        family: "grid",
        signature: "join(grid, other: Grid, on: String, how: String) -> Grid",
        description: "Join two grids from the same source kind (inner/left/right/full).",
    },
    FnEntry {
        name: "g.col",
        family: "grid",
        signature: "col(grid, name: String) -> Col",
        description: "Grab one column for a reduction (max/avg/...).",
    },
    FnEntry {
        name: "g.head",
        family: "grid",
        signature: "head(grid, n: i64) -> Grid",
        description: "Take the first n rows (LIMIT).",
    },
    FnEntry {
        name: "g.size",
        family: "grid",
        signature: "size(grid) -> i64",
        description: "Row count (collects a SELECT count()).",
    },
    FnEntry {
        name: "g.columns",
        family: "grid",
        signature: "columns(grid) -> Array<String>",
        description: "The grid's column names (collects head(1)).",
    },
    FnEntry {
        name: "g.records",
        family: "grid",
        signature: "records(grid) -> Array<Map>",
        description: "Materialize the grid into an array of row maps (the bounded escape hatch).",
    },
    FnEntry {
        name: "grouped.agg",
        family: "grid",
        signature: "agg(grouped: GroupedGrid, exprs: Array) -> Grid",
        description: "Apply aggregate expressions to a group_by (turns it back into a Grid).",
    },
    FnEntry {
        name: "c.max",
        family: "grid",
        signature: "max(col: Col) -> Dynamic",
        description: "Reduce a column to its max (collects).",
    },
    FnEntry {
        name: "c.min",
        family: "grid",
        signature: "min(col: Col) -> Dynamic",
        description: "Reduce a column to its min.",
    },
    FnEntry {
        name: "c.avg",
        family: "grid",
        signature: "avg(col: Col) -> Dynamic",
        description: "Reduce a column to its mean (alias: mean).",
    },
    FnEntry {
        name: "c.mean",
        family: "grid",
        signature: "mean(col: Col) -> Dynamic",
        description: "Reduce a column to its mean (alias of avg).",
    },
    FnEntry {
        name: "c.sum",
        family: "grid",
        signature: "sum(col: Col) -> Dynamic",
        description: "Reduce a column to its sum.",
    },
    FnEntry {
        name: "c.count",
        family: "grid",
        signature: "count(col: Col) -> Dynamic",
        description: "Reduce a column to its row count.",
    },
    FnEntry {
        name: "c.std",
        family: "grid",
        signature: "std(col: Col) -> Dynamic",
        description: "Reduce a column to its standard deviation.",
    },
    FnEntry {
        name: "c.first",
        family: "grid",
        signature: "first(col: Col) -> Dynamic",
        description: "The first value of a column.",
    },
    FnEntry {
        name: "c.last",
        family: "grid",
        signature: "last(col: Col) -> Dynamic",
        description: "The last value of a column.",
    },
    FnEntry {
        name: "c.p",
        family: "grid",
        signature: "p(col: Col, pct: i64) -> Dynamic",
        description: "The pct-th percentile of a column.",
    },
    // ---- timeseries (verbs/timeseries.rs): plan-builders over a (ts, value) grid ----
    FnEntry {
        name: "rollup",
        family: "timeseries",
        signature: "rollup(grid, every: String, agg: String) -> Grid",
        description: "Time-bucket + aggregate (avg/min/max/sum/count/last/first).",
    },
    FnEntry {
        name: "lag",
        family: "timeseries",
        signature: "lag(grid, col: String, n: i64) -> Grid",
        description: "Shift a column back n rows over ts order (window function).",
    },
    FnEntry {
        name: "delta",
        family: "timeseries",
        signature: "delta(grid, col: String) -> Grid",
        description: "Per-row difference of a column (col - LAG(col)).",
    },
    FnEntry {
        name: "rate",
        family: "timeseries",
        signature: "rate(grid, col: String) -> Grid",
        description: "Per-row rate of change (col - LAG(col)) over ts order.",
    },
    FnEntry {
        name: "interpolate",
        family: "timeseries",
        signature: "interpolate(grid, method: String) -> Grid",
        description: "LOCF/identity interpolation (v1 identity plan; re-points to Frame in Phase 2).",
    },
    FnEntry {
        name: "gapfill",
        family: "timeseries",
        signature: "gapfill(grid, every: String) -> Grid",
        description: "Validate a cadence; v1 identity plan (regular-grid gapfill deferred to the store/Frame).",
    },
    FnEntry {
        name: "resample",
        family: "timeseries",
        signature: "resample(grid, every: String, agg: String) -> Grid",
        description: "Alias of rollup — resample at a cadence with an aggregate.",
    },
    // ---- chart (verbs/chart.rs): make a rule's rows chart-shaped (pure, over collected rows) ----
    FnEntry {
        name: "timeseries",
        family: "chart",
        signature: "timeseries(rows, ts) -> Array  |  timeseries(rows, ts, keep: Array) -> Array",
        description: "Normalize the named column to epoch-ms, rename it `time`, sort ascending; the 3-arg form trims to `time` + kept value columns.",
    },
    FnEntry {
        name: "wide",
        family: "chart",
        signature: "wide(rows: Array, ts: String, series: String, value: String) -> Array",
        description: "Long→wide pivot: one row per timestamp, one numeric column per distinct series (multi-line shape).",
    },
    FnEntry {
        name: "category",
        family: "chart",
        signature: "category(rows: Array, name: String, value: String) -> Array",
        description: "Trim to one label column + one numeric column (the bar/pie shape).",
    },
    // ---- emit (verbs/emit.rs): findings + log lines ----
    FnEntry {
        name: "emit",
        family: "emit",
        signature: "emit(map: Map) -> ()",
        description: "Record a finding (level + data) for the run output.",
    },
    FnEntry {
        name: "alert",
        family: "emit",
        signature: "alert(map: Map) -> ()",
        description: "Record a finding marked alert:true (host routes it to inbox + outbox).",
    },
    FnEntry {
        name: "log",
        family: "emit",
        signature: "log(msg: String) -> ()",
        description: "Record an info log line for the run output.",
    },
    // ---- ai (verbs/ai.rs): the AI-gateway handle (metered + nsql-fenced) ----
    FnEntry {
        name: "ai.ask",
        family: "ai",
        signature: "ask(ai, question: String) -> Grid",
        description: "Propose SQL from the model, then re-validate it through the data seam (the fence).",
    },
    FnEntry {
        name: "ai.complete",
        family: "ai",
        signature: "complete(ai, prompt: String [, context: String | grid: Grid]) -> String",
        description: "Free-form completion (optional context string OR bounded grid rows). Metered.",
    },
    FnEntry {
        name: "ai.classify",
        family: "ai",
        signature: "classify(ai, grid: Grid, labels: Array) -> Array<Map>",
        description: "Attach a per-row label from the model (rejects an over-large grid before charging).",
    },
    FnEntry {
        name: "ai.embed",
        family: "ai",
        signature: "embed(ai, text: String) -> Array<Float>",
        description: "An embedding vector for the text (errors 'not supported' if the backend has none).",
    },
    // ---- messaging: inbox (verbs/inbox.rs) ----
    FnEntry {
        name: "inbox.list",
        family: "messaging",
        signature: "list(inbox, channel: String) -> Array",
        description: "Read a channel's attention items (uncharged read).",
    },
    FnEntry {
        name: "inbox.record",
        family: "messaging",
        signature: "record(inbox, item: Map) -> ()",
        description: "Raise an attention item (charged write; deterministic id if omitted).",
    },
    FnEntry {
        name: "inbox.resolve",
        family: "messaging",
        signature: "resolve(inbox, item_id: String, decision: String) -> ()",
        description: "Close an item (approved/rejected/deferred). Charged write.",
    },
    FnEntry {
        name: "inbox.request_approval",
        family: "messaging",
        signature: "request_approval(inbox, req: Map) -> String",
        description: "Raise a needs:approval item + stage its held on_approve effect (rules-approvals).",
    },
    // ---- messaging: outbox (verbs/outbox.rs) ----
    FnEntry {
        name: "outbox.enqueue",
        family: "messaging",
        signature: "enqueue(outbox, effect: Map) -> ()",
        description: "Stage a must-deliver effect (charged write; deterministic id if omitted).",
    },
    FnEntry {
        name: "outbox.status",
        family: "messaging",
        signature: "status(outbox [, id: String]) -> Dynamic",
        description: "Read the queue status (whole workspace, or one effect by id). Uncharged read.",
    },
    // ---- messaging: channel (verbs/channel.rs) ----
    FnEntry {
        name: "channel.post",
        family: "messaging",
        signature: "post(channel, cid: String, item: Map) -> Dynamic",
        description: "Post a chat item (text only — a rule can't spawn agent/query runs). Charged.",
    },
    FnEntry {
        name: "channel.history",
        family: "messaging",
        signature: "history(channel, cid: String [, n: i64]) -> Array",
        description: "Read the last n items of a channel (n<=0 = whole history). Uncharged read.",
    },
    FnEntry {
        name: "channel.edit",
        family: "messaging",
        signature: "edit(channel, cid: String, mid: String, patch: Map) -> Dynamic",
        description: "Edit a message body (charged; idempotent on mid).",
    },
    FnEntry {
        name: "channel.delete",
        family: "messaging",
        signature: "delete(channel, cid: String, mid: String) -> ()",
        description: "Delete a message (charged; idempotent on mid).",
    },
    FnEntry {
        name: "channel.list",
        family: "messaging",
        signature: "list(channel) -> Array",
        description: "List the workspace's channels. Uncharged read.",
    },
    // ---- insight (verbs/insight.rs): the rule producer door onto the insight plane ----
    FnEntry {
        name: "insight.raise",
        family: "insight",
        signature: "raise(insight, item: Map) -> String",
        description: "Raise (or dedup-upsert) a durable insight; returns its id. Charged write; no-op on a route:false panel run.",
    },
    FnEntry {
        name: "insight.ack",
        family: "insight",
        signature: "ack(insight, id: String) -> ()",
        description: "Acknowledge an insight (open -> acked). Charged write; no-op on a route:false panel run.",
    },
    FnEntry {
        name: "insight.close",
        family: "insight",
        signature: "close(insight, id: String [, note: String]) -> ()",
        description: "Close an insight (maps to the insight.resolve verb: * -> resolved). Charged write; no-op on a route:false panel run.",
    },
];
