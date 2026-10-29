//! `sla_state` — where a case stands against its deadline, as one closed word (case-plane scope §2).
//!
//! The record carries the facts (`due_at`, `breached_ts`, `closed`); this is the one reading of them
//! a filter can ask for, so the queue and the insight roster's case lens cannot each grow their own.
//! It is relative to a clock the CALLER supplies (`now`), like the `snoozed` axis: no wall clock here.
//!
//! **The clock runs only while the case is Open** ([`crate::sla_follow`], Insights review
//! 2026-10-29). An open case in any other status — actioned, assigned, emailed, on hold — reads as
//! [`SlaState::Paused`]: its clock is stopped, so it is neither overdue nor due, and it carries on
//! from where it stopped if the case comes back to Open. A breach already recorded is history and
//! reads [`SlaState::Breached`] whatever the case does next. The clock stops for good once a case is
//! closed: `Breached` only if the breach was recorded, `Met` otherwise.
//!
//! One responsibility: a case and a clock to its SLA state.

use serde::{Deserialize, Serialize};

use crate::case::Case;

/// The window before `due_at` that counts as due soon when the caller names none: four hours.
pub const DEFAULT_DUE_SOON_MS: u64 = 4 * 3_600_000;

/// How a case stands against its deadline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlaState {
    /// The breach is recorded, or the case is open and its deadline has passed.
    Breached,
    /// Open, not breached, and inside the due-soon window.
    DueSoon,
    /// Open, with a deadline further off than the due-soon window.
    OnTrack,
    /// No service policy has clocked this case: there is no deadline to stand against.
    NoDeadline,
    /// Closed with a deadline and no recorded breach.
    Met,
    /// Open work out of the Open status (actioned, assigned, emailed, on hold): its clock is not
    /// running, so it is neither breached nor due, whatever its deadline says.
    Paused,
}

/// The state of `case` at `now`, with `due_soon_ms` as the due-soon window.
pub fn sla_state(case: &Case, now: u64, due_soon_ms: u64) -> SlaState {
    // A breach is history: it reads first, whatever the case has done since.
    if case.breached_ts.is_some() {
        return SlaState::Breached;
    }
    let Some(due_at) = case.due_at else {
        return SlaState::NoDeadline;
    };
    if case.closed {
        return SlaState::Met;
    }
    // Out of Open the clock is stopped; a pause still being booked reads the same.
    if !crate::sla_follow::is_open(case, now) || case.sla_paused_at.is_some() {
        return SlaState::Paused;
    }
    if now >= due_at {
        SlaState::Breached
    } else if due_at - now <= due_soon_ms {
        SlaState::DueSoon
    } else {
        SlaState::OnTrack
    }
}
