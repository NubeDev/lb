//! `control-engine.remove-node` — soft-delete a component and its descendants.
//! Maps onto `ControlEngine::remove_node(node) -> DeletedItems`.
//!
//! Returns the soft-deleted component + edge UIDs to the caller — they are CE's
//! 24h-undo handle, which S8's `restore` follow-up consumes. `DeletedItems` is an
//! in-process type (no serde derive), so its UID lists are projected to JSON here.
//!
//! Args (beyond `appliance`): a required uid-keyed `node`. Self-checks
//! `mcp:control-engine.remove-node:call` FIRST.

use rubix_ce::{ControlEngine, EngineInstanceId};
use serde_json::{json, Value};

use crate::args::require_node_key;
use crate::host::{HostCtx, HostError};

/// Run `control-engine.remove-node`.
pub async fn run(
    host: &HostCtx,
    engine: &dyn ControlEngine,
    instance: &EngineInstanceId,
    input: &Value,
) -> Result<Value, HostError> {
    host.require("control-engine.remove-node")?;

    let node = require_node_key(input, instance).map_err(HostError::BadInput)?;
    let deleted = engine
        .remove_node(&node)
        .await
        .map_err(|e| HostError::BadResponse(e.to_string()))?;

    // Project the in-process UID lists to plain integers — the 24h-undo handle the
    // caller keeps and hands back to a future `restore`.
    let components: Vec<u32> = deleted.component_uids.iter().map(|u| u.0).collect();
    let edges: Vec<u32> = deleted.edge_uids.iter().map(|u| u.0).collect();
    Ok(json!({ "deleted": { "component_uids": components, "edge_uids": edges } }))
}
