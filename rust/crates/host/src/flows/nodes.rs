//! `flows.nodes` — the merged node registry for the calling workspace (node-descriptor-scope "The
//! merged registry"). Read-only, one cap (`mcp:flows.nodes:call`), **derived not stored**: the host
//! walks the workspace's `install` records (each carrying its validated `[[node]]` blocks) and
//! unions them with the five built-ins. The editor palette renders entirely from this response.
//!
//! The descriptor declares **no** capabilities — reading the catalog reveals only *what could run*;
//! the executing tool's own cap gates actual execution (`caller ∩ install-grant`,
//! extension-nodes-scope). So the palette is broadly readable; the deny lives at run time.
//!
//! The merge is symmetric (rule 1): built-in vs extension is data in the union, never an `if native`
//! branch. Workspace-scoped (rule 6): an extension installed in ws-A contributes nodes to ws-A's
//! `flows.nodes` and is **absent** from ws-B's registry.

use lb_assets::{list_installs, Install};
use lb_auth::Principal;
use lb_caps::{check, Action, Decision, Request, Surface};
use lb_flows::{
    builtin_descriptors, merge_registry, validate_node_block, NodeDescriptor, NodeKind,
};
use lb_store::Store;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FlowsNodesError {
    #[error("denied")]
    Denied,
    #[error("{0}")]
    Internal(String),
}

/// The merged node registry for workspace `ws` = the five built-ins ∪ every installed extension's
/// validated `[[node]]` descriptors. The install records already carry validated blocks (validated
/// at manifest parse); they are re-validated here defensively so a hand-edited install record with a
/// dangling tool binding is dropped (not fatal — one bad ext never blinds the whole palette).
pub async fn flows_nodes(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<NodeDescriptor>, FlowsNodesError> {
    authorize(principal, ws)?;
    merged_registry_internal(store, ws)
        .await
        .map_err(|e| FlowsNodesError::Internal(e.to_string()))
}

/// The merged registry WITHOUT the `mcp:flows.nodes:call` gate — for internal callers (e.g. the
/// save-time config re-validation) that already hold their own authority. Workspace-scoped by
/// `list_installs`; never another workspace's nodes.
pub async fn merged_registry_internal(store: &Store, ws: &str) -> Result<Vec<NodeDescriptor>, String> {
    let installs = list_installs(store, ws).await.map_err(|e| e.to_string())?;
    let mut ext_descriptors = Vec::new();
    for install in installs {
        for block in &install.nodes {
            // Re-validate defensively: a corrupted install drops THIS node, never the whole registry.
            if lb_flows::compile_schema(&block.config).is_err() {
                continue;
            }
            let tool_names = install_tool_names(&install);
            if let Ok(desc) = validate_node_block(block, &install.ext_id, &tool_names) {
                ext_descriptors.push(desc);
            }
        }
    }
    Ok(merge_registry(builtin_descriptors(), ext_descriptors))
}

/// The tool names an install's nodes may bind. The install record persists `granted` caps (not the
/// manifest's tool list), so reconstruct the bindable tool set from the granted `mcp:<ext>.<tool>`
/// caps — the exact set the runtime would authorize a node call against. A cap string is
/// `mcp:<ext>.<tool>:call`; strip the prefix and the `:call` suffix to recover the bare tool name.
/// A node whose `tool` is not in this set is dropped (it could not run anyway — no install grant).
fn install_tool_names(install: &Install) -> Vec<String> {
    let prefix = format!("mcp:{}.", install.ext_id);
    install
        .granted
        .iter()
        .filter_map(|c| {
            c.strip_prefix(&prefix)
                .and_then(|t| t.strip_suffix(":call"))
                .map(|t| t.to_string())
        })
        .collect()
}

fn authorize(principal: &Principal, ws: &str) -> Result<(), FlowsNodesError> {
    let req = Request::new(ws, Surface::Mcp, "flows.nodes", Action::Call);
    match check(principal, &req) {
        Decision::Allowed => Ok(()),
        Decision::Denied(_) => Err(FlowsNodesError::Denied),
    }
}

// `NodeKind` re-export keeps the cap-surface honest (a future descriptor filter uses it); referenced
// here so the import does not drift if the palette filtering lands.
#[allow(dead_code)]
fn _kind_used(_: NodeKind) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_tool_names_extracts_ext_tools() {
        let install = Install::new(
            "mqtt",
            "0.1.0",
            vec![
                "mcp:mqtt.publish:call".into(),
                "mcp:mqtt.subscribe:call".into(),
                "mcp:other.x:call".into(),
                "store:flow:read".into(),
            ],
            0,
        );
        let tools = install_tool_names(&install);
        assert_eq!(tools, vec!["publish".to_string(), "subscribe".to_string()]);
        // a cap without the :call suffix is NOT a tool grant (it is something else) — excluded.
        let install = Install::new(
            "mqtt",
            "0.1.0",
            vec!["mcp:mqtt.publish:call".into(), "mcp:mqtt.subscribe".into()],
            0,
        );
        let tools = install_tool_names(&install);
        assert_eq!(tools, vec!["publish".to_string()]);
    }
}
