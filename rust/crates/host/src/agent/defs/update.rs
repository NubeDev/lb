//! `agent.def.update {id, patch}` (ADMIN) — edit a **custom** workspace definition. Gated by
//! `mcp:agent.def.update:call`.
//!
//! Same ordered walls as create: (1) a `builtin.*` id is `BadInput` before the caps gate (read-only
//! tier); (2) caps gate; (3) the record must already exist in the workspace namespace (`NotFound`
//! otherwise — a ws-B admin editing a ws-A id sees the same `NotFound`, the hard wall); (4) the merged
//! `runtime` is validated against the node registry. Absent patch fields leave the current value.

use lb_auth::Principal;
use lb_mcp::{authorize_tool, ToolError};
use serde::Deserialize;

use super::model::DefinitionEndpoint;
use super::store::{get_definition, upsert_definition};
use super::validate::{reject_reserved, validate_runtime};
use crate::boot::Node;

/// A partial edit to a custom definition — every field optional (absent = unchanged).
#[derive(Debug, Default, Deserialize)]
pub struct DefinitionPatch {
    pub label: Option<String>,
    pub description: Option<String>,
    pub runtime: Option<String>,
    pub model_endpoint: Option<DefinitionEndpoint>,
}

/// Update a custom definition in `ws` by merging `patch` into the stored record.
pub async fn agent_def_update(
    node: &Node,
    principal: &Principal,
    ws: &str,
    id: &str,
    patch: DefinitionPatch,
) -> Result<(), ToolError> {
    // (1) Reserved tier, before the caps gate.
    reject_reserved(id)?;
    // (2) Caps gate.
    authorize_tool(principal, ws, "agent.def.update").map_err(|_| ToolError::Denied)?;

    // (3) The custom record must already exist in THIS workspace (the hard wall).
    let mut def = get_definition(&node.store, ws, id, false)
        .await
        .map_err(|_| ToolError::Denied)?
        .ok_or(ToolError::NotFound)?;

    if let Some(label) = patch.label {
        def.label = label;
    }
    if let Some(description) = patch.description {
        def.description = Some(description);
    }
    if let Some(runtime) = patch.runtime {
        def.runtime = runtime;
    }
    if let Some(endpoint) = patch.model_endpoint {
        def.model_endpoint = endpoint;
    }
    def.builtin = false;

    // (4) The (possibly changed) runtime must be one the node offers.
    validate_runtime(node, &def.runtime)?;

    upsert_definition(&node.store, ws, &def)
        .await
        .map_err(|_| ToolError::Denied)
}
