//! The **sla-clock reactor** — resolve the contract, compute the deadlines, arm the breach alarm
//! (`docs/scope/insights/case-plane-scope.md` §"Reactors", the sla-clock row).
//!
//! # Compute a deadline; never a composite score
//!
//! There is no weight, no points, no "P1". A case's place in the queue is `due_at` ascending — a
//! wall-clock fact a human can argue with and a contract can be held to. Everything here exists to
//! produce that one instant honestly and to be able to say which clause produced it.
//!
//! # The clock never pauses
//!
//! Both deadlines are a **pure function of `(policy, opened_ts)`**. `opened_ts` never moves and the
//! policy only changes when the *contract* or the case's `(site, category, severity)` facts change.
//! That is not a rule this file remembers to follow — it is the shape of the arithmetic
//! ([`lb_cases::respond_by`] / [`lb_cases::due_at`], which this file deliberately does NOT
//! reimplement). Consequences, all structural:
//!   * a **snooze** cannot move a deadline: it is not an input, so a recompute after one returns
//!     the identical instants (and [`lb_cases::set_deadlines`] then writes nothing at all);
//!   * neither can `waiting_on`. Waiting on a purchase order, a client, or a contractor records
//!     **who holds the ball**; it never stops time. A contract that stopped whenever we were
//!     waiting on somebody would measure our patience, not our service;
//!   * a **severity escalation** DOES move it, because severity is a policy-match axis. A case that
//!     got worse is governed by whatever clause covers the worse thing.
//!
//! # Recompute always; write only on a change
//!
//! [`apply_sla`] is called on every grouping pass rather than only at open. It is cheap and
//! idempotent — the crate verb no-ops when the answer is unchanged — and it means there is exactly
//! ONE code path that sets a deadline, so the escalation case cannot drift from the open case.
//!
//! # No matching policy ⇒ no deadline
//!
//! Absent is honest. A workspace that has declared no contract for this slice of work gets a case
//! with `respond_by`/`due_at` unset and an `sla` event saying `unmatched`. Defaulting to some
//! number of hours would put a promise nobody made in front of a client, and the queue would sort
//! on it as if it were real.
//!
//! **Rule 10.** `site`, `category` and `severity` are opaque strings handed to
//! [`lb_cases::match_policy`], which compares them with `==`. Nothing here knows a pack, a rule, a
//! category value, a site or an extension.

use std::sync::Arc;

use lb_cases::{Deadlines, ServicePolicy};

use super::error::CaseSvcError;
use crate::boot::Node;

/// The subject the SLA clock attributes its writes to. `system:` — visibly not a person in the case
/// history, and the identity the breach alarm fires under (see [`super::breach_reminder`]).
pub const SLA_ACTOR: &str = "system:sla-clock";

/// Resolve the policy for case `case_id`, write its deadlines, and (re)arm the breach alarm.
///
/// Idempotent: recomputing an unchanged answer writes nothing, appends no event and leaves the
/// existing reminder alone. Called at case open, at reactor-driven grouping, and after a severity
/// escalation — the same function every time, deliberately.
// SCOPE: docs/scope/insights/case-plane-scope.md §"Reactors" (sla-clock)
pub async fn apply_sla(
    node: &Arc<Node>,
    ws: &str,
    case_id: &str,
    now: u64,
) -> Result<(), CaseSvcError> {
    let store = &node.store;
    let Some(case) = lb_cases::get(store, ws, case_id).await? else {
        return Err(CaseSvcError::BadInput(format!("no such case: {case_id}")));
    };
    // A closed case's deadlines are history. Recomputing them would rewrite what a client was told
    // at the time — and a case that breached and was later resolved must keep saying so.
    if case.closed {
        return Ok(());
    }

    let policy = lb_cases::match_policy(
        store,
        ws,
        case.site.as_deref(),
        case.category.as_deref(),
        Some(&case.severity),
    )
    .await?;
    let deadlines = deadlines_for(policy.as_ref(), case.opened_ts);

    let changed = lb_cases::set_deadlines(
        store,
        ws,
        case_id,
        policy.as_ref().map(|p| p.id.as_str()),
        deadlines,
        SLA_ACTOR,
        now,
    )
    .await?;

    // Only re-arm when the answer actually moved. Re-saving an unchanged reminder would resurrect a
    // one-shot that has already fired (`save` resets `runs`), so the case would be "breached" again
    // on every subsequent raise — harmless because the breach write is set-once, but it would fill
    // the job table and the reactor log with firings that decide nothing.
    if changed.is_some() {
        super::breach_reminder::arm_breach_alarm(node, ws, case_id, deadlines.due_at, now).await;
    }
    Ok(())
}

/// The two instants `policy` produces for a case opened at `opened_ts`, or both `None` when no
/// policy governs it.
///
/// [`lb_cases::NEVER`] (the sentinel a calendar that never opens returns) folds to `None`: "this
/// case is due at the end of time" is not a deadline, and storing `u64::MAX` would sort it to the
/// bottom of the queue while still *looking* like a promise. Absent says the true thing.
fn deadlines_for(policy: Option<&ServicePolicy>, opened_ts: u64) -> Deadlines {
    let Some(policy) = policy else {
        return Deadlines::default();
    };
    Deadlines {
        respond_by: finite(lb_cases::respond_by(policy, opened_ts)),
        due_at: finite(lb_cases::due_at(policy, opened_ts)),
    }
}

/// `None` for the never-due sentinel, `Some(instant)` for a real deadline.
fn finite(instant: u64) -> Option<u64> {
    (instant != lb_cases::NEVER).then_some(instant)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lb_cases::{Calendar, DayHours, PolicyMatch};

    fn business_policy(respond_h: u32, resolve_h: u32, holidays: Vec<String>) -> ServicePolicy {
        // Mon–Fri 09:00–17:00 UTC. A fixed-offset zone keeps the unit case free of DST noise; the
        // DST behaviour is `lb_cases::deadline`'s own responsibility and its own tests.
        let mut week = [DayHours::CLOSED; 7];
        for day in week.iter_mut().take(5) {
            *day = DayHours::new(9 * 60, 17 * 60);
        }
        ServicePolicy {
            id: "p".into(),
            name: "p".into(),
            r#match: PolicyMatch::default(),
            active: true,
            respond_h,
            resolve_h,
            calendar: Calendar::Business {
                tz: "UTC".into(),
                hours: week,
                holidays,
            },
            party_window_h: 48,
            hold_down_days: 14,
        }
    }

    /// A calendar that never opens yields the `NEVER` sentinel, and this layer must NOT store it.
    #[test]
    fn a_never_opening_calendar_yields_absent_deadlines_not_u64_max() {
        let policy = ServicePolicy {
            calendar: Calendar::Business {
                tz: "UTC".into(),
                hours: [DayHours::CLOSED; 7],
                holidays: Vec::new(),
            },
            ..business_policy(2, 8, Vec::new())
        };
        let d = deadlines_for(Some(&policy), 1_767_000_000_000);
        assert_eq!(d.respond_by, None);
        assert_eq!(d.due_at, None);
    }

    /// No policy is not "zero hours" and not "some default" — it is nothing at all.
    #[test]
    fn no_policy_means_no_deadline() {
        assert_eq!(deadlines_for(None, 123), Deadlines::default());
    }

    /// The scope's named case, at this layer: Friday 16:00 + 2 business hours = Monday 10:00.
    #[test]
    fn friday_afternoon_spills_to_monday_morning() {
        // 2026-01-09 is a Friday. 16:00 UTC.
        let friday_1600 = 1_767_974_400_000_u64;
        let d = deadlines_for(Some(&business_policy(2, 8, Vec::new())), friday_1600);
        // Monday 2026-01-12 10:00 UTC.
        assert_eq!(d.respond_by, Some(1_768_212_000_000));
    }
}
