//! `sla_state` — where a case stands against its deadline, as one closed word (case-plane scope §2).
//!
//! The record carries the facts (`due_at`, `breached_ts`, `closed`); this is the one reading of them
//! a filter can ask for, so the queue and the insight roster's case lens cannot each grow their own.
//! It is relative to a clock the CALLER supplies (`now`), like the `snoozed` axis: no wall clock here.
//!
//! **The clock pauses only for a snooze with an end date** ([`crate::sla_pause`]): a paused case
//! reads as it stood when the pause began, so it cannot drift into breached while nobody may act on
//! it. Any other parked or waiting case is still due soon or breached. The clock STOPS once a case is
//! closed, and a deadline that passes afterwards is not a breach. A closed case is therefore
//! `Breached` only if the breach was recorded, and `Met` otherwise.
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
}

/// The state of `case` at `now`, with `due_soon_ms` as the due-soon window.
pub fn sla_state(case: &Case, now: u64, due_soon_ms: u64) -> SlaState {
    if case.breached_ts.is_some() {
        return SlaState::Breached;
    }
    let Some(due_at) = case.due_at else {
        return SlaState::NoDeadline;
    };
    if case.closed {
        return SlaState::Met;
    }
    let now = case.sla_paused_at.map_or(now, |at| at.min(now));
    if now >= due_at {
        SlaState::Breached
    } else if due_at - now <= due_soon_ms {
        SlaState::DueSoon
    } else {
        SlaState::OnTrack
    }
}
