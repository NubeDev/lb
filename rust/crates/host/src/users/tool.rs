//! The MCP bridge for user verbs — host-native tools under the one MCP contract (admin-crud scope).
//! `user.create` / `user.list` / `user.disable` / `user.enable` / `user.delete`. Each authorizes
//! inside the verb (the deny gate); denials are opaque. `user_login_check` is NOT bridged — it is
//! the pre-mint seam the gateway calls directly, with no principal yet.

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::{json, Value};

use super::{user_create, user_delete, user_disable, user_enable, user_list, UsersError};

/// Dispatch a `user.*` MCP call. `input` is the verb's JSON arguments. `ts` is caller-injected for
/// `create` (no wall-clock — testing §3).
pub async fn call_users_tool(
    store: &Store,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "user.create" => {
            user_create(
                store,
                principal,
                ws,
                str_arg(input, "user")?,
                opt_str(input, "role").unwrap_or("member"),
                opt_str(input, "cred_ref").unwrap_or("dev"),
                u64_arg(input, "ts"),
            )
            .await
            .map_err(users_to_tool)?;
            Ok(json!({ "ok": true }))
        }
        "user.list" => {
            let users = user_list(store, principal, ws)
                .await
                .map_err(users_to_tool)?;
            Ok(json!({ "users": users }))
        }
        "user.disable" => {
            user_disable(store, principal, ws, str_arg(input, "user")?)
                .await
                .map_err(users_to_tool)?;
            Ok(json!({ "ok": true }))
        }
        "user.enable" => {
            user_enable(store, principal, ws, str_arg(input, "user")?)
                .await
                .map_err(users_to_tool)?;
            Ok(json!({ "ok": true }))
        }
        "user.delete" => {
            let revoked = user_delete(store, principal, ws, str_arg(input, "user")?)
                .await
                .map_err(users_to_tool)?;
            Ok(json!({ "ok": true, "revoked": revoked }))
        }
        _ => Err(ToolError::NotFound),
    }
}

fn users_to_tool(e: UsersError) -> ToolError {
    match e {
        UsersError::Denied => ToolError::Denied,
        UsersError::Disabled => ToolError::BadInput("user is disabled".into()),
        UsersError::Store(s) => ToolError::Extension(s.to_string()),
    }
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    opt_str(input, key).ok_or_else(|| ToolError::BadInput(format!("missing/!string arg: {key}")))
}

fn opt_str<'a>(input: &'a Value, key: &str) -> Option<&'a str> {
    input.get(key).and_then(|v| v.as_str())
}

fn u64_arg(input: &Value, key: &str) -> u64 {
    input.get(key).and_then(|v| v.as_u64()).unwrap_or(0)
}
