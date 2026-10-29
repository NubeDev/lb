//! The case-reconcile loop driver (case-plane scope).
//!
//! One detached tick per node drives [`super::reconcile_cases`], [`super::backfill_case_facets`],
//! [`lb_cases::number_existing_cases`], [`super::release_expired_snoozes`], [`super::follow_sla_clocks`] and [`super::close_self_cleared_cases`] for each workspace,
//! modelled on `insight/reactor.rs::spawn_insight_digest_reactors` — role/config placement under the
//! `BootConfig::reactors` toggle, not a runtime election. The pass is idempotent, so the idempotence
//! IS the backstop against accidental double-drive; there is nothing to elect.
//!
//! This is the only place a wall clock is read on the case plane. Everything below it takes the
//! injected `now` (testing §3), which is why every reconcile behaviour in the test suite is driven
//! by calling `reconcile_cases` with a fixed clock rather than by waiting for a tick.

use std::sync::Arc;
use std::time::Duration;

use crate::boot::Node;

/// Spawn the per-node case reconcile loop. Returns immediately; the loop runs for the life of the
/// node, ticking every `period`.
pub fn spawn_case_reactors(node: Arc<Node>, workspaces: Vec<String>, period: Duration) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(period);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            let now = super::clock::now_ms();
            for ws in &workspaces {
                match super::reconcile_cases(&node, ws, now).await {
                    Ok(0) => {}
                    Ok(n) => {
                        tracing::info!(%ws, grouped = n, "case reconcile: ungrouped insights given cases")
                    }
                    Err(e) => {
                        tracing::warn!(%ws, error = %format!("{e:?}"), "case reconcile tick failed")
                    }
                }
                // Same tick, same idempotence: once the estate is repaired this is one scan that
                // fills nothing. It rides here rather than on its own timer because a facet gap and
                // an ungrouped insight are the same kind of debt — a record that predates a change.
                match super::backfill_case_facets(&node, ws, now).await {
                    Ok(0) => {}
                    Ok(n) => {
                        tracing::info!(%ws, repaired = n, "case facet backfill: missing facet echoes filled")
                    }
                    Err(e) => {
                        tracing::warn!(%ws, error = %format!("{e:?}"), "case facet backfill tick failed")
                    }
                }
                // The case numbers, the same kind of debt: cases from before numbering existed get
                // theirs on the first tick after an upgrade. Afterwards one record read and nothing.
                match lb_cases::number_existing_cases(&node.store, ws, now).await {
                    Ok(0) => {}
                    Ok(n) => {
                        tracing::info!(%ws, numbered = n, "case numbers: existing cases numbered")
                    }
                    Err(e) => {
                        tracing::warn!(%ws, error = %format!("{e:?}"), "case numbering tick failed")
                    }
                }
                // Before self-clear: a snooze that has run out is a case back in the queue, and
                // self-clear must see it as it now stands.
                match super::release_expired_snoozes(&node, ws, now).await {
                    Ok(0) => {}
                    Ok(n) => {
                        tracing::info!(%ws, released = n, "snooze expiry: expired snoozes ended")
                    }
                    Err(e) => {
                        tracing::warn!(%ws, error = %format!("{e:?}"), "snooze expiry tick failed")
                    }
                }
                // After the releases, so a hold that just ended is judged as the case now stands: the
                // SLA clock runs only while a case is Open, and its deadline moves with it.
                match super::follow_sla_clocks(&node, ws, now).await {
                    Ok(0) => {}
                    Ok(n) => {
                        tracing::info!(%ws, moved = n, "sla follow: clocks stopped or restarted")
                    }
                    Err(e) => {
                        tracing::warn!(%ws, error = %format!("{e:?}"), "sla follow tick failed")
                    }
                }
                // Last, so it judges cases the passes above have just opened, repaired or released. A policy
                // that has not opted in costs one policy read and nothing else.
                match super::close_self_cleared_cases(&node, ws, now).await {
                    Ok(0) => {}
                    Ok(n) => {
                        tracing::info!(%ws, closed = n, "case self-clear: untouched cases whose insights cleared resolved")
                    }
                    Err(e) => {
                        tracing::warn!(%ws, error = %format!("{e:?}"), "case self-clear tick failed")
                    }
                }
            }
        }
    });
}
