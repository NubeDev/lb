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
    /// The delegation upper bound (the caller's caps) for a run-scoped token (agent-key-lifecycle
    /// D1–D5). `None` for an ordinary token. When present, `caps::check` gate 2b enforces the
    /// caller bound — so a run token cannot widen past the human who asked, even though the token
    /// was verified (not derived) on this node. Kept OUT of the hot path for ordinary tokens:
    /// `#[serde(default)]` + `skip_serializing_if` so a regular session token is byte-identical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constraint: Option<Vec<String>>,
    /// The run id this token is scoped to (agent-key-lifecycle D3). `None` for an ordinary token;
    /// `Some(job_id)` for an external-agent run token. The gateway's `verify_token` consults the
    /// job's status when this is set — a terminal run's token is refused even if unexpired (hard
    /// cancel is instant, D3). `#[serde(default)]` keeps legacy tokens deserializable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
}
