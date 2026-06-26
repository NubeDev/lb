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
    /// audit (this one has been tried `attempts` times). The relay re-delivers `Failed` too.
    Failed,
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
    /// How many delivery attempts have been made — for audit (backoff policy is deferred).
    pub attempts: u32,
    /// Caller-injected logical timestamp (no wall-clock — testing §3).
    pub ts: u64,
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
            ts,
        }
    }
}
