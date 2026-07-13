//! The MCP bridge for authz verbs — host-native tools under the one MCP contract (README §6.5). The
//! admin UI, agents, and extensions reach `grants.*` / `roles.*` / `teams.*` / `authz.*` the SAME
//! way they reach any wasm tool. Each verb authorizes first (the deny gate) inside the verb;
//! denials are opaque (`ToolError::Denied`).

use lb_auth::Principal;
use lb_authz::{Scope, Subject};
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::{json, Value};

use super::{
    authz_check_scoped, authz_resolve, authz_scope_filter, grants_assign, grants_list,
    grants_list_scoped, grants_revoke, revoke_tokens, roles_define, roles_delete, roles_list,
    teams_create, teams_list, AuthzError,
};

/// Dispatch a `grants.*` / `roles.*` / `teams.*` / `authz.*` MCP call. `input` is the verb's JSON
/// arguments.
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
                &scope_arg(input)?,
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
                &scope_arg(input)?,
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
        "grants.list_scoped" => {
            let grants = grants_list_scoped(store, principal, ws, &subject(input)?)
                .await
                .map_err(authz_to_tool)?;
            Ok(json!({ "grants": grants }))
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
        // access-console scope — the three verbs that close the access-graph gaps.
        "authz.resolve" => {
            let caps = authz_resolve(store, principal, ws, &subject(input)?)
                .await
                .map_err(authz_to_tool)?;
            Ok(json!({ "caps": caps }))
        }
        "authz.revoke-tokens" => {
            let revoked = revoke_tokens(store, principal, ws, &subject(input)?)
                .await
                .map_err(authz_to_tool)?;
            Ok(json!({ "grants_revoked": revoked }))
        }
        // entity-scoped-grants scope — the scoped read API extensions reach via host.call-tool.
        "authz.check_scoped" => authz_check_scoped(store, principal, ws, input).await,
        "authz.scope_filter" => authz_scope_filter(store, principal, ws, input).await,
        "roles.delete" => {
            let affected = roles_delete(store, principal, ws, str_arg(input, "name")?)
                .await
                .map_err(authz_to_tool)?;
            Ok(json!({ "affected": affected }))
        }
        _ => Err(ToolError::NotFound),
    }
}

/// Parse the `{ "subject": "user:ada" }` argument into a [`Subject`].
fn subject(input: &Value) -> Result<Subject, ToolError> {
    let raw = str_arg(input, "subject")?;
    Subject::parse(raw).ok_or_else(|| ToolError::BadInput(format!("bad subject: {raw}")))
}

/// Parse the optional `{ "scope": { "kind": "ids", "table": "child", "ids": [...] } }` argument.
/// Absent or null → `Scope::All` (today's behaviour). A **present-but-malformed** selector is a
/// hard `BadInput` — never a silent fallback to `All`, which would grant every row when the caller
/// asked for a subset (review fix: fail closed, not open).
fn scope_arg(input: &Value) -> Result<Scope, ToolError> {
    match input.get("scope") {
        Some(v) if !v.is_null() => serde_json::from_value(v.clone())
            .map_err(|e| ToolError::BadInput(format!("bad scope selector: {e}"))),
        _ => Ok(Scope::All),
    }
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
        AuthzError::Immutable(r) => ToolError::BadInput(format!("built-in role is immutable: {r}")),
        AuthzError::Store(s) => ToolError::Extension(s.to_string()),
    }
}

/// Parse a required string argument. `pub(crate)` — this MCP bridge owns the authz verbs' arg
/// parsing; `scoped.rs` (the `authz.check_scoped`/`scope_filter` wrappers) reuses it rather than
/// duplicating (FILE-LAYOUT: one owner, no utils file).
pub(crate) fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadInput(format!("missing/!string arg: {key}")))
}
