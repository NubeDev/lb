//! The `ce_appliance` registry's store access — the three generic `store.*` callbacks the registry
//! verbs and `resolve.rs` share. All isolation is structural: every call rides the sidecar's
//! ws-scoped `LB_EXT_TOKEN`, so a read/write/delete can only touch this workspace's namespace (§7).
//!
//! - [`get`] / [`list`] read via `store.query` (a parse-allowlisted `SELECT` — the read-only verb);
//! - [`put`] upserts via `store.write` (gated `store:ce_appliance:write`);
//! - [`remove`] erases via `store.delete`.
//!
//! `store.write` wraps the value in a `{ data: … }` envelope and `store.query`'s `SELECT` returns that
//! envelope's inner object per row — so a row is the `Appliance` JSON directly (we `SELECT data`).

use serde_json::{json, Value};

use super::record::{Appliance, TABLE};
use crate::host::{HostCtx, HostError};

/// Upsert an `Appliance` record via `store.write`. Gated `store:ce_appliance:write` (host-side).
pub async fn put(host: &HostCtx, appliance: &Appliance) -> Result<(), HostError> {
    let value = serde_json::to_value(appliance)
        .map_err(|e| HostError::BadResponse(format!("serialize appliance: {e}")))?;
    host.client()
        .call_tool(
            "store.write",
            json!({ "table": TABLE, "id": appliance.id, "value": value }),
        )
        .await?;
    Ok(())
}

/// Read one appliance by id via `store.query`. `Ok(None)` when absent in this workspace (a namespace
/// miss — the isolation not-found). The `$id` is bound (never interpolated) so it is injection-safe.
pub async fn get(host: &HostCtx, id: &str) -> Result<Option<Appliance>, HostError> {
    let out = host
        .client()
        .call_tool(
            "store.query",
            json!({
                "sql": format!("SELECT data FROM {TABLE} WHERE data.id = $id"),
                "vars": { "id": id },
            }),
        )
        .await?;
    let rows = rows_of(&out);
    match rows.first().and_then(data_field) {
        Some(data) => Appliance::from_data(&data)
            .map(Some)
            .map_err(HostError::BadResponse),
        None => Ok(None),
    }
}

/// List every appliance in this workspace via `store.query`, sorted by id for a stable UI order.
/// (The sort is done host-side in Rust: SurrealDB's `ORDER BY` requires the idiom be in the selection,
/// and `store.query`'s parse gate keeps the SELECT to a single projected `data` column.)
pub async fn list(host: &HostCtx) -> Result<Vec<Appliance>, HostError> {
    let out = host
        .client()
        .call_tool(
            "store.query",
            json!({ "sql": format!("SELECT data FROM {TABLE}") }),
        )
        .await?;
    let mut appliances = Vec::new();
    for row in rows_of(&out) {
        if let Some(data) = data_field(&row) {
            appliances.push(Appliance::from_data(&data).map_err(HostError::BadResponse)?);
        }
    }
    appliances.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(appliances)
}

/// Erase an appliance record via `store.delete` (idempotent). Gated `store:ce_appliance:write`.
pub async fn remove(host: &HostCtx, id: &str) -> Result<(), HostError> {
    host.client()
        .call_tool("store.delete", json!({ "table": TABLE, "id": id }))
        .await?;
    Ok(())
}

/// The `rows` array of a `store.query` `{ columns, rows }` result (empty on any other shape).
fn rows_of(out: &Value) -> Vec<Value> {
    out.get("rows")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// Unwrap a `SELECT data …` row's `data` field (the stored envelope's inner object).
fn data_field(row: &Value) -> Option<Value> {
    row.get("data").cloned()
}
