//! **Teams** as a first-class authz primitive: a `team(team, name)` record (authz-grants scope),
//! promoting the S4 implicit `member` edge into a named entity an admin can list and manage.
//!
//! The membership *edges* (`team -[member]-> user`) stay in `lb_assets` relations — `team_create`
//! only records the team's existence + display name so it is listable; `lb_host::members` adds/
//! removes the edges. `create` is idempotent on the team id (re-creating updates the display name).
//! Workspace-namespaced (§7); raw, no authorization here.

use lb_store::{list as store_list, write, Store, StoreError};
use serde::{Deserialize, Serialize};

/// The store table team records live in, within a workspace namespace.
pub const TEAM_TABLE: &str = "team";

/// The constant `kind` discriminant so [`team_list`] can equality-filter every team row.
const KIND: &str = "team";

/// The `kind` a deleted team record carries. `teams.delete` upserts this tombstone (the store has no
/// row-delete; tombstone is the sync-idempotent §6.8 choice); `team_list`'s `kind`-equality filter
/// excludes it, so a deleted team vanishes from the listing without a row-delete.
pub const TOMBSTONE: &str = "__deleted__";

/// A team: its id (the value the `member` edge's `a` uses) and a human display name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Team {
    /// The team id — the same string the `member` edge keys on.
    pub team: String,
    /// A human-friendly display name.
    pub name: String,
    /// Constant discriminant so `team_list` selects every row.
    pub kind: String,
}

impl Team {
    pub fn new(team: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            team: team.into(),
            name: name.into(),
            kind: KIND.to_string(),
        }
    }
}

/// Create (or update the display name of) team `team` in workspace `ws`. Idempotent on the id.
pub async fn team_create(
    store: &Store,
    ws: &str,
    team: &str,
    name: &str,
) -> Result<(), StoreError> {
    let record = Team::new(team, name);
    let value = serde_json::to_value(&record).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, TEAM_TABLE, team, &value).await
}

/// Tombstone team `team` in workspace `ws` (the record half of `teams.delete` — the host service
/// also cascades the member edges + grant revoke). Idempotent; deleting an absent team is a no-op.
pub async fn team_delete(store: &Store, ws: &str, team: &str) -> Result<(), StoreError> {
    let tombstone = serde_json::json!({ "kind": TOMBSTONE, "team": team });
    write(store, ws, TEAM_TABLE, team, &tombstone).await
}

/// Every team in workspace `ws` (for the teams admin read surface + the cap resolver's team walk).
pub async fn team_list(store: &Store, ws: &str) -> Result<Vec<Team>, StoreError> {
    let rows = store_list(store, ws, TEAM_TABLE, "kind", KIND).await?;
    rows.into_iter()
        .map(|v| serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string())))
        .collect()
}
