//! The **channel query worker** (channels-query-charts scope). Runs INLINE inside `channel::post`
//! when the posted item is a `kind:"query"` request: it executes the SQL through the existing
//! `federation.query` verb (the ONE execution path — SELECT-only re-validated host-side, DSN stays
//! in the secret store), builds the result payload (capped + chart-picked), and posts a
//! `kind:"query_result"` (or `kind:"query_error"`) item back into the same channel under a system
//! identity. The whole exchange lives in durable channel history and streams over SSE.
//!
//! Security invariants (do not weaken):
//!   - Running a query needs TWO grants, checked in order: channel `bus:chan/{cid}:pub` (the member
//!     already passed it to post the `query` item), then `federation.query`'s datasource grant when
//!     the worker executes — run UNDER THE POSTER'S principal, so a member without the grant is
//!     denied here.
//!   - The deny path is **opaque**: a missing grant AND a missing source collapse to the same
//!     "query not permitted" message, so the poster learns nothing about whether the source exists.
//!   - **Re-entrancy guard:** only `kind:"query"` triggers work. The worker's own `query_result` /
//!     `query_error` items are themselves channel posts and must NOT be treated as new queries — an
//!     infinite loop is one absent guard away. Guarded explicitly and tested.
//!   - The worker holds NO durable state — everything is in the inbox item or on the bus. A failure
//!     inside the worker never fails the originating post; it posts a `query_error` (or nothing).

use lb_auth::Principal;
use lb_inbox::Item;
use lb_supervisor::OsLauncher;
use serde_json::Value;

use super::chart::pick_chart;
use super::payload::{error_body, parse_payload, result_body, ItemPayload, QueryPayload};
use crate::boot::Node;

/// The system identity the worker posts results/errors under. The result item is a host-posted
/// system message (the worker IS the host answering), so it does not re-run the channel `pub` gate.
pub(crate) const WORKER_AUTHOR: &str = "system:query-worker";

/// The hard result cap (channels-query-charts scope, decided): at most 500 rows / 256 KB on the
/// result payload, truncated with a flag. Keeps a `SELECT *` from bloating history or the bus frame.
pub(crate) const MAX_ROWS: usize = 500;
pub(crate) const MAX_BYTES: usize = 256 * 1024;

/// If `item` is a `kind:"query"` request, run it and post the result/error item. Otherwise (chat,
/// `query_result`, `query_error`, …) do nothing — the re-entrancy guard. Never errors: a worker
/// failure is logged away as a `query_error` item (or swallowed if even that cannot land); the
/// originating post has already succeeded.
pub async fn run_if_query(node: &Node, poster: &Principal, ws: &str, cid: &str, item: &Item) {
    // RE-ENTRANCY GUARD: only a `kind:"query"` item triggers work. A `query_result`/`query_error`
    // (or plain chat) parses to something else / None and returns here — the worker never feeds on
    // its own output.
    let Some(ItemPayload::Query(QueryPayload { source, sql })) = parse_payload(&item.body) else {
        return;
    };

    let ts = item.ts.saturating_add(1);
    match run_query(node, poster, ws, &source, &sql).await {
        Ok((columns, rows, truncated)) => {
            // The sidecar returns rows as column-aligned ARRAYS (the `federation.query` wire shape);
            // the chart picker keys cells by column NAME, so zip a keyed view of the (already-capped)
            // rows just for picking. The PERSISTED payload keeps the compact array rows — the UI maps
            // a chart series' field name back to its column index. Without this zip every result
            // plotted as `chart: null` (the array cells never matched the picker's `row.get(col)`).
            let keyed = keyed_rows(&columns, &rows);
            let chart = pick_chart(&columns, &keyed);
            let body = result_body(&source, &sql, columns, rows, chart, truncated);
            let _ = post_worker_item(node, ws, cid, &item.id, body, ts).await;
        }
        Err(msg) => {
            let body = error_body(&source, &sql, &msg);
            let _ = post_worker_item(node, ws, cid, &item.id, body, ts).await;
        }
    }
}

/// Execute `sql` against `source` under `poster`'s authority and return the capped
/// `(columns, rows, truncated)`. An error is already the opaque/honest message string for a
/// `query_error` item (the deny/no-source collapse happens here).
async fn run_query(
    node: &Node,
    poster: &Principal,
    ws: &str,
    source: &str,
    sql: &str,
) -> Result<(Vec<String>, Vec<Value>, bool), String> {
    let launcher = OsLauncher;
    let out = crate::federation::federation_query(node, &launcher, poster, ws, source, sql, 0)
        .await
        .map_err(federation_error_message)?;

    let columns = out
        .get("columns")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    let rows = out
        .get("rows")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    Ok(cap_result(columns, rows))
}

/// Map a `federation.query` error to the message a `query_error` item carries. The deny path
/// (missing `mcp:federation.query:call`) AND a not-found source collapse to the SAME opaque
/// "query not permitted" — so the poster cannot tell a forbidden source from a missing one (the
/// existence-leak invariant). A bad SELECT or a sidecar fault is an honest, distinct message.
fn federation_error_message(e: crate::federation::FederationError) -> String {
    use crate::federation::FederationError as F;
    match e {
        // Opaque collapse: no grant OR no such source → identical message (no existence leak).
        F::Denied | F::NotFound | F::EndpointRefused => "query not permitted".to_string(),
        F::BadSql(m) => format!("rejected sql: {m}"),
        F::BadInput(m) => format!("bad request: {m}"),
        F::Sidecar(m) => format!("query failed: {m}"),
        F::Store(_) => "query not permitted".to_string(),
    }
}

/// Enforce the row/byte cap on the result. Returns `(columns, rows, truncated)`. Rows beyond
/// `MAX_ROWS` are dropped first; if the serialized payload still exceeds `MAX_BYTES`, rows are
/// trimmed from the tail until it fits (or the table is empty). `truncated` is set iff anything was
/// dropped. Pure over the input — no IO, deterministic.
fn cap_result(columns: Vec<String>, mut rows: Vec<Value>) -> (Vec<String>, Vec<Value>, bool) {
    let mut truncated = false;
    if rows.len() > MAX_ROWS {
        rows.truncate(MAX_ROWS);
        truncated = true;
    }
    // Byte cap: trim from the tail until the serialized result body fits. Rebuild the body shape
    // (columns + current rows) for the size check; stop early once it fits.
    while serialized_size(&columns, &rows) > MAX_BYTES && !rows.is_empty() {
        rows.pop();
        truncated = true;
    }
    (columns, rows, truncated)
}

/// Zip column-aligned ARRAY rows into JSON OBJECTS keyed by column name — the shape the chart
/// picker reads (`row.get(col)`). The `federation.query` wire shape is `rows: [[c0, c1, …], …]`;
/// the picker needs `{col0: c0, col1: c1, …}`. Pure, allocation-light, used only to feed the picker
/// (the persisted payload keeps the compact arrays). A non-array row (defensive) is passed through
/// as-is so a row that is already an object still works.
fn keyed_rows(columns: &[String], rows: &[Value]) -> Vec<Value> {
    rows.iter()
        .map(|row| match row.as_array() {
            Some(cells) => {
                let obj: serde_json::Map<String, Value> =
                    columns.iter().cloned().zip(cells.iter().cloned()).collect();
                Value::Object(obj)
            }
            None => row.clone(),
        })
        .collect()
}

/// The serialized size of the result body for the byte cap check (the columns + rows envelope,
/// without the chart — the chart is negligible and computed after capping).
fn serialized_size(columns: &[String], rows: &[Value]) -> usize {
    serde_json::to_vec(&serde_json::json!({
        "kind": "query_result",
        "columns": columns,
        "rows": rows,
    }))
    .map(|v| v.len())
    .unwrap_or(usize::MAX)
}

/// Post a worker result/error item under the system identity via the shared channel `deliver`
/// (STATE-first, then MOTION) — no `pub` gate (the host is posting its own answer). The id ties the
/// answer to the request (`q:<request-id>`) so a client can correlate them; `ts` orders after.
async fn post_worker_item(
    node: &Node,
    ws: &str,
    cid: &str,
    request_id: &str,
    body: String,
    ts: u64,
) -> Result<(), super::error::ChannelError> {
    let item = Item::new(format!("q:{request_id}"), cid, WORKER_AUTHOR, body, ts);
    // A worker failure to persist is swallowed at the caller — the originating post already
    // succeeded; we do not want a query-answer hiccup to surface as a channel post error.
    super::post::deliver(&node.store, &node.bus, ws, cid, item)
        .await
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn cap_truncates_rows_beyond_max() {
        let cols = vec!["v".to_string()];
        let rows: Vec<Value> = (0..MAX_ROWS + 5).map(|i| json!({"v": i})).collect();
        let (c, r, truncated) = cap_result(cols, rows);
        assert_eq!(r.len(), MAX_ROWS);
        assert!(truncated);
        assert_eq!(c, vec!["v".to_string()]);
    }

    #[test]
    fn cap_trims_for_byte_limit() {
        // One column, but huge string values that blow the 256 KB cap → trimmed + flagged.
        let big = "x".repeat(100_000);
        let cols = vec!["v".to_string()];
        let rows: Vec<Value> = (0..10).map(|_| json!({"v": big.clone()})).collect();
        let (_c, r, truncated) = cap_result(cols, rows);
        assert!(truncated);
        assert!(serialized_size(&vec!["v".to_string()], &r) <= MAX_BYTES);
    }

    #[test]
    fn cap_marks_untruncated_when_under_limits() {
        let cols = vec!["v".to_string()];
        let rows = vec![json!({"v": 1}), json!({"v": 2})];
        let (_, _, truncated) = cap_result(cols, rows);
        assert!(!truncated);
    }

    // Regression (debugging/channels/query-result-chart-always-null.md): `federation.query` returns
    // rows as column-aligned ARRAYS, but the chart picker keys by column name. `keyed_rows` zips them
    // so a temporal/numeric result actually plots; before this the picker saw array cells, matched
    // nothing, and EVERY query_result came back `chart: null`.
    #[test]
    fn keyed_rows_zips_arrays_into_objects_so_the_picker_plots() {
        let cols = vec!["day".to_string(), "signups".to_string()];
        let rows = vec![
            json!(["2024-01-01", 3]),
            json!(["2024-01-02", 5]),
            json!(["2024-01-03", 7]),
        ];
        let keyed = keyed_rows(&cols, &rows);
        assert_eq!(keyed[0], json!({"day": "2024-01-01", "signups": 3}));
        // The whole point: the picker now yields a chart (it returned None on the raw arrays).
        assert!(
            pick_chart(&cols, &rows).is_none(),
            "raw array rows do not plot"
        );
        assert!(
            pick_chart(&cols, &keyed).is_some(),
            "keyed rows plot a chart"
        );
    }

    #[test]
    fn opaque_collapse_for_deny_and_not_found() {
        use crate::federation::FederationError as F;
        assert_eq!(federation_error_message(F::Denied), "query not permitted");
        assert_eq!(federation_error_message(F::NotFound), "query not permitted");
        // A bad SELECT stays honest and distinct.
        assert!(federation_error_message(F::BadSql("nope".into())).contains("rejected sql"));
    }
}
