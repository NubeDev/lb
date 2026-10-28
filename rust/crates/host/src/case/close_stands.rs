//! `close_stands` — a person's close holds until the alarm fires again (Insights review: "closing a
//! still-active alarm reopens it").
//!
//! Closing a case does not resolve its insight: the work is done, the alarm may still be active.
//! The reconcile backstop gives every open insight without an open case one, so on its next tick it
//! used to reopen (a recent `fixed`, "repair did not hold") or re-open a new case for an alarm that
//! had not fired again — undoing the close within a minute. The backstop exists for insights left
//! without a case by a restart or a crash, not to second-guess a close.
//!
//! So the backstop leaves an insight alone when its last case closed at or after the insight's last
//! firing (`last_ts`). A new firing advances `last_ts` and goes through the inline grouping at raise,
//! which reopens or opens a case exactly as before. "Fires again" is lb's meaning: a producer that
//! re-raises on every evaluation while the condition holds counts as firing again.

use lb_insights::Insight;
use lb_store::Store;

use super::error::CaseSvcError;

/// Whether `insight`'s last case was closed after it last fired, so the close stands.
pub(super) async fn close_stands(
    store: &Store,
    ws: &str,
    insight: &Insight,
) -> Result<bool, CaseSvcError> {
    let Some(last) = lb_cases::last_closed_case_for_insight(store, ws, &insight.id).await? else {
        return Ok(false);
    };
    Ok(last
        .resolved_ts
        .is_some_and(|closed| closed >= insight.last_ts))
}
