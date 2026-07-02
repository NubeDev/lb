//! The `ros` **runnable** verbs — `start`/`stop`/`status` (resource-verbs Tier-2 grammar): they arm,
//! park, and inspect the poll task for one connection. `restart` is `stop` then `start` (below). This
//! is where the reusable poll engine (`poller/`) is wired to a live ROS connection: `start` resolves
//! the connection's `RosApi` + shadow enable, wraps them in a `RosSource`, builds an `IngestSink` over
//! the host callback, and spawns the loop into the process-lived `PollRegistry`.
//!
//! Every verb runs its own capability self-check first (`host.require`) — the inbound `native.call`
//! carries no caller identity, so the fine-grained `mcp:ros.start:call` gate is the sidecar's job
//! (see `host.rs`). A denial refuses BEFORE any task is armed or REST call is made.
//!
//! **Poll cadence:** seeded from the connection shadow's `poll_rate` (seconds, operator-overridable —
//! the resolved scope decision), defaulting when unset. The backoff cap is a multiple of that base.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

use super::req_str;
use crate::host::{HostCtx, HostError};
use crate::poller::ros_source::RosSource;
use crate::poller::run::{spawn_poll, PollRegistry};
use crate::poller::sink::IngestSink;
use crate::resolve::{resolve_api, RosApiFactory};

/// Default poll cadence when the shadow carries no `poll_rate` (seconds).
const DEFAULT_POLL_SECS: u64 = 5;
/// Backoff ceiling as a multiple of the base interval — a down box is retried no slower than this.
const BACKOFF_CAP_MULT: u32 = 12;

/// `ros.start {ros_uuid}` — arm the poll task for a connection. Resolves the connection (shadow +
/// token → `RosApi`), builds a `RosSource` (carrying the connection-level enable) + `IngestSink`, and
/// spawns the loop into the registry (idempotent: a re-start restarts cleanly).
pub async fn start(
    host: &HostCtx,
    factory: &dyn RosApiFactory,
    registry: &Arc<PollRegistry>,
    input: &Value,
) -> Result<Value, HostError> {
    host.require("ros.start")?;
    let ros_uuid = req_str(input, "ros_uuid")?;

    // The shadow gives base_url (via resolve) AND the connection-level enable + poll_rate. Absent
    // connection → not_found (no task armed).
    let shadow = match crate::shadow::get_ros(host, &ros_uuid).await? {
        Some(s) => s,
        None => return Ok(json!({ "error": "not_found", "ros_uuid": ros_uuid })),
    };
    let api = match resolve_api(host, factory, &ros_uuid).await? {
        Some(api) => api,
        None => return Ok(json!({ "error": "not_found", "ros_uuid": ros_uuid })),
    };

    let source = RosSource::new(api, host.ws(), &ros_uuid, shadow.enable);
    let sink = IngestSink::new(host.clone());
    let interval = Duration::from_secs(shadow.poll_rate.unwrap_or(DEFAULT_POLL_SECS).max(1));
    let max_backoff = interval * BACKOFF_CAP_MULT;

    let task = spawn_poll(Arc::new(source), Arc::new(sink), interval, max_backoff);
    registry.start(&ros_uuid, task);
    Ok(json!({ "ros_uuid": ros_uuid, "running": true }))
}

/// `ros.stop {ros_uuid}` — park the connection's poll task (abort the loop; state survives in
/// store/series/outbox, rule 4). `running:false` whether or not a task was armed (idempotent).
pub async fn stop(
    host: &HostCtx,
    registry: &Arc<PollRegistry>,
    input: &Value,
) -> Result<Value, HostError> {
    host.require("ros.stop")?;
    let ros_uuid = req_str(input, "ros_uuid")?;
    let was = registry.stop(&ros_uuid);
    Ok(json!({ "ros_uuid": ros_uuid, "running": false, "was_running": was }))
}

/// `ros.status {ros_uuid}` — the poll task's live counters (last-ok/last-fail, sample count,
/// consecutive failures). A connection with no armed task reports `running:false`.
pub async fn status(
    host: &HostCtx,
    registry: &Arc<PollRegistry>,
    input: &Value,
) -> Result<Value, HostError> {
    host.require("ros.status")?;
    let ros_uuid = req_str(input, "ros_uuid")?;
    match registry.status(&ros_uuid) {
        Some(st) => {
            let mut v =
                serde_json::to_value(&st).map_err(|e| HostError::BadResponse(e.to_string()))?;
            v["ros_uuid"] = json!(ros_uuid);
            Ok(v)
        }
        None => Ok(json!({ "ros_uuid": ros_uuid, "running": false })),
    }
}

/// `ros.restart {ros_uuid}` — stop then start (resource-verbs: restart = stop+start).
pub async fn restart(
    host: &HostCtx,
    factory: &dyn RosApiFactory,
    registry: &Arc<PollRegistry>,
    input: &Value,
) -> Result<Value, HostError> {
    host.require("ros.restart")?;
    let ros_uuid = req_str(input, "ros_uuid")?;
    registry.stop(&ros_uuid);
    // Re-arm through `start` (which self-checks `ros.start` too — defense in depth; the caller needs
    // both grants, matching the manifest).
    start(host, factory, registry, input)
        .await
        .map(|_| json!({ "ros_uuid": ros_uuid, "running": true, "restarted": true }))
}
