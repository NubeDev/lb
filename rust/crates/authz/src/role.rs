//! **Roles**: `role(name -> caps[])` records — named cap bundles a workspace can define beyond the
//! three built-ins (authz-grants scope). `define` / `list` / `caps`, raw and workspace-namespaced.
//!
//! Assigning a role to a user/team is *not* a verb here — it is an ordinary `grant_assign` of the
//! synthetic cap `role:<name>` to that subject; [`resolve_caps`](crate::resolve_caps) expands it.
//! Keeping role-assignment as a grant means one assign/revoke path, not two. `define` is idempotent
//! on the role name (re-defining replaces the cap set — last write wins).
//!
//! Built-in roles (super-admin / workspace-admin / member) are seeded by the caller, not here; a
//! custom role may only bundle caps the definer themselves holds (the no-widening rule lives in the
//! host service, which has the principal — this crate is the raw store).

use lb_store::{
    list as store_list, read, write, write_batch, DeleteBatch, Store, StoreError, UpsertBatch,
};
use serde::{Deserialize, Serialize};

use crate::grant::{grant_id, Grant, TOMBSTONE};

/// The store table roles live in, within a workspace namespace.
pub const ROLE_TABLE: &str = "role";

/// The constant `kind` discriminant so [`role_list`] can equality-filter every role row.
const KIND: &str = "role";

/// A role: a named bundle of capability strings, workspace-scoped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Role {
    /// The role name (`operator`, `auditor`).
    pub name: String,
    /// The capability strings this role bundles.
    pub caps: Vec<String>,
    /// Constant discriminant so `role_list` selects every row.
    pub kind: String,
}

impl Role {
    pub fn new(name: impl Into<String>, caps: Vec<String>) -> Self {
        Self {
            name: name.into(),
            caps,
            kind: KIND.to_string(),
        }
    }
}

/// Define (or replace) role `name` with `caps` in workspace `ws`. Idempotent on the name.
pub async fn role_define(
    store: &Store,
    ws: &str,
    name: &str,
    caps: &[String],
) -> Result<(), StoreError> {
    let role = Role::new(name, caps.to_vec());
    let value = serde_json::to_value(&role).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, ROLE_TABLE, name, &value).await
}

/// The caps role `name` bundles in `ws`, or empty if the role is undefined (an undefined role
/// contributes nothing — deny by default).
pub async fn role_caps(store: &Store, ws: &str, name: &str) -> Result<Vec<String>, StoreError> {
    match read(store, ws, ROLE_TABLE, name).await? {
        Some(v) => {
            let role: Role =
                serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string()))?;
            Ok(role.caps)
        }
        None => Ok(Vec::new()),
    }
}

/// Every role defined in workspace `ws` (for the roles/grants admin read surface).
pub async fn role_list(store: &Store, ws: &str) -> Result<Vec<Role>, StoreError> {
    let rows = store_list(store, ws, ROLE_TABLE, "kind", KIND).await?;
    rows.into_iter()
        .map(|v| serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string())))
        .collect()
}

/// Delete role `name` from workspace `ws` AND un-assign it from every subject holding a live
/// `role:<name>` grant — all in ONE store transaction (the access-console `roles.delete` cascade).
/// Idempotent: a repeat delete (role gone, no assignees) is a no-op success returning 0. Returns
/// the number of subjects whose `role:<name>` grant was tombstoned (the consequence count the UI
/// shows before confirm).
///
/// The cascade finds assignees by listing the grant rows whose `cap` is exactly `role:<name>`
/// (tombstoned grants carry `__revoked__`, so they never match), then tombstones each in the same
/// batch that deletes the role record. Bounded by [`MAX_BATCH`](lb_store::MAX_BATCH); a role with
/// more assignees than the batch cap fails fast rather than holding an unbounded transaction.
pub async fn role_delete(store: &Store, ws: &str, name: &str) -> Result<usize, StoreError> {
    let cap = format!("role:{name}");
    // The live `role:<name>` grants = every grant row whose stored `cap` equals it.
    let rows = store_list(store, ws, crate::grant::GRANT_TABLE, "cap", &cap).await?;
    // Collect the cascade writes as OWNED data first, so the batch can borrow stable storage.
    // Each entry is the tombstone value + its grant record id (the cascade un-assigns the role).
    let mut tombstones: Vec<(String, serde_json::Value)> = Vec::with_capacity(rows.len());
    for v in &rows {
        let grant: Grant =
            serde_json::from_value(v.clone()).map_err(|e| StoreError::Decode(e.to_string()))?;
        let tombstone = serde_json::json!({ "subject": grant.subject.as_key(), "cap": TOMBSTONE });
        tombstones.push((grant_id(&grant.subject, &cap, &grant.scope), tombstone));
    }
    let affected = tombstones.len();
    let upserts: Vec<UpsertBatch<'_>> = tombstones
        .iter()
        .map(|(id, tombstone)| UpsertBatch {
            table: crate::grant::GRANT_TABLE,
            id: id.as_str(),
            value: tombstone,
        })
        .collect();
    let deletes = vec![DeleteBatch {
        table: ROLE_TABLE,
        id: name,
    }];
    // An empty upsert set is fine: write_batch requires total>0, and the role delete keeps it ≥1.
    if upserts.is_empty() {
        write_batch(store, ws, &[], &deletes).await?;
    } else {
        write_batch(store, ws, &upserts, &deletes).await?;
    }
    Ok(affected)
}
