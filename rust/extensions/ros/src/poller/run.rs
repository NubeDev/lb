//! The async **poll task** — the runnable half of the engine: it owns a tokio task that drives
//! `poll_once` on a schedule with `Backoff`, and a shared status the `ros.status` verb reads. This is
//! the only time-aware part; the tick logic (`poll_once`) and the schedule math (`Backoff`) are pure
//! and tested without sleeping (`poller.rs`), so this file is thin glue: spawn, tick, sleep, repeat,
//! stop.
//!
//! **Runnable-trait grammar (resource-verbs Tier 2):** `ros.start` arms a task per connection,
//! `ros.stop` parks it (aborts the loop; state is in the store/series/outbox — rule 4 — so nothing is
//! lost), `ros.status` reports last-ok / last-fail / sample count / consecutive failures. `restart` =
//! stop + start (wired in the handler).
//!
//! **Stateless-extension safety:** the task holds NO durable state — only live counters. A hot-reload
//! drops the task; the config records (enable flags), the series, and pending outbox effects survive,
//! and `start` re-arms from them. The `PollRegistry` maps `ros_uuid → task handle` so a second `start`
//! is idempotent (returns the running task) and `stop` finds the right one.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::task::JoinHandle;

use super::poller::{poll_once, Backoff, SeqState, TickOutcome};
use super::sink::Sink;
use super::source::Source;

/// The live status a running poll task publishes and `ros.status` reads. Logical `ts`es (the tick
/// counter the loop stamps) — not wall-clock — so the status is deterministic in a test and carries no
/// clock dependency into core-adjacent code.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct PollStatus {
    pub running: bool,
    /// Total samples committed since this task started (across all ticks).
    pub samples: u64,
    /// Ticks that succeeded / failed since start.
    pub ok_ticks: u64,
    pub fail_ticks: u64,
    /// The tick `ts` of the last success / failure, if any.
    pub last_ok: Option<u64>,
    pub last_fail: Option<u64>,
    /// Consecutive failures right now (0 when healthy) — the backoff depth.
    pub consecutive_failures: u32,
    /// The last failure reason (diagnostic; never a token).
    pub last_fail_reason: Option<String>,
}

/// A shared, lock-guarded status cell the loop updates and `status()` snapshots.
type SharedStatus = Arc<Mutex<PollStatus>>;

/// One connection's running poll task: the tokio handle (to abort on stop) + its shared status.
pub struct PollTask {
    handle: JoinHandle<()>,
    status: SharedStatus,
}

impl PollTask {
    /// Snapshot the current status (what `ros.status` returns).
    pub fn status(&self) -> PollStatus {
        self.status.lock().unwrap().clone()
    }

    /// Stop the task: abort the loop and mark not-running. Idempotent (a second stop is a no-op abort).
    pub fn stop(&self) {
        self.handle.abort();
        self.status.lock().unwrap().running = false;
    }
}

/// Spawn a poll loop for one connection. `interval` is the base cadence between successful ticks;
/// `max_backoff` caps the failure backoff. The loop stamps a monotonically increasing tick `ts` (so a
/// test/status has an ordering without a wall-clock) and threads the per-series `SeqState`.
///
/// The task is `'static` (owns its `source`/`sink`), so it survives after the caller returns — the
/// `PollTask` handle is what `stop`/`status` reach it through.
pub fn spawn_poll(
    source: Arc<dyn Source>,
    sink: Arc<dyn Sink>,
    interval: Duration,
    max_backoff: Duration,
) -> PollTask {
    let status: SharedStatus = Arc::new(Mutex::new(PollStatus {
        running: true,
        ..Default::default()
    }));
    let loop_status = status.clone();
    let base_ms = interval.as_millis() as u64;
    let max_ms = max_backoff.as_millis().max(base_ms as u128) as u64;

    let handle = tokio::spawn(async move {
        let mut seq = SeqState::new();
        let mut backoff = Backoff::new(base_ms.max(1), max_ms);
        let mut tick_ts: u64 = 0;
        loop {
            tick_ts += 1;
            let outcome = match poll_once(source.as_ref(), sink.as_ref(), &mut seq, tick_ts).await {
                Ok(o) => o,
                // A sink/host callback failure is a transport problem — treat it like a failed tick so
                // the loop backs off rather than spinning; record the reason.
                Err(e) => TickOutcome::Failed {
                    reason: format!("sink: {e}"),
                },
            };
            apply_outcome(&loop_status, tick_ts, &outcome);
            let delay = backoff.next_delay_ms(&outcome);
            {
                let mut s = loop_status.lock().unwrap();
                s.consecutive_failures = backoff.consecutive_failures();
            }
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }
    });

    PollTask { handle, status }
}

/// Fold one tick's outcome into the shared status (the only place status mutates in the loop).
fn apply_outcome(status: &SharedStatus, tick_ts: u64, outcome: &TickOutcome) {
    let mut s = status.lock().unwrap();
    match outcome {
        TickOutcome::Ok { samples } => {
            s.ok_ticks += 1;
            s.samples += *samples as u64;
            s.last_ok = Some(tick_ts);
        }
        TickOutcome::Failed { reason } => {
            s.fail_ticks += 1;
            s.last_fail = Some(tick_ts);
            s.last_fail_reason = Some(reason.clone());
        }
    }
}

/// The per-(sidecar) map of `ros_uuid → running task`. Guards `start` idempotency and lets `stop`/
/// `status` find a connection's task. Held by the sidecar for the life of the process; a hot-reload
/// drops it and `start` re-arms (rule 4).
#[derive(Default)]
pub struct PollRegistry {
    tasks: Mutex<HashMap<String, PollTask>>,
}

impl PollRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Arm a connection's poll task, replacing any existing one (a re-`start` restarts cleanly). Returns
    /// nothing; inspect via `status`.
    pub fn start(&self, ros_uuid: &str, task: PollTask) {
        let mut tasks = self.tasks.lock().unwrap();
        if let Some(old) = tasks.insert(ros_uuid.to_string(), task) {
            old.stop();
        }
    }

    /// True iff a task is currently armed for this connection.
    pub fn is_running(&self, ros_uuid: &str) -> bool {
        self.tasks.lock().unwrap().contains_key(ros_uuid)
    }

    /// Stop + remove a connection's task. `false` if none was armed.
    pub fn stop(&self, ros_uuid: &str) -> bool {
        match self.tasks.lock().unwrap().remove(ros_uuid) {
            Some(task) => {
                task.stop();
                true
            }
            None => false,
        }
    }

    /// The status of a connection's task (`None` if not armed → `ros.status` reports `running:false`).
    pub fn status(&self, ros_uuid: &str) -> Option<PollStatus> {
        self.tasks.lock().unwrap().get(ros_uuid).map(|t| t.status())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::HostError;
    use crate::poller::sink::SampleOut;
    use crate::poller::source::{PollTarget, Reading, SourceError};
    use async_trait::async_trait;

    struct OneTargetSource;
    #[async_trait]
    impl Source for OneTargetSource {
        async fn targets(&self) -> Result<Vec<PollTarget>, SourceError> {
            Ok(vec![PollTarget::enabled("p", "s.p")])
        }
        async fn read(&self, t: &PollTarget, ts: u64) -> Result<Reading, SourceError> {
            Ok(Reading {
                series: t.series.clone(),
                value: serde_json::json!(1),
                ts,
            })
        }
    }

    struct NoopSink;
    #[async_trait]
    impl Sink for NoopSink {
        async fn write(&self, _b: &[SampleOut]) -> Result<(), HostError> {
            Ok(())
        }
    }

    #[tokio::test(start_paused = true)]
    async fn spawn_ticks_and_stop_parks() {
        let task = spawn_poll(
            Arc::new(OneTargetSource),
            Arc::new(NoopSink),
            Duration::from_millis(10),
            Duration::from_millis(100),
        );
        // With the clock paused, let a few intervals elapse deterministically.
        tokio::time::advance(Duration::from_millis(35)).await;
        tokio::task::yield_now().await;
        let st = task.status();
        assert!(st.running, "task reports running");
        assert!(st.ok_ticks >= 1, "at least one tick fired: {st:?}");
        task.stop();
        assert!(!task.status().running, "stop parks the task");
    }

    #[tokio::test(start_paused = true)]
    async fn registry_start_is_idempotent_and_stop_finds_it() {
        let reg = PollRegistry::new();
        let mk = || {
            spawn_poll(
                Arc::new(OneTargetSource),
                Arc::new(NoopSink),
                Duration::from_millis(10),
                Duration::from_millis(100),
            )
        };
        reg.start("r1", mk());
        assert!(reg.is_running("r1"));
        reg.start("r1", mk()); // replaces cleanly, still one task
        assert!(reg.is_running("r1"));
        assert!(reg.stop("r1"), "stop finds the armed task");
        assert!(!reg.is_running("r1"));
        assert!(!reg.stop("r1"), "second stop is a no-op");
    }
}
