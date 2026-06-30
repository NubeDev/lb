//! `telemetry.purge` — the SINGLE destructive op over the capped ring (telemetry-console scope).
//! Clears the `telemetry` table in `ws`. Gated by a **separate, higher node-admin** capability
//! (`mcp:telemetry.purge:call`), never the default workspace read grant — the ring is recent
//! history, but wiping it is still an admin act (and one the audit ledger would record, were it
//! shipped). There is no per-row `telemetry.delete` verb by design: writes come from the Layer only.

use lb_auth::Principal;
use lb_store::Store;
use lb_telemetry::TABLE;

use super::authorize::authorize_telemetry;
use super::error::TelemetrySvcError;

/// Delete every telemetry row in `ws`. Returns the count removed (a SurrealDB `DELETE ... RETURN
/// BEFORE` yields the deleted rows). Gated by `mcp:telemetry.purge:call`; workspace-walled.
pub async fn telemetry_purge(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<u64, TelemetrySvcError> {
    authorize_telemetry(principal, ws, "telemetry.purge")?;
    let mut resp = store
        .query_ws(
            ws,
            "DELETE FROM type::table($tb) WHERE ws = $ws RETURN BEFORE",
            vec![
                ("tb".into(), serde_json::Value::String(TABLE.to_string())),
                ("ws".into(), serde_json::Value::String(ws.to_string())),
            ],
        )
        .await?;
    let removed: Vec<serde_json::Value> = resp
        .take(0)
        .map_err(|e| TelemetrySvcError::Store(lb_store::StoreError::Decode(e.to_string())))?;
    Ok(removed.len() as u64)
}
