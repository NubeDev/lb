//! The MCP bridge for team-membership verbs — host-native tools under the one MCP contract
//! (README §6.5). `members.add` / `members.list` / `members.remove`.
//!
//! **Why this file exists.** The three verbs shipped with their caps in the role bundles
//! (`mcp:members.add:call`, `mcp:members.list:call`) and their authorization written — and no
//! dispatch arm, so every caller got `no such tool`. A cap plus a catalog row is NOT reachability:
//! a verb is reachable only when some `call_*_tool` matches its name. The visible symptom was the
//! assign picker: `case.assignees` answers "who do I share a team with?", every workspace had zero
//! joinable teams because nothing could write a `member` edge over MCP, so the picker was empty on
//! every node and the only offer was *Assign to me*.
//!
//! Each verb authorizes inside itself (the deny gate); denials are opaque (`ToolError::Denied`), so
//! this bridge adds no gate of its own — it only names the arguments.

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::{json, Value};

use super::{add_team_member, list_members, remove_member, MembersError};

/// Dispatch a `members.*` MCP call. `input` is the verb's JSON arguments.
pub async fn call_members_tool(
    store: &Store,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "members.add" => {
            add_team_member(
                store,
                principal,
                ws,
                str_arg(input, "team")?,
                str_arg(input, "user")?,
            )
            .await
            .map_err(members_to_tool)?;
            Ok(json!({ "ok": true }))
        }
        "members.list" => {
            let members = list_members(store, principal, ws, str_arg(input, "team")?)
                .await
                .map_err(members_to_tool)?;
            Ok(json!({ "members": members }))
        }
        "members.remove" => {
            remove_member(
                store,
                principal,
                ws,
                str_arg(input, "team")?,
                str_arg(input, "user")?,
            )
            .await
            .map_err(members_to_tool)?;
            Ok(json!({ "ok": true }))
        }
        _ => Err(ToolError::NotFound),
    }
}

fn members_to_tool(e: MembersError) -> ToolError {
    match e {
        MembersError::Denied => ToolError::Denied,
        MembersError::Store(s) => ToolError::Extension(s.to_string()),
    }
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadInput(format!("missing/!string arg: {key}")))
}
