//! The `ce_appliance` registry record (control-engine scope, S4) — the workspace-scoped map from an
//! appliance **id** to the CE it names: a local CE on this node, or one owned by another enrolled LB
//! node. It is the extension's OWN table, written through the GENERIC `store:ce_appliance:*` verbs
//! (`store.write`/`store.query`/`store.delete`) — no host table code (the core-ignorance invariant).
//!
//! Physical key: `ce_appliance:{id}` in the workspace's SurrealDB namespace (the `{ws}` is the
//! namespace, not part of the id — the platform's structural workspace wall, README §7). So a ws-B
//! read of a ws-A appliance is a namespace miss → not-found, with no existence leak across the wall.

use serde::{Deserialize, Serialize};

/// The registry table name — the `store:ce_appliance:<action>` cap resource + the `store.*` `table` arg.
pub const TABLE: &str = "ce_appliance";

/// How an appliance's CE is reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// A CE on THIS node's localhost — the sidecar connects directly via `ce-client-rust` (`base`).
    Local,
    /// A CE owned by another enrolled LB node — reached by the host routing the `control-engine.*`
    /// call over Zenoh to that `node`, whose identical sidecar then serves it locally.
    Appliance,
}

/// A registered appliance. `node` is an enrolled machine principal's node id (`api-keys`
/// `kind="appliance"` + `edge-trust`, reused as-is); `base` is the CE's http(s) origin on THAT node's
/// localhost. `secret_ref` (S5) will point at a mediated CE credential; unset in S4.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Appliance {
    /// The stable, workspace-unique appliance id (the `store.*` record id + the `appliance` selector).
    pub id: String,
    /// A human name for the UI (never load-bearing for routing).
    pub name: String,
    /// Local (this node's CE) vs appliance (another node's CE).
    pub mode: Mode,
    /// The owning enrolled node's id. For `Local`, this node; for `Appliance`, the remote node.
    pub node: String,
    /// The CE origin on the owning node's localhost, e.g. `http://127.0.0.1:7979`.
    pub base: String,
    /// A future (S5) mediated-secret reference for the CE credential; `None` in S4.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_ref: Option<String>,
    /// The logical write timestamp (no wall-clock in the sidecar core; the host supplies it).
    pub ts: u64,
}

impl Appliance {
    /// Parse a `ce_appliance` record's JSON `data` (as returned by `store.query`) into an `Appliance`.
    pub fn from_data(v: &serde_json::Value) -> Result<Self, String> {
        serde_json::from_value(v.clone()).map_err(|e| format!("bad ce_appliance record: {e}"))
    }
}
