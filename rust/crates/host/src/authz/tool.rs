//! The MCP bridge for authz verbs — host-native tools under the one MCP contract (README §6.5). The
//! admin UI, agents, and extensions reach `grants.*` / `roles.*` / `teams.*` the SAME way they reach
//! any wasm tool. Each verb authorizes first (the deny gate) inside the verb; denials are opaque
//! (`ToolError::Denied`).

use lb_auth::Principal;
use lb_authz::Subject;
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::{json, Value};

use super::{
    grants_assign, grants_list, grants_revoke, roles_define, roles_list, teams_create, teams_list,
    AuthzError,
};

/// Dispatch a `grants.*` / `roles.*` / `teams.*` MCP call. `input` is the verb's JSON arguments.
pub async fn call_authz_tool(
    store: &Store,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "grants.assign" => {
            grants_assign(
                store,
                principal,
                ws,
                &subject(input)?,
                str_arg(input, "cap")?,
            )
            .await
            .map_err(authz_to_tool)?;
            Ok(json!({ "ok": true }))
        }
        "grants.revoke" => {
            grants_revoke(
                store,
                principal,
                ws,
                &subject(input)?,
                str_arg(input, "cap")?,
            )
            .await
            .map_err(authz_to_tool)?;
            Ok(json!({ "ok": true }))
        }
        "grants.list" => {
            let caps = grants_list(store, principal, ws, &subject(input)?)
                .await
                .map_err(authz_to_tool)?;
            Ok(json!({ "caps": caps }))
        }
        "roles.define" => {
            roles_define(store, principal, ws, str_arg(input, "name")?, &caps(input)?)
                .await
                .map_err(authz_to_tool)?;
            Ok(json!({ "ok": true }))
        }
        "roles.list" => {
            let roles = roles_list(store, principal, ws)
                .await
                .map_err(authz_to_tool)?;
            Ok(json!({ "roles": roles }))
        }
        "teams.create" => {
            teams_create(
                store,
                principal,
                ws,
                str_arg(input, "team")?,
                str_arg(input, "name")?,
            )
            .await
            .map_err(authz_to_tool)?;
            Ok(json!({ "ok": true }))
        }
        "teams.list" => {
            let teams = teams_list(store, principal, ws)
                .await
                .map_err(authz_to_tool)?;
            Ok(json!({ "teams": teams }))
        }
        _ => Err(ToolError::NotFound),
    }
}

/// Parse the `{ "subject": "user:ada" }` argument into a [`Subject`].
fn subject(input: &Value) -> Result<Subject, ToolError> {
    let raw = str_arg(input, "subject")?;
    Subject::parse(raw).ok_or_else(|| ToolError::BadInput(format!("bad subject: {raw}")))
}

/// Parse the `{ "caps": [...] }` array argument into owned strings.
fn caps(input: &Value) -> Result<Vec<String>, ToolError> {
    input
        .get("caps")
        .and_then(|v| v.as_array())
        .ok_or_else(|| ToolError::BadInput("missing caps array".into()))?
        .iter()
        .map(|c| {
            c.as_str()
                .map(str::to_string)
                .ok_or_else(|| ToolError::BadInput("cap not a string".into()))
        })
        .collect()
}

fn authz_to_tool(e: AuthzError) -> ToolError {
    match e {
        AuthzError::Denied => ToolError::Denied,
        AuthzError::Widen(c) => ToolError::BadInput(format!("cannot grant a cap you lack: {c}")),
        AuthzError::Store(s) => ToolError::Extension(s.to_string()),
    }
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadInput(format!("missing/!string arg: {key}")))
}
