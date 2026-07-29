//! `spawn_reminder_reactors` — the background driver that ticks [`react_to_reminders`] on a cadence,
//! the twin of [`spawn_approval_reactors`](crate::spawn_approval_reactors) /
//! [`spawn_flow_reactors`](crate::spawn_flow_reactors).
//!
//! **Previously never booted** — the same missing-driver class as the ingest drain and the retention
//! GC before them. `react_to_reminders` shipped, is tested, and was reachable from nothing but a test
//! and the manual `reminder.fire` verb: on a real node every `enabled` reminder sat at its due
//! timestamp forever. A user could author a cron schedule, see it listed with a "next run" time, and
//! have it never once fire — the schedule surface was decorative.
//!
//! One detached owner per node. The cadence must be well under a minute because cron granularity is
//! one minute and `advance()` does **not** backfill: a missed minute is a skipped firing, not a
//! deferred one. Errors are logged, never fatal — one bad workspace must not stop the tick, and the
//! next pass re-scans. Symmetric across edge/cloud (rule 1): whether a node runs it is config.

use std::sync::Arc;
use std::time::Duration;

use crate::boot::Node;

use super::react::react_to_reminders;

/// The reminder scan cadence. Cron resolves to the minute and a firing is skipped rather than
/// backfilled, so the tick has to be comfortably finer than a minute; each pass is a cheap indexed
/// scan of the workspace's due set.
pub const REMINDER_PERIOD: Duration = Duration::from_secs(10);

/// Spawn the detached reminder tick for the given workspaces. Returns immediately; the loop runs for
/// the life of the node.
pub fn spawn_reminder_reactors(node: Arc<Node>, workspaces: Vec<String>, period: Duration) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(period);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            for ws in &workspaces {
                // SECONDS, not millis — the reminder clock is a logical second clock (`reminder.*`
                // stamps `ts` in seconds and croner parses against it). Feeding it millis would put
                // every `next_attempt_ts` ~55,000 years in the past and fire every reminder at once.
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                match react_to_reminders(&node, ws, now).await {
                    Ok(pass) if pass.fired > 0 || pass.denied > 0 => {
                        tracing::info!(
                            ws = %ws,
                            fired = pass.fired,
                            denied = pass.denied,
                            skipped = pass.skipped,
                            "reminder reactor fired due reminders"
                        );
                    }
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!(ws = %ws, error = %e, "reminder pass failed")
                    }
                }
            }
        }
    });
}
