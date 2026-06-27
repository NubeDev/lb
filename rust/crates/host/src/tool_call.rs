//! `call_tool` — the host's generic MCP bridge entry, exposed so a *transport* (the gateway's
//! `POST /mcp/call`) can forward an extension page/widget's `{tool, args}` through the **one** MCP
//! contract (rule 7, ui-federation scope). It is `lb_mcp::call` with the node's registry + bus filled
//! in, so the gateway stays thin over `lb_host` (the house pattern) and the authorize gate
//! (workspace-first, then `mcp:<tool>:call`) runs unchanged — a bridged caller is denied exactly like
//! any other.

use lb_auth::Principal;
use lb_mcp::ToolError;

use crate::boot::Node;

/// Call `qualified_tool` (`"<ext>.<tool>"`) as `principal` in `ws` with a JSON input string, returning
/// the tool's JSON output. Authorization runs first inside `lb_mcp::call`; the workspace is the
/// caller's (the gateway derives it from the token, never the page).
pub async fn call_tool(
    node: &Node,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input_json: &str,
) -> Result<String, ToolError> {
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
