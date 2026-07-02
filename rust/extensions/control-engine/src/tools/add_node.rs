//! `control-engine.add-node` — create a component under a parent (or the root).
//! Maps onto `ControlEngine::add_node(parent, NewNode)`; returns the new node's
//! keyed identity `{ uid, kind }`.
//!
//! Args (beyond the envelope `appliance`): `type` (required, `vendor-ext::component`);
//! an optional uid-keyed `parent` (`NodeRefArg`; absent → the engine root); an
//! optional `name` (CE supplies a sanitized default when absent — passed through as
//! `None`); optional `initial_values` (a name→scalar object).
//!
//! Self-checks `mcp:control-engine.add-node:call` FIRST (the inbound `native.call`
//! carries no caller identity — the sidecar enforces its own per-verb grant).

use rubix_ce::{ControlEngine, EngineInstanceId, NewNode};
use serde_json::{json, Value};

use crate::args::{value_pairs, NodeRefArg};
use crate::host::{HostCtx, HostError};

/// Run `control-engine.add-node`.
pub async fn run(
    host: &HostCtx,
    engine: &dyn ControlEngine,
    instance: &EngineInstanceId,
    input: &Value,
) -> Result<Value, HostError> {
    host.require("control-engine.add-node")?;

    let type_str = input
        .get("type")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| HostError::BadInput("missing/empty arg: type".into()))?
        .to_string();

    let parent: NodeRefArg = match input.get("parent") {
        Some(v) => serde_json::from_value(v.clone())
            .map_err(|e| HostError::BadInput(format!("bad parent arg: {e}")))?,
        None => NodeRefArg::default(),
    };

    let name = input
        .get("name")
        .and_then(Value::as_str)
        .map(str::to_string);

    let initial_values = match input.get("initial_values") {
        Some(v) => value_pairs(v).map_err(HostError::BadInput)?,
        None => Vec::new(),
    };

    let spec = NewNode {
        type_str,
        name,
        initial_values,
    };
    let key = engine
        .add_node(parent.to_node_ref(instance), spec)
        .await
        .map_err(|e| HostError::BadResponse(e.to_string()))?;

    Ok(json!({ "uid": key.uid.0, "kind": "component" }))
}
