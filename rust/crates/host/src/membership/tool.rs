//! The MCP bridge for membership verbs — host-native tools under the one MCP contract
//! (global-identity scope). `membership.add` / `membership.remove` / `membership.list`. Each
//! authorizes inside the verb (the deny gate); denials are opaque. `membership_login_resolve` is NOT
//! bridged — it is the pre-mint seam the gateway calls directly, with no principal yet.

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::{json, Value};

use super::{membership_add, membership_list, membership_remove, MembershipError};

/// Dispatch a `membership.*` MCP call. `input` is the verb's JSON arguments.
pub async fn call_membership_tool(
    store: &Store,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "membership.add" => {
            membership_add(
                store,
                principal,
                ws,
                str_arg(input, "sub")?,
                u64_arg(input, "ts"),
            )
            .await
            .map_err(membership_to_tool)?;
            Ok(json!({ "ok": true }))
        }
        "membership.remove" => {
            let revoked = membership_remove(
                store,
                principal,
                ws,
                str_arg(input, "sub")?,
                now_or_wall(input),
            )
            .await
            .map_err(membership_to_tool)?;
            Ok(json!({ "ok": true, "grants_revoked": revoked }))
        }
        "membership.list" => {
            let members = membership_list(store, principal, ws)
                .await
                .map_err(membership_to_tool)?;
            Ok(json!({ "members": members }))
        }
        _ => Err(ToolError::NotFound),
    }
}

fn membership_to_tool(e: MembershipError) -> ToolError {
    match e {
        MembershipError::Denied => ToolError::Denied,
        MembershipError::Store(s) => ToolError::Extension(s.to_string()),
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

/// The revoke stamp for a marker written over MCP: the caller's `ts` (unix seconds) when it passes
/// one, else the host's wall clock — the clock the gateway stamps tokens with, which is what the
/// marker is compared against. A missing `ts` must NOT become `0`: a zero stamp means "refuse every
/// token", the old lock-out-for-good behaviour.
fn now_or_wall(input: &Value) -> u64 {
    input
        .get("ts")
        .and_then(|v| v.as_u64())
        .filter(|t| *t > 0)
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        })
}
