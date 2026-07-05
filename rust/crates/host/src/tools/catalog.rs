//! `tools.catalog` — the one new verb the `/` + `@` command palette reads (channels-command-palette
//! scope). Returns, for the calling principal in this workspace, ONLY the MCP tools they are
//! authorized to call — registered tools ∩ caps held — each as a descriptor `{ name, title, group,
//! input_schema }`. The menu IS the permission model rendered: a denied tool is **absent**, never
//! greyed, so a caller learns nothing about a tool they cannot run (no existence leak).
//!
//! Cardinal rule (scope "Risks"): the catalog MUST advertise a tool only if the call itself would
//! allow it. So for every reachable tool it runs the **SAME `authorize_tool` gate** `call_tool`
//! runs (`lb_mcp::authorize_tool`) — one gate, two callers. The catalog can therefore NEVER offer a
//! tool that then denies, and NEVER hide one that would pass.
//!
//! Gated by `mcp:tools.catalog:call` (workspace-first). It leaks only the tool *shapes* the caller
//! may already run, never data; every UI-capable principal holds the gate (it must, or there is no
//! palette). Read-only and derived live (registry + host-native descriptors + the caller's grants) —
//! owns no record, so a restart loses nothing.

use lb_auth::Principal;
use lb_mcp::{authorize_tool, ToolDescriptor};
use serde::Serialize;

use crate::boot::Node;

use super::descriptor::host_descriptors;

/// The catalog response — the caller's authorized tool set, each with its descriptor.
#[derive(Debug, Clone, Serialize)]
pub struct ToolsCatalog {
    pub ws: String,
    pub tools: Vec<ToolDescriptor>,
}

/// Read the authorized tool catalog for `ws` as `principal`. The workspace is the caller's (the
/// gateway derives it from the token, never the request). Denials are opaque.
///
/// The flow: gate the verb itself, then enumerate host-native descriptors + the registry's
/// extension descriptors, qualify each name, run the call's authorize gate per tool, and keep only
/// the authorized subset. Sorted by qualified name so the palette renders a stable order.
pub async fn tools_catalog(
    node: &Node,
    principal: &Principal,
    ws: &str,
) -> Result<ToolsCatalog, lb_mcp::ToolError> {
    // Gate the verb itself first — without `mcp:tools.catalog:call` the catalog denies opaquely.
    authorize_tool(principal, ws, "tools.catalog")?;

    let mut tools = Vec::new();

    // Host-native verbs (qualified names, declared in code). Each is gated by the same authorize
    // call its dispatch runs — a host verb the caller lacks `mcp:<verb>:call` for is dropped.
    for mut d in host_descriptors() {
        if authorize_tool(principal, ws, &d.name).is_ok() {
            if d.title.is_empty() {
                d.title = d.name.clone();
            }
            tools.push(d);
        }
    }

    // The REST of the host-native inventory (`system/catalog.rs` HOST_TOOLS — the authoritative
    // verb list `system.tools` serves), so the catalog honors its own contract: "every tool this
    // principal may run", not just the verbs that have grown a guided palette descriptor. Without
    // this, `reachable_tools` (the agent run's menu) could never advertise `datasource.list`,
    // `store.query`, `series.*`, `viz.query`, … regardless of caps or persona — see
    // debugging/agent/persona-menu-missing-tools-catalog-descriptor-only.md. Rows are name-only
    // (title = the one-line summary, no arg schema — a verb gains a schema by adding a descriptor
    // above); a verb already covered by a rich descriptor is skipped, and each row is filtered to
    // the verbs the MCP bridge can actually dispatch (`is_host_native`) then gated by the SAME
    // per-verb authorize call — advertise only what a call would allow.
    for info in crate::system::host_catalog() {
        if tools.iter().any(|t| t.name == info.tool) {
            continue;
        }
        if !crate::tool_call::is_host_native(&info.tool) {
            continue;
        }
        if authorize_tool(principal, ws, &info.tool).is_ok() {
            tools.push(ToolDescriptor {
                name: info.tool,
                title: info.description,
                group: info.group,
                input_schema: None,
                result: None,
            });
        }
    }

    // Extension-contributed tools (bare names from the registry, qualified `<ext>.<tool>`).
    for (ext_id, descriptors) in node.registry.descriptor_entries() {
        for mut d in descriptors {
            let qualified = format!("{ext_id}.{}", d.name);
            if authorize_tool(principal, ws, &qualified).is_ok() {
                d.name = qualified;
                if d.group.is_empty() {
                    d.group = ext_id.clone();
                }
                if d.title.is_empty() {
                    d.title = d.name.clone();
                }
                tools.push(d);
            }
        }
    }

    tools.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(ToolsCatalog {
        ws: ws.to_string(),
        tools,
    })
}
