//! `agent.def.create {id, label, runtime, model_endpoint, description?}` (ADMIN) — author a **custom**
//! workspace definition. Gated by `mcp:agent.def.create:call`.
//!
//! Order of checks (matters): (1) a `builtin.*` id is rejected `BadInput` BEFORE the caps gate
//! (read-only tier); (2) caps gate; (3) the `runtime` is validated against the node registry; then the
//! record UPSERTs into the workspace namespace (the hard wall). A re-create of the same id is an
//! idempotent overwrite (LWW) — `create` and `update` share the upsert, differing only by intent.

use lb_auth::Principal;
use lb_mcp::{authorize_tool, ToolError};

use super::model::AgentDefinition;
use super::store::upsert_definition;
use super::validate::{reject_reserved, validate_runtime};
use crate::boot::Node;

/// Create a custom definition in `ws`. `def.builtin` from client input is ignored — a custom write is
/// always tier `false` (built-ins are seed-only).
pub async fn agent_def_create(
    node: &Node,
    principal: &Principal,
    ws: &str,
    def: &AgentDefinition,
) -> Result<(), ToolError> {
    // (1) Reserved tier, before the caps gate — a `builtin.*` id is read-only regardless of caps.
    reject_reserved(&def.id)?;
    // (2) Caps gate.
    authorize_tool(principal, ws, "agent.def.create").map_err(|_| ToolError::Denied)?;
    if def.id.is_empty() {
        return Err(ToolError::BadInput("missing id".into()));
    }
    // (3) Runtime must be one the node offers.
    validate_runtime(node, &def.runtime)?;

    let record = AgentDefinition {
        builtin: false,
        ..def.clone()
    };
    upsert_definition(&node.store, ws, &record)
        .await
        .map_err(|_| ToolError::Denied)
}
