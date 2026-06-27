//! The workspace directory record (collaboration scope, slice 2).
//!
//! One row per workspace, in a reserved namespace. Small: the workspace id, a display name, and a
//! logical `ts`. The `kind` constant lets the generic store equality filter enumerate all of them
//! (the same trick the workflow directory / channel registry use). `ts` is caller-injected (no
//! wall-clock — testing §3).

use serde::{Deserialize, Serialize};

/// The reserved namespace the workspace directory lives in. Leading underscore marks it
/// node-internal; operators must not name a real workspace this.
pub const WORKSPACES_NS: &str = "_lb_workspaces";

/// The table within that namespace.
pub const TABLE: &str = "workspace";

/// The constant `kind` discriminant so `workspace_list` can equality-filter every row.
pub const KIND: &str = "workspace";

/// The `kind` a **hard-deleted (purged)** workspace carries. `workspace.delete --hard` upserts this
/// tombstone in the directory; sync apply must respect it so a stale synced edge can't resurrect the
/// workspace (admin-crud scope risk: "a resurrected workspace is a real isolation hole"). Soft
/// archive is a `status` flip on the live record; this is the irreversible directory tombstone.
pub const TOMBSTONE: &str = "__purged__";

/// A workspace's lifecycle status. `Active` is listable + mintable; `Archived` (soft-delete) is
/// hidden + un-mintable but reversible (data retained); a purge is the directory [`TOMBSTONE`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkspaceStatus {
    Active,
    Archived,
}

/// A workspace in the node's directory: its id (= the namespace), a human display name, and a
/// logical timestamp. Stable on `ws` — re-creating upserts (last name/ts win).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceRecord {
    /// The workspace id — the SurrealDB namespace, the hard wall (§7).
    pub ws: String,
    /// A human-friendly display name for the switcher.
    pub name: String,
    /// A constant discriminant (`workspace`) so `workspace_list` can select every row.
    pub kind: String,
    /// Lifecycle status. Defaults to `Active` for records written before this field existed
    /// (collaboration's `workspace_create` predates the admin-crud lifecycle).
    #[serde(default = "active_status")]
    pub status: WorkspaceStatus,
    /// Caller-injected logical timestamp (no wall-clock — testing §3).
    pub ts: u64,
}

fn active_status() -> WorkspaceStatus {
    WorkspaceStatus::Active
}

impl WorkspaceRecord {
    pub fn new(ws: impl Into<String>, name: impl Into<String>, ts: u64) -> Self {
        Self {
            ws: ws.into(),
            name: name.into(),
            kind: KIND.to_string(),
            status: WorkspaceStatus::Active,
            ts,
        }
    }
}
