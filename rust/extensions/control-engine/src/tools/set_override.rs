//! `control-engine.set-override` — set a temporary manual override on a property
//! with a TTL. Maps onto `ControlEngine::set_override(node, prop, value, ttl)`.
//!
//! Args (beyond `appliance`): a required uid-keyed `node`, `property` (name),
//! `value` (scalar), and `ttl_secs` (u64, `0` = permanent → `Duration::from_secs`).
//! Self-checks `mcp:control-engine.set-override:call` FIRST.

use std::time::Duration;

use rubix_ce::{ControlEngine, EngineInstanceId};
use serde_json::{json, Value};

use crate::args::{flex_value, require_node_key};
use crate::host::{HostCtx, HostError};

/// Run `control-engine.set-override`.
pub async fn run(
    host: &HostCtx,
    engine: &dyn ControlEngine,
    instance: &EngineInstanceId,
    input: &Value,
) -> Result<Value, HostError> {
    host.require("control-engine.set-override")?;

    let node = require_node_key(input, instance).map_err(HostError::BadInput)?;
    let property = input
        .get("property")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| HostError::BadInput("missing/empty arg: property".into()))?;
    let value = flex_value(
        input
            .get("value")
            .ok_or_else(|| HostError::BadInput("missing arg: value".into()))?,
    )
    .map_err(HostError::BadInput)?;
    let ttl_secs = input.get("ttl_secs").and_then(Value::as_u64).unwrap_or(0);

    engine
        .set_override(&node, property, value, Duration::from_secs(ttl_secs))
        .await
        .map_err(|e| HostError::BadResponse(e.to_string()))?;

    Ok(json!({ "ok": true }))
}
