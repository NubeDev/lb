//! The gated `agent.memory.*` host verbs (agent-memory scope) — the capability chokepoint over the
//! raw `store.rs` layer, plus the member wall, the workspace-scope write gate, the secret lint, and
//! the size bounds.
//!
//! Gates, per verb:
//!   - the **MCP gate** — `mcp:agent.memory.<verb>:call` (workspace-first, opaque deny), like every
//!     tool. `list`/`get` are member-level reads; `set`/`delete` are member-level writes on the
//!     caller's OWN scope.
//!   - the **workspace-scope write gate** — a `set`/`delete` targeting the shared `workspace` scope
//!     ALSO needs `store:agent_memory/workspace:write` (a distinct cap), so an admin can decide
//!     whether every member's agent may write shared memory or only curators. A `member`-scope write
//!     needs only the verb cap (a run always may curate its own member memory).
//!   - the **member wall** — the target scope is derived from the principal (`resolve`), never an
//!     argument; a caller can never name another member.
//!
//! Deny is opaque (a caller learns nothing). `set` additionally enforces the description/body bounds
//! and the best-effort secret lint (a `BadInput`, not a silent accept).

use lb_auth::Principal;
use lb_caps::{check, Action, Decision, Request, Surface};
use lb_mcp::{authorize_tool, ToolError};
use lb_store::Store;

use super::index::render_index;
use super::lint::looks_like_secret;
use super::model::{Memory, MemoryKind, MemoryScope, MAX_BODY, MAX_DESCRIPTION};
use super::resolve::{addressed_scope, read_scopes, write_scope};
use super::store::{delete_memory, list_memories, read_memory, upsert_memory};

/// `agent.memory.list` (member) — the derived index rows across the caller's read scopes
/// (`workspace` + `member:self`), newest-updated first. Never another member's rows.
pub async fn memory_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<Memory>, ToolError> {
    authorize_tool(principal, ws, "agent.memory.list").map_err(|_| ToolError::Denied)?;
    list_memories(store, ws, &read_scopes(principal))
        .await
        .map_err(|_| ToolError::Denied)
}

/// `agent.memory.get` (member) — one fact by `{scope, slug}`, scope derived from the principal.
/// `None` if absent. A `member` get can only ever read the caller's own member scope (the wall).
pub async fn memory_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
    scope_arg: Option<&str>,
    slug: &str,
) -> Result<Option<Memory>, ToolError> {
    authorize_tool(principal, ws, "agent.memory.get").map_err(|_| ToolError::Denied)?;
    let scope = addressed_scope(principal, scope_arg)
        .ok_or_else(|| ToolError::BadInput("scope must be \"workspace\" or \"member\"".into()))?;
    read_memory(store, ws, &scope, slug)
        .await
        .map_err(|_| ToolError::Denied)
}

/// `agent.memory.set` (member, own scope) — upsert a fact by `{scope, slug}`. Enforces the bounds +
/// the secret lint; a `workspace`-scope write also needs the workspace-scope write cap. The target
/// scope is derived (member = the caller), never taken from a user id.
#[allow(clippy::too_many_arguments)]
pub async fn memory_set(
    store: &Store,
    principal: &Principal,
    ws: &str,
    scope_arg: Option<&str>,
    slug: &str,
    description: &str,
    kind: &str,
    body: &str,
    ts: u64,
) -> Result<Memory, ToolError> {
    authorize_tool(principal, ws, "agent.memory.set").map_err(|_| ToolError::Denied)?;

    let scope = write_scope(principal, scope_arg)
        .ok_or_else(|| ToolError::BadInput("scope must be \"workspace\" or \"member\"".into()))?;

    // A write to the SHARED workspace scope needs the distinct workspace-scope write cap (an admin
    // decides whether every member's agent may write shared memory). A member-scope write does not.
    if matches!(scope, MemoryScope::Workspace) {
        gate_workspace_write(principal, ws)?;
    }

    // Bounds (scope decided): a clear BadInput, never a silent truncate.
    if slug.trim().is_empty() {
        return Err(ToolError::BadInput("slug must not be empty".into()));
    }
    if description.chars().count() > MAX_DESCRIPTION {
        return Err(ToolError::BadInput(format!(
            "description exceeds {MAX_DESCRIPTION} chars"
        )));
    }
    if body.len() > MAX_BODY {
        return Err(ToolError::BadInput(format!(
            "body exceeds {MAX_BODY} bytes"
        )));
    }
    let kind = MemoryKind::parse(kind)
        .ok_or_else(|| ToolError::BadInput("kind must be user|feedback|project|reference".into()))?;

    // Best-effort secret lint (not a gate): reject an obvious credential shape so a careless/poisoned
    // write does not persist a key into memory.
    if let Some(reason) = looks_like_secret(&format!("{description}\n{body}")) {
        return Err(ToolError::BadInput(format!(
            "refusing to store apparent secret in memory: {reason}"
        )));
    }

    let mem = Memory {
        scope: scope.key(),
        slug: slug.to_string(),
        description: description.to_string(),
        body: body.to_string(),
        kind,
        updated_at: ts,
        updated_by: principal.sub().to_string(),
    };
    upsert_memory(store, ws, &mem)
        .await
        .map_err(|_| ToolError::Denied)?;
    Ok(mem)
}

/// `agent.memory.delete` (member, own scope) — remove a fact by `{scope, slug}`. A `workspace`-scope
/// delete needs the workspace-scope write cap too. Idempotent.
pub async fn memory_delete(
    store: &Store,
    principal: &Principal,
    ws: &str,
    scope_arg: Option<&str>,
    slug: &str,
) -> Result<(), ToolError> {
    authorize_tool(principal, ws, "agent.memory.delete").map_err(|_| ToolError::Denied)?;
    let scope = write_scope(principal, scope_arg)
        .ok_or_else(|| ToolError::BadInput("scope must be \"workspace\" or \"member\"".into()))?;
    if matches!(scope, MemoryScope::Workspace) {
        gate_workspace_write(principal, ws)?;
    }
    delete_memory(store, ws, &scope, slug)
        .await
        .map_err(|_| ToolError::Denied)
}

/// Render the derived index text for injection at session start (the compact catalog). `None` when
/// the caller has no readable memory (inject nothing). Denies are swallowed to `None` — injecting
/// memory is best-effort context, never a run failure (mirrors the skill catalog).
pub async fn memory_index_for_injection(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Option<String> {
    match memory_list(store, principal, ws).await {
        Ok(rows) => render_index(&rows),
        Err(_) => None,
    }
}

/// The distinct workspace-scope write gate: `store:agent_memory/workspace:write`. Uses the shared
/// `caps::check` chokepoint under the (possibly derived) principal, so an agent run is bounded by
/// `agent ∩ caller` here too. Opaque deny.
fn gate_workspace_write(principal: &Principal, ws: &str) -> Result<(), ToolError> {
    let req = Request::new(ws, Surface::Store, "agent_memory/workspace", Action::Write);
    match check(principal, &req) {
        Decision::Allowed => Ok(()),
        Decision::Denied(_) => Err(ToolError::Denied),
    }
}
