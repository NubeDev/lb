//! The Principal — a verified identity resolved from a token, ready for `caps::check`.
//!
//! This is the post-verification view the rest of the host uses: who, which workspace, and
//! what capabilities. The raw JWT and signing never leave the `auth` crate.

use serde::{Deserialize, Serialize};

/// RBAC roles (README §6.6). Ordered most→least privileged is not encoded here on purpose —
/// the check path reads `caps`, not `role`; roles only gate what is minted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    SuperAdmin,
    WorkspaceAdmin,
    Member,
}

/// A verified actor. Construct it only via `auth::verify` — there is no public raw
/// constructor, so an unverified principal cannot exist by accident.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Principal {
    sub: String,
    ws: String,
    role: Role,
    caps: Vec<String>,
}

impl Principal {
    /// Crate-internal: built by `verify` after the signature and expiry check pass.
    pub(crate) fn new(sub: String, ws: String, role: Role, caps: Vec<String>) -> Self {
        Self {
            sub,
            ws,
            role,
            caps,
        }
    }

    /// The global identity (`user:…` / `key:…`).
    pub fn sub(&self) -> &str {
        &self.sub
    }

    /// The workspace this principal is scoped to — the hard wall, checked first.
    pub fn ws(&self) -> &str {
        &self.ws
    }

    pub fn role(&self) -> Role {
        self.role
    }

    /// The held capability strings (auth-caps grammar). Read by `caps::check`.
    pub fn caps(&self) -> &[String] {
        &self.caps
    }
}
