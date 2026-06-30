//! `secret.set {path, value, visibility?}` — create/overwrite a secret, owner-stamped and
//! `Private` by default (secrets scope). The MCP `mcp:secret.set:call` gate runs in the
//! dispatcher first; the crate then enforces `secret:<path>:write` (gate 2) and gate 3 (an
//! overwrite is owner-only). `owner` is `principal.sub()` — the host stamps it, never the guest.

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_secrets::Visibility;
use serde_json::{json, Value};

use crate::boot::Node;

use super::{map_err, req_str};

pub async fn secret_set(
    node: &Node,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    let path = req_str(input, "path")?;
    let value = req_str(input, "value")?;
    let visibility = input
        .get("visibility")
        .and_then(|v| v.as_str())
        .map(|s| Visibility::parse(s).ok_or_else(|| ToolError::BadInput("bad visibility".into())))
        .transpose()?
        .unwrap_or_default();
    lb_secrets::set_with(&node.store, principal, ws, &path, &value, visibility)
        .await
        .map_err(map_err)?;
    Ok(json!({ "ok": true }))
}
