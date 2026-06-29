//! The MCP bridge for the `viz.*` surface — host-native tools under the one MCP contract (README
//! §6.5). UI, agents, and extensions reach `viz.query` the SAME way they reach any tool. The verb
//! gate (`mcp:viz.query:call`, workspace-first) runs FIRST; then the resolver dispatches each target
//! under the caller's authority (see [`super::query`]). Host-native — the gateway's `POST /mcp/call`
//! routes `viz.*` here via [`crate::call_tool_at_depth`] (the `viz.` prefix is host-native), exactly
//! like `dashboard.*`.
//!
//! `viz.query` takes its logical `now` from the args (the caller's clock — determinism §3) so target
//! dispatch threads a deterministic timestamp; absent, it defaults to 0 (a pure read needs none).

use std::sync::Arc;

use lb_auth::Principal;
use lb_mcp::ToolError;
use serde_json::Value;

use super::authorize::authorize_viz;
use super::error::VizError;
use super::query::viz_query;
use crate::boot::Node;

/// Dispatch a `viz.<verb>` MCP call at `depth` (the resolver re-enters dispatch at `depth + 1` per
/// target). `input` is the verb's JSON args; the return is the verb's JSON result.
pub async fn call_viz_tool(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
    depth: u32,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "viz.query" => {
            authorize_viz(principal, ws, qualified_tool).map_err(to_tool)?;
            // The panel spec — the whole cell (`sources[]`/`transformations[]`/`source`). Accept it
            // under `panel`, or treat the input itself as the panel for a bare call.
            let panel = input.get("panel").unwrap_or(input);
            let now = input.get("now").and_then(Value::as_u64).unwrap_or(0);
            let out = viz_query(node, principal, ws, panel, now, depth)
                .await
                .map_err(to_tool)?;
            Ok(out)
        }
        _ => Err(ToolError::NotFound),
    }
}

/// Map the viz resolver error onto the MCP tool error (denials opaque). A denied TARGET never reaches
/// here — it became an empty frame inside the resolver; only the verb gate / bad input surface.
fn to_tool(e: VizError) -> ToolError {
    match e {
        VizError::Denied => ToolError::Denied,
        VizError::BadInput(m) => ToolError::BadInput(m),
    }
}
