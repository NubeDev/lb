//! `secret.delete {path}` — owner-only erase (secrets scope). The crate enforces
//! `secret:<path>:write` (gate 2) and gate 3 (owner-only).

use lb_auth::Principal;
use lb_mcp::ToolError;
use serde_json::{json, Value};

use crate::boot::Node;

use super::{map_err, req_str};

pub async fn secret_delete(
    node: &Node,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    let path = req_str(input, "path")?;
    lb_secrets::delete(&node.store, principal, ws, &path)
        .await
        .map_err(map_err)?;
    Ok(json!({ "ok": true }))
}
