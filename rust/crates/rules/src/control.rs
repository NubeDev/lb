//! `RunControl` — the cooperative pause/cancel intent for one run (long-running-rules-scope).
//!
//! A job-backed run shares one `RunControl` between the evaluating thread and the host's control
//! verbs (`rules.runs.suspend`/`cancel`). The cage's `on_progress` governor observes the flag
//! between bytecode operations and aborts with a typed token; the engine maps the token onto
//! [`RuleError::Paused`](crate::RuleError)/[`RuleError::Cancelled`](crate::RuleError). Pause is
//! safe at *any* tick because every durable effect a rule writes is deterministic-id + upsert
//! (rules-messaging-scope): a resume replays the body from the top and already-landed writes
//! land on the same ids — memoized `job.step` blocks skip re-spends.

use std::sync::atomic::{AtomicU8, Ordering};

/// The abort token `on_progress` raises for a pause request. Kept as a `const` so the engine's
/// error mapping and the sandbox agree on one string.
pub const ABORT_PAUSED: &str = "rule run paused";
/// The abort token `on_progress` raises for a cancel request.
pub const ABORT_CANCELLED: &str = "rule run cancelled";

const RUN: u8 = 0;
const PAUSE: u8 = 1;
const CANCEL: u8 = 2;

/// What the controller asked the run to do. `Cancel` outranks `Pause` (a pause never downgrades a
/// cancel).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlIntent {
    Run,
    Pause,
    Cancel,
}

/// Shared, lock-free control intent for one run. Default is `Run`.
#[derive(Debug, Default)]
pub struct RunControl {
    flag: AtomicU8,
}

impl RunControl {
    /// Ask the run to park at the next governor tick. A no-op if cancel was already requested.
    pub fn request_pause(&self) {
        let _ = self
            .flag
            .compare_exchange(RUN, PAUSE, Ordering::AcqRel, Ordering::Acquire);
    }

    /// Ask the run to abort at the next governor tick. Outranks a pending pause.
    pub fn request_cancel(&self) {
        self.flag.store(CANCEL, Ordering::Release);
    }

    /// The current intent.
    pub fn intent(&self) -> ControlIntent {
        match self.flag.load(Ordering::Acquire) {
            PAUSE => ControlIntent::Pause,
            CANCEL => ControlIntent::Cancel,
            _ => ControlIntent::Run,
        }
    }

    /// Whether a pause or cancel is pending — what `job.should_stop()` reads.
    pub fn stop_requested(&self) -> bool {
        !matches!(self.intent(), ControlIntent::Run)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pause_never_downgrades_cancel() {
        let c = RunControl::default();
        c.request_cancel();
        c.request_pause();
        assert_eq!(c.intent(), ControlIntent::Cancel);
    }

    #[test]
    fn cancel_outranks_pause() {
        let c = RunControl::default();
        c.request_pause();
        assert_eq!(c.intent(), ControlIntent::Pause);
        c.request_cancel();
        assert_eq!(c.intent(), ControlIntent::Cancel);
        assert!(c.stop_requested());
    }
}
