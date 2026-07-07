//! `Status` — the `open → acked → resolved` lifecycle (insights umbrella scope).
//!
//! Transitions are owned by the host verbs `insight.ack` / `insight.resolve`. The lifecycle is
//! load-bearing for the ladder's **ack suppression** (notify scope): per-key deliveries are
//! suppressed while `acked`, but escalation/re-open breakthroughs still fire (a `critical`
//! escalation un-suppresses by definition — re-open flips status back to `open`).

use serde::{Deserialize, Serialize};

/// The insight lifecycle. Ordered by progression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Open,
    Acked,
    Resolved,
}
