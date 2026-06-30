//! `secret.get {path}` — the three-gate read (secrets scope). Returns the value to an authorized
//! direct consumer: the owner (Private), or any workspace member past gates 1+2 when Workspace.
//! The MCP `mcp:secret.get:call` gate runs first; the crate then enforces `secret:<path>:get` and
//! the owner wall.

use lb_auth::Principal;
use lb_mcp::ToolError;
use serde_json::{json, Value};

use crate::boot::Node;

use super::{map_err, req_str};

pub async fn secret_get(
    node: &Node,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    let path = req_str(input, "path")?;
    let value = lb_secrets::get(&node.store, principal, ws, &path)
        .await
        .map_err(map_err)?;
    Ok(json!({ "path": path, "value": value }))
}
