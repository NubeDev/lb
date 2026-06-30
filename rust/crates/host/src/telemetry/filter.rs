//! The telemetry query-filter → SurrealQL WHERE codec (telemetry-console scope). One place owns the
//! mapping from the console's composable filters (source / actor / level / outcome / trace_id /
//! free-text over msg / time range) to a bounded, bind-parameterized SurrealQL fragment, so the
//! filter and the read verb cannot drift. **The caller's `ws` is appended unconditionally** — the
//! filter is a *refinement* of the wall, never a substitute for it (a ws-B query can never name a
//! ws-A row, no matter what it passes here).
//!
//! Level is matched as a **minimum severity** (the scope's "level ≥ X"): `min_level = warn` matches
//! `warn` and `error`. Free-text is a case-insensitive substring on `msg` (a `string::toLowerCase`
//! containment — the cap keeps the ring small enough that this stays cheap; an index is a noted
//! follow-up for a very busy node).

use lb_telemetry::Level;

/// The page size ceiling — a snapshot query returns at most this many rows per call (the cap keeps
/// the ring bounded; this keeps one call bounded).
pub const MAX_PAGE: usize = 200;

/// The composable console filters. All optional; `ws` is added by the read verb, NOT here.
#[derive(Debug, Clone, Default)]
pub struct QueryFilter {
    pub source: Option<String>,
    pub actor: Option<String>,
    pub min_level: Option<Level>,
    pub outcome: Option<String>,
    pub trace_id: Option<String>,
    pub text: Option<String>,
    /// Inclusive lower bound on `ts` (milliseconds-since-epoch, the host's logical clock).
    pub since: Option<u64>,
    /// Exclusive upper bound on `ts`.
    pub until: Option<u64>,
}

impl QueryFilter {
    /// Build the `WHERE` fragment + bind pairs. `ws` is prepended as the FIRST, unconditional clause
    /// — the hard wall. `seq` is added by the caller for cursor paging (NOT here).
    pub fn where_clause(&self, ws: &str) -> (String, Vec<(String, serde_json::Value)>) {
        let mut clauses = vec!["ws = $ws".to_string()];
        let mut binds = vec![("ws".into(), serde_json::Value::String(ws.to_string()))];

        if let Some(s) = &self.source {
            clauses.push("source = $source".into());
            binds.push(("source".into(), serde_json::Value::String(s.clone())));
        }
        if let Some(a) = &self.actor {
            clauses.push("actor = $actor".into());
            binds.push(("actor".into(), serde_json::Value::String(a.clone())));
        }
        if let Some(lvl) = self.min_level {
            // level ≥ X = any of the levels at-or-above X in the severity order.
            let set: Vec<serde_json::Value> = levels_at_or_above(lvl)
                .into_iter()
                .map(|l| serde_json::Value::String(l.as_str().into()))
                .collect();
            clauses.push("level IN $levels".into());
            binds.push(("levels".into(), serde_json::Value::Array(set)));
        }
        if let Some(o) = &self.outcome {
            clauses.push("outcome = $outcome".into());
            binds.push(("outcome".into(), serde_json::Value::String(o.clone())));
        }
        if let Some(t) = &self.trace_id {
            clauses.push("trace_id = $trace_id".into());
            binds.push(("trace_id".into(), serde_json::Value::String(t.clone())));
        }
        if let Some(txt) = &self.text {
            // Case-insensitive substring over msg: lowercase both sides. The cap keeps the ring small
            // enough that this containment scan stays cheap; a full-text index is a noted follow-up.
            clauses.push("string::contains(string::lowercase(msg), $txt)".into());
            binds.push(("txt".into(), serde_json::Value::String(txt.to_lowercase())));
        }
        if let Some(s) = self.since {
            clauses.push("ts >= $since".into());
            binds.push(("since".into(), serde_json::Value::Number(s.into())));
        }
        if let Some(u) = self.until {
            clauses.push("ts < $until".into());
            binds.push(("until".into(), serde_json::Value::Number(u.into())));
        }
        (clauses.join(" AND "), binds)
    }
}

/// Levels at or above `lvl` in severity order (error > warn > info > debug > trace). `min_level =
/// warn` → `[warn, error]`.
fn levels_at_or_above(lvl: Level) -> Vec<Level> {
    let order = [
        Level::Error,
        Level::Warn,
        Level::Info,
        Level::Debug,
        Level::Trace,
    ];
    let start = order
        .iter()
        .position(|l| *l == lvl)
        .unwrap_or(order.len() - 1);
    order[..=start].to_vec()
}

/// A paged snapshot result: the rows (newest-first) + the `seq` cursor to fetch the next older page
/// (`None` when the page was short — the end was reached).
#[derive(Debug, Clone, serde::Serialize)]
pub struct QueryPage {
    pub rows: Vec<serde_json::Value>,
    pub next: Option<String>,
}
