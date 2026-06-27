//! `store.query` — the read-only SurrealQL verb (widget-builder Slice A, the "direct SurrealDB"
//! source). Authorize (gate 1+2), **parse-allowlist to a single read** (`parse.rs`, the load-bearing
//! gate), then run inside the **caller's workspace namespace** (set host-side from the token — never a
//! namespace named in the SQL) with a hard row cap + statement timeout, and shape the rows into
//! `{ columns, rows }` the dashboard's views render unchanged.
//!
//! The bound (`MAX_QUERY_ROWS` / `QUERY_TIMEOUT_SECS`) is appended to the parsed-clean query as a
//! `LIMIT … TIMEOUT …` wrapper so even a `SELECT` with no `LIMIT` cannot return more than the ceiling
//! or run longer than the bound. An unbounded analytical scan is a **job**, not this synchronous verb.

use lb_auth::Principal;
use lb_store::Store;
use serde_json::Value;

use super::authorize::authorize_store_query;
use super::error::StoreQueryError;
use super::model::{QueryResult, MAX_QUERY_ROWS, QUERY_TIMEOUT_SECS};
use super::parse::{ensure_read_only, ReadKind};

/// Run a read-only `sql` (with optional `$`-bound `vars`) in `ws` and return its columns + rows.
/// Gated `mcp:store.query:call`; parse-allowlisted to a single `SELECT`/`INFO`/`SHOW`; bounded to
/// [`MAX_QUERY_ROWS`] rows and [`QUERY_TIMEOUT_SECS`] seconds. Namespace-scoped — a ws-B caller
/// reaches only ws-B rows, structurally.
pub async fn store_query_run(
    store: &Store,
    principal: &Principal,
    ws: &str,
    sql: &str,
    vars: Vec<(String, Value)>,
) -> Result<QueryResult, StoreQueryError> {
    authorize_store_query(principal, ws, "store.query")?;

    // The boundary: parse + allowlist by statement kind BEFORE the SQL ever reaches the store. A
    // write/schema/namespace statement is refused here, structurally, not by a string match. We get
    // back the (single) statement kind so we can bound a `SELECT` without breaking `INFO`/`SHOW`.
    let kind = ensure_read_only(sql)?;

    // A `SELECT` is wrapped in a bounded sub-select so the row cap + timeout apply regardless of the
    // author's clauses (`($q)` is a subquery over the already-validated statement; we re-cap to the
    // ceiling and bound the wall time). `INFO`/`SHOW` are inherently bounded (one structured row) and
    // cannot be subqueried, so they run as-is.
    let bounded = match kind {
        ReadKind::Select => {
            format!("SELECT * FROM ({sql}) LIMIT {MAX_QUERY_ROWS} TIMEOUT {QUERY_TIMEOUT_SECS}s")
        }
        ReadKind::Introspection => sql.to_string(),
    };

    let mut resp = store.query_ws(ws, &bounded, vars).await?;
    let rows: Vec<Value> = resp
        .take(0)
        .map_err(|e| StoreQueryError::Store(lb_store::StoreError::Decode(e.to_string())))?;

    Ok(QueryResult {
        columns: columns_of(&rows),
        rows,
    })
}

/// The union of object keys across `rows`, in first-seen order — the column set the table header /
/// chart axis picker offers. A scalar/array row contributes no columns (the views fall back to a
/// single value/JSON cell).
fn columns_of(rows: &[Value]) -> Vec<String> {
    let mut seen = Vec::new();
    for row in rows {
        if let Some(obj) = row.as_object() {
            for key in obj.keys() {
                if !seen.iter().any(|c| c == key) {
                    seen.push(key.clone());
                }
            }
        }
    }
    seen
}
