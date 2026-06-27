//! `call_tool` — the host's generic MCP bridge entry, exposed so a *transport* (the gateway's
//! `POST /mcp/call`) can forward an extension page/widget's `{tool, args}` through the **one** MCP
//! contract (rule 7, ui-federation scope). It runs the workspace-first, then `mcp:<tool>:call`
//! authorize gate and then dispatches — so a bridged caller is denied exactly like any other.
//!
//! Two dispatch families share the one contract:
//!   - **extension tools** (`<ext>.<tool>`, wasm or routed native) — resolved in the runtime
//!     `Registry` and run via `lb_mcp::call`.
//!   - **host-native tools** (`series.*` / `ingest.*`) — NOT in the runtime registry (they are host
//!     verbs over the embedded store, not components), so `lb_mcp::call` alone would `NotFound` them.
//!     The bridge must reach them too: a federated page reads platform data through `series.find` /
//!     `series.latest` here exactly as it would any extension tool. We authorize with the SAME MCP
//!     gate first (opaque `Denied`), then delegate to `call_ingest_tool` (which re-checks its own
//!     store-surface gate). This is the seam the `proof-panel` page exercises end to end; before it,
//!     `/mcp/call` could not dispatch a host-native verb at all (see
//!     debugging/extensions/bridge-cannot-dispatch-host-native-series.md).

use lb_auth::Principal;
use lb_mcp::{authorize_tool, ToolError};
use serde_json::Value;

use crate::boot::Node;
use crate::ingest::call_ingest_tool;

/// The host-native verb prefixes the bridge dispatches over the embedded store (not the runtime
/// registry). Kept narrow on purpose — only the read-only series surface a federated page is meant to
/// reach (plus `ingest.*` for symmetry; the bridge scope filter + the per-verb gate still apply).
fn is_host_native(qualified_tool: &str) -> bool {
    qualified_tool.starts_with("series.") || qualified_tool.starts_with("ingest.")
}

/// Call `qualified_tool` as `principal` in `ws` with a JSON input string, returning the tool's JSON
/// output. Authorization runs first; the workspace is the caller's (the gateway derives it from the
/// token, never the page). Host-native `series.*`/`ingest.*` verbs dispatch over the store; everything
/// else (`<ext>.<tool>`) routes through the runtime registry / bus.
pub async fn call_tool(
    node: &Node,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input_json: &str,
) -> Result<String, ToolError> {
    if is_host_native(qualified_tool) {
        // Same MCP gate as any tool (workspace-first, then `mcp:<tool>:call`) so a denied bridged
        // caller is opaque and indistinguishable from a missing tool — then delegate to the host verb.
        authorize_tool(principal, ws, qualified_tool)?;
        let input: Value = serde_json::from_str(input_json)
            .map_err(|e| ToolError::BadInput(format!("input json: {e}")))?;
        let out = call_ingest_tool(&node.store, principal, ws, qualified_tool, &input).await?;
        return serde_json::to_string(&out).map_err(|e| ToolError::Extension(e.to_string()));
    }

    lb_mcp::call(
        &node.registry,
        &node.bus,
        principal,
        ws,
        qualified_tool,
        input_json,
    )
    .await
}
