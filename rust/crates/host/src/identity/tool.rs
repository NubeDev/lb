//! The MCP bridge for identity verbs — host-native tools under the one MCP contract (global-identity
//! scope). `identity.create` / `identity.get` / `identity.list` / `identity.workspaces`. Each
//! authorizes inside the verb (the deny gate); denials are opaque.

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::{json, Value};

use super::{identity_create, identity_get, identity_list, identity_workspaces, IdentityError};

/// Dispatch an `identity.*` MCP call. `input` is the verb's JSON arguments. `ts` is caller-injected
/// for `create` (no wall-clock — testing §3).
pub async fn call_identity_tool(
    store: &Store,
    principal: &Principal,
    _ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "identity.create" => {
            let view = identity_create(
                store,
                principal,
                str_arg(input, "sub")?,
                opt_str(input, "display_name"),
                u64_arg(input, "ts"),
            )
            .await
            .map_err(identity_to_tool)?;
            Ok(json!({ "identity": view }))
        }
        "identity.get" => {
            let view = identity_get(store, principal, str_arg(input, "sub")?)
                .await
                .map_err(identity_to_tool)?;
            Ok(json!({ "identity": view }))
        }
        "identity.list" => {
            let views = identity_list(store, principal)
                .await
                .map_err(identity_to_tool)?;
            Ok(json!({ "identities": views }))
        }
        "identity.workspaces" => {
            let workspaces = identity_workspaces(store, principal, str_arg(input, "sub")?)
                .await
                .map_err(identity_to_tool)?;
            Ok(json!({ "workspaces": workspaces }))
        }
        _ => Err(ToolError::NotFound),
    }
}

fn identity_to_tool(e: IdentityError) -> ToolError {
    match e {
        IdentityError::Denied => ToolError::Denied,
        IdentityError::Store(s) => ToolError::Extension(s.to_string()),
    }
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadInput(format!("missing/!string arg: {key}")))
}

fn opt_str<'a>(input: &'a Value, key: &str) -> Option<&'a str> {
    input.get(key).and_then(|v| v.as_str())
}

fn u64_arg(input: &Value, key: &str) -> u64 {
    input.get(key).and_then(|v| v.as_u64()).unwrap_or(0)
}
