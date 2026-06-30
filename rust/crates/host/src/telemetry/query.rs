//! `telemetry.query` — the gated, **workspace-walled**, paged snapshot read over the capped
//! `telemetry` ring (telemetry-console scope). The caller's `ws` is hard-appended to the filter
//! server-side (never client-side): a ws-B query returns zero ws-A rows regardless of the filter.
//!
//! Newest-first, seq-cursor paged (stable under concurrent writes — an offset drifts when the ring
//! evicts; a seq cursor does not). The page size is hard-capped ([`MAX_PAGE`]).

use lb_auth::Principal;
use lb_store::Store;
use lb_telemetry::TABLE;

use super::authorize::authorize_telemetry;
use super::error::TelemetrySvcError;
use super::filter::{QueryFilter, QueryPage, MAX_PAGE};

/// One stored telemetry row, as the console renders it (the schema the Layer wrote, minus the
/// `cap_key`/`seq` bookkeeping fields). Top-level fields the filters index on.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TelemetryRow {
    pub seq: String,
    pub level: String,
    pub ws: String,
    pub actor: String,
    pub tool: String,
    pub source: String,
    pub trace_id: String,
    pub outcome: String,
    pub ts: u64,
    pub msg: String,
    #[serde(default)]
    pub fields: serde_json::Value,
}

/// Read a bounded, filtered page of `ws`'s telemetry, newest-first. `cursor` is the `seq` of the
/// oldest row of the previous page (resume strictly older); `None` for the first (newest) page.
/// `limit` is clamped to [`MAX_PAGE`]. Gated by `mcp:telemetry.read:call`; the `ws` wall is in the
/// filter itself, so even a granted caller cannot reach another workspace's rows.
pub async fn telemetry_query(
    store: &Store,
    principal: &Principal,
    ws: &str,
    filter: &QueryFilter,
    limit: usize,
    cursor: Option<&str>,
) -> Result<QueryPage, TelemetrySvcError> {
    authorize_telemetry(principal, ws, "telemetry.query")?;
    let n = limit.clamp(1, MAX_PAGE);

    let (mut where_sql, mut binds) = filter.where_clause(ws);
    // Page by seq, newest-first: rows strictly older than the cursor. The cursor is the seq of the
    // oldest row in the prior page.
    let sql = match cursor {
        Some(c) => {
            where_sql.push_str(" AND seq < $cursor");
            binds.push(("cursor".into(), serde_json::Value::String(c.to_string())));
            format!(
                "SELECT seq, level, ws, actor, tool, source, trace_id, outcome, ts, msg, fields \
                 FROM type::table($tb) WHERE {where_sql} ORDER BY seq DESC LIMIT {n}"
            )
        }
        None => format!(
            "SELECT seq, level, ws, actor, tool, source, trace_id, outcome, ts, msg, fields \
             FROM type::table($tb) WHERE {where_sql} ORDER BY seq DESC LIMIT {n}"
        ),
    };
    binds.insert(
        0,
        ("tb".into(), serde_json::Value::String(TABLE.to_string())),
    );

    let mut resp = store.query_ws(ws, &sql, binds).await?;
    let rows: Vec<serde_json::Value> = resp
        .take(0)
        .map_err(|e| TelemetrySvcError::Store(lb_store::StoreError::Decode(e.to_string())))?;

    // The next cursor is the oldest row's seq (the last in a newest-first page). A short page is the
    // end (`None`).
    let next = if rows.len() == n {
        rows.last()
            .and_then(|r| r.get("seq"))
            .and_then(|s| s.as_str())
            .map(String::from)
    } else {
        None
    };
    Ok(QueryPage { rows, next })
}
