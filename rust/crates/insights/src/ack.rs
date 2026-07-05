//! `ack` — move an insight `open → acked` (insights umbrella scope).
//!
//! Records who acked and when (logical ts). An acked fault re-firing bumps `count` but does NOT
//! re-page anyone (the ladder's ack-suppression, notify scope). `status_by` is host-stamped from
//! the principal — never caller-supplied (a caller cannot forge another reviewer's ack). No-op if
//! already `acked`; refused (returns `BadInput`) if `resolved` (a resolved insight stays resolved
//! — re-open is the raise verb's job, not ack's).
//!
//! **STUB**: the state-transition + refuse-on-resolved body is deferred — see the punch-list.

use lb_store::{write, Store};

use crate::error::InsightsError;
use crate::insight::OCC_TABLE;
use crate::insight_id::record_id;
use crate::status::Status;

/// Ack insight `id` in workspace `ws` as `acked_by` at logical ts `ts`. `acked_by` is the
/// principal's `sub` (host-supplied, never caller-supplied).
// SCOPE: docs/scope/insights/insights-scope.md §"MCP surface" (insight.ack)
// SCOPE: docs/scope/insights/insight-notify-scope.md §"Ack means 'I know'" (the suppression rule)
pub async fn ack(
    store: &Store,
    ws: &str,
    id: &str,
    acked_by: &str,
    ts: u64,
) -> Result<(), InsightsError> {
    let Some(mut insight) = crate::get::get(store, ws, id).await? else {
        return Err(InsightsError::BadInput(format!("no such insight: {id}")));
    };
    match insight.status {
        // A resolved insight stays resolved — re-open is the raise verb's job, not ack's.
        Status::Resolved => {
            return Err(InsightsError::BadInput(
                "resolved insights stay resolved — re-open via raise".into(),
            ));
        }
        // Idempotent: already acked ⇒ no-op success.
        Status::Acked => return Ok(()),
        Status::Open => {}
    }
    insight.status = Status::Acked;
    insight.status_by = Some(acked_by.to_string());
    insight.status_ts = Some(ts);
    let value = serde_json::to_value(&insight)
        .map_err(|e| InsightsError::Store(lb_store::StoreError::Decode(e.to_string())))?;
    write(store, ws, OCC_TABLE, &record_id(id), &value).await?;
    Ok(())
}
