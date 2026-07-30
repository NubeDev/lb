//! `workspace.reconcile` — repair a listable-but-**memberless** workspace (workspace-provision
//! scope, NubeDev/lb#121). Re-runs the shared bootstrap (membership, role seed, role grants, skill
//! grants) for a workspace whose directory row exists but whose membership set is EMPTY — the orphan
//! class the old best-effort `workspace_create` could leave behind.
//!
//! **The sharpest security edge in the scope:** reconcile grants an admin into a workspace the
//! caller need not belong to. It is therefore strictly limited to workspaces with NO live member —
//! it is never a way to add yourself to a populated workspace (that is `members.add`, gated inside
//! that workspace). Gated by `mcp:workspace.reconcile:call` against the caller's own workspace;
//! bundled admin-tier today, with "super-admin only" tracked as the scope's open question 4.

use lb_auth::Principal;
use lb_authz::membership_has_any;
use lb_mcp::authorize_tool;
use lb_store::{read, Store};
use serde::Serialize;

use super::bootstrap::{apply_bootstrap, build_bootstrap, normalize_admin};
use super::default_skills::DEFAULT_CORE_SKILLS;
use super::error::WorkspacesError;
use super::model::{TABLE, TOMBSTONE, WORKSPACES_NS};

/// The reconcile reply: what the repair fixed.
#[derive(Debug, Clone, Serialize)]
pub struct ReconcileReport {
    pub ws: String,
    pub admin_sub: String,
    pub fixed: Vec<String>,
}

/// Re-run the bootstrap for the memberless workspace `ws`, installing `admin` (defaults to the
/// caller) as its first admin. Refused for a workspace with any live member.
pub async fn workspace_reconcile(
    store: &Store,
    principal: &Principal,
    ws: &str,
    admin: Option<&str>,
    ts: u64,
) -> Result<ReconcileReport, WorkspacesError> {
    authorize_tool(principal, principal.ws(), "workspace.reconcile")
        .map_err(|_| WorkspacesError::Denied)?;

    // Only a workspace the directory actually lists is reconcilable — a namespace with no directory
    // row is invisible to every lifecycle verb and needs an adopt-style path, not this one.
    let Some(existing) = read(store, WORKSPACES_NS, TABLE, ws).await? else {
        return Err(WorkspacesError::Invalid(
            "workspace is not in the directory".into(),
        ));
    };
    if existing.get("kind").and_then(|k| k.as_str()) == Some(TOMBSTONE) {
        return Err(WorkspacesError::Purged);
    }
    // The hard limit: any live member ⇒ not an orphan ⇒ refuse.
    if membership_has_any(store, ws).await? {
        return Err(WorkspacesError::Denied);
    }

    let admin_sub = normalize_admin(admin.unwrap_or(principal.sub()));
    let skills: Vec<String> = DEFAULT_CORE_SKILLS.iter().map(|s| s.to_string()).collect();
    let plan = build_bootstrap(store, ws, &admin_sub, &skills, ts).await?;
    apply_bootstrap(store, ws, &plan)
        .await
        .map_err(|source| WorkspacesError::ProvisionFailed {
            stage: "bootstrap",
            source,
        })?;

    let mut fixed = vec!["membership".to_string(), "role_grants".to_string()];
    if !plan.skills_granted.is_empty() {
        fixed.push("skill_grants".to_string());
    }
    Ok(ReconcileReport {
        ws: ws.to_string(),
        admin_sub: plan.admin_sub,
        fixed,
    })
}
