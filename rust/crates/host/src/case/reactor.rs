//! The case-reconcile loop driver (case-plane scope).
//!
//! One detached tick per node drives [`super::reconcile_cases`] and [`super::backfill_case_facets`]
//! for each workspace, modelled on
//! `insight/reactor.rs::spawn_insight_digest_reactors` — role/config placement under the
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
            }
        }
    });
}
