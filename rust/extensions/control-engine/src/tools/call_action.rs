//! `control-engine.call-action` — invoke a named action on a component. Maps onto
//! `ControlEngine::call_action(node, action, Params) -> ActionResult`; returns the
//! action's `returns` as a name→value object.
//!
//! `ActionResult` is an in-process type (no serde derive), but its `returns` values
//! are `FlexValue` (which DOES derive `Serialize`), so the returns are projected to
//! a JSON object here.
//!
//! Args (beyond `appliance`): a required uid-keyed `node`, `action` (name), and an
//! optional `params` object (name→scalar). Self-checks
//! `mcp:control-engine.call-action:call` FIRST.

use rubix_ce::{ControlEngine, EngineInstanceId};
use serde_json::{json, Map, Value};

use crate::args::{require_node_key, value_pairs};
use crate::host::{HostCtx, HostError};

/// Run `control-engine.call-action`.
pub async fn run(
    host: &HostCtx,
    engine: &dyn ControlEngine,
    instance: &EngineInstanceId,
    input: &Value,
) -> Result<Value, HostError> {
    host.require("control-engine.call-action")?;

    let node = require_node_key(input, instance).map_err(HostError::BadInput)?;
    let action = input
        .get("action")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| HostError::BadInput("missing/empty arg: action".into()))?;
    let params = match input.get("params") {
        Some(v) => value_pairs(v).map_err(HostError::BadInput)?,
        None => Vec::new(),
    };

    let result = engine
        .call_action(&node, action, params)
        .await
        .map_err(|e| HostError::BadResponse(e.to_string()))?;

    // Project the name→FlexValue returns to a JSON object (FlexValue serializes).
    let mut returns = Map::new();
    for (name, value) in result.returns {
        let v = serde_json::to_value(value)
            .map_err(|e| HostError::BadResponse(format!("serialize return: {e}")))?;
        returns.insert(name, v);
    }
    Ok(json!({ "returns": Value::Object(returns) }))
}
