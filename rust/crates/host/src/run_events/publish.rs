//! Publish a [`RunEvent`] onto the run's bus subject (agent-run scope Part 3) — how the loop turns
//! its durable transcript appends into live motion a watcher sees. Called by the loop right *after*
//! it persists the corresponding transcript event, so the stream never leads the record (the record
//! is the transcript; the stream is a projection of it — §3.3).
//!
//! Fire-and-forget: a publish failure (no subscribers, a transient bus error) is **not** fatal to
//! the run — the durable transcript is intact and a watcher catches up from the snapshot. So this
//! returns `()` and swallows the bus error after logging the shape; the loop must never die because
//! nobody was watching.

use lb_bus::{publish, Bus};
use lb_run_events::RunEvent;

use super::subject::run_subject;

/// Publish `event` for run `job_id` in workspace `ws`. Best-effort: serialization or bus failure is
/// dropped (the transcript remains the source of truth). The payload is the JSON encoding of the
/// `RunEvent` — the exact bytes the gateway SSE route and the ACP encoder decode.
pub async fn publish_run_event(bus: &Bus, ws: &str, job_id: &str, event: &RunEvent) {
    let Ok(bytes) = serde_json::to_vec(event) else {
        return;
    };
    let rel = run_subject(job_id);
    let _ = publish(bus, ws, &rel, &bytes).await;
}
