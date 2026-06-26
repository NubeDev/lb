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

use crate::registry::Registry;

/// Call `<ext>.<tool>` as `principal` with a JSON input string. Returns the JSON output, or a
/// [`ToolError`]. The single public entry to the MCP tool surface.
pub async fn call(
    registry: &Registry,
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

    // 2. resolve the "<ext>.<tool>" name to a hosting extension (only reached once authorized).
    let target = resolve::resolve(registry, qualified_tool)?;

    // 3. dispatch into the hosting instance (local in S1; routed in S3 — same call site).
    dispatch::dispatch(target, qualified_tool, input_json).await
}
