//! `control-engine.watch` — the live COV feed (slice-6). Arms a CE COV subscription for an appliance's
//! scope and pumps each decoded event, re-encoded to the frame JSON (`frame.rs`), onto a workspace-scoped
//! series via the host `ingest.write` callback — the shipped `series` motion + gateway
//! `GET /series/{series}/stream` SSE is the live read S7 opens. This is the **fallback plumbing** decided
//! in slice-6 §"Sequencing fallback": zero core change, behind the same tool name + frame contract as the
//! future generic extension-watch primitive (tracked as a named migration follow-up in the session doc).
//!
//! **Lifecycle (arm-on-first / disarm-on-last):** a `WatchRegistry` (an in-memory pump-handle map — a
//! connection pool, NOT durable state, §3.4) keys one live pump per series and tracks a subscriber count.
//! First subscriber arms the pump (opens the CE WS lazily via `subscribe_cov`, spawns the pump task);
//! each further subscriber for the SAME `(appliance, scope)` shares it (refcount++). Last subscriber gone
//! drops the pump, which drops the `CovStream` (and the WS). `control-engine.appliance.remove` (S4)
//! force-disarms every live watch for an appliance.
//!
//! **Reconnect:** on a CE WS drop the pump re-subscribes with bounded backoff (mirrors `ws.ts`'s
//! STABLE_MS idea) — the subscriber sees a gap, not a dead stream. The pump lives until refcount hits 0.
//!
//! Fire-and-forget motion, no persistence (rule 3): a failed `ingest.write` for one frame is logged-by-
//! dropping and the pump continues; the durable authority is CE itself, not this feed.

pub mod frame;
pub mod pump;
pub mod scope_uids;
pub mod series;
pub mod verb;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use rubix_ce::{ControlEngine, CovScope};
use tokio::task::JoinHandle;

use crate::host::HostCtx;

/// One live pump: its task handle + the count of logical subscribers sharing it. Dropping `task` aborts
/// the pump (which drops its `CovStream`/WS). Refcount reaches 0 → the whole entry is removed → disarmed.
struct Armed {
    task: JoinHandle<()>,
    subscribers: usize,
    /// The appliance selector this pump watches — the key `appliance.remove` force-disarms on.
    appliance: String,
}

impl Drop for Armed {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// The sidecar's in-memory live-watch registry: series → armed pump. Shared across the control loop; a
/// kill + respawn rebuilds it lazily on the next `watch` call (stateless, §3.4).
#[derive(Clone, Default)]
pub struct WatchRegistry {
    armed: Arc<Mutex<HashMap<String, Armed>>>,
}

impl WatchRegistry {
    /// A fresh, empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Arm (or join) the watch for `series`/`scope` on `engine`, publishing onto `series` via `host`'s
    /// `ingest.write`. First caller spawns the pump; subsequent callers for the same series refcount up.
    /// Returns after the (idempotent) arm — the pump runs in the background.
    pub fn arm(
        &self,
        host: HostCtx,
        engine: Arc<dyn ControlEngine>,
        appliance: &str,
        series: &str,
        scope: CovScope,
    ) {
        let mut map = self.armed.lock().unwrap();
        if let Some(a) = map.get_mut(series) {
            a.subscribers += 1;
            return;
        }
        let task = tokio::spawn(pump::run(host, engine, series.to_string(), scope));
        map.insert(
            series.to_string(),
            Armed {
                task,
                subscribers: 1,
                appliance: appliance.to_string(),
            },
        );
    }

    /// Drop one subscriber from `series`. The last one out removes the entry, which aborts the pump
    /// (dropping its `CovStream` + WS). A no-op for an unknown series.
    pub fn release(&self, series: &str) {
        let mut map = self.armed.lock().unwrap();
        if let Some(a) = map.get_mut(series) {
            a.subscribers = a.subscribers.saturating_sub(1);
            if a.subscribers == 0 {
                map.remove(series); // Drop aborts the pump task → CovStream/WS torn down.
            }
        }
    }

    /// Force-disarm every live watch for `appliance` (the `appliance.remove` hook, S4). Removes each
    /// matching entry regardless of subscriber count — the appliance is gone, so its feeds must stop.
    pub fn disarm_appliance(&self, appliance: &str) {
        let mut map = self.armed.lock().unwrap();
        map.retain(|_, a| a.appliance != appliance);
    }

    /// The number of live (armed) series — a test seam for arm/disarm assertions.
    #[must_use]
    pub fn armed_count(&self) -> usize {
        self.armed.lock().unwrap().len()
    }

    /// The subscriber count for `series` (0 if not armed) — a test seam for refcount assertions.
    #[must_use]
    pub fn subscribers(&self, series: &str) -> usize {
        self.armed
            .lock()
            .unwrap()
            .get(series)
            .map_or(0, |a| a.subscribers)
    }
}

// ---------------------------------------------------------------------------------
// Lifecycle tests (arm-on-first / disarm-on-last, reconnect, appliance.remove force-disarm), driven
// against the ONE sanctioned fake (`ce_fake`) + its instrumented counters. The pump's `ingest.write`
// is fire-and-forget over a `SidecarClient` pointed at an unreachable gateway — the writes fail
// silently (motion, §3.3) and the pump keeps running, which is exactly what we assert on. No process,
// no real store; the routed SSE end-to-end proof is the separate two-node integration test.
// ---------------------------------------------------------------------------------
#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use crate::ce_fake::CeFake;
    use lb_sidecar_client::{Config, SidecarClient};
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    fn host() -> HostCtx {
        // A client pointed at a dead gateway — ingest.write fails, but that is fire-and-forget motion.
        let client = SidecarClient::with_config(Config::new(
            "http://127.0.0.1:1",
            "t",
            "ws1",
            "control-engine",
        ));
        HostCtx::with_parts(client, vec!["mcp:control-engine.watch:call".into()], "ws1")
    }

    async fn settle() {
        tokio::time::sleep(Duration::from_millis(120)).await;
    }

    #[tokio::test]
    async fn arm_on_first_then_disarm_on_last() {
        let fake = CeFake::seeded();
        let engine: Arc<dyn ControlEngine> = fake.clone();
        let watches = WatchRegistry::new();
        let series = "ce-cov:plant-1:test";

        // Two subscribers for the SAME series share ONE pump (refcount = 2, one CE subscription).
        watches.arm(
            host(),
            engine.clone(),
            "plant-1",
            series,
            CovScope::default(),
        );
        watches.arm(
            host(),
            engine.clone(),
            "plant-1",
            series,
            CovScope::default(),
        );
        settle().await;
        assert_eq!(
            watches.subscribers(series),
            2,
            "two logical subscribers share the pump"
        );
        assert_eq!(watches.armed_count(), 1, "one armed series");
        assert_eq!(
            fake.active_cov.load(Ordering::SeqCst),
            1,
            "exactly one CE COV subscription"
        );

        // Drop once → still armed.
        watches.release(series);
        settle().await;
        assert_eq!(watches.subscribers(series), 1, "still one subscriber");
        assert_eq!(
            fake.active_cov.load(Ordering::SeqCst),
            1,
            "CE subscription still live"
        );

        // Drop the last → disarm: the pump is aborted, the CovStream drops, the fake sees it.
        watches.release(series);
        settle().await;
        assert_eq!(watches.armed_count(), 0, "no armed series");
        assert_eq!(
            fake.active_cov.load(Ordering::SeqCst),
            0,
            "the fake's COV subscription dropped on last release"
        );
    }

    #[tokio::test]
    async fn appliance_remove_force_disarms_a_live_watch() {
        let fake = CeFake::seeded();
        let engine: Arc<dyn ControlEngine> = fake.clone();
        let watches = WatchRegistry::new();
        let series = "ce-cov:plant-9:test";

        watches.arm(host(), engine, "plant-9", series, CovScope::default());
        settle().await;
        assert_eq!(fake.active_cov.load(Ordering::SeqCst), 1);

        // Removing the appliance tears the pump down regardless of subscriber count.
        watches.disarm_appliance("plant-9");
        settle().await;
        assert_eq!(watches.armed_count(), 0, "watch force-disarmed");
        assert_eq!(
            fake.active_cov.load(Ordering::SeqCst),
            0,
            "CE subscription torn down"
        );
    }

    #[tokio::test]
    async fn ce_ws_drop_mid_watch_reconnects() {
        let fake = CeFake::seeded();
        let engine: Arc<dyn ControlEngine> = fake.clone();
        let watches = WatchRegistry::new();
        let series = "ce-cov:plant-r:test";

        watches.arm(host(), engine, "plant-r", series, CovScope::default());
        settle().await;
        assert_eq!(
            fake.cov_subscribes.load(Ordering::SeqCst),
            1,
            "one subscribe on arm"
        );

        // Simulate a CE WS drop; the pump observes the stream end and re-subscribes (a gap, not a
        // dead stream): the fake sees a SECOND subscribe, and the subscription is live again.
        fake.drop_ws();
        // Wait past the pump's BACKOFF_MIN (200ms) + the fake's re-subscribe.
        tokio::time::sleep(Duration::from_millis(600)).await;
        assert!(
            fake.cov_subscribes.load(Ordering::SeqCst) >= 2,
            "the pump re-subscribed after the WS drop: {}",
            fake.cov_subscribes.load(Ordering::SeqCst)
        );
        assert_eq!(watches.armed_count(), 1, "still armed across the reconnect");
        assert_eq!(
            fake.active_cov.load(Ordering::SeqCst),
            1,
            "subscription live again"
        );
    }
}
