//! `secret.list {}` — workspace-scoped **metadata only** (secrets scope). Returns
//! `{path, owner, visibility}` for each secret in the workspace; **never values** (the mediation
//! invariant). The crate enforces `secret:**:get` (the browse grant) — listing is a deliberate
//! capability, not a free side-channel.

use lb_auth::Principal;
use lb_mcp::ToolError;
use serde::Serialize;
use serde_json::{json, Value};

use crate::boot::Node;

use super::map_err;

#[derive(Debug, Serialize)]
struct MetaOut {
    path: String,
    owner: String,
    visibility: String,
}

pub async fn secret_list(
    node: &Node,
    principal: &Principal,
    ws: &str,
    _input: &Value,
) -> Result<Value, ToolError> {
    let metas = lb_secrets::list(&node.store, principal, ws)
        .await
        .map_err(map_err)?;
    let out: Vec<MetaOut> = metas
        .into_iter()
        .map(|m| MetaOut {
            path: m.path,
            owner: m.owner,
            visibility: m.visibility.as_str().to_string(),
        })
        .collect();
    Ok(json!({ "secrets": out }))
}
