//! The MCP bridge for the credential verb — `identity.set_credential` under the one MCP contract
//! (login-hardening scope). Admin management of a user's password rides the same mediated path as
//! every other admin action; the verb authorizes inside (`mcp:identity.manage:call`), denials opaque.
//! The secret VALUE arrives here and is hashed before any store write — it is never echoed back.

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::{json, Value};

use super::{identity_set_credential, CredentialError};

/// Dispatch an `identity.set_credential` MCP call. Returns `{ ok: true }` — a credential write never
/// returns the hash (secrets rule §6.7). `ts` is caller-injected (no wall-clock — testing §3).
pub async fn call_credential_tool(
    store: &Store,
    principal: &Principal,
    _ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "identity.set_credential" => {
            identity_set_credential(
                store,
                principal,
                str_arg(input, "user")?,
                str_arg(input, "secret")?,
                u64_arg(input, "ts"),
            )
            .await
            .map_err(credential_to_tool)?;
            Ok(json!({ "ok": true }))
        }
        _ => Err(ToolError::NotFound),
    }
}

fn credential_to_tool(e: CredentialError) -> ToolError {
    match e {
        CredentialError::Denied => ToolError::Denied,
        CredentialError::BadInput(m) => ToolError::BadInput(m),
        CredentialError::Hash(m) => ToolError::Extension(m),
        CredentialError::Store(s) => ToolError::Extension(s.to_string()),
    }
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadInput(format!("missing/!string arg: {key}")))
}

fn u64_arg(input: &Value, key: &str) -> u64 {
    input.get(key).and_then(|v| v.as_u64()).unwrap_or(0)
}
