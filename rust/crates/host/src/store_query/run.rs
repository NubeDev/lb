//! `store.query` — the read-only SurrealQL verb (widget-builder Slice A, the "direct SurrealDB"
//! source). Authorize (gate 1+2), refuse any statement naming the secret plane (`secret_wall.rs`),
//! then run inside the **caller's workspace namespace** on a session the engine will not let write
//! (`lb_store::Store::query_ws_readonly`), with a hard row cap + statement timeout, and shape the
//! rows into `{ columns, rows }` the dashboard's views render unchanged.
//!
//! The read-only property used to be a host-side parse-allowlist. SurrealDB 3 sealed the API that
//! rested on, so it moved into the session itself — see `parse.rs` for the full account of what
//! changed and which remaining checks are safety versus ergonomics.
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
use super::parse::{ensure_read_only_with_vars, ReadKind};

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

    // Two different boundaries, in order of who enforces them:
    //
    //   * the SECRET-PLANE wall, host-side and load-bearing — a `VIEWER` can read those tables, so
    //     nothing below us will stop it (`secret_wall.rs`);
    //   * the read-only property, which is NOT decided here any more. `query_ws_readonly` runs the
    //     statement in a session the engine will not let write, and will not let reach another
    //     workspace. This call additionally turns a write into a clear message rather than the
    //     empty result the engine would return, and reports the shape so a `SELECT` can be bounded
    //     without breaking `INFO`/`SHOW` (`parse.rs` says which part is safety and which is not).
    let kind = ensure_read_only_with_vars(sql, &vars)?;

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

    // The read-only session — not `query_ws`. This is where the guarantee actually lives.
    let mut resp = store.query_ws_readonly(ws, &bounded, vars).await?;
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
