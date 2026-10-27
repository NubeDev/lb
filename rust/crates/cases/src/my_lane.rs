//! `my_lane` — which of My Insights' five lanes a case is in for the caller (Insights review #16).
//!
//! The lanes are personal and disjoint: a case is in at most one, checked in this order, so a case
//! that is both mine and snoozed reads *On hold*, and one I emailed reads *Emailed* rather than
//! *My action*:
//!
//! 1. **closed** — closed by me in the last seven days;
//! 2. **hold** — open, snoozed now, and mine or snoozed by me;
//! 3. **emailed** — open, sent out by me, and the ball is still with the party (`waiting_on`);
//! 4. **action** — open and mine;
//! 5. **assigned** — open, assigned by me to someone who is not me.
//!
//! "Me" is the host's subject set (the person plus their teams), the one definition of "mine" the
//! `mine` lane already uses. "By me" compares the recorded actor against the same set.

use std::collections::BTreeSet;

use crate::case::Case;
use crate::list::Lane;

/// How far back the *Closed* lane looks.
pub const CLOSED_LANE_MS: u64 = 7 * 24 * 3_600_000;

/// The personal lane `case` is in for the caller whose subjects are `me`, at `now`, or `None`.
pub fn my_lane(case: &Case, me: &BTreeSet<String>, now: u64) -> Option<Lane> {
    let is_me = |s: &Option<String>| s.as_deref().is_some_and(|s| me.contains(s));
    if case.closed {
        let recent = case
            .resolved_ts
            .is_some_and(|t| now.saturating_sub(t) <= CLOSED_LANE_MS);
        return (recent && is_me(&case.resolved_by)).then_some(Lane::Closed);
    }
    let mine = is_me(&case.assigned_to);
    let snoozed = case.snooze_until.is_some_and(|u| u > now);
    if snoozed && (mine || is_me(&case.snoozed_by)) {
        return Some(Lane::Hold);
    }
    if case.waiting_on.is_some() && is_me(&case.emailed_by) {
        return Some(Lane::Emailed);
    }
    if mine {
        return Some(Lane::Action);
    }
    if case.assigned_to.is_some() && is_me(&case.assigned_by) {
        return Some(Lane::Assigned);
    }
    None
}
