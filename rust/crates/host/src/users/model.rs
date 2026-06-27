//! The `user` record — identity as administered data (admin-crud scope). One row per `(ws, user)` in
//! the workspace's own namespace (per-workspace user records, one global principal id — the resolved
//! open question). Backs `user.create`/`list`/`disable`/`enable`/`delete`.
//!
//! The dev credential is **not inline** — `cred_ref` points at a mediated store so `user.list` can
//! never leak it and the real IdP attaches there later (the resolved lean). `active=false` is what
//! the login path checks to refuse minting (`disable` bites login). `kind` lets `user.list`
//! equality-filter every row.

use serde::{Deserialize, Serialize};

/// The store table user records live in, within a workspace namespace.
pub const TABLE: &str = "user";

/// The constant `kind` discriminant so `user_list` can select every row.
pub const KIND: &str = "user";

/// The `kind` a deleted user record carries. The store has no row-delete, so `user.delete` upserts
/// this tombstone (sync-idempotent, §6.8 — like the relation tombstone); `list`/`login_check` read
/// a tombstoned record as absent. A hard global purge is the separate node-directory action.
pub const TOMBSTONE: &str = "__deleted__";

/// A user in a workspace: the global principal id, an `active` flag the login path checks, a role
/// hint, and a **reference** to the mediated credential (never the credential itself).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserRecord {
    /// The global principal id (`user:ada`) — the same across workspaces; grants are per-ws.
    pub user: String,
    /// Whether this user may mint a session in this workspace. `disable` flips it false.
    pub active: bool,
    /// A role hint for the admin UI (the authoritative caps come from grants, not this).
    pub role: String,
    /// An opaque reference to the mediated dev credential — never the secret, never returned by
    /// `user.list`. The real IdP attaches at this seam.
    pub cred_ref: String,
    /// Constant discriminant so `user_list` selects every row.
    pub kind: String,
    /// Caller-injected logical timestamp (no wall-clock — testing §3).
    pub ts: u64,
}

impl UserRecord {
    pub fn new(
        user: impl Into<String>,
        role: impl Into<String>,
        cred_ref: impl Into<String>,
        ts: u64,
    ) -> Self {
        Self {
            user: user.into(),
            active: true,
            role: role.into(),
            cred_ref: cred_ref.into(),
            kind: KIND.to_string(),
            ts,
        }
    }
}

/// The credential-free view `user.list` returns — the record minus `cred_ref` (the secret-ish
/// field is mediated, never enumerated). One concept: "a user as the admin UI sees it".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserView {
    pub user: String,
    pub active: bool,
    pub role: String,
}

impl From<UserRecord> for UserView {
    fn from(r: UserRecord) -> Self {
        Self {
            user: r.user,
            active: r.active,
            role: r.role,
        }
    }
}
