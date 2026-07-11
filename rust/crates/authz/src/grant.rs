//! The **grant store**: `grant(subject -> cap, scope)` records — the authoritative source of what
//! Gate 2 checks, not the token alone (authz-grants scope). `assign` / `revoke` / `list`, raw and
//! workspace-namespaced (authorization is the host service).
//!
//! Identity is `(subject, cap, scope)`: re-assigning the same cap+scope upserts (idempotent);
//! `revoke` writes a tombstone so the row syncs idempotently (§6.8) rather than vanishing under a
//! peer — the same revoke-as-upsert-to-tombstone choice `lb_assets::unrelate` makes. `list` filters
//! on the flat `subject` column, so a subject's grants are one `store::list`.
//!
//! **Entity-scoped grants** (entity-scoped-grants scope): the `scope` field narrows a grant's reach
//! to a subset of a table's rows within the workspace. `All` (default) = today's behaviour; `Ids`
//! = only the listed ids. Old records (no `scope` field) deserialize to `All` with zero migration.

use lb_store::{list as store_list, read, write, Store, StoreError};
use serde::{Deserialize, Serialize};

use crate::scope::Scope;
use crate::subject::Subject;

/// The store table grants live in, within a workspace namespace.
pub const GRANT_TABLE: &str = "grant";

/// The marker a revoked grant carries; [`grant_list`] / [`granted`] treat it as absent. Mirrors
/// `lb_assets`'s relation tombstone so the sync apply is uniform. `pub(crate)` so the role-cascade
/// delete can write the same tombstone shape in its batch.
pub(crate) const TOMBSTONE: &str = "__revoked__";

/// A durable grant: `subject` holds `cap`, in some workspace, optionally narrowed by `scope`.
/// `subject` is denormalized to its `kind:name` string so listing a subject's grants is one
/// field-equality query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grant {
    /// The grant target (`user:ada` / `team:facilities` / `role:operator`).
    pub subject: Subject,
    /// The capability string granted (e.g. `mcp:hvac.setpoint:call`).
    pub cap: String,
    /// The entity-scope selector (entity-scoped-grants scope). `All` = every row (today's
    /// behaviour); `Ids` = only the listed ids in `table`. Defaults to `All` so old records
    /// deserialize with zero migration.
    #[serde(default, skip_serializing_if = "Scope::is_all")]
    pub scope: Scope,
}

impl Grant {
    pub fn new(subject: Subject, cap: impl Into<String>) -> Self {
        Self {
            subject,
            cap: cap.into(),
            scope: Scope::All,
        }
    }

    /// Construct with an explicit scope (entity-scoped-grants scope).
    pub fn with_scope(subject: Subject, cap: impl Into<String>, scope: Scope) -> Self {
        Self {
            subject,
            cap: cap.into(),
            scope,
        }
    }
}

/// Stable record id for `(subject, cap, scope)`. `::` separates the parts; subject keys use a
/// single `:` and caps use `:`/`.`/`/` — none contain `::`, so the key is unambiguous. For `All`
/// scope the key is `subject::cap` (unchanged from pre-scope — backward compat). `pub(crate)` so
/// the role-cascade delete can address the same grant rows in its batch.
pub(crate) fn grant_id(subject: &Subject, cap: &str, scope: &Scope) -> String {
    let base = format!("{}::{}", subject.as_key(), cap);
    match scope.key().is_empty() {
        true => base,
        false => format!("{base}::{}", scope.key()),
    }
}

/// Assign `cap` to `subject` in workspace `ws` (scope `All` — today's behaviour). Idempotent.
pub async fn grant_assign(
    store: &Store,
    ws: &str,
    subject: &Subject,
    cap: &str,
) -> Result<(), StoreError> {
    grant_assign_scoped(store, ws, subject, cap, &Scope::All).await
}

/// Assign `cap` to `subject` in workspace `ws` with `scope` (entity-scoped-grants scope).
/// Idempotent (re-assign upserts the same row).
pub async fn grant_assign_scoped(
    store: &Store,
    ws: &str,
    subject: &Subject,
    cap: &str,
    scope: &Scope,
) -> Result<(), StoreError> {
    let grant = Grant::with_scope(subject.clone(), cap, scope.clone());
    let value = serde_json::to_value(&grant).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(
        store,
        ws,
        GRANT_TABLE,
        &grant_id(subject, cap, scope),
        &value,
    )
    .await
}

/// Revoke `cap` from `subject` in workspace `ws` (scope `All`). Idempotent; tombstoned (not
/// deleted) so it replays cleanly under sync.
pub async fn grant_revoke(
    store: &Store,
    ws: &str,
    subject: &Subject,
    cap: &str,
) -> Result<(), StoreError> {
    grant_revoke_scoped(store, ws, subject, cap, &Scope::All).await
}

/// Revoke `cap` with `scope` from `subject` in workspace `ws` (entity-scoped-grants scope).
/// Idempotent; tombstoned (not deleted) so it replays cleanly under sync.
pub async fn grant_revoke_scoped(
    store: &Store,
    ws: &str,
    subject: &Subject,
    cap: &str,
    scope: &Scope,
) -> Result<(), StoreError> {
    let tombstone = serde_json::json!({ "subject": subject.as_key(), "cap": TOMBSTONE });
    write(
        store,
        ws,
        GRANT_TABLE,
        &grant_id(subject, cap, scope),
        &tombstone,
    )
    .await
}

/// Every live cap `subject` holds directly in workspace `ws` (tombstoned grants skipped). Empty if
/// none — never another workspace's grants (the namespace wall, §7). Deduplicated: a subject
/// holding the same cap through multiple scoped grants sees the cap once (the resolver unions the
/// scopes).
pub async fn grant_list(
    store: &Store,
    ws: &str,
    subject: &Subject,
) -> Result<Vec<String>, StoreError> {
    let grants = grant_list_scoped(store, ws, subject).await?;
    let mut caps: Vec<String> = grants.into_iter().map(|g| g.cap).collect();
    caps.sort();
    caps.dedup();
    Ok(caps)
}

/// Every live grant `subject` holds directly in workspace `ws` — the full records including scope
/// (entity-scoped-grants scope). Tombstoned grants skipped. This is what the scoped resolver and
/// the revoke seam use; [`grant_list`] is the caps-only projection.
pub async fn grant_list_scoped(
    store: &Store,
    ws: &str,
    subject: &Subject,
) -> Result<Vec<Grant>, StoreError> {
    let rows = store_list(store, ws, GRANT_TABLE, "subject", &subject.as_key()).await?;
    let mut grants = Vec::new();
    for v in rows {
        if v.get("cap").and_then(|c| c.as_str()) == Some(TOMBSTONE) {
            continue;
        }
        let grant: Grant =
            serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string()))?;
        grants.push(grant);
    }
    Ok(grants)
}

/// Does `subject` hold `cap` live in `ws` (any scope)? A point check for callers (and tests) that
/// need one grant rather than the whole list. Checks the `All`-scope grant id (today's behaviour);
/// for scoped grants use [`grant_list_scoped`] or the scoped resolver.
pub async fn granted(
    store: &Store,
    ws: &str,
    subject: &Subject,
    cap: &str,
) -> Result<bool, StoreError> {
    match read(store, ws, GRANT_TABLE, &grant_id(subject, cap, &Scope::All)).await? {
        Some(v) => Ok(v.get("cap").and_then(|c| c.as_str()) != Some(TOMBSTONE)),
        None => Ok(false),
    }
}
