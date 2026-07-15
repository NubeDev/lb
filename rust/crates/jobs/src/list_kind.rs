//! List a workspace's jobs of one `kind`, terminal rows included — the owner-verb read behind
//! `rules.runs.list` (long-running-rules-scope) and any future kind-scoped job view. Unlike
//! [`pending`](crate::pending) (the reactor drain — resumable only), this is the *observe* read:
//! an operator wants to see the done/failed/cancelled history too. Backed by the same
//! `(data.kind, data.status)` composite index; an optional `status` narrows to one value.
//!
//! Raw verb — the owning host service authorizes (caps + workspace) before calling this.

use lb_store::{Store, StoreError};
use serde_json::Value;

use super::model::Job;
use super::TABLE;

/// Safety ceiling on rows one list query returns (a caller `limit` above this is clamped).
const MAX_LIST: usize = 500;

/// Return up to `limit` jobs of `kind` in workspace `ws`, newest logical-ts first, optionally
/// narrowed to one stored `status` value (`"running"`/`"suspended"`/`"done"`/`"failed"`/
/// `"cancelled"` — the kebab-case on-disk form).
pub async fn list_kind(
    store: &Store,
    ws: &str,
    kind: &str,
    status: Option<&str>,
    limit: usize,
) -> Result<Vec<Job>, StoreError> {
    let limit = limit.clamp(1, MAX_LIST);
    let status_clause = if status.is_some() {
        " AND data.status = $status"
    } else {
        ""
    };
    // SurrealDB requires the ORDER BY idiom to appear in the selection, so `ts` is projected
    // alongside `data` (the decode below drops it — it only exists to satisfy the parser).
    let sql = format!(
        "SELECT data, data.ts AS ts FROM type::table($tb) \
         WHERE data.kind = $kind{status_clause} \
         ORDER BY ts DESC \
         LIMIT {limit}"
    );
    let mut bindings = vec![
        ("tb".into(), Value::String(TABLE.to_string())),
        ("kind".into(), Value::String(kind.to_string())),
    ];
    if let Some(s) = status {
        bindings.push(("status".into(), Value::String(s.to_string())));
    }
    let mut resp = store.query_ws(ws, &sql, bindings).await?;
    let rows: Vec<Value> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        // Each row is `{ data: <job body> }` (the wrapped shape). A row that fails to decode is
        // skipped, never fatal to the read (matches `pending`).
        let inner = match row {
            Value::Object(mut o) => o.remove("data").unwrap_or(Value::Null),
            other => other,
        };
        if let Ok(job) = serde_json::from_value::<Job>(inner) {
            out.push(job);
        }
    }
    Ok(out)
}
