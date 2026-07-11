//! The MCP bridge for invite verbs — `invite.create` / `list` / `revoke` / `resend` (invites
//! scope). The pre-auth `accept` verb is NOT here — it lives in the gateway route
//! `POST /public/invite/accept` (no principal, token-gated).
//!
//! `now` is the caller-injected logical clock (no wall-clock — testing §3), read from the `now`
//! field of the JSON input. Absent → 0 (fine for audit timestamps).

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::{json, Value};

use super::error::InviteError;
use super::{invite_create, invite_list, invite_resend, invite_revoke};

/// Convert an `InviteError` to a `ToolError` (the one conversion site, so the type is known).
fn to_tool(e: InviteError) -> ToolError {
    e.into()
}

/// Dispatch an `invite.*` MCP call. `input` is the verb's JSON arguments.
pub async fn call_invite_tool(
    store: &Store,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    let now = input.get("now").and_then(|v| v.as_u64()).unwrap_or(0);
    match qualified_tool {
        "invite.create" => {
            let email = str_arg(input, "email")?;
            let role = input.get("role").and_then(|v| v.as_str()).unwrap_or("");
            let team = input.get("team").and_then(|v| v.as_str()).unwrap_or("");
            let payload = input.get("payload").and_then(|v| v.as_str());
            let expires_ts = input
                .get("expires_ts")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let token = invite_create(
                store, principal, ws, email, role, team, payload, expires_ts, now,
            )
            .await
            .map_err(to_tool)?;
            Ok(json!({ "token": token }))
        }
        "invite.list" => {
            let invites = invite_list(store, principal, ws).await.map_err(to_tool)?;
            Ok(json!({ "invites": invites }))
        }
        "invite.revoke" => {
            let token_hash = str_arg(input, "token_hash")?;
            let revoked = invite_revoke(store, principal, ws, token_hash)
                .await
                .map_err(to_tool)?;
            Ok(json!({ "revoked": revoked }))
        }
        "invite.resend" => {
            let token_hash = str_arg(input, "token_hash")?;
            let token = invite_resend(store, principal, ws, token_hash, now)
                .await
                .map_err(to_tool)?;
            Ok(json!({ "token": token }))
        }
        _ => Err(ToolError::NotFound),
    }
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadInput(format!("missing/!string arg: {key}")))
}
