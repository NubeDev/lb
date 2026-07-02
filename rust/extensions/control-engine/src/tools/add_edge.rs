//! `control-engine.add-edge` — wire a dataflow edge from a source property to a
//! target property. Maps onto `ControlEngine::add_edge(EdgeSpec)`; returns the new
//! edge's keyed identity `{ uid, kind: "edge" }`.
//!
//! Args (beyond `appliance`): `source` + `target` (each a required uid-keyed node),
//! and `source_property` + `target_property` (names). Self-checks
//! `mcp:control-engine.add-edge:call` FIRST.

use rubix_ce::{ControlEngine, EdgeSpec, EngineInstanceId, NodeKey};
use serde_json::{json, Value};

use crate::args::NodeKeyArg;
use crate::host::{HostCtx, HostError};

/// Run `control-engine.add-edge`.
pub async fn run(
    host: &HostCtx,
    engine: &dyn ControlEngine,
    instance: &EngineInstanceId,
    input: &Value,
) -> Result<Value, HostError> {
    host.require("control-engine.add-edge")?;

    let source = endpoint(input, "source", instance)?;
    let target = endpoint(input, "target", instance)?;
    let source_property = req_str(input, "source_property")?;
    let target_property = req_str(input, "target_property")?;

    let key = engine
        .add_edge(EdgeSpec {
            source,
            source_property,
            target,
            target_property,
        })
        .await
        .map_err(|e| HostError::BadResponse(e.to_string()))?;

    Ok(json!({ "uid": key.uid.0, "kind": "edge" }))
}

/// Parse a required uid-keyed endpoint node (`source`/`target`).
fn endpoint(input: &Value, key: &str, instance: &EngineInstanceId) -> Result<NodeKey, HostError> {
    let v = input
        .get(key)
        .ok_or_else(|| HostError::BadInput(format!("missing arg: {key}")))?;
    let arg: NodeKeyArg = serde_json::from_value(v.clone())
        .map_err(|e| HostError::BadInput(format!("bad {key} arg: {e}")))?;
    Ok(arg.to_node_key(instance))
}

fn req_str(input: &Value, key: &str) -> Result<String, HostError> {
    input
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| HostError::BadInput(format!("missing/empty arg: {key}")))
}
