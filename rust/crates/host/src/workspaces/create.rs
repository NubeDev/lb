//! `workspace_create` — register a workspace in the node directory so it appears in the switcher.
//!
//! Gated by `mcp:workspace.create:call` against the session's own workspace (from the token). Writes
//! the directory record in the reserved namespace; idempotent on the workspace id (re-creating upserts
//! the display name / ts). This does NOT provision the namespace's data — a workspace's namespace
//! springs into existence on first write to it; this only makes it *listable*.
//!
//! First-member bootstrap (global-identity scope, decision #3): creating a workspace auto-memberships
//! the creator AND grants them `role:workspace-admin`, so a brand-new workspace always has exactly one
//! admin and is never orphaned. Idempotent — re-creating a workspace that already has the creator as a
//! member is a no-op membership-wise.

use lb_auth::Principal;
use lb_authz as raw;
use lb_mcp::authorize_tool;
use lb_store::{read, write, Store};

use super::default_skills::{grant_default_core_skills, DEFAULT_CORE_SKILLS};
use super::error::WorkspacesError;
use super::model::{WorkspaceRecord, TABLE, TOMBSTONE, WORKSPACES_NS};

/// Register workspace `ws` with display `name` in the directory, as `principal`, AND bootstrap the
/// creator as the first `workspace-admin` member. Returns the record.
pub async fn workspace_create(
    store: &Store,
    principal: &Principal,
    ws: &str,
    name: &str,
    ts: u64,
) -> Result<WorkspaceRecord, WorkspacesError> {
    authorize_tool(principal, principal.ws(), "workspace.create")
        .map_err(|_| WorkspacesError::Denied)?;
    // A purged (tombstoned) workspace must never resurrect via a re-create (admin-crud: tombstone
    // wins over a stale edge or a re-issue). Return the would-be record without overwriting.
    if let Some(existing) = read(store, WORKSPACES_NS, TABLE, ws).await? {
        if existing.get("kind").and_then(|k| k.as_str()) == Some(TOMBSTONE) {
            return Ok(WorkspaceRecord::new(ws, name, ts));
        }
    }
    let record = WorkspaceRecord::new(ws, name, ts);
    let value =
        serde_json::to_value(&record).map_err(|e| lb_store::StoreError::Decode(e.to_string()))?;
    write(store, WORKSPACES_NS, TABLE, ws, &value).await?;
    // First-member bootstrap (decision #3): the creator is the workspace's first admin. Best-effort —
    // a membership/grant write error never fails the directory write (the workspace existing is the
    // contract; the bootstrap is the convenience). Idempotent on re-create.
    let creator = format!("user:{}", principal.sub().trim_start_matches("user:"));
    if raw::membership_is_member(store, ws, &creator)
        .await
        .unwrap_or(false)
    {
        return Ok(record);
    }
    let _ = raw::membership_add_raw(store, ws, &creator, ts).await;
    if let Some(name_part) = creator.strip_prefix("user:") {
        let subject = lb_authz::Subject::User(name_part.to_string());
        let _ = raw::grant_assign(store, ws, &subject, "role:member").await;
        let _ = raw::grant_assign(store, ws, &subject, "role:workspace-admin").await;
    }
    // Default core-skill grant set (core-skills scope): a FRESH workspace's agent is useful out of the
    // box. We reach this branch only on genuine first creation (the membership check above early-returns
    // an already-existing workspace), so a later admin *revoke* is never undone by a re-create. The
    // compiled-in read-only defaults (`core.lb-cli`/`core.query`/`core.store-read`); the binary may
    // widen the set via `LB_DEFAULT_CORE_SKILLS` on its own seeded workspace. Best-effort, like the
    // membership bootstrap — each is an ordinary, revocable `grant:skill/{id}` edge.
    let defaults: Vec<String> = DEFAULT_CORE_SKILLS.iter().map(|s| s.to_string()).collect();
    grant_default_core_skills(store, ws, &defaults).await;
    Ok(record)
}
