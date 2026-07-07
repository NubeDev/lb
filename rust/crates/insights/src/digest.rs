//! `compute_due_digests` — the durable digest pass's PURE-over-the-store core
//! (insight-notify-scope.md).
//!
//! Scans `insight_notify` rows whose window has elapsed and `pending.count > 0`, feeds each due key
//! a `LadderInput::Tick` through the pure `ladder_step` (decay quiet keys, advance windows, zero
//! pending), persists the new state, and groups the resulting `Digest` deliveries into **one
//! [`PendingDigest`] per (sub, window)** — the aggregate the host then `channel.post`s under the
//! sub's stored principal (fire-time re-checked; on deny the host flips the sub dormant).
//!
//! The channel I/O + fire-time re-check + dormancy live in the HOST reactor (it needs `&Node` +
//! caps), not here (this crate is store-only, README §7). This function is the deterministic,
//! injected-clock brain; the host is the effectful shell (the flows/reminders split).

use std::collections::BTreeMap;

use lb_store::Store;

use crate::error::InsightsError;
use crate::ladder::{ladder_step, DeliveryReason, LadderInput};
use crate::notify_state::TABLE;
use crate::notify_store::{all_notify, write_notify};
use crate::policy::Policy;
use crate::severity::Severity;
use crate::subscription::Subscription;
use crate::table_scan::scan_all;

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

/// One aggregated digest the host must deliver — all of a sub's due keys in one message. The host
/// turns this into a single `channel.post` (fire-time re-checked) with a deep link to the filtered
/// Insights page. Idempotent per `(sub_id, window_start)` — the host derives the digest item id
/// from those two so a re-drive upserts the same Item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingDigest {
    /// The sub this digest is for (destination channel + stored principal live on its record).
    pub sub_id: String,
    /// The window this digest closes — the digest item id derives from `(sub_id, window_start)`.
    pub window_start: u64,
    /// The distinct dedup keys aggregated (the top-K the message names).
    pub keys: Vec<String>,
    /// Total raises summarized across all keys.
    pub count: u64,
    /// The worst severity across all keys (the digest headline severity).
    pub max_severity: Severity,
}

/// Run the digest computation for workspace `ws` at logical time `now`: tick every due ladder key,
/// persist the advanced state, and return the aggregated digests to deliver. `subs` supplies each
/// key's `throttle_override`/`muted`; `kill_off` names owners whose per-member kill switch is off
/// (their deliveries are suppressed — accounting still advances so a re-enable picks up sane
/// digests). Pure w.r.t. wall-clock (all timing on `now`) — the same state + `now` yields the same
/// result (determinism §3).
// SCOPE: docs/scope/insights/insight-notify-scope.md §"The digest reactor" + §"The state machine"
pub async fn compute_due_digests(
    store: &Store,
    ws: &str,
    now: u64,
    policy: &Policy,
    subs: &[Subscription],
    kill_off: &std::collections::HashSet<String>,
) -> Result<(DigestPass, Vec<PendingDigest>), InsightsError> {
    let by_id: BTreeMap<&str, &Subscription> = subs.iter().map(|s| (s.id.as_str(), s)).collect();

    let states = all_notify(store, ws).await?;
    let mut pass = DigestPass::default();
    // Aggregate deliveries per sub: (window_start, keys, count, max_severity).
    let mut agg: BTreeMap<String, PendingDigest> = BTreeMap::new();

    for state in states {
        let window = policy.windows[(state.level as usize).min(4)].max(1);
        // Only tick a row whose window has elapsed OR that carries pending — a not-yet-due quiet
        // row is left untouched (the scan is idempotent for it).
        let elapsed = now.saturating_sub(state.window_start) >= window;
        if !elapsed && state.pending.count == 0 {
            continue;
        }
        let sub = by_id.get(state.sub_id.as_str()).copied();
        let (throttle, muted, kill_on) = match sub {
            Some(s) => (
                s.throttle_override,
                s.muted || s.dormant_reason.is_some(),
                !kill_off.contains(&s.owner),
            ),
            // Orphaned state (sub deleted) — never deliver; still advance the state so it decays.
            None => (None, true, true),
        };
        let key = state.dedup_key.clone();
        let sub_id = state.sub_id.clone();
        // The pending count the tick is about to digest (ladder_step zeroes it) — captured for the
        // aggregate message before it's consumed.
        let pending_count = state.pending.count;
        let (next, deliveries) = ladder_step(
            Some(state),
            LadderInput::Tick { now },
            policy,
            throttle,
            muted,
            kill_on,
        );
        let digest_window = next.window_start;
        write_notify(store, ws, &next).await?;

        for d in deliveries {
            if d.reason != DeliveryReason::Digest {
                continue;
            }
            pass.deliveries += 1;
            pass.keys_aggregated += 1;
            let entry = agg.entry(sub_id.clone()).or_insert_with(|| PendingDigest {
                sub_id: sub_id.clone(),
                // The earliest window across the sub's keys anchors the idempotency id.
                window_start: digest_window,
                keys: Vec::new(),
                count: 0,
                max_severity: Severity::Info,
            });
            entry.window_start = entry.window_start.min(digest_window);
            if !entry.keys.contains(&key) {
                entry.keys.push(key.clone());
            }
            entry.count = entry.count.saturating_add(pending_count.max(1));
            entry.max_severity = entry.max_severity.max(d.severity);
        }
    }

    pass.windows_fired = agg.len();
    Ok((pass, agg.into_values().collect()))
}

/// The store table name re-exported for the host reactor's digest-item idempotency + tests.
pub const NOTIFY_TABLE: &str = TABLE;

/// Every ladder-state row in a workspace — re-exported so the host reactor / tests can inspect the
/// persisted state without reaching into the private store module.
pub async fn scan_notify_rows(
    store: &Store,
    ws: &str,
) -> Result<Vec<serde_json::Value>, InsightsError> {
    Ok(scan_all(store, ws, TABLE).await?)
}
