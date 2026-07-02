//! The durable **`native_status`** projection — the workspace's record of a sidecar's lifecycle
//! intent + restart count (native-tier scope). The live PID is runtime-only (the `SidecarMap`); THIS
//! is the durable state a restart re-derives from, so no durable state is lost across a kill+respawn
//! (the stateless-extension guarantee). SurrealDB records in the workspace namespace → structurally
//! workspace-isolated (a ws-B read sees no ws-A status, §7).
//!
//! Why a projection beside the S4 `Install` record (not a new authority): `Install` answers "what is
//! this extension allowed here" (the grant); `native_status` answers "what should be running, and
//! how many times has it crashed" — the supervision intent + a counter. Two small records, one per
//! concern (FILE-LAYOUT), both workspace-namespaced.

use lb_store::{read, write, Store, StoreError};
use serde::{Deserialize, Serialize};

/// The cache table within a workspace namespace. One place owns the name.
pub(crate) const TABLE: &str = "native_status";

/// The lifecycle a native sidecar should be in. Durable intent — a boot reconciler (follow-up) would
/// re-spawn every `Started` sidecar from these records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lifecycle {
    Started,
    Stopped,
}

/// The durable status of one native extension in a workspace: its intended lifecycle and how many
/// times the supervisor has restarted it. `ts` is the injected logical timestamp of the last change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeStatus {
    pub ext_id: String,
    pub version: String,
    pub lifecycle: Lifecycle,
    pub restart_count: u32,
    /// The logical timestamp from which this sidecar has been serving calls cleanly (set on install
    /// and on each restart, since a restart re-opens the fault window). The cool-off clock for decay:
    /// once a successful call observes `now - healthy_since >= cooloff`, the restart count decays to
    /// zero (a transient crash stops permanently poisoning the budget). `None` before the first
    /// timestamped health signal (legacy records deserialize to `None`, treated as "just became
    /// healthy" on the next call).
    #[serde(default)]
    pub healthy_since: Option<u64>,
    pub ts: u64,
}

impl NativeStatus {
    pub fn new(ext_id: impl Into<String>, version: impl Into<String>, ts: u64) -> Self {
        Self {
            ext_id: ext_id.into(),
            version: version.into(),
            lifecycle: Lifecycle::Started,
            restart_count: 0,
            healthy_since: Some(ts),
            ts,
        }
    }
}

/// Persist `status` for its extension in workspace `ws` (upsert, keyed by `ext_id`).
pub async fn record_status(
    store: &Store,
    ws: &str,
    status: &NativeStatus,
) -> Result<(), StoreError> {
    let value = serde_json::to_value(status).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, &status.ext_id, &value).await
}

/// Read the durable status for `ext_id` in workspace `ws`. `None` if this workspace has no native
/// status for it (never installed here, or installed in another workspace — invisible, §7).
pub async fn read_status(
    store: &Store,
    ws: &str,
    ext_id: &str,
) -> Result<Option<NativeStatus>, StoreError> {
    match read(store, ws, TABLE, ext_id).await? {
        Some(value) => {
            let s: NativeStatus =
                serde_json::from_value(value).map_err(|e| StoreError::Decode(e.to_string()))?;
            Ok(Some(s))
        }
        None => Ok(None),
    }
}
