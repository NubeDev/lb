//! `control-engine.appliance.list` — list this workspace's registered appliances (a read verb, so
//! `appliance.list` is gated read-side). Self-checks `mcp:control-engine.appliance.list:call`; the
//! `store.query` callback is workspace-walled, so a ws-B caller sees only ws-B's appliances (§7).

use serde_json::{json, Value};

use crate::appliance::store;
use crate::host::{HostCtx, HostError};

/// Run `appliance.list`. Returns `{ appliances: [...] }` (the full records, secret_ref omitted when unset).
pub async fn run(host: &HostCtx) -> Result<Value, HostError> {
    host.require("control-engine.appliance.list")?;
    let appliances = store::list(host).await?;
    let value = serde_json::to_value(&appliances)
        .map_err(|e| HostError::BadResponse(format!("serialize appliances: {e}")))?;
    Ok(json!({ "appliances": value }))
}
