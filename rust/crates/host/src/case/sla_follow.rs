//! `follow_sla_clocks` — keep every open case's SLA clock in step with its status, on the tick
//! (Insights review 2026-10-29: the clock runs only while a case is Open).
//!
//! Every write already stops or starts the clock as it moves a case out of or back into Open
//! (`lb_cases`'s `sla_follow`, run by the crate's one write path). Two things no write covers:
//!
//!   1. **The cases stored before the rule** — out of Open with no pause on record. The crate pass
//!      pauses them from their last activity, the best record of when they left Open.
//!   2. **The deadline itself.** A pause that ends credits its time to `sla_paused_ms`, and the
//!      deadline is a pure function of that ([`super::sla_clock`]); it moves, and its breach alarm is
//!      re-armed, only when [`super::sla_clock::apply_sla`] recomputes it. So every open case is
//!      recomputed here — a no-op when nothing moved (the crate verb writes only on a change), and
//!      on the first tick after an upgrade the pass that moves every open case onto the respond
//!      target.
//!
//! Same shape as [`super::release_expired_snoozes`] and [`super::close_self_cleared_cases`]: derived
//! from the durable records each tick, idempotent, best-effort per case.
//!
//! One responsibility: the SLA follow pass.

use std::sync::Arc;

use lb_cases::Case;

use super::error::CaseSvcError;
use super::self_clear::decode_all;
use crate::boot::Node;

/// Settle every open case's pause and recompute its deadline. Returns how many pauses moved.
pub async fn follow_sla_clocks(
    node: &Arc<Node>,
    ws: &str,
    now: u64,
) -> Result<usize, CaseSvcError> {
    let moved = lb_cases::settle_sla_clocks(&node.store, ws, now).await?;
    let open: Vec<Case> = decode_all::<Case>(node, ws, lb_cases::CASE_TABLE)
        .await?
        .into_iter()
        .filter(|c| !c.closed)
        .collect();
    for case in open {
        if let Err(e) = super::sla_clock::apply_sla(node, ws, &case.id, now).await {
            tracing::warn!(ws, case_id = %case.id, error = ?e, "sla follow: deadline not recomputed");
        }
    }
    Ok(moved)
}
