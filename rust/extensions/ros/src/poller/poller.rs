//! The reusable poll **engine** — driver-agnostic loop + schedule + backoff + batch shaping over a
//! `Source` (read seam) and a `Sink` (write seam). No ROS vocabulary reaches here; `RosSource` adapts
//! `RosApi` into the `Source` trait and this loop drives it exactly as it would any future driver.
//!
//! **One tick** (`poll_once`, the unit-testable core):
//!   1. `source.targets()` — the tree + its four enable flags. `Unreachable`/error here fails the tick.
//!   2. `gating::resolve` — keep only leaves where connection ∧ network ∧ device ∧ point are all on.
//!   3. `source.read(target, ts)` per enabled leaf — a per-target `NotFound`/`Other` drops that leaf;
//!      an `Unreachable` fails the whole tick (the box went down mid-walk → back off, don't half-write).
//!   4. one `sink.write(batch)` for the surviving readings, each stamped with a monotonic per-series
//!      `seq` (the ingest dedup key the engine owns; producer is host-forced).
//!
//! **Schedule + backoff** live in `run` (the async loop): a fixed base interval between successful
//! ticks; on a *tick failure* an exponential backoff (base·2ⁿ, capped) instead, reset on the next
//! success. `poll_once` is pure w.r.t. time (it takes the tick `ts` and the seq state), so scheduling
//! and backoff are tested by driving `poll_once` directly and by inspecting the `Backoff` calculator —
//! no wall-clock, no sleeping test.

use std::collections::HashMap;

use super::gating;
use super::sink::{SampleOut, Sink};
use super::source::{Source, SourceError};
use crate::host::HostError;

/// The per-series monotonic sequence the engine threads across ticks — the `seq` half of ingest's
/// `(series, producer, seq)` dedup key. A fresh reading each tick gets the next seq; the map lives for
/// the life of the poll task (a connection's `start`→`stop`).
#[derive(Debug, Default)]
pub struct SeqState {
    next: HashMap<String, u64>,
}

impl SeqState {
    pub fn new() -> Self {
        Self::default()
    }

    /// The next seq for `series`, advancing the counter. Starts at 0 per series.
    fn bump(&mut self, series: &str) -> u64 {
        let n = self.next.entry(series.to_string()).or_insert(0);
        let cur = *n;
        *n += 1;
        cur
    }
}

/// The outcome of one tick — what the loop uses to pick the next delay, and what `status` reports.
#[derive(Debug, Clone, PartialEq)]
pub enum TickOutcome {
    /// A committed batch of `n` samples (n may be 0: everything gated off is a *success*, not a
    /// failure — we polled correctly and there was nothing to write, so no backoff).
    Ok { samples: usize },
    /// The tick failed as a whole (targets enumeration failed, or a read was `Unreachable`) — the
    /// loop backs off. Carries the reason for `status`/logs (never a token).
    Failed { reason: String },
}

/// Run one poll tick against the source + sink. Pure w.r.t. time: `ts` is the tick's logical timestamp
/// and `seq` is the caller-owned per-series counter, so the loop's scheduling is separable and this is
/// unit-testable with a stub source + recording sink. Returns the tick outcome; a `Sink` write failure
/// surfaces as `Err` (a host/transport problem, distinct from a source-side tick failure).
pub async fn poll_once(
    source: &dyn Source,
    sink: &dyn Sink,
    seq: &mut SeqState,
    ts: u64,
) -> Result<TickOutcome, HostError> {
    let targets = match source.targets().await {
        Ok(t) => t,
        Err(e) => {
            return Ok(TickOutcome::Failed {
                reason: e.to_string(),
            })
        }
    };
    let live = gating::resolve(&targets);

    let mut batch: Vec<SampleOut> = Vec::with_capacity(live.len());
    for target in &live {
        match source.read(target, ts).await {
            Ok(reading) => {
                let s = seq.bump(&reading.series);
                batch.push(SampleOut::from_reading(&reading, s));
            }
            // The box went down mid-walk: abandon the tick and back off rather than commit a partial
            // batch (a half-written cycle would look like real gaps in the series).
            Err(SourceError::Unreachable(m)) => {
                return Ok(TickOutcome::Failed {
                    reason: format!("unreachable: {m}"),
                })
            }
            // One dead/absent point does not sink the tick — drop it, keep the rest.
            Err(SourceError::NotFound(_)) | Err(SourceError::Other(_)) => continue,
        }
    }

    let n = batch.len();
    sink.write(&batch).await?;
    Ok(TickOutcome::Ok { samples: n })
}

/// The delay calculator: a base interval between successful ticks, exponential backoff after failures.
/// Separated from the async loop so the backoff schedule is asserted without sleeping.
#[derive(Debug, Clone)]
pub struct Backoff {
    base_ms: u64,
    max_ms: u64,
    /// Consecutive failures since the last success — the backoff exponent.
    fails: u32,
}

impl Backoff {
    pub fn new(base_ms: u64, max_ms: u64) -> Self {
        Self {
            base_ms,
            max_ms,
            fails: 0,
        }
    }

    /// Record a tick outcome and return the delay to wait before the next tick. A success resets the
    /// backoff to the base interval; a failure grows it geometrically (base·2ⁿ) up to `max_ms`.
    pub fn next_delay_ms(&mut self, outcome: &TickOutcome) -> u64 {
        match outcome {
            TickOutcome::Ok { .. } => {
                self.fails = 0;
                self.base_ms
            }
            TickOutcome::Failed { .. } => {
                // Saturating shift so a long outage never overflows; cap at max_ms.
                let factor = 1u64.checked_shl(self.fails).unwrap_or(u64::MAX);
                let delay = self.base_ms.saturating_mul(factor).min(self.max_ms);
                self.fails = self.fails.saturating_add(1);
                delay
            }
        }
    }

    /// Consecutive failures since the last success (for `status`).
    pub fn consecutive_failures(&self) -> u32 {
        self.fails
    }
}

#[cfg(test)]
mod tests {
    use super::super::source::{PollTarget, Reading};
    use super::*;
    use async_trait::async_trait;
    use std::sync::Mutex;

    /// A stub `Source` with no box and no gateway: a fixed target set + per-id canned values, plus an
    /// `unreachable` switch and a set of ids whose `read` returns `NotFound` (a dead point). This is the
    /// whole point of the seam — the loop/gating/backoff/batch are proven here with zero infrastructure.
    struct StubSource {
        targets: Vec<PollTarget>,
        values: HashMap<String, f64>,
        unreachable_targets: bool,
        unreachable_read: Mutex<bool>,
        not_found: Vec<String>,
    }

    impl StubSource {
        fn new(targets: Vec<PollTarget>) -> Self {
            let values = targets.iter().map(|t| (t.id.clone(), 1.0)).collect();
            Self {
                targets,
                values,
                unreachable_targets: false,
                unreachable_read: Mutex::new(false),
                not_found: Vec::new(),
            }
        }
    }

    #[async_trait]
    impl Source for StubSource {
        async fn targets(&self) -> Result<Vec<PollTarget>, SourceError> {
            if self.unreachable_targets {
                return Err(SourceError::Unreachable("stub down".into()));
            }
            Ok(self.targets.clone())
        }

        async fn read(&self, target: &PollTarget, ts: u64) -> Result<Reading, SourceError> {
            if *self.unreachable_read.lock().unwrap() {
                return Err(SourceError::Unreachable("stub down mid-walk".into()));
            }
            if self.not_found.contains(&target.id) {
                return Err(SourceError::NotFound(target.id.clone()));
            }
            let v = self.values.get(&target.id).copied().unwrap_or(0.0);
            Ok(Reading {
                series: target.series.clone(),
                value: serde_json::json!(v),
                ts,
            })
        }
    }

    /// A `Sink` that records every batch it is handed — the batch-shaping assertion surface.
    #[derive(Default)]
    struct RecordingSink {
        batches: Mutex<Vec<Vec<SampleOut>>>,
    }

    #[async_trait]
    impl Sink for RecordingSink {
        async fn write(&self, batch: &[SampleOut]) -> Result<(), HostError> {
            self.batches.lock().unwrap().push(batch.to_vec());
            Ok(())
        }
    }

    fn tgt(id: &str, conn: bool, net: bool, dev: bool, point: bool) -> PollTarget {
        PollTarget {
            id: id.into(),
            series: format!("ros.ws.r.n.d.{id}"),
            connection_enable: conn,
            network_enable: net,
            device_enable: dev,
            point_enable: point,
        }
    }

    #[tokio::test]
    async fn tick_writes_only_enabled_targets_as_one_batch() {
        let source = StubSource::new(vec![
            tgt("a", true, true, true, true),
            tgt("b", true, true, false, true), // device off → gated out
            tgt("c", true, true, true, true),
        ]);
        let sink = RecordingSink::default();
        let mut seq = SeqState::new();

        let out = poll_once(&source, &sink, &mut seq, 100).await.unwrap();
        assert_eq!(out, TickOutcome::Ok { samples: 2 });

        let batches = sink.batches.lock().unwrap();
        assert_eq!(batches.len(), 1, "exactly one ingest.write per tick");
        let series: Vec<&str> = batches[0].iter().map(|s| s.series.as_str()).collect();
        assert_eq!(
            series,
            vec!["ros.ws.r.n.d.a", "ros.ws.r.n.d.c"],
            "only enabled leaves, in order"
        );
        assert!(batches[0].iter().all(|s| s.ts == 100), "tick ts stamped");
        assert!(
            batches[0].iter().all(|s| s.seq == 0),
            "first tick seq=0 per series"
        );
    }

    #[tokio::test]
    async fn each_gating_level_silences_its_target() {
        // One target per level-off, plus one fully-on. Only the fully-on survives; the AND is exact
        // through the real engine path (not just the gating unit).
        let source = StubSource::new(vec![
            tgt("conn_off", false, true, true, true),
            tgt("net_off", true, false, true, true),
            tgt("dev_off", true, true, false, true),
            tgt("point_off", true, true, true, false),
            tgt("live", true, true, true, true),
        ]);
        let sink = RecordingSink::default();
        let mut seq = SeqState::new();
        poll_once(&source, &sink, &mut seq, 1).await.unwrap();
        let batches = sink.batches.lock().unwrap();
        let series: Vec<&str> = batches[0].iter().map(|s| s.series.as_str()).collect();
        assert_eq!(
            series,
            vec!["ros.ws.r.n.d.live"],
            "only the all-enabled leaf polls"
        );
    }

    #[tokio::test]
    async fn seq_advances_per_series_across_ticks() {
        let source = StubSource::new(vec![tgt("a", true, true, true, true)]);
        let sink = RecordingSink::default();
        let mut seq = SeqState::new();
        for expected in 0..3u64 {
            poll_once(&source, &sink, &mut seq, expected * 10)
                .await
                .unwrap();
            let batches = sink.batches.lock().unwrap();
            assert_eq!(
                batches.last().unwrap()[0].seq,
                expected,
                "seq is monotonic per series across ticks"
            );
        }
    }

    #[tokio::test]
    async fn unreachable_targets_fails_tick_no_write() {
        let mut source = StubSource::new(vec![tgt("a", true, true, true, true)]);
        source.unreachable_targets = true;
        let sink = RecordingSink::default();
        let mut seq = SeqState::new();
        let out = poll_once(&source, &sink, &mut seq, 1).await.unwrap();
        assert!(
            matches!(out, TickOutcome::Failed { .. }),
            "down box fails the tick"
        );
        assert!(
            sink.batches.lock().unwrap().is_empty(),
            "no partial write on failure"
        );
    }

    #[tokio::test]
    async fn unreachable_mid_walk_abandons_tick() {
        let source = StubSource::new(vec![
            tgt("a", true, true, true, true),
            tgt("b", true, true, true, true),
        ]);
        *source.unreachable_read.lock().unwrap() = true;
        let sink = RecordingSink::default();
        let mut seq = SeqState::new();
        let out = poll_once(&source, &sink, &mut seq, 1).await.unwrap();
        assert!(matches!(out, TickOutcome::Failed { .. }));
        assert!(
            sink.batches.lock().unwrap().is_empty(),
            "no half-batch committed"
        );
    }

    #[tokio::test]
    async fn not_found_target_dropped_rest_survive() {
        let mut source = StubSource::new(vec![
            tgt("dead", true, true, true, true),
            tgt("live", true, true, true, true),
        ]);
        source.not_found = vec!["dead".into()];
        let sink = RecordingSink::default();
        let mut seq = SeqState::new();
        let out = poll_once(&source, &sink, &mut seq, 1).await.unwrap();
        assert_eq!(
            out,
            TickOutcome::Ok { samples: 1 },
            "one dead point, one survives"
        );
        let batches = sink.batches.lock().unwrap();
        let series: Vec<&str> = batches[0].iter().map(|s| s.series.as_str()).collect();
        assert_eq!(series, vec!["ros.ws.r.n.d.live"]);
    }

    #[tokio::test]
    async fn all_gated_off_is_success_empty_batch() {
        let source = StubSource::new(vec![tgt("a", true, false, true, true)]);
        let sink = RecordingSink::default();
        let mut seq = SeqState::new();
        let out = poll_once(&source, &sink, &mut seq, 1).await.unwrap();
        assert_eq!(
            out,
            TickOutcome::Ok { samples: 0 },
            "nothing to poll is a clean tick"
        );
        // The batch handed to the sink is empty — the real `IngestSink` no-ops on that (no ingest.write),
        // which the recording sink shows by receiving a single empty batch.
        let batches = sink.batches.lock().unwrap();
        assert_eq!(batches.len(), 1);
        assert!(
            batches[0].is_empty(),
            "empty batch when everything is gated off"
        );
    }

    #[test]
    fn backoff_grows_then_resets_on_success() {
        let mut b = Backoff::new(100, 2000);
        let fail = TickOutcome::Failed { reason: "x".into() };
        let ok = TickOutcome::Ok { samples: 1 };
        assert_eq!(b.next_delay_ms(&fail), 100, "first failure = base");
        assert_eq!(b.next_delay_ms(&fail), 200, "then 2x");
        assert_eq!(b.next_delay_ms(&fail), 400, "then 4x");
        assert_eq!(b.next_delay_ms(&ok), 100, "success resets to base");
        assert_eq!(b.consecutive_failures(), 0);
        assert_eq!(
            b.next_delay_ms(&fail),
            100,
            "backoff restarts from base after a success"
        );
    }

    #[test]
    fn backoff_caps_at_max() {
        let mut b = Backoff::new(100, 500);
        let fail = TickOutcome::Failed { reason: "x".into() };
        let delays: Vec<u64> = (0..8).map(|_| b.next_delay_ms(&fail)).collect();
        assert!(
            delays.iter().all(|&d| d <= 500),
            "never exceeds max: {delays:?}"
        );
        assert_eq!(*delays.last().unwrap(), 500, "saturates at the cap");
    }
}
