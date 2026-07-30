//! The shared **bootstrap write-set builder** — the one place that knows what a freshly provisioned
//! workspace must contain (workspace-provision scope). `provision`, `reconcile`, and (through
//! provision) `create` all assemble their in-namespace rows here, so the three verbs cannot drift.
//!
//! The plan holds every row that goes INSIDE the target workspace's namespace: the built-in role
//! records (seed-if-absent, so a redefined custom role is never clobbered), the first-member
//! membership, the admin's role grants, and the default core-skill edges (write-if-absent, so a
//! previously revoked skill edge is never resurrected by a re-run). The directory row is NOT part of
//! the plan — it lives in the reserved [`super::model::WORKSPACES_NS`] namespace and is written by
//! the caller *after* the plan applies (ordering: directory row last, so a torn provision can never
//! leave a listable-but-memberless workspace).
//!
//! [`apply_bootstrap`] applies the whole plan as ONE [`lb_store::write_batch`] transaction: either
//! every bootstrap row lands or none does.

use lb_authz::{grant_row, membership_row, Subject};
use lb_store::{read, write_batch, Store, StoreError, UpsertBatch};
use serde_json::Value;

use crate::assets::{GRANT, GRANT_SCOPE};
use crate::authz::{ROLE_MEMBER, ROLE_VIEWER, ROLE_WORKSPACE_ADMIN};
use lb_assets::relation_row;
use lb_authz::{Role, ROLE_TABLE};

/// The roles the bootstrap grants the first admin.
pub const BOOTSTRAP_ADMIN_ROLES: &[&str] = &["role:member", "role:workspace-admin"];

/// One row of the bootstrap batch, owned so the plan can outlive its inputs.
struct PlanRow {
    table: &'static str,
    id: String,
    value: Value,
}

/// Everything the bootstrap will write into the target workspace's namespace, plus the report
/// fields the provision reply carries.
pub struct BootstrapPlan {
    /// The normalized first-admin subject (`user:...`).
    pub admin_sub: String,
    /// The role grants the plan carries for the admin.
    pub roles_granted: Vec<String>,
    /// The skill edges the plan carries (existing edges — live or revoked — are skipped).
    pub skills_granted: Vec<String>,
    rows: Vec<PlanRow>,
}

/// Build the bootstrap write set for workspace `ws` with `admin_sub` as its first admin. Reads the
/// target namespace to keep the plan idempotent-safe: role records are seeded only if absent, and a
/// skill edge that already has a row (live OR tombstoned) is skipped so a revoke is never undone.
pub async fn build_bootstrap(
    store: &Store,
    ws: &str,
    admin_sub: &str,
    skills: &[String],
    ts: u64,
) -> Result<BootstrapPlan, StoreError> {
    let mut rows: Vec<PlanRow> = Vec::new();

    // Built-in role records, seed-if-absent (mirrors `ensure_builtin_authz_roles`, batched).
    for (name, caps) in [
        (ROLE_VIEWER, crate::authz::viewer_role_caps()),
        (ROLE_MEMBER, crate::authz::member_role_caps()),
        (
            ROLE_WORKSPACE_ADMIN,
            crate::authz::workspace_admin_role_caps(),
        ),
    ] {
        if read(store, ws, ROLE_TABLE, name).await?.is_none() {
            let role = Role::new(name, caps);
            let value =
                serde_json::to_value(&role).map_err(|e| StoreError::Decode(e.to_string()))?;
            rows.push(PlanRow {
                table: ROLE_TABLE,
                id: name.to_string(),
                value,
            });
        }
    }

    // First-member membership + the admin's role grants.
    let (table, id, value) = membership_row(admin_sub, ts)?;
    rows.push(PlanRow { table, id, value });
    let name_part = admin_sub.trim_start_matches("user:");
    let subject = Subject::User(name_part.to_string());
    let mut roles_granted = Vec::new();
    for role in BOOTSTRAP_ADMIN_ROLES {
        let (table, id, value) = grant_row(&subject, role)?;
        rows.push(PlanRow { table, id, value });
        roles_granted.push(role.to_string());
    }

    // Default core-skill edges, write-if-absent (a revoked edge stays revoked).
    let mut skills_granted = Vec::new();
    for skill in skills {
        let (table, id, value) = relation_row(GRANT, skill, GRANT_SCOPE)?;
        if read(store, ws, table, &id).await?.is_some() {
            continue;
        }
        rows.push(PlanRow { table, id, value });
        skills_granted.push(skill.clone());
    }

    Ok(BootstrapPlan {
        admin_sub: admin_sub.to_string(),
        roles_granted,
        skills_granted,
        rows,
    })
}

/// Apply the plan to workspace `ws` as ONE atomic batch. Either every bootstrap row lands or none
/// does — there is no partially-bootstrapped intermediate inside the namespace.
pub async fn apply_bootstrap(
    store: &Store,
    ws: &str,
    plan: &BootstrapPlan,
) -> Result<(), StoreError> {
    let upserts: Vec<UpsertBatch<'_>> = plan
        .rows
        .iter()
        .map(|r| UpsertBatch {
            table: r.table,
            id: &r.id,
            value: &r.value,
        })
        .collect();
    write_batch(store, ws, &upserts, &[]).await
}

/// Normalize a first-admin subject to the `user:{name}` form the membership/grant keys use.
pub fn normalize_admin(sub: &str) -> String {
    format!("user:{}", sub.trim_start_matches("user:"))
}
