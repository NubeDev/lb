//! `conflict` — the one place that recognizes SurrealDB's **retryable optimistic-transaction
//! conflict** and shapes the bounded backoff every retry loop in this crate shares.
//!
//! SurrealDB's durable (`kv-surrealkv`) and in-memory (`kv-mem`) engines both run optimistic MVCC:
//! two transactions opened over the same snapshot that touch overlapping keys let one commit and
//! abort the other with `read or write conflict … This transaction can be retried`. That abort is
//! transient, not corruption — a fresh retry over the new snapshot re-applies and commits. A later
//! read of a half-applied `rev` can also surface as `Invalid revision '…'`, which the same retry
//! resolves once the winning write is visible.
//!
//! The engine exposes **no typed variant** for this through the embedded `Surreal` surface, so the
//! only signal is the message. Keeping the match string in exactly ONE function means the ingest
//! commit path, the capped-table trim, the rev-bumping `write_locked`, and `increment` can never
//! drift in what they call "retryable" (they used to each carry their own copy of this string).

use std::time::Duration;

use crate::open::StoreError;

/// How many times a retryable transaction conflict is retried before the error is surfaced. High
/// enough that a realistic contention burst lands, low enough to never spin.
pub(crate) const MAX_CONFLICT_RETRIES: usize = 16;

/// True when a [`StoreError`] is SurrealDB's retryable optimistic-transaction conflict. Matched on
/// the message because SurrealDB exposes no typed variant for it through the embedded surface — this
/// is the SINGLE authoritative matcher; never duplicate the string.
///
/// `"The query was not executed due to a failed transaction"` is included deliberately. SurrealDB 3
/// reports a conflict-aborted `BEGIN…COMMIT` that way whenever the abort surfaces on a statement
/// AFTER the one that actually conflicted: the real cause is swallowed and every caller sees only
/// that sentence. Excluding it meant a whole ingest batch was surfaced as a hard error for a
/// conflict the retry was built to absorb. It is genuinely generic, so a transaction that failed for
/// some OTHER reason is now retried too — bounded, and every retrying caller here is idempotent, so
/// the cost is the backoff budget (tens of milliseconds) before the same error surfaces anyway.
pub(crate) fn is_retryable_conflict(e: &StoreError) -> bool {
    let m = e.to_string();
    m.contains("can be retried")
        || m.contains("read or write conflict")
        || m.contains("Invalid revision")
        || m.contains("failed transaction")
}

/// The shared jittered-in-shape backoff for attempt `attempt` (1-based). Escalating and
/// sub-millisecond so a burst of collided writers DESYNCHRONIZES rather than livelocks — a bare
/// retry lets the same two re-collide on the very next tick. Capped at `attempt.min(6)` so the
/// sleep stays small even deep into the retry budget.
pub(crate) fn conflict_backoff(attempt: usize) -> Duration {
    Duration::from_micros(50 * (1 << attempt.min(6)) as u64)
}
