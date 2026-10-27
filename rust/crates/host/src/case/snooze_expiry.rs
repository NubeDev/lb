//! `release_expired_snoozes` — end every snooze whose time has passed.
//!
//! A snooze is stored as `snooze_until`; nothing happens at that instant on its own, so a case
//! parked until Monday would still carry the park (and, if parking moved it to `actioned`, stay
//! there) long after Monday. This pass ends each expired snooze through the ordinary un-snooze
//! gesture ([`lb_cases::snooze`] with `until = now`), so the history gets the same `snoozed`
//! (`cleared`) event a person's un-park writes, and an `actioned` case goes back to `to_action`
//! ([`lb_cases`]'s snooze release rule).
//!
//! Same shape as [`super::close_self_cleared_cases`]: derived from the durable records each tick,
//! idempotent (an ended snooze is cleared, so a second pass finds nothing), best-effort per case.

use std::sync::Arc;

use lb_cases::Case;

use super::error::CaseSvcError;
use crate::boot::Node;

/// The actor stamped on an expiry. Not a person: the clock ran out, the platform ended the park.
pub const SNOOZE_EXPIRY_ACTOR: &str = "system:snooze-expiry";

/// End every open case's snooze whose `snooze_until` is at or before `now`. Returns how many ended.
pub async fn release_expired_snoozes(
    node: &Arc<Node>,
    ws: &str,
    now: u64,
) -> Result<usize, CaseSvcError> {
    let expired: Vec<Case> = super::self_clear::decode_all::<Case>(node, ws, lb_cases::CASE_TABLE)
        .await?
        .into_iter()
        .filter(|c| !c.closed && c.snooze_until.is_some_and(|u| u <= now))
        .collect();
    let mut released = 0usize;
    for case in expired {
        match lb_cases::snooze(
            &node.store,
            ws,
            &case.id,
            now,
            None,
            SNOOZE_EXPIRY_ACTOR,
            now,
        )
        .await
        {
            Ok(_) => {
                released += 1;
                // An expiry that ended an SLA pause has moved the deadlines; the clock writes them.
                if let Err(e) = super::sla_clock::apply_sla(node, ws, &case.id, now).await {
                    tracing::warn!(ws, case_id = %case.id, error = %e, "snooze expiry: sla recompute failed")
                }
            }
            // One case that will not release must not stop the pass; the next tick retries it.
            Err(e) => {
                tracing::warn!(ws, case_id = %case.id, error = %e, "snooze expiry: release failed")
            }
        }
    }
    Ok(released)
}
