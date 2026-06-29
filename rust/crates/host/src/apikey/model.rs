//! The `apikey` record + the credential-free views the admin verbs return (api-keys scope). One row
//! per `(ws, id)` at `apikey:{ws}:{id}` in the workspace's own namespace (the hard wall, §7). The
//! record carries the **peppered hash** (`HMAC-SHA256(pepper, secret_field)`, hex) — never the raw
//! secret, which leaves the host exactly once at create. A revoked key is a `status =
//! "__revoked__"` tombstone (sync-idempotent, §6.8 — like the grant/user tombstones), not a delete.
//!
//! The `kind` field is the labelling `appliance | cli | api | agent` tag — **labelling, not a
//! security boundary** (the resolved caps are always the boundary). `kind_discrim` is the constant
//! list-filter discriminant (every row carries `"apikey"` so [`lb_store::list`] selects them all).

use lb_authz::Subject;
use serde::{Deserialize, Serialize};

/// The store table apikey records live in, within a workspace namespace.
pub const TABLE: &str = "apikey";

/// The constant `kind_discrim` discriminant so [`apikey_list`](super::list) selects every row.
pub const KIND_DISCRIM: &str = "apikey";

/// The `status` a revoked key carries. Auth treats it as absent (refused); `list` surfaces it as
/// "revoked" for audit. Tombstoned (not deleted) so it replays cleanly under sync.
pub const TOMBSTONE_STATUS: &str = "__revoked__";

/// The label-shaped `kind` values (labelling only). Not enforced as exhaustive — a custom kind
/// string is stored verbatim — but these are the recognized defaults.
pub const KINDS: &[&str] = &["appliance", "cli", "api", "agent"];

/// A stored API key: identity + label + the peppered hash of its secret. The raw secret is NEVER
/// here (shown once at create); only `key_hash` is persisted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiKeyRecord {
    /// The key id (Crockford base32, no `_`/`.`). The record id and the `key:{id}` grant subject.
    pub id: String,
    /// The workspace this key is walled to (mirrors the namespace; the wall is the namespace).
    pub ws: String,
    /// A human label (`rooftop-hvac`). Display only.
    pub label: String,
    /// The labelling kind (`appliance | cli | api | agent`). NOT a security boundary.
    pub kind: String,
    /// `HMAC-SHA256(pepper, secret_field)` as 64-char hex — never the secret.
    pub key_hash: String,
    /// The non-secret display stub `lbk_{ws}.{id}` for the list view.
    pub prefix: String,
    /// `"active"` or `"__revoked__"` (the tombstone).
    pub status: String,
    /// Caller-injected creation timestamp (no wall-clock — testing §3).
    pub created_ts: u64,
    /// The expiry instant (unix secs); `0` means never expires. Checked lazily at auth.
    pub expires_at: u64,
    /// Constant `"apikey"` so [`lb_store::list`] filters every row.
    pub kind_discrim: String,
    /// The list order key (== `created_ts`).
    pub ts: u64,
}

impl ApiKeyRecord {
    /// Build a fresh active record. `ts` is the caller-injected logical clock (no wall-clock).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        ws: impl Into<String>,
        label: impl Into<String>,
        kind: impl Into<String>,
        key_hash: impl Into<String>,
        prefix: impl Into<String>,
        created_ts: u64,
        expires_at: u64,
    ) -> Self {
        Self {
            id: id.into(),
            ws: ws.into(),
            label: label.into(),
            kind: kind.into(),
            key_hash: key_hash.into(),
            prefix: prefix.into(),
            status: "active".to_string(),
            created_ts,
            expires_at,
            kind_discrim: KIND_DISCRIM.to_string(),
            ts: created_ts,
        }
    }

    /// Is this record revoked (tombstoned)?
    pub fn is_revoked(&self) -> bool {
        self.status == TOMBSTONE_STATUS
    }

    /// Has this record expired at logical time `now`? `expires_at == 0` means never.
    pub fn is_expired(&self, now: u64) -> bool {
        self.expires_at != 0 && now >= self.expires_at
    }
}

/// The `key:{id}` grant subject for a key record.
pub fn key_subject(id: &str) -> Subject {
    Subject::Key(id.to_string())
}

/// The credential-free list view — `id`, label, kind, the non-secret `prefix`, status, timing, the
/// assigned role names, and the read-only/read-write/custom badge. Carries **no hash and no secret**
/// (asserted in a test): `user.list`/`apikey.list` must never enumerate a credential.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiKeyView {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub prefix: String,
    pub status: String,
    pub created_ts: u64,
    pub expires_at: u64,
    /// The role names assigned to this key (from its `role:` grants).
    pub roles: Vec<String>,
    /// `"read-only"` | `"read-write"` | `"custom"` (derived from the assigned roles).
    pub badge: String,
}

/// The `apikey.get` view: the list view PLUS the full resolved cap set. Still no hash/secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiKeyFull {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub prefix: String,
    pub status: String,
    pub created_ts: u64,
    pub expires_at: u64,
    pub roles: Vec<String>,
    pub badge: String,
    /// The key's full resolved cap set (direct grants + role expansion).
    pub caps: Vec<String>,
}
