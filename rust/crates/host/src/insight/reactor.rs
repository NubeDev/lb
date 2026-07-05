//! `react_to_insight_digests` — the durable digest reactor's effectful shell (insight-notify
//! scope). The pure brain (`lb_insights::compute_due_digests`) scans the ladder state, ticks due
//! keys on the injected clock, persists the advanced state, and returns one aggregated
//! [`lb_insights::PendingDigest`] per (sub, window). This shell turns each into ONE `channel.post`
//! under the sub's stored principal (fire-time re-checked; deny ⇒ dormant), idempotent per
//! `(sub, window_start)` via the derived digest item id.
//!
//! Placement follows the flows/approval reactor precedent: one detached spawn loop per node drives
//! a workspace's digests (role/config placement, not a runtime election). The idempotent item id is
//! the backstop against accidental double-drive.

use std::sync::Arc;
use std::time::Duration;

use lb_insights::{compute_due_digests, DigestPass, PendingDigest, Severity};

use super::error::InsightSvcError;
use super::notify::{deliver_to_sub, kill_off_owners, load_subs};
use crate::boot::Node;

/// Drive one digest scan for workspace `ws` at logical time `now`. Returns the pass accounting.
/// Idempotent per `(sub, window_start)` — re-running after the state was consumed posts nothing new.
// SCOPE: docs/scope/insights/insight-notify-scope.md §"The digest reactor"
pub async fn react_to_insight_digests(
    node: &Arc<Node>,
    ws: &str,
    now: u64,
) -> Result<DigestPass, InsightSvcError> {
    let policy = lb_insights::policy_get(&node.store, ws).await?;
    let subs = load_subs(&node.store, ws).await;
    let kill_off = kill_off_owners(&node.store, ws, &subs).await;

    let (pass, digests) =
        compute_due_digests(&node.store, ws, now, &policy, &subs, &kill_off).await?;

    for digest in digests {
        if let Some(sub) = subs.iter().find(|s| s.id == digest.sub_id) {
            let item_id = format!("digest:{}:{}", digest.sub_id, digest.window_start);
            let body = digest_body(&digest);
            deliver_to_sub(node, ws, sub, &item_id, &body, now).await;
        }
    }
    Ok(pass)
}

/// The mechanical v1 digest message — count + worst severity + the keys, with a deep link. A rich
/// `render:` table is the named follow-up (notify scope).
fn digest_body(d: &PendingDigest) -> String {
    let sev = match d.max_severity {
        Severity::Critical => "critical",
        Severity::Warning => "warning",
        Severity::Info => "info",
    };
    let keys = d.keys.join(", ");
    format!(
        "{} occurrence(s) across {} insight(s) this window — worst: {} — keys: {} [view]",
        d.count,
        d.keys.len(),
        sev,
        keys
    )
}

/// The system principal subject the digest reactor loop attributes its scans to.
const REACTOR_SUB: &str = "node:insight-digest";

/// Spawn the per-node digest reactor loop (the flows/approval precedent). One detached task ticks
/// every `period`, driving `react_to_insight_digests` for each workspace on the wall clock (the
/// only place a wall clock is read — the pure state machine stays injected-clock, testing §3).
pub fn spawn_insight_digest_reactors(node: Arc<Node>, workspaces: Vec<String>, period: Duration) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(period);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            for ws in &workspaces {
                if let Err(e) = react_to_insight_digests(&node, ws, now).await {
                    tracing::warn!(%ws, error = %format!("{e:?}"), "insight digest reactor tick failed");
                }
            }
        }
    });
    let _ = REACTOR_SUB;
}
