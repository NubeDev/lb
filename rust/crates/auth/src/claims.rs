//! The JWT claim set — the §13 forever token shape (auth-caps scope).
//!
//! Deliberately small: a single workspace claim (`ws`, the hard wall), a role, and the
//! capability strings. `iat`/`exp` are seconds; tests inject the clock (never wall-clock).

use serde::{Deserialize, Serialize};

use crate::principal::Role;

/// The signed claim set inside a token.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Claims {
    /// Global identity, e.g. `user:ada` or `key:ci-bot`.
    pub sub: String,
    /// THE workspace claim — the hard isolation wall. Singular: one token, one workspace.
    pub ws: String,
    /// The actor's role; gates which caps are minted, not the inner check.
    pub role: Role,
    /// Capability strings in the auth-caps grammar (`<surface>:<resource>:<action>`).
    pub caps: Vec<String>,
    /// Issued-at (unix seconds). Injected in tests.
    pub iat: u64,
    /// Expiry (unix seconds). Injected in tests.
    pub exp: u64,
}
