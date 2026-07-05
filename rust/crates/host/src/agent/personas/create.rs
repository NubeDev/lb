//! `agent.persona.create {id, label, identity, granted_tools, grounding_skills, extends, ...}` (ADMIN)
//! — author a **custom** workspace persona. Gated by `mcp:agent.persona.create:call`.
//!
//! Order of checks (matters): (1) a `builtin.*` id is rejected `BadInput` BEFORE the caps gate
//! (read-only tier); (2) caps gate; (3) field validation (glob grammar + `extends` self/dup shape),
//! then the cross-store `extends` cycle/depth walk; then the record UPSERTs into the workspace
//! namespace (the hard wall). A re-create of the same id is an idempotent overwrite (LWW) — `create`
//! and `update` share the upsert, differing only by intent.

use lb_auth::Principal;
use lb_mcp::{authorize_tool, ToolError};

use super::model::Persona;
use super::store::upsert_persona;
use super::validate::{reject_reserved, validate_extends, validate_fields};
use crate::boot::Node;

/// Create a custom persona in `ws`. `persona.builtin` from client input is ignored — a custom write is
/// always tier `false` (built-ins are seed-only).
pub async fn agent_persona_create(
    node: &Node,
    principal: &Principal,
    ws: &str,
    persona: &Persona,
) -> Result<(), ToolError> {
    // (1) Reserved tier, before the caps gate — a `builtin.*` id is read-only regardless of caps.
    reject_reserved(&persona.id)?;
    // (2) Caps gate.
    authorize_tool(principal, ws, "agent.persona.create").map_err(|_| ToolError::Denied)?;
    if persona.id.is_empty() {
        return Err(ToolError::BadInput("missing id".into()));
    }
    // (3) Field grammar (globs, extends self/dup), then the cross-store cycle/depth walk.
    validate_fields(persona)?;
    validate_extends(&node.store, principal, ws, persona).await?;

    let record = Persona {
        builtin: false,
        ..persona.clone()
    };
    upsert_persona(&node.store, ws, &record)
        .await
        .map_err(|_| ToolError::Denied)
}
