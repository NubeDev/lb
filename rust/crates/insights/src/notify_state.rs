//! `NotifyState` — the per-`(subscription, dedup_key)` ladder state (insight-notify-scope.md).
//!
//! One row per (sub, active key). Bounded in practice by dedup; the retention follow-up sweeps
//! rows for resolved-and-quiet keys. The pure `ladder_step` function in [`crate::ladder`] is the
//! ONLY writer of this state's transitions — every field here is its input/output shape.

use serde::{Deserialize, Serialize};

use crate::severity::Severity;

/// The store table notify-state rows live in. One per workspace namespace.
pub const TABLE: &str = "insight_notify";

/// One ladder state row. Keyed by `(ws, sub_id, dedup_key)` — the record id is derived from those.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotifyState {
    /// The subscription this state is for.
    pub sub_id: String,
    /// The dedup key this state tracks (one ladder per (sub, key)).
    pub dedup_key: String,
    /// The current ladder level `0..=4` (L0 immediate … L4 monthly).
    pub level: u8,
    /// Logical ts at the start of the current accumulation window.
    pub window_start: u64,
    /// Raises seen this window (the escalate-threshold counter).
    pub window_hits: u64,
    /// What the next digest will say (zeroed after each digest send).
    pub pending: PendingAccumulator,
    /// Logical ts of the last delivered post/digest for this key.
    #[serde(default)]
    pub last_sent_ts: u64,
    /// The previous firing's severity — the escalation breakthrough check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_severity: Option<Severity>,
}

/// The pending accumulator — what the next digest message will summarize. Zeroed after each send.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingAccumulator {
    /// How many raises landed during the muted/digested window.
    pub count: u64,
    /// Logical ts of the first raise in the pending window.
    #[serde(default)]
    pub first_ts: u64,
    /// Logical ts of the most recent raise in the pending window.
    #[serde(default)]
    pub last_ts: u64,
    /// The worst severity seen in the pending window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_severity: Option<Severity>,
}
