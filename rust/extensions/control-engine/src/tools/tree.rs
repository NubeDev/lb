//! `control-engine.tree` — read a component subtree (the structural source of truth
//! CE re-numbers on restart). Maps onto `ControlEngine::get_tree`.
//!
//! Args (beyond the envelope `appliance`): an optional uid-keyed `node`
//! (`NodeRefArg`; absent → the engine root) and an optional integer `depth`
//! (default `-1` = the whole subtree). The `Tree` result is returned VERBATIM: its
//! `nodes` (`ComponentDto`) + `edges` (`EdgeDto`) serialize straight through
//! (control-engine scope open-question resolution — see the session doc).

use rubix_ce::{ControlEngine, EngineInstanceId};
use serde_json::{json, Value};

use crate::args::NodeRefArg;

/// Run `control-engine.tree`. Parses the optional `node`/`depth` args, calls
/// `get_tree`, and serializes `{ nodes, edges }` verbatim from the trait's DTOs.
pub async fn run(
    engine: &dyn ControlEngine,
    instance: &EngineInstanceId,
    input: &Value,
) -> Result<Value, String> {
    let node: NodeRefArg = match input.get("node") {
        Some(v) => serde_json::from_value(v.clone()).map_err(|e| format!("bad node arg: {e}"))?,
        None => NodeRefArg::default(),
    };
    let depth = input.get("depth").and_then(Value::as_i64).unwrap_or(-1) as i32;

    let tree = engine
        .get_tree(node.to_node_ref(instance), depth)
        .await
        .map_err(|e| e.to_string())?;

    // Verbatim: the wiresheet already speaks engine DTOs — re-shaping buys nothing.
    Ok(json!({ "nodes": tree.nodes, "edges": tree.edges }))
}
