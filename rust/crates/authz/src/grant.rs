//! The **grant store**: `grant(subject -> cap)` records — the authoritative source of what Gate 2
//! checks, not the token alone (authz-grants scope). `assign` / `revoke` / `list`, raw and
//! workspace-namespaced (authorization is the host service).
//!
//! Identity is `(subject, cap)`: re-assigning the same cap upserts (idempotent); `revoke` writes a
//! tombstone so the row syncs idempotently (§6.8) rather than vanishing under a peer — the same
//! revoke-as-upsert-to-tombstone choice `lb_assets::unrelate` makes. `list` filters on the flat
//! `subject` column, so a subject's grants are one `store::list`.

use lb_store::{list as store_list, read, write, Store, StoreError};
use serde::{Deserialize, Serialize};

use crate::subject::Subject;

/// The store table grants live in, within a workspace namespace.
pub const GRANT_TABLE: &str = "grant";

/// The marker a revoked grant carries; [`grant_list`] / [`granted`] treat it as absent. Mirrors
/// `lb_assets`'s relation tombstone so the sync apply is uniform.
const TOMBSTONE: &str = "__revoked__";

/// A durable grant: `subject` holds `cap`, in some workspace. `subject` is denormalized to its
/// `kind:name` string so listing a subject's grants is one field-equality query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grant {
    /// The grant target (`user:ada` / `team:facilities` / `role:operator`).
    pub subject: Subject,
    /// The capability string granted (e.g. `mcp:hvac.setpoint:call`).
    pub cap: String,
}

impl Grant {
    pub fn new(subject: Subject, cap: impl Into<String>) -> Self {
        Self {
            subject,
            cap: cap.into(),
        }
    }
}

/// Stable record id for `(subject, cap)`. `::` separates the parts; subject keys use a single `:`
/// and caps use `:`/`.`/`/` — none contain `::`, so the key is unambiguous.
fn grant_id(subject: &Subject, cap: &str) -> String {
    format!("{}::{}", subject.as_key(), cap)
}

/// Assign `cap` to `subject` in workspace `ws`. Idempotent (re-assign upserts the same row).
pub async fn grant_assign(
    store: &Store,
    ws: &str,
    subject: &Subject,
    cap: &str,
) -> Result<(), StoreError> {
    let grant = Grant::new(subject.clone(), cap);
    let value = serde_json::to_value(&grant).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, GRANT_TABLE, &grant_id(subject, cap), &value).await
}

/// Revoke `cap` from `subject` in workspace `ws`. Idempotent; a never-assigned grant revokes to
/// the same tombstone harmlessly. Tombstoned (not deleted) so it replays cleanly under sync.
pub async fn grant_revoke(
    store: &Store,
    ws: &str,
    subject: &Subject,
    cap: &str,
) -> Result<(), StoreError> {
    let tombstone = serde_json::json!({ "subject": subject.as_key(), "cap": TOMBSTONE });
    write(store, ws, GRANT_TABLE, &grant_id(subject, cap), &tombstone).await
}

/// Every live cap `subject` holds directly in workspace `ws` (tombstoned grants skipped). Empty if
/// none — never another workspace's grants (the namespace wall, §7).
pub async fn grant_list(
    store: &Store,
    ws: &str,
    subject: &Subject,
) -> Result<Vec<String>, StoreError> {
    let rows = store_list(store, ws, GRANT_TABLE, "subject", &subject.as_key()).await?;
    let mut caps = Vec::new();
    for v in rows {
        if v.get("cap").and_then(|c| c.as_str()) == Some(TOMBSTONE) {
            continue;
        }
        let grant: Grant =
            serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string()))?;
        caps.push(grant.cap);
    }
    Ok(caps)
}

/// Does `subject` hold `cap` live in `ws`? A point check for callers (and tests) that need one
/// grant rather than the whole list.
pub async fn granted(
    store: &Store,
    ws: &str,
    subject: &Subject,
    cap: &str,
) -> Result<bool, StoreError> {
    match read(store, ws, GRANT_TABLE, &grant_id(subject, cap)).await? {
        Some(v) => Ok(v.get("cap").and_then(|c| c.as_str()) != Some(TOMBSTONE)),
        None => Ok(false),
    }
}
