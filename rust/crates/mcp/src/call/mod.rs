//! The tool-call pipeline, one phase per file (mcp scope, FILE-LAYOUT §3).
//!
//! Orchestration only: resolve the name, authorize (the deny gate), dispatch. `authorize`
//! runs before `dispatch` and — critically — a denied caller learns nothing about whether the
//! tool exists (the error carries no existence signal). That ordering is a tested contract.

mod authorize;
mod dispatch;
mod error;
mod resolve;

pub use error::ToolError;

use lb_auth::Principal;
use lb_bus::Bus;

use crate::registry::Registry;

/// The MCP authorize gate, exposed for **host-native tools** (e.g. the asset verbs) that are not
/// wasm extensions but must still be reached through the one MCP contract (README §6.5). A host
/// tool runs this first — workspace-first, then the `mcp:<tool>:call` capability — so the MCP
/// surface enforces the same isolation + deny as a routed extension call, *before* delegating to
/// the host verb (which adds its own store-surface capability + membership/grant gate).
pub fn authorize_tool(
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
) -> Result<(), ToolError> {
    authorize::authorize(principal, ws, qualified_tool)
}

/// Call `<ext>.<tool>` as `principal` with a JSON input string. Returns the JSON output, or a
/// [`ToolError`]. The single public entry to the MCP tool surface.
///
/// `bus` + `ws` carry the routed path: if the extension is hosted on another node, `dispatch`
/// routes the (already-authorized) call over the workspace-scoped queryable. Authorization
/// always runs HERE first, workspace-first — the remote node never sees an unauthorized call.
pub async fn call(
    registry: &Registry,
    bus: &Bus,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input_json: &str,
) -> Result<String, ToolError> {
    // 1. authorize FIRST — the DENY gate. Workspace isolation, then the
    //    mcp:<ext>.<tool>:call capability. Running it before resolve guarantees a denied
    //    caller cannot distinguish "not allowed" from "tool doesn't exist": both paths that
    //    a unauthorized caller can reach return `Denied` with no existence signal.
    authorize::authorize(principal, ws, qualified_tool)?;

    // 2. resolve the "<ext>.<tool>" name to a target (local instance or remote node) — only
    //    reached once authorized.
    let target = resolve::resolve(registry, qualified_tool)?;

    // 3. dispatch: call the local instance, or route over the bus to the hosting node. The
    //    seam is identical whether the ext is local or remote — that is the S3 point.
    dispatch::dispatch(&target, bus, ws, qualified_tool, input_json).await
}
