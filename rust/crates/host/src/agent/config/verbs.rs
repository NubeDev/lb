//! The gated `agent.config.*` host verbs (agent-config scope) — the capability chokepoint over the
//! raw `store.rs` layer, plus the write-time validation against the node's runtime registry.
//!
//!   - `agent.config.get` — read the workspace's agent config. **Member-level**
//!     (`mcp:agent.config.get:call`): a member must read it to render the Settings/Agent surface and,
//!     later, to know which runtime an invoke will use. Returns `None` when unset.
//!   - `agent.config.set` — merge a patch. **Admin-gated** (`mcp:agent.config.set:call`, beside
//!     `prefs.set_default` / `agent.policy.set`). A chosen `default_runtime` is validated against the
//!     node's `RuntimeRegistry` — an id the node cannot run is a `BadInput`, never a silent accept.
//!
//! Both authorize first via the shared `lb_mcp::authorize_tool` chokepoint (workspace-first, then
//! capability, opaque deny), then touch the store.

use lb_auth::Principal;
use lb_mcp::{authorize_tool, ToolError};

use super::model::AgentConfig;
use super::store::{get_agent_config, set_agent_config};
use crate::boot::Node;

/// `agent.config.get` (member) — the workspace's stored agent config (`None` if unset).
pub async fn agent_config_get(
    node: &Node,
    principal: &Principal,
    ws: &str,
) -> Result<Option<AgentConfig>, ToolError> {
    authorize_tool(principal, ws, "agent.config.get").map_err(|_| ToolError::Denied)?;
    get_agent_config(&node.store, ws)
        .await
        .map_err(|_| ToolError::Denied)
}

/// `agent.config.set` (ADMIN) — merge `patch` into the workspace's agent config. Gated by the
/// admin-only `mcp:agent.config.set:call`; a non-admin is denied opaquely. A `default_runtime` in the
/// patch must be an id the node's registry offers.
pub async fn agent_config_set(
    node: &Node,
    principal: &Principal,
    ws: &str,
    patch: &AgentConfig,
) -> Result<(), ToolError> {
    authorize_tool(principal, ws, "agent.config.set").map_err(|_| ToolError::Denied)?;

    // Validate the chosen runtime against what THIS node can actually run — a workspace cannot select
    // a runtime the node does not offer (registry drift is surfaced as a BadInput at write time, not a
    // silent accept that breaks the invoke path later).
    if let Some(id) = patch.default_runtime.as_deref() {
        let registry = node.runtimes();
        if !registry.ids().iter().any(|known| known == id) {
            return Err(ToolError::BadInput(format!(
                "unknown runtime {id:?} (node offers: {:?})",
                registry.ids()
            )));
        }
    }

    set_agent_config(&node.store, ws, patch)
        .await
        .map_err(|_| ToolError::Denied)
}
