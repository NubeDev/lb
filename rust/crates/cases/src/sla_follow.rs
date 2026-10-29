//! `sla_follow` — the SLA clock runs only while a case is **Open** (Insights review 2026-10-29).
//!
//! The SLA is the time to a first response: it starts when the case opens and stops the moment the
//! case moves out of Open — actioned, assigned, emailed, put on hold or closed ([`crate::case_status`],
//! the one status word). If the case comes back to Open (a reopen, a hold that returns it) the clock
//! carries on from the time it has already used: only time spent Open counts.
//!
//! **One place, every verb.** Rather than each verb remembering to stop or start the clock, the
//! single write path ([`crate::save`]) asks [`follow`] on every write: out of Open with no pause
//! running starts one at the write's instant; back in Open with a pause running ends it and credits
//! the business time it lasted to `sla_paused_ms`, which moves the deadline forward on the host's
//! next recompute (the deadlines stay a pure function of `(policy, opened_ts, sla_paused_ms)`).
//!
//! A case with no deadline has no clock to stop, and a closed case's clock has stopped for good: the
//! breach, if any, is history and is never cleared ([`crate::mark_breached`]).
//!
//! [`settle_all`] is the same rule applied to every open case by the host's tick, for the cases
//! stored before this rule (out of Open with no pause) and for a hold that ends by time alone.
//!
//! One responsibility: keep a case's SLA pause in step with whether it is Open.

use lb_store::{scan_all, write, Store};
use serde_json::Value;

use crate::business_span::business_ms_between;
use crate::calendar::Calendar;
use crate::case::{Case, TABLE};
use crate::case_event::EventKind;
use crate::case_status::{case_status, CaseStatus};
use crate::error::CasesError;
use crate::event_append::append_event;

/// The actor stamped on a pause or a resume: the platform keeps the clock, nobody chose it.
pub const SLA_FOLLOW_ACTOR: &str = "system:sla-clock";

/// Is `case` Open at `now` — the one status the SLA clock runs in?
pub fn is_open(case: &Case, now: u64) -> bool {
    case_status(case, now) == CaseStatus::Open
}

/// Bring `case`'s pause in step with its status at `ts`, editing the struct only. Returns the event
/// to append once the write lands, or `None` when nothing moved.
pub(crate) async fn follow(
    store: &Store,
    ws: &str,
    case: &mut Case,
    ts: u64,
) -> Result<Option<(EventKind, Value)>, CasesError> {
    if case.closed || case.due_at.is_none() {
        return Ok(None);
    }
    let open = is_open(case, ts);
    match case.sla_paused_at {
        None if !open => {
            case.sla_paused_at = Some(ts);
            let data = serde_json::json!({ "due_at": case.due_at, "policy_id": case.policy_id });
            Ok(Some((EventKind::SlaPaused, data)))
        }
        Some(from) if open => {
            case.sla_paused_at = None;
            let calendar = policy_calendar(store, ws, case.policy_id.as_deref()).await?;
            let paused_ms = business_ms_between(&calendar, from, ts);
            case.sla_paused_ms = case.sla_paused_ms.saturating_add(paused_ms);
            let data = serde_json::json!({ "from": from, "paused_ms": paused_ms });
            Ok(Some((EventKind::SlaResumed, data)))
        }
        _ => Ok(None),
    }
}

/// [`follow`] over every open case in `ws` at `now`, for the host's tick. A case found out of Open
/// with no pause is paused from its `last_activity_ts` — the last thing that changed it, which is
/// the best record of when it left Open — and the write does NOT bump that activity (the platform
/// keeping its books is not activity). Returns how many cases moved.
pub async fn settle_all(store: &Store, ws: &str, now: u64) -> Result<usize, CasesError> {
    let mut moved = 0usize;
    for row in scan_all(store, ws, TABLE).await? {
        let inner = match row.data {
            Value::Object(mut obj) => obj.remove("data").unwrap_or(Value::Object(obj)),
            other => other,
        };
        let Ok(mut case) = serde_json::from_value::<Case>(inner) else {
            continue;
        };
        if case.closed || case.due_at.is_none() {
            continue;
        }
        let open = is_open(&case, now);
        let at = match case.sla_paused_at {
            None if !open => case.last_activity_ts.min(now),
            Some(_) if open => now,
            _ => continue,
        };
        let Some((kind, data)) = follow(store, ws, &mut case, at).await? else {
            continue;
        };
        let value = serde_json::to_value(&case).map_err(CasesError::decode)?;
        write(store, ws, TABLE, &case.id, &value).await?;
        append_event(store, ws, &case.id, kind, SLA_FOLLOW_ACTOR, data, at).await?;
        moved += 1;
    }
    Ok(moved)
}

/// The calendar the case's deadlines are measured on. A policy since deleted falls back to
/// wall-clock time: the pause was real time, and counting none of it would breach the case for
/// time it was told it had.
async fn policy_calendar(
    store: &Store,
    ws: &str,
    policy_id: Option<&str>,
) -> Result<Calendar, CasesError> {
    let Some(id) = policy_id else {
        return Ok(Calendar::Always);
    };
    Ok(crate::policy_list::policy_list(store, ws, true)
        .await?
        .into_iter()
        .find(|p| p.id == id)
        .map(|p| p.calendar)
        .unwrap_or(Calendar::Always))
}
