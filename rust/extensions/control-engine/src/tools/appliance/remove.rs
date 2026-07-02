//! `control-engine.appliance.remove` — delete a `ce_appliance` record (a registry write, gated by its
//! own `mcp:control-engine.appliance.remove:call` + host-side `store:ce_appliance:write`). It does NOT
//! reach into the CE (scope: `remove` only deletes the record; S6 will disarm any live watch). A ws-B
//! caller cannot remove a ws-A appliance — the `store.delete` callback is workspace-walled, and an
//! absent record deletes idempotently (no cross-ws existence signal).
//!
//! S6: removing an appliance **force-disarms** any live COV watch for it (the pump/CovStream is torn
//! down) — a feed for a gone appliance must stop. This runs after the cap self-check, before the record
//! delete, so a denied caller never affects a live watch.

use serde_json::{json, Value};

use crate::appliance::store;
use crate::host::{HostCtx, HostError};
use crate::watch::WatchRegistry;

/// Run `appliance.remove`. Returns `{ id, removed: true }`. Idempotent (removing an absent id is Ok).
/// Force-disarms any live watch for `id` (S6) before deleting the record.
pub async fn run(
    host: &HostCtx,
    watches: &WatchRegistry,
    input: &Value,
) -> Result<Value, HostError> {
    host.require("control-engine.appliance.remove")?;
    let id = input
        .get("id")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| HostError::BadInput("missing/empty arg: id".into()))?;
    // Tear down any live COV feed for this appliance (S6) — the pump drops its CovStream/WS.
    watches.disarm_appliance(id);
    store::remove(host, id).await?;
    Ok(json!({ "id": id, "removed": true }))
}
