//! `Policy` — the per-workspace notify/insights policy record (insight-notify-scope.md).
//!
//! One record per workspace, admin-owned. Absent record ⇒ compiled defaults (the seed pattern:
//! defaults live in code, the record stores overrides only). Hosts the ladder windows/cooldown,
//! the escalation threshold, the occurrence ring cap (occurrences scope), and the sub cap
//! (subscriptions scope).

use serde::{Deserialize, Serialize};

use crate::occurrence::MAX_DATA_BYTES;
use crate::subscription::MAX_PER_WORKSPACE as DEFAULT_SUB_CAP;

/// The store table the policy record lives in. One row per workspace namespace, id = ws id.
pub const TABLE: &str = "insight_policy";

/// The hard bounds on the occurrence ring cap (`[0, 1000]`; 0 = occurrences disabled — raise still
/// works, nothing stored). Admin-settable inside this range only.
pub const RING_CAP_MIN: usize = 0;
pub const RING_CAP_MAX: usize = 1_000;
/// The default ring cap when no policy record overrides.
pub const RING_CAP_DEFAULT: usize = 100;

/// The five ladder levels. Pinned by a sub's `throttle_override` (skip escalate/decay; keep
/// breakthroughs + ack-suppression).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThrottleOverride {
    /// L0 — post per raise (subject to cooldown).
    Immediate,
    /// L1 — one digest per hour.
    Hourly,
    /// L2 — one digest per day.
    Daily,
    /// L3 — one digest per week.
    Weekly,
    /// L4 — one digest per month.
    Monthly,
}

impl ThrottleOverride {
    /// The level index this pin corresponds to.
    pub fn level(self) -> u8 {
        match self {
            ThrottleOverride::Immediate => 0,
            ThrottleOverride::Hourly => 1,
            ThrottleOverride::Daily => 2,
            ThrottleOverride::Weekly => 3,
            ThrottleOverride::Monthly => 4,
        }
    }
}

/// The per-workspace policy record. Stores OVERRIDES only — absent fields fall back to
/// [`defaults`]. One row per workspace (id = ws id).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Policy {
    /// The L0 per-key cooldown (logical ts units — the injected clock's unit).
    #[serde(default = "default_cooldown")]
    pub cooldown: u64,
    /// The five ladder window sizes (logical ts units). `[L0, L1, L2, L3, L4]` — L0 is the
    /// immediate-cooldown window (often equal to `cooldown`); L1..L4 are the digest windows.
    #[serde(default = "default_windows")]
    pub windows: [u64; 5],
    /// How many delivery-worth of noise within a window escalates the level by one.
    #[serde(default = "default_escalation_threshold")]
    pub escalation_threshold: u64,
    /// The occurrence ring cap (occurrences scope). Hard bounds `[0, 1000]`.
    #[serde(default = "default_ring_cap")]
    pub ring_cap: usize,
    /// The workspace's subscription cap (subscriptions scope).
    #[serde(default = "default_sub_cap")]
    pub sub_cap: usize,
}

/// The compiled defaults (the seed pattern — absent policy record ⇒ these apply).
pub fn defaults() -> Policy {
    Policy {
        cooldown: default_cooldown(),
        windows: default_windows(),
        escalation_threshold: default_escalation_threshold(),
        ring_cap: default_ring_cap(),
        sub_cap: default_sub_cap(),
    }
}

fn default_cooldown() -> u64 {
    // 15 minutes in the clock's unit (treated as ms throughout — the injected clock's convention).
    15 * 60 * 1_000
}

fn default_windows() -> [u64; 5] {
    [
        15 * 60 * 1_000,           // L0 immediate — 15 min cooldown
        60 * 60 * 1_000,           // L1 hourly
        24 * 60 * 60 * 1_000,      // L2 daily
        7 * 24 * 60 * 60 * 1_000,  // L3 weekly
        30 * 24 * 60 * 60 * 1_000, // L4 monthly
    ]
}

fn default_escalation_threshold() -> u64 {
    3
}

fn default_ring_cap() -> usize {
    RING_CAP_DEFAULT
}

fn default_sub_cap() -> usize {
    DEFAULT_SUB_CAP
}

/// Validate an admin-supplied policy patch against the hard bounds. Returns the (possibly
/// clamped-to-bounds) value or a `BadInput` error for an out-of-bounds ring cap. The setters use
/// this so a bad admin write is a clean reject, never a silent bad state.
pub fn validate_ring_cap(cap: usize) -> Result<usize, String> {
    if !(RING_CAP_MIN..=RING_CAP_MAX).contains(&cap) {
        return Err(format!(
            "ring_cap {cap} out of bounds [{RING_CAP_MIN}..{RING_CAP_MAX}]"
        ));
    }
    Ok(cap)
}

/// The size cap re-exported for the occurrence appender (single source of truth for the constant).
pub fn occurrence_data_cap() -> usize {
    MAX_DATA_BYTES
}
