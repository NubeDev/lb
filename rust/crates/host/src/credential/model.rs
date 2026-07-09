//! The **credential record** — a per-`(workspace, user)` argon2 password hash (login-hardening
//! scope). State, not motion. Stored in the workspace's OWN namespace (the hard wall §7): a
//! password set in `acme` is a row in `acme`'s namespace, so it can never authenticate a login into
//! `beta`. The record holds ONLY a PHC hash string (salt embedded) — never a plaintext, never
//! returned by any read (secrets rule §6.7). Keyed by the bare user handle (`user:ada`).

use serde::{Deserialize, Serialize};

/// The store table credential records live in, within a workspace namespace.
pub const CREDENTIAL_TABLE: &str = "credential";

/// The constant `kind` discriminant (parity with the other workspace tables; lets a future
/// `credential.list`-style scan filter, though secrets are never enumerated by value).
pub const CREDENTIAL_KIND: &str = "credential";

/// A stored credential: the user it authenticates and the argon2 PHC hash string. The plaintext
/// never lands here — `set` hashes before write, `verify` compares against `phc` in constant time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credential {
    /// The global identity handle this credential authenticates (`user:ada`).
    pub sub: String,
    /// Constant discriminant.
    pub kind: String,
    /// The argon2id PHC hash string (`$argon2id$v=19$m=...$...$...`). Secret-class: salt embedded,
    /// never a plaintext, never returned by a read.
    pub phc: String,
    /// Caller-injected logical set timestamp (no wall-clock — testing §3).
    pub set_ts: u64,
}

impl Credential {
    pub fn new(sub: impl Into<String>, phc: impl Into<String>, set_ts: u64) -> Self {
        Self {
            sub: sub.into(),
            kind: CREDENTIAL_KIND.to_string(),
            phc: phc.into(),
            set_ts,
        }
    }
}
