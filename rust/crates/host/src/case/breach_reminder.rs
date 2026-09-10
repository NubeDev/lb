//! The breach **alarm** — a durable one-shot reminder that fires `case.breach` at `due_at`
//! (case-plane scope §"Reactors", the sla-clock row).
//!
//! # Why a reminder at all
//!
//! Because the alternative is a timer in a process, and a process restarts. The reminder record is
//! workspace-scoped durable state driven by a shipped reactor (`spawn_reminder_reactors`), so an
//! armed breach survives a crash, a redeploy and a node move. This is deliberately NOT a new scan
//! loop: `node/src/reactors.rs` documents the missing-driver class repeatedly — a scheduled thing
//! with no driver is a decorative surface — and the reminder reactor is a driver that already runs.
//!
//! # How a one-shot is expressed, and why THIS way
//!
//! `reminder_create` derives `next_attempt_ts` from the cron string, which resolves only to the
//! **minute**. A deadline does not: `due_at` is an exact millisecond produced by business-hours
//! arithmetic. Rounding it to a cron minute fires the alarm up to 59 seconds EARLY, and an SLA tool
//! that declares a breach before the deadline has passed is worse than one that is a minute late.
//!
//! So the reminder is **constructed directly and saved** with `next_attempt_ts` set to the exact
//! deadline second and `max_runs: Some(1)`:
//!   * `due()` selects on `next_attempt_ts <= now`, so the alarm is armed to the exact instant;
//!   * `advance()` sees `runs (1) >= max_runs (1)`, marks the reminder `Done` and disables it — one
//!     firing, ever, with no second slot to compute.
//!
//! The `schedule` field still carries a **real, valid 5-field cron naming that exact minute**, and
//! that is load-bearing rather than decorative: a DENIED firing takes `react.rs`'s `reschedule`
//! path, which calls `next_after(&schedule, now)`. A placeholder or empty schedule would make that
//! return `BadCron`, and `react_to_reminders` propagates the error — one un-grantable case would
//! abort the whole workspace's reminder pass, killing every unrelated schedule in it.
//!
//! *Rejected: `reminder_create` with a derived cron.* It would have been fewer lines and gone
//! through the gated door, but it buys the early-fire above, and the door's gate
//! (`mcp:reminder.create:call`) is a **user-tier CRUD cap** that would then have to be granted to a
//! system actor to arm an alarm nobody asked for. Arming is not a user gesture.
//!
//! # The fire-time principal
//!
//! `fire_reminder` re-resolves the stored `principal_sub`'s caps from the **durable grant store**
//! at fire time (that is the reminders security model, and lb#176 is what happens when the lookup
//! is subtly wrong). A system actor holds no grants by default, so the alarm would resolve zero
//! caps and every breach would be a silent `denied=1`. The alarm therefore assigns itself exactly
//! one capability, in this workspace, idempotently: `mcp:case.breach:call` for
//! [`SLA_ACTOR`](super::sla_clock::SLA_ACTOR).
//!
//! That grant is the narrowest thing that can work and it is deliberately visible: it is an
//! ordinary row in the workspace's grant table, so `authz.resolve` shows exactly what the clock can
//! do, and an admin can revoke it. `mcp:case.breach:call` is in **no role bundle** (see
//! `super::breach`), so this grant is the only thing in the workspace that holds it.
//!
//! *Rejected: widening `reactor_caps()`.* That list mints a `Principal::routed` for the flow
//! reactor loop; the reminder fire path never consults it, so adding a cap there would have looked
//! like a fix and changed nothing.

use std::sync::Arc;

use chrono::{DateTime, Datelike, Timelike, Utc};
use lb_authz::{grant_assign, Subject};
use lb_reminders::{Action, Reminder};

use crate::boot::Node;

/// The capability the breach alarm fires under. Held by no role bundle — only by the grant this
/// file writes for the SLA clock's own subject.
pub const BREACH_CAP: &str = "mcp:case.breach:call";

/// The verb the alarm calls when it fires.
const BREACH_TOOL: &str = "case.breach";

/// The reminder id for a case's breach alarm. Derived from the case id, so re-arming after a
/// severity escalation **replaces** the alarm rather than adding a second one — `lb_reminders::save`
/// is an upsert on this id.
pub fn breach_reminder_id(case_id: &str) -> String {
    format!("case-breach:{case_id}")
}

/// Arm (or re-arm, or stand down) the breach alarm for `case_id`.
///
/// `due_at` is the case's resolution deadline in epoch **milliseconds**, or `None` when no policy
/// governs the case — in which case any existing alarm is disabled, because a case with no deadline
/// cannot breach one.
///
/// Best-effort and loud: the deadlines are already durable on the case by the time this runs, so an
/// unwritable reminder is a missing alarm, not a failed grouping. Failing the caller here would
/// turn "the breach notice is late" into "the insight could not be raised".
pub(super) async fn arm_breach_alarm(
    node: &Arc<Node>,
    ws: &str,
    case_id: &str,
    due_at: Option<u64>,
    now: u64,
) {
    let store = &node.store;
    let id = breach_reminder_id(case_id);

    let Some(due_at_ms) = due_at else {
        stand_down(node, ws, &id, now).await;
        return;
    };
    let Some(schedule) = cron_for(due_at_ms) else {
        tracing::warn!(
            ws,
            case_id,
            due_at_ms,
            "breach alarm not armed: undecodable deadline"
        );
        return;
    };

    // The one grant the alarm needs, in this workspace, before the alarm that depends on it exists.
    // Idempotent (an upsert on a derived row id), so re-arming costs one write and never drifts.
    if let Err(e) = grant_assign(
        store,
        ws,
        &Subject::User(super::sla_clock::SLA_ACTOR.to_string()),
        BREACH_CAP,
    )
    .await
    {
        tracing::warn!(ws, case_id, error = %e, "breach alarm grant not written; the alarm would fire denied");
        return;
    }

    let mut reminder = Reminder::new(
        &id,
        schedule,
        // ONE firing, ever. `advance()` marks it Done rather than computing a second slot.
        Some(1),
        Action::McpTool {
            tool: BREACH_TOOL.to_string(),
            // No `ts`: the firing stamps the host clock, and the breach instant written on the case
            // is the DEADLINE itself (`lb_cases::mark_breached`), not the moment the alarm rang. A
            // reactor that ticks late must not make a case look less late than it was.
            args: serde_json::json!({ "case_id": case_id }),
        },
        super::sla_clock::SLA_ACTOR,
        to_secs(now),
    );
    // The exact deadline second — see the module doc on why this is set here and not derived from
    // the cron. `due()` compares `next_attempt_ts <= now` on a SECOND clock; the case plane's
    // timestamps are milliseconds, and mixing the two puts the alarm ~55,000 years out.
    reminder.next_attempt_ts = due_at_ms.div_ceil(1000);

    if let Err(e) = lb_reminders::save(store, ws, &reminder).await {
        tracing::warn!(ws, case_id, error = %e, "breach alarm not armed; the deadline is durable but nothing will ring it");
    }
}

/// Disable an existing alarm for a case that no longer has a deadline. A missing reminder is the
/// ordinary case and not an error — most cases are armed once and never disarmed.
async fn stand_down(node: &Arc<Node>, ws: &str, id: &str, now: u64) {
    let Ok(Some(mut reminder)) = lb_reminders::load(&node.store, ws, id).await else {
        return;
    };
    if !reminder.enabled {
        return;
    }
    reminder.enabled = false;
    reminder.ts = to_secs(now);
    if let Err(e) = lb_reminders::save(&node.store, ws, &reminder).await {
        tracing::warn!(ws, id, error = %e, "breach alarm could not be stood down");
    }
}

/// A valid 5-field cron naming the exact minute of `due_at_ms` in UTC. See the module doc for why
/// this exists even though `next_attempt_ts` is what actually arms the alarm.
fn cron_for(due_at_ms: u64) -> Option<String> {
    let dt: DateTime<Utc> = DateTime::from_timestamp_millis(due_at_ms as i64)?;
    Some(format!(
        "{} {} {} {} *",
        dt.minute(),
        dt.hour(),
        dt.day(),
        dt.month()
    ))
}

/// The case plane stamps milliseconds; the reminder plane stamps seconds. One conversion, one place.
fn to_secs(ms: u64) -> u64 {
    ms / 1000
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cron must be a REAL expression, because a denied firing reschedules through it — see the
    /// module doc. This is the test that catches somebody "simplifying" it to a placeholder.
    #[test]
    fn the_derived_schedule_is_a_valid_cron() {
        // 2026-01-12 10:00:00 UTC — the Monday the scope's Friday-afternoon case lands on.
        let cron = cron_for(1_768_212_000_000).expect("a real instant decodes");
        assert_eq!(cron, "0 10 12 1 *");
        assert!(lb_reminders::is_valid(&cron), "{cron} must parse as cron");
    }

    /// A deadline carrying seconds keeps them: the alarm rounds UP to the next whole second, never
    /// down into declaring a breach before the deadline passed.
    #[test]
    fn a_sub_second_deadline_rounds_up_never_early() {
        assert_eq!(1_768_212_000_001_u64.div_ceil(1000), 1_768_212_001);
        assert_eq!(1_768_212_000_000_u64.div_ceil(1000), 1_768_212_000);
    }

    /// One case, one alarm id — the whole idempotency story in one line.
    #[test]
    fn the_alarm_id_is_derived_from_the_case() {
        assert_eq!(breach_reminder_id("c1"), "case-breach:c1");
    }
}
