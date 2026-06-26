//! The outbox **effect** record — one normalized must-deliver intent every target collapses into
//! (README §6.10, outbox scope "the record shape").
//!
//! An effect is *state*: it lives in the store at `outbox:{id}` within a workspace, so a
//! must-deliver intent (open a PR, post a comment, notify, sync-publish) survives a crash and an
//! outage. `idempotency_key` is the contract with the receiver: it dedups on this, so the relay's
//! at-least-once retry never double-sends (outbox scope). `ts` is a caller-injected logical
//! timestamp (testing §3 — no wall-clock inside the crate).

use serde::{Deserialize, Serialize};

/// Where an effect is in its delivery lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EffectStatus {
    /// Not yet acknowledged by the target — the relay must (re)deliver it. The durable backstop:
    /// an effect that crashed mid-delivery is still `Pending`, so the next relay pass re-sends it.
    Pending,
    /// The target acknowledged delivery. A later relay pass skips it (no double-send).
    Delivered,
    /// The last attempt failed; the effect stays schedulable. Kept distinct from `Pending` for
    /// audit (this one has been tried `attempts` times). The relay re-delivers `Failed` too — but
    /// not before `next_attempt_ts` (backoff), and not once `attempts` hits `max_attempts`.
    Failed,
    /// The effect exhausted `max_attempts` without an ack — a poison message. **Terminal:** the
    /// relay never re-delivers a dead-lettered effect (it is no longer schedulable), but the row is
    /// kept for audit/observability and a manual replay. This is the backoff/dead-letter answer the
    /// outbox scope deferred: a perpetually-failing effect stops retrying and is parked here.
    DeadLettered,
}

/// A must-deliver effect = a durable, idempotent intent to the outside world. `id` is
/// workspace-unique and stable (re-`enqueue` upserts the same `outbox:{id}` row). `payload` is
/// opaque to this crate — the target adapter interprets it. `idempotency_key` is what the receiver
/// dedups on, so an at-least-once re-delivery is a no-op (outbox scope).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Effect {
    /// Stable effect id, unique within the workspace. Re-enqueuing the same id is idempotent.
    pub id: String,
    /// The delivery target (`github`, `email`, `sync`, …) — selects the relay's `Target` adapter.
    pub target: String,
    /// The action the target should perform (`create_pr`, `comment`, `notify`, …).
    pub action: String,
    /// Opaque per-target payload (the PR title/body, the comment text, …). Not interpreted here.
    pub payload: String,
    /// The stable dedup key the receiver honors — the at-least-once → effectively-once bridge.
    pub idempotency_key: String,
    pub status: EffectStatus,
    /// How many delivery attempts have been made — drives both backoff (the n-th retry waits longer)
    /// and dead-lettering (at `max_attempts`, the effect is parked).
    pub attempts: u32,
    /// The retry ceiling: once `attempts` reaches this, a further failure dead-letters the effect
    /// instead of leaving it schedulable. Defaults to [`DEFAULT_MAX_ATTEMPTS`]; raise it per effect
    /// for a target that is expected to be down for a while.
    pub max_attempts: u32,
    /// The earliest logical `ts` the relay may retry this effect — the backoff gate. Set on each
    /// failure to `failure_ts + backoff(attempts)`; a relay pass at `now < next_attempt_ts` skips
    /// it (still owed, just not yet). `0` on a fresh effect (deliver immediately).
    pub next_attempt_ts: u64,
    /// Caller-injected logical timestamp (no wall-clock — testing §3).
    pub ts: u64,
}

/// The default retry ceiling before an effect is dead-lettered. Chosen small enough that a poison
/// message is parked quickly, large enough to ride out a brief target outage across several passes.
pub const DEFAULT_MAX_ATTEMPTS: u32 = 5;

/// The backoff delay (in logical `ts` units) before the `attempts`-th retry — exponential, capped.
/// Pure function of the attempt count so it is deterministic under injected `ts` (testing §3): after
/// 1 failure wait 1, then 2, 4, 8, … capped at [`MAX_BACKOFF`]. The relay sets
/// `next_attempt_ts = failure_ts + backoff(attempts)`.
pub fn backoff(attempts: u32) -> u64 {
    const MAX_BACKOFF: u64 = 64;
    // attempts is ≥1 when this is called (it is incremented before). 1<<(n-1), saturating + capped.
    let shift = attempts.saturating_sub(1).min(20);
    (1u64 << shift).min(MAX_BACKOFF)
}

impl Effect {
    /// Build a fresh pending effect. Explicit (no `Default`) so every field is a deliberate choice
    /// at the call site — an effect with an empty target or idempotency key is almost always a bug.
    pub fn new(
        id: impl Into<String>,
        target: impl Into<String>,
        action: impl Into<String>,
        payload: impl Into<String>,
        idempotency_key: impl Into<String>,
        ts: u64,
    ) -> Self {
        Self {
            id: id.into(),
            target: target.into(),
            action: action.into(),
            payload: payload.into(),
            idempotency_key: idempotency_key.into(),
            status: EffectStatus::Pending,
            attempts: 0,
            max_attempts: DEFAULT_MAX_ATTEMPTS,
            next_attempt_ts: 0,
            ts,
        }
    }

    /// Override the retry ceiling for this effect (builder style). Below `1` is clamped to `1` — an
    /// effect must get at least one attempt before it can be dead-lettered.
    pub fn with_max_attempts(mut self, max_attempts: u32) -> Self {
        self.max_attempts = max_attempts.max(1);
        self
    }
}
