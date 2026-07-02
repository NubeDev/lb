//! `control-engine.appliance.remove` — delete a `ce_appliance` record (a registry write, gated by its
//! own `mcp:control-engine.appliance.remove:call` + host-side `store:ce_appliance:write`). It does NOT
//! reach into the CE (scope: `remove` only deletes the record; S6 will disarm any live watch). A ws-B
//! caller cannot remove a ws-A appliance — the `store.delete` callback is workspace-walled, and an
//! absent record deletes idempotently (no cross-ws existence signal).

use serde_json::{json, Value};

use crate::appliance::store;
use crate::host::{HostCtx, HostError};

/// Run `appliance.remove`. Returns `{ id, removed: true }`. Idempotent (removing an absent id is Ok).
pub async fn run(host: &HostCtx, input: &Value) -> Result<Value, HostError> {
    host.require("control-engine.appliance.remove")?;
    let id = input
        .get("id")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| HostError::BadInput("missing/empty arg: id".into()))?;
    store::remove(host, id).await?;
    Ok(json!({ "id": id, "removed": true }))
}
