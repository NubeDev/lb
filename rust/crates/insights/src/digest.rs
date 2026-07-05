//! `react_to_insight_digests` — the durable digest reactor (insight-notify-scope.md).
//!
//! A durable scan on the injected clock over `insight_notify` rows whose
//! `window_start + window(level)` has elapsed and `pending.count > 0` → compose **one digest per
//! (sub, window)** aggregating all that sub's due keys (not one message per key — the whole point),
//! `channel.post` under the sub's stored principal (fire-time re-checked per the subscriptions
//! scope), zero the pendings, advance windows, apply decay for quiet keys. Idempotent per
//! `(sub, window_start)` — the digest item id is derived from it (the inbox idempotency contract).
//!
//! Follows the flows/reminders **owner-election** precedent so exactly one node drives a
//! workspace's digests (without it two nodes double-post; the idempotent item id is the backstop).
//!
//! **STUB**: the compose/aggregate/idempotency logic + owner-election body is deferred — this is
//! a named load-bearing piece of the implementing session. See the punch-list.

use lb_store::Store;

use crate::error::InsightsError;
use crate::ladder::Delivery;

/// One pass's accounting — the digest reactor's return for a single scan (mirrors the other
/// reactors' `*Pass` types — `RelayPass`, `FlowReactorPass`, etc.).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DigestPass {
    /// How many digest windows fired this pass.
    pub windows_fired: usize,
    /// How many individual keys were aggregated into those digests.
    pub keys_aggregated: usize,
    /// How many deliveries the ladder emitted (digest + any breakthroughs that landed this pass).
    pub deliveries: usize,
}

/// Drive one digest scan for workspace `ws` at logical time `now`. Returns the pass accounting.
/// Idempotent per `(sub, window_start)` — re-running with the same `now` re-upserts the same
/// digest items (no duplicate posts).
// SCOPE: docs/scope/insights/insight-notify-scope.md §"The digest reactor"
pub async fn react_to_insight_digests(
    _store: &Store,
    _ws: &str,
    _now: u64,
) -> Result<DigestPass, InsightsError> {
    // 1. Owner-election guard (the flows/reminders precedent): if THIS node is not the elected
    //    driver for ws ⇒ return DigestPass::default() (another node is driving).
    // 2. Scan `insight_notify` rows in ws; filter to those whose
    //    `window_start + policy.windows[level] <= now` AND `pending.count > 0`.
    // 3. Group by sub_id; for each sub compose ONE digest message aggregating all due keys
    //    (count, max_severity, top-K keys); the digest item id = `digest:{sub}:{window_start}`.
    // 4. For each aggregated key: feed a `LadderInput::Tick{now}` through `ladder_step` (decay
    //    for quiet keys, advance windows, zero pending). Collect deliveries.
    // 5. `channel.post` each delivery under the sub's stored principal (fire-time re-check; on
    //    deny flip dormant per the subscriptions scope). The digest item upsert is idempotent.
    // 6. Return DigestPass accounting.
    let _delivery_sample = Delivery {
        sub_id: String::new(),
        insight_id: String::new(),
        dedup_key: String::new(),
        reason: crate::ladder::DeliveryReason::Digest,
        severity: crate::severity::Severity::Info,
        ts: 0,
    };
    todo!("insights: digest reactor (compose + owner-election + idempotent upsert) — SCOPE: notify-scope.md §The digest reactor")
}
