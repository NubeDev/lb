//! `workspace.provision` — stand up a complete, enterable workspace in one verb (workspace-provision
//! scope, NubeDev/lb#121). Directory row, first-member membership, built-in role seed, admin role
//! grants, and default core-skill grants as one unit: the in-namespace bootstrap applies as ONE
//! atomic `write_batch` transaction, and the directory row is written **last**, so a torn provision
//! can never leave a listable-but-memberless workspace (the observed orphan class).
//!
//! Gated by `mcp:workspace.provision:call` against the **caller's own** workspace — the target `ws`
//! is the object, never the authorization context, so no `auth.switch` is needed and the caller's
//! session is untouched (the reply carries no token). `admin` defaults to the caller or names another
//! existing identity. Idempotent: re-provisioning an active workspace returns the current record and
//! grants nothing (a later admin revoke is never undone); a purged tombstone is never resurrected.
//!
//! Honest durability note: `lb_store` exposes no explicit flush point — durability is the store
//! engine's per-transaction commit (SurrealKV append log). The atomicity here is the batch + the
//! directory-last ordering; a missing flush primitive is flagged in `docs/scope/store/`.

use lb_auth::Principal;
use lb_authz::membership_list;
use lb_mcp::authorize_tool;
use lb_store::{read, write, Store};
use serde::Serialize;

use super::bootstrap::{apply_bootstrap, build_bootstrap, normalize_admin};
use super::default_skills::DEFAULT_CORE_SKILLS;
use super::error::WorkspacesError;
use super::model::{WorkspaceRecord, TABLE, TOMBSTONE, WORKSPACES_NS};

/// The provision reply: the directory record plus what was bootstrapped, so the caller can show
/// truth rather than assume it. Carries NO token — the caller's session is untouched.
#[derive(Debug, Clone, Serialize)]
pub struct ProvisionReport {
    pub record: WorkspaceRecord,
    pub admin_sub: String,
    pub roles_granted: Vec<String>,
    pub skills_granted: Vec<String>,
}

/// Provision workspace `ws` with display `name` and `admin` (defaults to the caller) as its first
/// admin, granting `skills` (defaults to the compiled-in core set) — as one unit.
pub async fn workspace_provision(
    store: &Store,
    principal: &Principal,
    ws: &str,
    name: &str,
    admin: Option<&str>,
    skills: Option<&[String]>,
    ts: u64,
) -> Result<ProvisionReport, WorkspacesError> {
    authorize_tool(principal, principal.ws(), "workspace.provision")
        .map_err(|_| WorkspacesError::Denied)?;
    provision_authorized(store, principal, ws, name, admin, skills, ts).await
}

/// The post-authorization provision body, shared with `workspace_create`'s thin delegation (each
/// verb keeps its own capability gate; the semantics are one).
pub(super) async fn provision_authorized(
    store: &Store,
    principal: &Principal,
    ws: &str,
    name: &str,
    admin: Option<&str>,
    skills: Option<&[String]>,
    ts: u64,
) -> Result<ProvisionReport, WorkspacesError> {
    if ws.is_empty() || name.is_empty() {
        return Err(WorkspacesError::Invalid(
            "workspace id and name must be non-empty".into(),
        ));
    }
    if ws.starts_with('_') {
        return Err(WorkspacesError::Invalid(
            "workspace ids starting with '_' are reserved".into(),
        ));
    }
    let admin_sub = normalize_admin(admin.unwrap_or(principal.sub()));

    // Tombstone wins (admin-crud: never resurrect a purge); an existing ACTIVE/ARCHIVED workspace
    // makes re-provision an idempotent no-op that returns the current record — never a silent
    // re-bootstrap that would undo a later admin revoke.
    if let Some(existing) = read(store, WORKSPACES_NS, TABLE, ws).await? {
        if existing.get("kind").and_then(|k| k.as_str()) == Some(TOMBSTONE) {
            return Err(WorkspacesError::Purged);
        }
        let record: WorkspaceRecord = serde_json::from_value(existing)
            .map_err(|e| lb_store::StoreError::Decode(e.to_string()))?;
        return Ok(ProvisionReport {
            record,
            admin_sub,
            roles_granted: Vec::new(),
            skills_granted: Vec::new(),
        });
    }

    // No directory row. If the namespace nonetheless has live members, provisioning it would grant
    // the named admin into a POPULATED workspace the caller need not belong to — refuse; that
    // repair is `workspace.adopt` territory. One carve-out keeps a torn provision retryable: a
    // namespace whose ONLY live member is the requested admin is this provision's own bootstrap
    // that crashed before the directory write, and re-running it grants nothing new.
    let members = membership_list(store, ws).await?;
    if !(members.is_empty() || (members.len() == 1 && members[0].sub == admin_sub)) {
        return Err(WorkspacesError::Denied);
    }

    let default_skills: Vec<String>;
    let skills = match skills {
        Some(s) => s,
        None => {
            default_skills = DEFAULT_CORE_SKILLS.iter().map(|s| s.to_string()).collect();
            &default_skills
        }
    };

    // The in-namespace bootstrap: ONE atomic batch. Then — and only then — the directory row, so a
    // failure anywhere leaves the workspace absent from `workspace_list` and `login_workspaces`.
    let plan = build_bootstrap(store, ws, &admin_sub, skills, ts)
        .await
        .map_err(|source| WorkspacesError::ProvisionFailed {
            stage: "plan",
            source,
        })?;
    apply_bootstrap(store, ws, &plan)
        .await
        .map_err(|source| WorkspacesError::ProvisionFailed {
            stage: "bootstrap",
            source,
        })?;

    let record = WorkspaceRecord::new(ws, name, ts);
    let value =
        serde_json::to_value(&record).map_err(|e| lb_store::StoreError::Decode(e.to_string()))?;
    write(store, WORKSPACES_NS, TABLE, ws, &value)
        .await
        .map_err(|source| WorkspacesError::ProvisionFailed {
            stage: "directory",
            source,
        })?;

    Ok(ProvisionReport {
        record,
        admin_sub: plan.admin_sub,
        roles_granted: plan.roles_granted,
        skills_granted: plan.skills_granted,
    })
}
