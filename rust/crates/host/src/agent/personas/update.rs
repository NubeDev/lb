//! `agent.persona.update {id, patch}` (ADMIN) — edit a **custom** workspace persona. Gated by
//! `mcp:agent.persona.update:call`.
//!
//! Same ordered walls as create: (1) a `builtin.*` id is `BadInput` before the caps gate (read-only
//! tier); (2) caps gate; (3) the record must already exist in the workspace namespace (`NotFound`
//! otherwise — a ws-B admin editing a ws-A id sees the same `NotFound`, the hard wall); (4) the merged
//! record is field-validated (glob grammar + `extends` self/dup) and its `extends` closure cycle/depth
//! walked. Absent patch fields leave the current value.

use lb_auth::Principal;
use lb_mcp::{authorize_tool, ToolError};
use serde::Deserialize;

use super::model::PolicyPreset;
use super::store::{get_persona, upsert_persona};
use super::validate::{reject_reserved, validate_extends, validate_fields};
use crate::boot::Node;

/// A partial edit to a custom persona — every field optional (absent = unchanged). A present list
/// REPLACES the stored list (not a merge — a persona's tool/skill set is edited as a whole in the
/// Settings editor).
#[derive(Debug, Default, Deserialize)]
pub struct PersonaPatch {
    pub label: Option<String>,
    pub description: Option<String>,
    pub identity: Option<String>,
    pub granted_tools: Option<Vec<String>>,
    pub grounding_skills: Option<Vec<String>>,
    pub extends: Option<Vec<String>>,
    /// Page surfaces (persona-session #5) — opaque strings the dock context-matches (rule 10).
    pub surfaces: Option<Vec<String>>,
    pub policy_preset: Option<PolicyPreset>,
    pub runtimes: Option<Vec<String>>,
}

/// Update a custom persona in `ws` by merging `patch` into the stored record.
pub async fn agent_persona_update(
    node: &Node,
    principal: &Principal,
    ws: &str,
    id: &str,
    patch: PersonaPatch,
) -> Result<(), ToolError> {
    // (1) Reserved tier, before the caps gate.
    reject_reserved(id)?;
    // (2) Caps gate.
    authorize_tool(principal, ws, "agent.persona.update").map_err(|_| ToolError::Denied)?;

    // (3) The custom record must already exist in THIS workspace (the hard wall).
    let mut persona = get_persona(&node.store, ws, id, false)
        .await
        .map_err(|_| ToolError::Denied)?
        .ok_or(ToolError::NotFound)?;

    if let Some(label) = patch.label {
        persona.label = label;
    }
    if let Some(description) = patch.description {
        persona.description = Some(description);
    }
    if let Some(identity) = patch.identity {
        persona.identity = identity;
    }
    if let Some(granted_tools) = patch.granted_tools {
        persona.granted_tools = granted_tools;
    }
    if let Some(grounding_skills) = patch.grounding_skills {
        persona.grounding_skills = grounding_skills;
    }
    if let Some(extends) = patch.extends {
        persona.extends = extends;
    }
    if let Some(surfaces) = patch.surfaces {
        persona.surfaces = surfaces;
    }
    if let Some(policy_preset) = patch.policy_preset {
        persona.policy_preset = Some(policy_preset);
    }
    if let Some(runtimes) = patch.runtimes {
        persona.runtimes = Some(runtimes);
    }
    persona.builtin = false;

    // (4) Re-validate the merged record (globs, extends self/dup), then the cross-store cycle/depth walk.
    validate_fields(&persona)?;
    validate_extends(&node.store, principal, ws, &persona).await?;

    upsert_persona(&node.store, ws, &persona)
        .await
        .map_err(|_| ToolError::Denied)
}
