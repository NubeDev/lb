//! `telemetry.trace` — fetch ONE correlated trace by `trace_id` (telemetry-console scope). The
//! timeline pivot: a row's `trace_id` is clicked, and this returns every telemetry row sharing it,
//! newest-last so the console renders the edge→hub→job→relay chain in order. **Workspace-walled** —
//! a ws-B caller gets only ws-B rows of the trace, never ws-A's hop (the same wall query enforces).

use lb_auth::Principal;
use lb_store::Store;
use lb_telemetry::TABLE;

use super::authorize::authorize_telemetry;
use super::error::TelemetrySvcError;

/// Read every telemetry row for `trace_id` in `ws`, ordered oldest-first (the timeline). Empty if
/// the trace has no rows in THIS workspace. Gated by `mcp:telemetry.read:call`; `ws` is hard-bound.
pub async fn telemetry_trace(
    store: &Store,
    principal: &Principal,
    ws: &str,
    trace_id: &str,
) -> Result<Vec<serde_json::Value>, TelemetrySvcError> {
    authorize_telemetry(principal, ws, "telemetry.trace")?;
    let sql = "SELECT seq, level, ws, actor, tool, source, trace_id, outcome, ts, msg, fields \
               FROM type::table($tb) WHERE ws = $ws AND trace_id = $trace_id ORDER BY seq ASC";
    let mut resp = store
        .query_ws(
            ws,
            sql,
            vec![
                ("tb".into(), serde_json::Value::String(TABLE.to_string())),
                ("ws".into(), serde_json::Value::String(ws.to_string())),
                (
                    "trace_id".into(),
                    serde_json::Value::String(trace_id.to_string()),
                ),
            ],
        )
        .await?;
    let rows: Vec<serde_json::Value> = resp
        .take(0)
        .map_err(|e| TelemetrySvcError::Store(lb_store::StoreError::Decode(e.to_string())))?;
    Ok(rows)
}
