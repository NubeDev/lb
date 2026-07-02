//! `control-engine.clear-override` — clear a manual override on a property. Maps
//! onto `ControlEngine::clear_override(node, prop)`.
//!
//! Args (beyond `appliance`): a required uid-keyed `node` and `property` (name).
//! Self-checks `mcp:control-engine.clear-override:call` FIRST.

use rubix_ce::{ControlEngine, EngineInstanceId};
use serde_json::{json, Value};

use crate::args::require_node_key;
use crate::host::{HostCtx, HostError};

/// Run `control-engine.clear-override`.
pub async fn run(
    host: &HostCtx,
    engine: &dyn ControlEngine,
    instance: &EngineInstanceId,
    input: &Value,
) -> Result<Value, HostError> {
    host.require("control-engine.clear-override")?;

    let node = require_node_key(input, instance).map_err(HostError::BadInput)?;
    let property = input
        .get("property")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| HostError::BadInput("missing/empty arg: property".into()))?;

    engine
        .clear_override(&node, property)
        .await
        .map_err(|e| HostError::BadResponse(e.to_string()))?;

    Ok(json!({ "ok": true }))
}
