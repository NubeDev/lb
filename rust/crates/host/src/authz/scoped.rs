//! Host wrappers for `authz.check_scoped` / `authz.scope_filter` (entity-scoped-grants scope).
//! These are the MCP-bridge entry points extensions reach via `host.call-tool`. They resolve the
//! calling principal's scoped caps from the real store and answer:
//!
//! - `check_scoped`: "may THIS principal reach record `(table, id)` under `cap`?"
//! - `scope_filter`: "which rows in `table` may THIS principal reach under `cap`?"
//!
//! The principal is the caller's own (from the token) — these verbs never accept a `user`
//! argument, so a caller can only learn its OWN reach (no information leak). The actual
//! enforcement still happens at the verb level (`caps::check`); these are informational — an
//! extension verb asks the wall "what can I reach?" so it doesn't re-implement the filter.

use lb_auth::Principal;
use lb_authz::{check_scoped_with, scope_filter_with, ScopeFilter};
use lb_mcp::{authorize_tool, ToolError};
use lb_store::Store;
use serde_json::{json, Value};

use super::tool::str_arg;
use crate::authz::LiveBuiltinRoleCaps;

/// `authz.check_scoped { cap, table, id }` — may the calling principal reach `(table, id)` under
/// `cap`? Returns `{ "allowed": bool }`. The principal is the caller's own. Gated by
/// `mcp:authz.check_scoped:call`.
pub async fn authz_check_scoped(
    store: &Store,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    authorize_tool(principal, ws, "authz.check_scoped")?;
    let cap = str_arg(input, "cap")?;
    let table = str_arg(input, "table")?;
    let id = str_arg(input, "id")?;
    let user = bare_user(principal.sub());
    let allowed = check_scoped_with(store, ws, user, cap, table, id, &LiveBuiltinRoleCaps)
        .await
        .map_err(|e| ToolError::Extension(e.to_string()))?;
    Ok(json!({ "allowed": allowed }))
}

/// `authz.scope_filter { cap, table }` — which rows in `table` may the calling principal reach
/// under `cap`? Returns `{ "filter": "all" }` or `{ "filter": { "ids": [...] } }`. The principal
/// is the caller's own. Gated by `mcp:authz.scope_filter:call`.
pub async fn authz_scope_filter(
    store: &Store,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    authorize_tool(principal, ws, "authz.scope_filter")?;
    let cap = str_arg(input, "cap")?;
    let table = str_arg(input, "table")?;
    let user = bare_user(principal.sub());
    let filter = scope_filter_with(store, ws, user, cap, table, &LiveBuiltinRoleCaps)
        .await
        .map_err(|e| ToolError::Extension(e.to_string()))?;
    Ok(match filter {
        ScopeFilter::All => json!({ "filter": "all" }),
        ScopeFilter::Ids(ids) => json!({ "filter": { "ids": ids } }),
    })
}

/// Strip the `user:` prefix from a principal sub (the resolver takes the bare name, matching how
/// grants are stored — `Subject::User("ada")`, not `Subject::User("user:ada")`). A sub without the
/// prefix is returned as-is (a non-user principal has no scoped grants in v1).
fn bare_user(sub: &str) -> &str {
    sub.strip_prefix("user:").unwrap_or(sub)
}
