//! The MCP bridge for the global-credential admin verb — `identity.set_password` under the one MCP
//! contract (email-login scope). Admin management of a person's global password rides the same
//! mediated path as every other admin action; the verb authorizes inside (`mcp:identity.manage:call`),
//! denials opaque. The secret VALUE arrives here and is hashed before any store write — never echoed.
//!
//! The self-service change (`POST /auth/password`) is NOT an MCP verb — it is a bespoke gateway route
//! (it authorizes on "valid token + correct old password", not on a capability), so it does not
//! dispatch here.

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::{json, Value};

use super::{identity_set_password, IdentityCredentialError};

/// Dispatch an `identity.set_password` MCP call. Returns `{ ok: true }` — a credential write never
/// returns the hash (§6.7). `ts` is caller-injected (no wall-clock — testing §3).
pub async fn call_identity_credential_tool(
    store: &Store,
    principal: &Principal,
    _ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "identity.set_password" => {
            identity_set_password(
                store,
                principal,
                str_arg(input, "sub")?,
                str_arg(input, "secret")?,
                u64_arg(input, "ts"),
            )
            .await
            .map_err(to_tool)?;
            Ok(json!({ "ok": true }))
        }
        _ => Err(ToolError::NotFound),
    }
}

fn to_tool(e: IdentityCredentialError) -> ToolError {
    match e {
        IdentityCredentialError::Denied => ToolError::Denied,
        IdentityCredentialError::BadInput(m) => ToolError::BadInput(m),
        IdentityCredentialError::BadOldSecret => ToolError::Denied,
        IdentityCredentialError::Hash(m) => ToolError::Extension(m),
        IdentityCredentialError::Store(s) => ToolError::Extension(s.to_string()),
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
