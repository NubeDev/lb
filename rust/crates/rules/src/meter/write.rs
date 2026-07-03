//! `WriteMeter` — the per-run write budget (rules-messaging-scope "Resolved decisions"). A sibling to
//! [`crate::meter::AiMeter`]: one shared budget charged by every **motion-producing write** across all
//! three messaging planes (inbox record/resolve, outbox enqueue, channel post/edit/delete), so a rhai
//! loop cannot enqueue ten thousand outbox effects (the DoS bound). **Reads are uncharged** — the
//! handles simply never call `charge` for `inbox.list`/`outbox.status`/`channel.history`/`list`.
//!
//! One meter, not per-plane: the honest DoS bound is a single "writes per run" number the author
//! already reasons about (mirroring the `AiMeter`). The default (`MAX_WRITES` = 32) comes from node
//! config; a per-workspace override record is additive later, not v1. Atomic so concurrent verb calls
//! within a run still bound; a rejected write is NOT counted (the `fetch_sub` rollback, ported from
//! `AiMeter`).

use std::sync::atomic::{AtomicU32, Ordering};

/// The write budget for one run. A charge past `max` rolls back and errors.
#[derive(Debug)]
pub struct WriteMeter {
    writes: AtomicU32,
    max: u32,
    /// The per-run counter feeding deterministic ids — bumped on each successful charge so an
    /// interleaved sequence of writes gets stable, monotonic ordinals across a re-run (no wall-clock,
    /// no random in core).
    seq: AtomicU32,
}

impl WriteMeter {
    pub fn new(max: u32) -> Self {
        Self {
            writes: AtomicU32::new(0),
            max,
            seq: AtomicU32::new(0),
        }
    }

    /// Charge one write. Rolls back and errors if it would exceed `max` (a rejected write is not
    /// counted). Returns the write's ordinal (its 0-based sequence within the run) for id derivation.
    pub fn charge(&self) -> Result<u32, String> {
        let prev = self.writes.fetch_add(1, Ordering::SeqCst);
        if prev >= self.max {
            self.writes.fetch_sub(1, Ordering::SeqCst);
            return Err(format!(
                "write budget exceeded: at most {} messaging writes per run",
                self.max
            ));
        }
        Ok(self.seq.fetch_add(1, Ordering::SeqCst))
    }

    pub fn writes_used(&self) -> u32 {
        self.writes.load(Ordering::SeqCst)
    }
}
