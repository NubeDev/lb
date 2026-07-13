//! Host wrappers for `authz.check_scoped` / `authz.scope_filter` (entity-scoped-grants scope, plus
//! the delegated-reach half of native-caller-identity scope). These are the MCP-bridge entry points
//! extensions reach via `host.call-tool` / the native callback. They resolve a subject's scoped caps
//! from the real store and answer:
//!
//! - `check_scoped`: "may `<subject>` reach record `(table, id)` under `cap`?"
//! - `scope_filter`: "which rows in `table` may `<subject>` reach under `cap`?"
//!
//! **Whose reach?** By default the CALLER's own (from the token) — a caller can always learn only
//! its own reach, no delegation cap needed, byte-for-byte the original behaviour. When the optional
//! `subject` argument is present, the verb resolves *that* subject's reach instead — but ONLY if the
//! caller holds the delegation marker cap `mcp:authz.delegate_reach:call`. Absent that cap, a present
//! `subject` is a hard **deny** (403), never a silent fallback to the caller's own reach: a native
//! sidecar that holds the delegation grant can answer "does `user:ana` reach `child:leo`?" for the
//! caller the frame carries; one without it cannot, and fails closed (native-caller-identity scope,
//! the sacred deny). Cross-workspace is impossible structurally: resolution reads only `ws`, so a
//! `subject` string resolves only within the caller's workspace.
//!
//! The actual enforcement still happens at the verb level (`caps::check`); these are informational —
//! an extension verb asks the wall "what can `<subject>` reach?" so it doesn't re-implement the filter.

use lb_auth::Principal;
use lb_authz::{check_scoped_with, scope_filter_with, ScopeFilter};
use lb_mcp::{authorize_tool, ToolError};
use lb_store::Store;
use serde_json::{json, Value};

use super::hold::holds_cap;
use super::tool::str_arg;
use crate::authz::LiveBuiltinRoleCaps;

/// The marker cap a caller must hold to name a `subject` other than itself on the reach verbs. It is
/// an ordinary, admin-revocable grant (install-approved per extension); it dispatches to no verb —
/// its sole meaning is "may delegate a reach question" (native-caller-identity scope).
const DELEGATE_REACH_CAP: &str = "mcp:authz.delegate_reach:call";

/// `authz.check_scoped { cap, table, id, subject? }` — may `subject` (default: the caller) reach
/// `(table, id)` under `cap`? Returns `{ "allowed": bool }`. Gated by `mcp:authz.check_scoped:call`;
/// a present `subject` additionally requires `mcp:authz.delegate_reach:call` (else 403).
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
    let subject = resolve_subject(principal, ws, input)?;
    let allowed = check_scoped_with(store, ws, &subject, cap, table, id, &LiveBuiltinRoleCaps)
        .await
        .map_err(|e| ToolError::Extension(e.to_string()))?;
    Ok(json!({ "allowed": allowed }))
}

/// `authz.scope_filter { cap, table, subject? }` — which rows in `table` may `subject` (default: the
/// caller) reach under `cap`? Returns `{ "filter": "all" }` or `{ "filter": { "ids": [...] } }`.
/// Gated by `mcp:authz.scope_filter:call`; a present `subject` additionally requires
/// `mcp:authz.delegate_reach:call` (else 403).
pub async fn authz_scope_filter(
    store: &Store,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    authorize_tool(principal, ws, "authz.scope_filter")?;
    let cap = str_arg(input, "cap")?;
    let table = str_arg(input, "table")?;
    let subject = resolve_subject(principal, ws, input)?;
    let filter = scope_filter_with(store, ws, &subject, cap, table, &LiveBuiltinRoleCaps)
        .await
        .map_err(|e| ToolError::Extension(e.to_string()))?;
    Ok(match filter {
        ScopeFilter::All => json!({ "filter": "all" }),
        ScopeFilter::Ids(ids) => json!({ "filter": { "ids": ids } }),
    })
}

/// Decide WHOSE reach a call resolves, and enforce the delegation wall.
///
/// - No `subject` argument → the caller's own bare sub (today's exact behaviour; no cap needed).
/// - `subject` present → require the caller to hold [`DELEGATE_REACH_CAP`]. Held → that subject's
///   bare name (resolved within `ws` only). NOT held → [`ToolError::Denied`], the opaque 403.
///
/// Fail CLOSED: a caller lacking the delegation cap never silently falls back to its own reach — it
/// is denied. This is the sacred invariant the downstream guardian-isolation product depends on.
fn resolve_subject(principal: &Principal, ws: &str, input: &Value) -> Result<String, ToolError> {
    match input.get("subject").and_then(Value::as_str) {
        None => Ok(bare_user(principal.sub()).to_string()),
        Some(subject) => {
            if !holds_cap(principal, ws, DELEGATE_REACH_CAP) {
                return Err(ToolError::Denied);
            }
            Ok(bare_user(subject).to_string())
        }
    }
}

/// Strip the `user:` prefix from a sub (the resolver takes the bare name, matching how grants are
/// stored — `Subject::User("ada")`, not `Subject::User("user:ada")`). A sub without the prefix is
/// returned as-is (a non-user subject has no scoped grants in v1).
fn bare_user(sub: &str) -> &str {
    sub.strip_prefix("user:").unwrap_or(sub)
}
