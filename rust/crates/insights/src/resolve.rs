//! `resolve` — move an insight `* → resolved` (insights umbrella scope).
//!
//! Idempotent (re-resolving a resolved insight is a no-op). Records who resolved and when. An
//! optional `note` rides the resolution (a human closure reason). `status_by` is host-stamped
//! from the principal — never caller-supplied. After resolve, a subsequent raise **re-opens**
//! (the raise verb's branch — this verb only closes).
//!
//! **STUB**: the transition body is deferred — see the punch-list.

use lb_store::{write, Store};
use serde_json::json;

use crate::error::InsightsError;
use crate::insight::OCC_TABLE;
use crate::insight_id::record_id;
use crate::status::Status;

/// Resolve insight `id` in workspace `ws` as `resolved_by` at logical ts `ts`, with an optional
/// `note`. Idempotent on an already-resolved insight.
// SCOPE: docs/scope/insights/insights-scope.md §"MCP surface" (insight.resolve)
pub async fn resolve(
    store: &Store,
    ws: &str,
    id: &str,
    resolved_by: &str,
    note: Option<&str>,
    ts: u64,
) -> Result<(), InsightsError> {
    let Some(mut insight) = crate::get::get(store, ws, id).await? else {
        return Err(InsightsError::BadInput(format!("no such insight: {id}")));
    };
    // Idempotent: re-resolving a resolved insight is a no-op.
    if insight.status == Status::Resolved {
        return Ok(());
    }
    insight.status = Status::Resolved;
    insight.status_by = Some(resolved_by.to_string());
    insight.status_ts = Some(ts);
    // The optional closure reason rides the record's body under a `resolution` key (the body is
    // free-form JSON; producers own the rest of the shape).
    if let Some(note) = note {
        if !insight.body.is_object() {
            insight.body = json!({});
        }
        if let Some(obj) = insight.body.as_object_mut() {
            obj.insert("resolution".into(), json!(note));
        }
    }
    let value = serde_json::to_value(&insight)
        .map_err(|e| InsightsError::Store(lb_store::StoreError::Decode(e.to_string())))?;
    write(store, ws, OCC_TABLE, &record_id(id), &value).await?;
    Ok(())
}
