//! The **global identity credential** — one argon2id password hash per *person*, in the reserved
//! `_lb_identity` namespace beside the identity record (email-login scope, the decision-#7 `cred_ref`
//! seam made real). This is the raw store layer: a `{sub, kind, phc, set_ts}` record keyed by `sub`,
//! read/written only through the mediated host verbs. The plaintext never lands here — the host
//! `credential` service hashes before `identity_credential_set` and compares in `credential_verify`.
//!
//! Contrast the shipped per-`(workspace, user)` `Credential` (`crates/host/src/credential/`): that
//! one lives in a *workspace* namespace and backs the legacy `POST /login`. THIS one is **global**
//! (one per identity, all workspaces) and lives in the system directory — a person has one password
//! everywhere, exactly like Slack. Secret-class (§6.7): never returned by any read/list/log.

use lb_store::{read, write, Store, StoreError};
use serde::{Deserialize, Serialize};

use crate::identity::IDENTITY_NS;

/// The table (within `_lb_identity`) global credential records live in.
pub const IDENTITY_CREDENTIAL_TABLE: &str = "identity_credential";

/// The constant `kind` discriminant (parity with the other reserved-namespace records).
pub const IDENTITY_CREDENTIAL_KIND: &str = "password";

/// A stored global credential: the identity it authenticates and the argon2id PHC hash. The plaintext
/// never lands here — the host verb hashes before write, `verify` compares against `phc`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityCredential {
    /// The global identity handle this credential authenticates (`user:ada`).
    pub sub: String,
    /// Constant discriminant (`password`). Leaves room for a future `oidc` kind behind the same seam.
    pub kind: String,
    /// The argon2id PHC hash string (`$argon2id$v=19$m=...$...$...`). Secret-class: salt embedded,
    /// never a plaintext, never returned by a read.
    pub phc: String,
    /// Caller-injected logical set timestamp (no wall-clock — testing §3).
    pub set_ts: u64,
}

impl IdentityCredential {
    pub fn new(sub: impl Into<String>, phc: impl Into<String>, set_ts: u64) -> Self {
        Self {
            sub: sub.into(),
            kind: IDENTITY_CREDENTIAL_KIND.to_string(),
            phc: phc.into(),
            set_ts,
        }
    }
}

/// Upsert the global credential for `sub` (rotation is last-write-wins). `phc` is an already-hashed
/// PHC string — this layer never sees a plaintext. Lands in the reserved `_lb_identity` namespace.
pub async fn identity_credential_set(
    store: &Store,
    sub: &str,
    phc: &str,
    set_ts: u64,
) -> Result<(), StoreError> {
    let record = IdentityCredential::new(sub, phc, set_ts);
    let value = serde_json::to_value(&record).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, IDENTITY_NS, IDENTITY_CREDENTIAL_TABLE, sub, &value).await
}

/// Read the stored PHC hash for `sub`, or `None` if no global credential is set. Read-only; the ONLY
/// reader is the login-path verify (the hash is never returned to a caller).
pub async fn identity_credential_phc(
    store: &Store,
    sub: &str,
) -> Result<Option<String>, StoreError> {
    let Some(value) = read(store, IDENTITY_NS, IDENTITY_CREDENTIAL_TABLE, sub).await? else {
        return Ok(None);
    };
    let record: IdentityCredential =
        serde_json::from_value(value).map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(Some(record.phc))
}
