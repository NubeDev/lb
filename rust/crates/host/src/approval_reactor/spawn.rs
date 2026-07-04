//! `spawn_approval_reactors` — the background driver that ticks [`react_to_approval_releases`] on a
//! cadence, the twin of [`spawn_flow_reactors`](crate::spawn_flow_reactors) /
//! [`spawn_agent_reactors`](crate::spawn_agent_reactors) (rules-approvals scope).
//!
//! Without this tick the release scan is dormant: a rule's held effect would stay held forever after
//! approval, because nothing calls the pass. One detached owner per node re-reads the durable
//! resolution set each tick and converges the held effects — exactly the flow/agent reactor shape.
//! Errors are logged, never fatal (a single bad ws must not stop the node's heartbeat; the next tick
//! re-scans). Symmetric across edge/cloud (rule 1) — placement is config (which node runs it), never a
//! code branch.

use std::sync::Arc;
use std::time::Duration;

use crate::boot::Node;

use super::react::react_to_approval_releases;

/// Spawn the detached release tick for the given workspaces. Returns immediately; the loop runs for the
/// life of the node. `period` is the scan cadence — a few seconds keeps a just-approved effect from
/// waiting long before the relay can pick it up, and each tick is a cheap ws-scoped resolution scan.
pub fn spawn_approval_reactors(node: Arc<Node>, workspaces: Vec<String>, period: Duration) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(period);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            for ws in &workspaces {
                match react_to_approval_releases(&node, ws).await {
                    Ok(pass) if pass.released > 0 || pass.discarded > 0 => {
                        tracing::info!(
                            ws = %ws,
                            released = pass.released,
                            discarded = pass.discarded,
                            "approval reactor released/discarded held effects"
                        );
                    }
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!(ws = %ws, error = %e, "approval release pass failed")
                    }
                }
            }
        }
    });
}
