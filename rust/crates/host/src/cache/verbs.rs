//! The `cache.stats` / `cache.purge` admin verbs (response-cache scope, MCP surface). Reached over
//! the one MCP bridge like every host-native verb; the outer dispatch gate already ran
//! `mcp:cache.stats:call` / `mcp:cache.purge:call` (the read/admin capability pair — a caller
//! without the cap is opaquely `Denied`, tested). Compiled only under `page-cache`.
//!
//! - `cache.stats` (read): a node-wide snapshot — hit/miss/eviction counts, entry count, weighted
//!   size, and a per-class breakdown. A cache you cannot observe cannot be tuned or trusted.
//! - `cache.purge` (admin): drop this workspace's cached reads — a bounded, synchronous generation
//!   bump (not a job). The operator's stale-data escape hatch; other workspaces are untouched.

use std::sync::Arc;

use lb_auth::Principal;
use lb_mcp::ToolError;
use serde_json::{json, Value};

use crate::boot::Node;

/// Dispatch a `cache.*` verb. `None`/disabled cache ⇒ an honest "disabled" snapshot for stats and a
/// no-op ok for purge (the verb exists — the feature is compiled in — it just has nothing to do).
pub async fn call_cache_tool(
    node: &Arc<Node>,
    _principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    _input: &Value,
) -> Result<Value, ToolError> {
    let cache = node.response_cache();
    match qualified_tool {
        "cache.stats" => match cache {
            Some(c) => Ok(c.stats_snapshot().await),
            None => Ok(json!({ "enabled": false })),
        },
        "cache.purge" => {
            if let Some(c) = cache {
                c.purge(ws);
            }
            Ok(json!({ "ok": true, "workspace": ws }))
        }
        _ => Err(ToolError::NotFound),
    }
}
