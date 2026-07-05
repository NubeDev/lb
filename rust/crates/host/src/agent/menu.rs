//! `reachable_tools` — build the agent loop's [`AllowedTool`] menu from the caller's **reachable
//! tool catalog** (default-agent-wiring #3). The loop needs a list of tools the model may propose;
//! this derives it from the SAME `tools.catalog` gate the `/`-command palette reads — "every tool
//! this principal may run in this workspace", each already `authorize_tool`-checked.
//!
//! **The menu is not a widening.** The catalog only lists tools whose call would itself be allowed
//! (`tools/catalog.rs`), and the loop RE-CHECKS every proposed call under the derived principal
//! (`agent_caps ∩ caller.caps`) at dispatch. So a tool absent from the menu is also denied if the
//! model proposes it anyway — the catalog is the honest *menu*, the wall is still the authority. A
//! tool the poster cannot run is therefore absent here AND denied downstream.
//!
//! Best-effort: a catalog read failure (e.g. the caller lacks `mcp:tools.catalog:call`) yields an
//! empty menu rather than an error — the run still drives (with nothing to propose), never fails on
//! menu assembly. The catalog descriptors carry the tool title as the model-facing description.

use std::sync::Arc;

use lb_auth::Principal;

use super::model_access::AllowedTool;
use crate::boot::Node;
use crate::tools::tools_catalog;

/// The qualified MCP tools `principal` may run in `ws`, as the loop's [`AllowedTool`] menu. Derived
/// live from `tools.catalog` (registry + host-native descriptors ∩ the caller's grants) — never a
/// stored list, so it always reflects the current grant set. An empty vec when the catalog is
/// unreadable (the run just has no tools to propose).
pub async fn reachable_tools(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
) -> Vec<AllowedTool> {
    match tools_catalog(node, principal, ws).await {
        Ok(catalog) => catalog
            .tools
            .into_iter()
            .map(|d| AllowedTool {
                name: d.name,
                // The model-facing hint: prefer the descriptor's title, fall back to the group.
                description: if d.title.is_empty() { d.group } else { d.title },
                // Carry the input schema so the model knows the tool's arguments (without it, every
                // tool looks argument-less and the model asks the user in prose rather than calling).
                input_schema: d.input_schema,
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}
