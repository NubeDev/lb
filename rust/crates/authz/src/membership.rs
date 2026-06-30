//! The **workspace membership** roster — `membership:{sub}` = `{sub, joined_ts}` in the workspace's
//! own namespace (global-identity scope, decision #2). This is the single source of truth for "who is
//! in this workspace"; the Access console People tab, teams, and the login resolver all read it. Role
//! is grant-driven (NO `role_hint` field) — on join the system grants the built-in `member` role
//! (decision #2); an admin grants more.
//!
//! `sub` is the global identity handle (`user:ada`) — the same key grants use. Leaving writes a
//! tombstone (not a row-delete) so the change replays idempotently under sync (§6.8) and a stale
//! synced edge cannot resurrect a removed member — the same discipline `grant_revoke` /
//! `assets::unrelate` follow.
//!
//! Raw verbs, no authorization here — the host `membership` service is the capability chokepoint
//! (`mcp:members.manage:call`).

use lb_store::{list as store_list, read, write, Store, StoreError};
use serde::{Deserialize, Serialize};

/// The store table membership records live in, within a workspace namespace.
pub const MEMBERSHIP_TABLE: &str = "membership";

/// The constant `kind` discriminant so [`membership_list`] can equality-filter every row.
pub const MEMBERSHIP_KIND: &str = "membership";

/// The `kind` a removed (left) membership carries. The store has no row-delete, so `membership_remove`
/// upserts this tombstone (sync-idempotent, §6.8); reads treat a tombstoned row as absent. A stale
/// synced edge re-applies the same tombstone, never resurrecting the member.
pub const MEMBERSHIP_TOMBSTONE: &str = "__left__";

/// A membership: the global `sub` and the join timestamp. The `(ws, sub)` pair is the membership; the
/// role is a grant, not a field (decision #2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Membership {
    /// The global identity handle (`user:ada`) — the same key grants use.
    pub sub: String,
    /// Constant discriminant so `membership_list` selects every row.
    pub kind: String,
    /// Caller-injected logical join timestamp (no wall-clock — testing §3).
    pub joined_ts: u64,
}

impl Membership {
    pub fn new(sub: impl Into<String>, joined_ts: u64) -> Self {
        Self {
            sub: sub.into(),
            kind: MEMBERSHIP_KIND.to_string(),
            joined_ts,
        }
    }
}

/// Add (or re-add) `sub` to workspace `ws`. Idempotent upsert — re-joining refreshes `joined_ts`.
pub async fn membership_add_raw(
    store: &Store,
    ws: &str,
    sub: &str,
    joined_ts: u64,
) -> Result<Membership, StoreError> {
    let membership = Membership::new(sub, joined_ts);
    let value = serde_json::to_value(&membership).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, MEMBERSHIP_TABLE, sub, &value).await?;
    Ok(membership)
}

/// Remove `sub` from workspace `ws` — writes the tombstone. Idempotent. Reads then treat the row as
/// absent. The host verb composes this with the shipped `revoke_subject` + `token_revoke_mark`.
pub async fn membership_remove_raw(store: &Store, ws: &str, sub: &str) -> Result<(), StoreError> {
    let tombstone = serde_json::json!({ "sub": sub, "kind": MEMBERSHIP_TOMBSTONE, "joined_ts": 0 });
    write(store, ws, MEMBERSHIP_TABLE, sub, &tombstone).await
}

/// The live membership for `sub` in `ws`, or `None` if absent or tombstoned (left).
pub async fn membership_get(
    store: &Store,
    ws: &str,
    sub: &str,
) -> Result<Option<Membership>, StoreError> {
    let Some(value) = read(store, ws, MEMBERSHIP_TABLE, sub).await? else {
        return Ok(None);
    };
    if value.get("kind").and_then(|k| k.as_str()) == Some(MEMBERSHIP_TOMBSTONE) {
        return Ok(None);
    }
    let membership: Membership =
        serde_json::from_value(value).map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(Some(membership))
}

/// Is `sub` a live member of `ws`? `false` if absent or tombstoned.
pub async fn membership_is_member(store: &Store, ws: &str, sub: &str) -> Result<bool, StoreError> {
    Ok(membership_get(store, ws, sub).await?.is_some())
}

/// Every live membership in `ws` (tombstoned rows skipped), sorted by `sub` for a stable roster.
pub async fn membership_list(store: &Store, ws: &str) -> Result<Vec<Membership>, StoreError> {
    let rows = store_list(store, ws, MEMBERSHIP_TABLE, "kind", MEMBERSHIP_KIND).await?;
    let mut members: Vec<Membership> = rows
        .into_iter()
        .map(|v| serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string())))
        .collect::<Result<_, _>>()?;
    members.sort_by(|a, b| a.sub.cmp(&b.sub));
    Ok(members)
}

/// Does workspace `ws` have ANY live member? Used by the login path to tell a brand-new (empty)
/// workspace — which the first login bootstraps as `workspace-admin` (decision #3) — from one that
/// already has a roster the requester must be added to (decision #4).
pub async fn membership_has_any(store: &Store, ws: &str) -> Result<bool, StoreError> {
    Ok(!membership_list(store, ws).await?.is_empty())
}
