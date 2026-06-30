//! The **global identity directory** — one record per person, in a reserved **system namespace**
//! `_lb_identity` (global-identity scope, decision #1). NOT a workspace: identity is the one thing
//! lifted out of the per-workspace wall, hub-writable and resolution-read-only, carrying no tenant
//! data — only `{sub, display_name?, created_ts}`. Mirrors the shipped reserved-namespace pattern
//! (`_lb_workflow_directory` / `_lb_workspaces`): the leading underscore marks it system-internal,
//! disallowed as a real workspace by operator convention.
//!
//! `sub` is the human handle (`user:ada`), **globally unique** (decision #6 — keeping it avoids
//! retrofitting every existing `Subject::User(sub)` grant row). `display_name` is a separate,
//! non-unique, per-identity field. No credential in v1 (decision #7) — the dev-login sits behind the
//! identity-resolution seam; OIDC attaches additively later.
//!
//! Raw verbs, no authorization here — the host `identity` service is the capability chokepoint
//! (`mcp:identity.manage:call`). Hub-only writes (decision #8); edges verify tokens offline and read
//! cached identity only. One verb per file would over-split a 4-verb record family; this is the raw
//! store layer (the host service is split per verb).

use lb_store::{list as store_list, read, write, Store, StoreError};
use serde::{Deserialize, Serialize};

/// The reserved system namespace the identity directory lives in. Leading underscore marks it
/// system-internal; operators must not name a real workspace this (the same convention as
/// `_lb_workflow_directory` / `_lb_workspaces`).
pub const IDENTITY_NS: &str = "_lb_identity";

/// The table within that namespace.
pub const IDENTITY_TABLE: &str = "identity";

/// The constant `kind` discriminant so [`identity_list`] can equality-filter every row.
pub const IDENTITY_KIND: &str = "identity";

/// A global identity: the globally-unique `sub` (the grant key, `user:ada`), an optional non-unique
/// display name, and the created timestamp. Deliberately secret-free — no credential field (decision
/// #7); the real IdP attaches behind a future `cred_ref` additive slice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    /// The globally-unique principal id (`user:ada`) — the same across workspaces; grants key on it.
    pub sub: String,
    /// A non-unique, per-identity human display name. Separate from `sub` (decision #6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Constant discriminant so `identity_list` selects every row.
    pub kind: String,
    /// Caller-injected logical timestamp (no wall-clock — testing §3).
    pub created_ts: u64,
}

impl Identity {
    pub fn new(sub: impl Into<String>, display_name: Option<String>, created_ts: u64) -> Self {
        Self {
            sub: sub.into(),
            display_name,
            kind: IDENTITY_KIND.to_string(),
            created_ts,
        }
    }
}

/// Create (or upsert) global identity `sub`. Idempotent on `sub` (re-creating updates the display
/// name / created_ts — last write wins). Provisions in NO workspace (decision #4 — joining is
/// `membership.add`).
pub async fn identity_create(
    store: &Store,
    sub: &str,
    display_name: Option<&str>,
    created_ts: u64,
) -> Result<Identity, StoreError> {
    let identity = Identity::new(sub, display_name.map(|s| s.to_string()), created_ts);
    let value = serde_json::to_value(&identity).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, IDENTITY_NS, IDENTITY_TABLE, sub, &value).await?;
    Ok(identity)
}

/// Read global identity `sub`, or `None` if it does not exist. Read-only (resolution path).
pub async fn identity_get(store: &Store, sub: &str) -> Result<Option<Identity>, StoreError> {
    let Some(value) = read(store, IDENTITY_NS, IDENTITY_TABLE, sub).await? else {
        return Ok(None);
    };
    let identity: Identity =
        serde_json::from_value(value).map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(Some(identity))
}

/// Every global identity, sorted by `sub` for a stable listing (testing §3).
pub async fn identity_list(store: &Store) -> Result<Vec<Identity>, StoreError> {
    let rows = store_list(store, IDENTITY_NS, IDENTITY_TABLE, "kind", IDENTITY_KIND).await?;
    let mut identities: Vec<Identity> = rows
        .into_iter()
        .map(|v| serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string())))
        .collect::<Result<_, _>>()?;
    identities.sort_by(|a, b| a.sub.cmp(&b.sub));
    Ok(identities)
}
