//! `case_status` — the one word a person reads for where a case is (Insights review 2026-10):
//! Open, Assigned, Emailed, On hold, Closed. The stage says how far the work has got; this says what
//! state it is in now, read off the case's own fields, so the queue's Status filter and the status
//! chip every surface draws cannot each grow their own reading.
//!
//! The order is the precedence: a closed case is Closed whatever else it carries; a running hold is
//! On hold; work out by email (past Open) is Emailed; work someone holds (past Open) is Assigned;
//! work nobody has started is Open. A case past Open with none of those is [`CaseStatus::Actioned`],
//! the stage's own word, which no status filter asks for.
//!
//! One responsibility: a case and a clock to its status word.

use serde::{Deserialize, Serialize};

use crate::case::{Case, WaitingOn, Workflow};

/// Where a case is, as one word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseStatus {
    Open,
    Assigned,
    Emailed,
    Hold,
    Closed,
    /// Past Open with no owner, email or hold: the stage's word, offered by no filter.
    Actioned,
}

/// The status of `case` at `now` (a hold whose end has passed is over).
pub fn case_status(case: &Case, now: u64) -> CaseStatus {
    if case.closed {
        return CaseStatus::Closed;
    }
    if case.snooze_until.is_some_and(|u| u > now) {
        return CaseStatus::Hold;
    }
    let started = case.workflow != Workflow::ToAction;
    let out = case.waiting_on == Some(WaitingOn::Contractor)
        || (case.emailed_by.is_some() && case.waiting_on.is_some());
    if started && out {
        return CaseStatus::Emailed;
    }
    if started && case.assigned_to.is_some() {
        return CaseStatus::Assigned;
    }
    if started {
        CaseStatus::Actioned
    } else {
        CaseStatus::Open
    }
}
