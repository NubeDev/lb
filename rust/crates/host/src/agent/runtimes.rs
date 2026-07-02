//! `agent.runtimes` — the read surface for the runtime picker (external-agent sub-scope #5, the
//! run-lifecycle "read surface"). Lists the runtimes/profiles THIS node has configured, so the
//! channel command palette can render a real runtime dropdown (the `x-lb:{widget:"runtime"}` arg)
//! instead of a typed `@id`. Read-only, list-only, workspace-scoped.
//!
//! Shape (the resolved open question — start minimal): `{ "default": <id>, "runtimes": [<sorted ids>] }`.
//! No health/version per profile — ids + default is all the picker needs; a richer per-profile shape
//! is a later addition, not this slice.
//!
//! Why it CANNOT leak cross-workspace data: the list is derived from the node's [`RuntimeRegistry`]
//! (`registry.rs`), a boot-time config map — it reads no store record, so there is structurally no
//! per-workspace data to cross. The workspace still gates the CALL (`authorize_tool` is
//! workspace-first), keeping the verb ws-scoped like every other MCP surface.
//!
//! Single responsibility: list the configured runtimes. The invoke path (`invoke_via_runtime`) and
//! the registry itself live elsewhere; this file only reads and shapes.

use lb_auth::Principal;
use lb_mcp::{authorize_tool, ToolError};
use serde_json::{json, Value};

use crate::boot::Node;

/// List the node's configured agent runtimes for `ws` as `principal`. Gated by
/// `mcp:agent.runtimes:call` (workspace-first); a caller without it gets an opaque [`ToolError::Denied`]
/// with no id leaked. Returns `{ "default": <default_id>, "runtimes": [<sorted ids>] }` — a default-only
/// node yields exactly `{ "default": "default", "runtimes": ["default"] }`.
pub async fn list_runtimes(
    node: &Node,
    principal: &Principal,
    ws: &str,
) -> Result<Value, ToolError> {
    authorize_tool(principal, ws, "agent.runtimes").map_err(|_| ToolError::Denied)?;
    let registry = node.runtimes();
    Ok(json!({
        "default": registry.default_id(),
        "runtimes": registry.ids(),
    }))
}
