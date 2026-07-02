//! The sidecar **relay loop** — the out-of-process delivery driver for `ros` outbox effects. It is the
//! native-tier peer of `github-workflow`'s in-process `relay_outbox` loop, but it runs in the sidecar
//! and reaches the host's durable outbox over the callback (the sidecar-drivable relay verbs added in
//! this slice): `outbox.due {target:"ros", now}` → deliver each through `RosTarget` → `mark_delivered`
//! / `mark_failed`. One loop per sidecar (all connections), armed at start.
//!
//! **Stateless (rule 4):** the loop holds no durable state — the `due` set IS the state, in the store.
//! A respawn re-reads it, so an effect that crashed mid-delivery is found again (never lost); the box
//! dedups on the slot, so re-delivery is a no-op (never double-applied). This is the must-deliver
//! guarantee `point.write` relies on.
//!
//! `relay_pass` (one scan+deliver+mark cycle) is the testable core; `spawn_relay` is the thin async
//! loop that calls it on an interval. Both take a logical `now` (the sidecar's wall-clock ts — data,
//! not an ordering key; the store's own seq orders), so a test drives `relay_pass` directly.

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use super::ros_target::{deliver, DeliverOutcome};
use crate::host::{HostCtx, HostError};
use crate::resolve::RosApiFactory;

/// The `target` string ROS effects carry (matches `handlers::point::ROS_TARGET`) — the `outbox.due`
/// filter so this relay pulls only ROS setpoint effects, never another target's.
const ROS_TARGET: &str = "ros";

/// The tally of one relay pass — how many effects were delivered, left for retry, or failed
/// (attempt counted; the host dead-letters at `max_attempts`).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RelayPass {
    pub delivered: usize,
    pub retried: usize,
    pub failed: usize,
}

/// Run one relay pass at logical `now`: pull the due `ros` effects, deliver each through `RosTarget`,
/// and mark the outcome back through the host. A callback failure pulling `due` aborts the pass (it
/// retries next tick); a per-effect delivery failure only affects that effect. Returns the tally.
pub async fn relay_pass(
    host: &HostCtx,
    factory: &dyn RosApiFactory,
    now: u64,
) -> Result<RelayPass, HostError> {
    let out = host
        .client()
        .call_tool("outbox.due", json!({ "target": ROS_TARGET, "now": now }))
        .await?;
    let effects = out
        .get("effects")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut pass = RelayPass::default();
    for effect in &effects {
        let id = effect
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let payload = effect
            .get("payload")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if id.is_empty() {
            continue;
        }
        match deliver(host, factory, payload).await {
            DeliverOutcome::Delivered => {
                mark_delivered(host, id).await?;
                pass.delivered += 1;
            }
            DeliverOutcome::Retry => {
                // Leave it schedulable but count the attempt + back off (mark_failed does both).
                mark_failed(host, id, now).await?;
                pass.retried += 1;
            }
            DeliverOutcome::Fail => {
                mark_failed(host, id, now).await?;
                pass.failed += 1;
            }
        }
    }
    Ok(pass)
}

async fn mark_delivered(host: &HostCtx, id: &str) -> Result<(), HostError> {
    host.client()
        .call_tool("outbox.mark_delivered", json!({ "id": id }))
        .await?;
    Ok(())
}

async fn mark_failed(host: &HostCtx, id: &str, now: u64) -> Result<(), HostError> {
    host.client()
        .call_tool("outbox.mark_failed", json!({ "id": id, "now": now }))
        .await?;
    Ok(())
}

/// Spawn the relay loop: run `relay_pass` every `interval`, forever (until the sidecar exits). `now_fn`
/// supplies the logical timestamp per pass (wall-clock in production; a test drives `relay_pass`
/// directly instead). A pass error is swallowed (logged upstream) so a transient host hiccup never
/// kills the loop — the next tick retries the same durable set.
pub fn spawn_relay<F>(
    host: HostCtx,
    factory: Arc<dyn RosApiFactory>,
    interval: Duration,
    now_fn: F,
) -> tokio::task::JoinHandle<()>
where
    F: Fn() -> u64 + Send + 'static,
{
    tokio::spawn(async move {
        loop {
            let _ = relay_pass(&host, factory.as_ref(), now_fn()).await;
            tokio::time::sleep(interval).await;
        }
    })
}
