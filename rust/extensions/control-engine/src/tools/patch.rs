//! `control-engine.patch` — write property values on a component. Maps onto
//! `ControlEngine::patch(node, Vec<PropPatch>)`; returns the resulting
//! `ComponentDto` VERBATIM (the same DTO shape the read verbs return).
//!
//! Args (beyond `appliance`): a required uid-keyed `node` and a `values` object
//! (name→scalar). Self-checks `mcp:control-engine.patch:call` FIRST.

use rubix_ce::{ControlEngine, EngineInstanceId, PropPatch};
use serde_json::{json, Value};

use crate::args::{require_node_key, value_pairs};
use crate::host::{HostCtx, HostError};

/// Run `control-engine.patch`.
pub async fn run(
    host: &HostCtx,
    engine: &dyn ControlEngine,
    instance: &EngineInstanceId,
    input: &Value,
) -> Result<Value, HostError> {
    host.require("control-engine.patch")?;

    let node = require_node_key(input, instance).map_err(HostError::BadInput)?;
    let values = input
        .get("values")
        .ok_or_else(|| HostError::BadInput("missing arg: values".into()))?;
    let props: Vec<PropPatch> = value_pairs(values)
        .map_err(HostError::BadInput)?
        .into_iter()
        .map(|(property, value)| PropPatch { property, value })
        .collect();

    let dto = engine
        .patch(&node, props)
        .await
        .map_err(|e| HostError::BadResponse(e.to_string()))?;

    // Verbatim: the wiresheet already speaks engine DTOs.
    Ok(json!({ "component": dto }))
}
