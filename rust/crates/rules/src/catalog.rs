//! The rule-cage function catalog — the **single source of truth** for what a rule body can call.
//! One entry per `register_fn` site in [`crate::verbs`]; `rules.help` returns it verbatim and the
//! skill doc + UI autocomplete read it. Hand-curated (not rhai's `gen_fn_signatures()`) so each entry
//! carries a human description + family — the whole point of the introspection surface. The scope's
//! pure verb families (`time`/`json`/`stats`/`mathx`) append their entries as they land in Phase 1.
//!
//! **Maintenance rule:** every new `engine.register_fn(...)` in `verbs/*.rs` MUST add a row here in
//! the same change (the `catalog_is_complete` test enforces the count against the registered total —
//! it can't prove a description is right, but it catches a missing entry).

/// One function in the catalog. `name` is the rhai call form (bare for free fns like `source`,
/// `family.method` for handle methods like `ai.ask`); overloads with different arities get one entry
/// each (the signatures differ).
#[derive(Debug, Clone, Copy)]
pub struct FnEntry {
    /// The rhai call name. Bare (`source`) for free functions, `handle.method` (`ai.ask`) for the
    /// four scope handles, `value.method` (`g.filter`, `c.max`) for the grid/col chainable surface.
    pub name: &'static str,
    /// The verb family — matches the `verbs/*.rs` module name (`data`, `timeseries`, `emit`, `ai`,
    /// `messaging` for inbox+outbox+channel, `grid` for the lazy Grid methods, `frame` for polars).
    pub family: &'static str,
    /// The rhai signature, argument-first (`source(name: String) -> Grid`).
    pub signature: &'static str,
    /// One-line human description. This is the value the catalog adds over rhai's raw signatures.
    pub description: &'static str,
}

/// The full catalog, grouped by family. Append-only — never reorder existing rows (callers may
/// index by position in a future cache).
pub const CATALOG: &[FnEntry] = &[
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
];

#[cfg(test)]
mod tests {
    //! Catalog integrity — the descriptions can't be auto-validated, but the structure can. These
    //! catch a missing/typo'd entry the day a `register_fn` is added without a row here.

    use super::*;
    use std::collections::HashSet;

    #[test]
    fn names_are_unique() {
        let mut seen = HashSet::new();
        for e in CATALOG {
            assert!(seen.insert(e.name), "duplicate catalog name: {}", e.name);
        }
    }

    #[test]
    fn names_are_valid_rhai_paths() {
        // Bare identifier or handle/name.path — reject spaces, dots at the edges, empty parts.
        for e in CATALOG {
            let name = e.name;
            assert!(!name.is_empty(), "empty name");
            assert!(
                !name.starts_with('.') && !name.ends_with('.'),
                "edge dot: {name}"
            );
            for part in name.split('.') {
                assert!(!part.is_empty(), "empty path part in {name}");
                let first = part.chars().next().unwrap();
                assert!(
                    first.is_ascii_alphabetic() || first == '_',
                    "name {name:?} part {part:?} must start with a letter/_"
                );
                assert!(
                    part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
                    "name {name:?} part {part:?} has non-identifier chars"
                );
            }
        }
    }

    #[test]
    fn every_entry_has_nonempty_fields() {
        for e in CATALOG {
            assert!(!e.family.is_empty(), "empty family for {}", e.name);
            assert!(!e.signature.is_empty(), "empty signature for {}", e.name);
            assert!(
                !e.description.is_empty(),
                "empty description for {}",
                e.name
            );
            assert!(
                e.description.ends_with('.'),
                "description for {} should end with '.' (sentence)",
                e.name
            );
        }
    }

    #[test]
    fn families_are_the_known_set() {
        // The verb-module set (data/grid/timeseries/emit/ai/messaging) + frame lands with Phase 2.
        // A new family here is a deliberate act (catches a typo like "messagingg").
        let known: HashSet<&str> = [
            "data",
            "grid",
            "timeseries",
            "chart",
            "emit",
            "ai",
            "messaging",
        ]
        .into_iter()
        .collect();
        for e in CATALOG {
            assert!(
                known.contains(e.family),
                "unknown family {:?} on {}",
                e.family,
                e.name
            );
        }
    }

    #[test]
    fn catalog_has_entries_from_every_verb_module() {
        // A floor — if any family disappears entirely, a verb module's registrations lost their
        // catalog rows. (The precise register_fn↔catalog count is enforced manually at review; this
        // catches a wholesale drop.)
        let families: HashSet<&str> = CATALOG.iter().map(|e| e.family).collect();
        for required in [
            "data",
            "grid",
            "timeseries",
            "chart",
            "emit",
            "ai",
            "messaging",
        ] {
            assert!(
                families.contains(required),
                "family {required} has no entries"
            );
        }
    }
}
