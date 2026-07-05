//! The **reactor directory** — the durable set of workspaces the node's background reactors service, so
//! a workspace can be onboarded (or retired) **without restarting the node** (relocated from the retired
//! workflow driver, rules-workflow-convergence scope). The "dynamic workspace set" it solves is generic:
//! the outbox relay reactor, the flow-approval reactor, and the flow/agent reactors all need a live
//! workspace list. A reactor re-reads this each tick; `register`/`deregister` mutate it at runtime; it
//! survives a restart because it is a record, not in-memory config.
//!
//! It lives in a **reserved namespace** ([`DIRECTORY_NS`]), not inside any tenant workspace: it is
//! node-level operator config (which workspaces this node drives), not a workspace's own data — and a
//! per-workspace directory is a chicken-and-egg (you would need to know a workspace to find the list
//! of workspaces). The reserved name is disallowed as a real workspace by operator convention, so it
//! can never collide with a tenant's namespace. This is the one deliberate exception to "every key is
//! workspace-scoped" — and it is exactly the kind of node-infrastructure state §7 carves out (like the
//! relay loop's existence): the *entries* still name real workspaces, and everything a reactor then does
//! with an entry (relay, approval-release) re-enters that workspace's namespace and its caps gate.
//!
//! Raw verbs — no MCP surface (the directory is infrastructure, like the relay, not a tool). A node's
//! operator/boot wiring calls these; they are not reachable through a workspace token.

use serde::{Deserialize, Serialize};

use lb_store::{list, read, write, Store, StoreError};

/// The reserved namespace the directory lives in. Leading underscore marks it node-internal; operators
/// must not name a real workspace this.
pub const DIRECTORY_NS: &str = "_lb_workflow_directory";

/// The table within that namespace.
const TABLE: &str = "workspace";

/// Whether a directory entry is currently serviced. Stored as a **kebab-case string** discriminant
/// (not a bool) so the generic store equality filter — which binds the compared value as a string —
/// can select on it (the same reason the outbox stores its `status` as a string, see `pending.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EntryStatus {
    Enabled,
    Disabled,
}

/// One workspace the driver services: its id, the channel job progress streams to, and its status (a
/// soft-disable that keeps the row for audit/re-enable without losing config). Deliberately
/// **secret-free** — the service principal is minted from caps by the binary at tick time, so no
/// credential is persisted here (that, and per-tenant webhook secrets, ride `lb-secrets`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceEntry {
    pub ws: String,
    pub channel: String,
    pub status: EntryStatus,
    /// Caller-injected logical timestamp (no wall-clock in the crate — testing §3).
    pub ts: u64,
}

impl WorkspaceEntry {
    pub fn new(ws: impl Into<String>, channel: impl Into<String>, ts: u64) -> Self {
        Self {
            ws: ws.into(),
            channel: channel.into(),
            status: EntryStatus::Enabled,
            ts,
        }
    }
}

/// Register (upsert) a workspace into the directory — enabled. Idempotent on `ws`: re-registering
/// updates the channel/ts and re-enables. Takes effect on the driver's next tick (no restart).
pub async fn register(store: &Store, ws: &str, channel: &str, ts: u64) -> Result<(), StoreError> {
    let entry = WorkspaceEntry::new(ws, channel, ts);
    let value = serde_json::to_value(&entry).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, DIRECTORY_NS, TABLE, ws, &value).await
}

/// Soft-disable a workspace: the row stays (audit / easy re-enable) but its status becomes
/// `Disabled`, so [`enabled_workspaces`] stops returning it and the driver stops servicing it next tick.
pub async fn deregister(store: &Store, ws: &str, ts: u64) -> Result<(), StoreError> {
    let entry = match read(store, DIRECTORY_NS, TABLE, ws).await? {
        Some(v) => {
            let mut e: WorkspaceEntry =
                serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string()))?;
            e.status = EntryStatus::Disabled;
            e.ts = ts;
            e
        }
        // Deregistering an unknown workspace is a no-op recorded as a disabled row (idempotent).
        None => WorkspaceEntry {
            ws: ws.into(),
            channel: String::new(),
            status: EntryStatus::Disabled,
            ts,
        },
    };
    let value = serde_json::to_value(&entry).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, DIRECTORY_NS, TABLE, ws, &value).await
}

/// Every **enabled** workspace entry, oldest→newest by `ts` — what the driver builds its bindings from
/// each tick. A disabled (deregistered) entry is excluded.
pub async fn enabled_workspaces(store: &Store) -> Result<Vec<WorkspaceEntry>, StoreError> {
    let rows = list(store, DIRECTORY_NS, TABLE, "status", "enabled").await?;
    let mut entries: Vec<WorkspaceEntry> = rows
        .into_iter()
        .map(|v| serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string())))
        .collect::<Result<_, _>>()?;
    entries.sort_by_key(|e| e.ts);
    Ok(entries)
}
