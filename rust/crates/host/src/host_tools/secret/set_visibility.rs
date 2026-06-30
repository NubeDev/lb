//! `secret.set_visibility {path, visibility}` — owner-only runtime toggle (secrets scope). The
//! owner flips `Private ↔ Workspace` without an admin re-grant. The crate enforces
//! `secret:<path>:write` (gate 2) and gate 3 (owner-only).

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_secrets::Visibility;
use serde_json::{json, Value};

use crate::boot::Node;

use super::{map_err, req_str};

pub async fn secret_set_visibility(
    node: &Node,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    let path = req_str(input, "path")?;
    let visibility = Visibility::parse(
        input
            .get("visibility")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::BadInput("missing arg: visibility".into()))?,
    )
    .ok_or_else(|| ToolError::BadInput("bad visibility".into()))?;
    lb_secrets::set_visibility(&node.store, principal, ws, &path, visibility)
        .await
        .map_err(map_err)?;
    Ok(json!({ "ok": true }))
}
