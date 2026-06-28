//! The bus subject a run's [`RunEvent`](lb_run_events::RunEvent) stream rides on (agent-run scope
//! Part 3). One place owns the key so the publisher (the loop) and the subscriber (`agent.watch`)
//! always agree, and so the workspace wall is structural: `lb_bus::publish`/`subscribe` prepend
//! `ws/{id}/`, so a watcher in ws-B physically cannot subscribe to a ws-A run's events (§7).
//!
//! The stream is **motion** (§3.3): a dropped subscriber misses deltas but re-reads the durable job
//! transcript to catch up (the snapshot in `watch.rs`). The transcript is the record; this subject
//! is never the record.

/// The workspace-relative subject for run `job_id`'s event stream. `lb_bus` walls it under
/// `ws/{id}/` — the result is `ws/{id}/run/{job_id}/events`. `run/` is a host-internal prefix (not a
/// caller-nameable `bus.*` subject), so it never collides with the `ext/`-namespaced user subjects.
pub fn run_subject(job_id: &str) -> String {
    format!("run/{job_id}/events")
}
