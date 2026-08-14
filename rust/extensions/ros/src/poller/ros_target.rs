//! `RosTarget` — the sidecar's **delivery adapter** for `ros`-targeted outbox effects (the setpoint
//! writes `handlers/point.rs::write` stages). It is the ROS peer of the host's `Target` trait, but it
//! lives in the sidecar because delivery needs the `RosApi` box client (which the host does not have).
//! The sidecar relay loop (`poller/relay.rs`) pulls `outbox.due {target:"ros"}`, hands each effect
//! here, and marks the outcome (`mark_delivered`/`mark_failed`) back through the host callback.
//!
//! **Idempotent at the priority slot:** delivering the same effect twice writes the same slot to the
//! same value — a no-op on the box (the ROS priority-array model is idempotent), so the outbox's
//! at-least-once retry is safe. **Transient vs terminal:** a box-unreachable delivery returns
//! `DeliverOutcome::Retry` (the relay leaves the effect schedulable → next pass retries); a bad
//! payload / bad uuid returns `Fail` (the attempt is counted → eventually dead-lettered, not retried
//! forever against a request that can never succeed).

use serde::Deserialize;

use crate::host::HostCtx;
use crate::resolve::{resolve_api, RosApiFactory};
use crate::ros_api::RosApiError;

/// The decoded payload of a `point.write` effect (mirrors what `handlers::point::write` stages).
/// `host_uuid` (the write's `X-Host` scoping) is defaulted so effects staged before it existed
/// still decode and deliver.
#[derive(Debug, Deserialize)]
struct WritePayload {
    ros_uuid: String,
    #[serde(default)]
    host_uuid: Option<String>,
    point_uuid: String,
    slot: u8,
    value: Option<f64>,
}

/// The decoded payload of a `schedule.write` effect (mirrors what `handlers::schedule::write` stages).
#[derive(Debug, Deserialize)]
struct ScheduleWritePayload {
    ros_uuid: String,
    #[serde(default)]
    host_uuid: Option<String>,
    schedule_uuid: String,
    schedule: serde_json::Value,
}

/// What one delivery attempt yielded — the relay maps this onto `mark_delivered` / `mark_failed`.
#[derive(Debug, PartialEq)]
pub enum DeliverOutcome {
    /// The box acknowledged the write — mark the effect delivered (terminal).
    Delivered,
    /// A transient failure (box unreachable) — leave the effect schedulable; the relay retries next
    /// pass (at-least-once). Carries why, so a dead-lettered effect's `last_error` names the real
    /// cause instead of a generic default (debugging, `mcp/gauge-panel-loses-extension-busy-race.md`
    /// sibling finding: a swallowed reason makes "why did this never land" undiagnosable from the
    /// outbox record alone).
    Retry(String),
    /// A permanent failure (bad payload / bad uuid / box refusal) — count the attempt so a request
    /// that can never succeed is eventually dead-lettered rather than retried forever. Carries why.
    Fail(String),
}

/// Deliver one `ros` outbox effect: decode its payload (branching on `action` — `point.write` vs
/// `schedule.write`, the only two staged shapes), resolve the connection's `RosApi`, and write.
/// `payload` is the effect's opaque payload string (the JSON `handlers::point::write` /
/// `handlers::schedule::write` staged). Pure w.r.t. the relay — the loop owns the durable scan + the
/// mark calls.
pub async fn deliver(
    host: &HostCtx,
    factory: &dyn RosApiFactory,
    action: &str,
    payload: &str,
) -> DeliverOutcome {
    match action {
        "schedule.write" => deliver_schedule_write(host, factory, payload).await,
        // Default to the point-write shape (the original, still-most-common effect) so an unexpected
        // `action` string doesn't silently no-op — it fails fast on the payload mismatch instead.
        _ => deliver_point_write(host, factory, payload).await,
    }
}

async fn deliver_point_write(
    host: &HostCtx,
    factory: &dyn RosApiFactory,
    payload: &str,
) -> DeliverOutcome {
    let w: WritePayload = match serde_json::from_str(payload) {
        Ok(w) => w,
        // A malformed payload can never succeed — fail it (→ dead-letter), don't retry forever.
        Err(e) => return DeliverOutcome::Fail(format!("malformed point.write payload: {e}")),
    };

    let api = match resolve_api(host, factory, &w.ros_uuid).await {
        Ok(Some(api)) => api,
        // Connection gone (deleted) or its token/shadow unresolvable — permanent for this effect.
        Ok(None) => {
            return DeliverOutcome::Fail(format!(
                "ros connection {} not found or unresolvable",
                w.ros_uuid
            ))
        }
        // A host callback failure resolving the connection is transient — retry.
        Err(e) => return DeliverOutcome::Retry(format!("resolving ros connection: {e}")),
    };

    match api
        .write_point_slot(w.host_uuid.as_deref(), &w.point_uuid, w.slot, w.value)
        .await
    {
        Ok(_) => DeliverOutcome::Delivered,
        // The box is down — retry next pass (the setpoint must eventually land).
        Err(e @ RosApiError::Unreachable(_)) => DeliverOutcome::Retry(e.to_string()),
        // A bad uuid, out-of-range slot, or box refusal can't be fixed by retrying.
        Err(e) => DeliverOutcome::Fail(e.to_string()),
    }
}

async fn deliver_schedule_write(
    host: &HostCtx,
    factory: &dyn RosApiFactory,
    payload: &str,
) -> DeliverOutcome {
    let w: ScheduleWritePayload = match serde_json::from_str(payload) {
        Ok(w) => w,
        Err(e) => return DeliverOutcome::Fail(format!("malformed schedule.write payload: {e}")),
    };

    let api = match resolve_api(host, factory, &w.ros_uuid).await {
        Ok(Some(api)) => api,
        Ok(None) => {
            return DeliverOutcome::Fail(format!(
                "ros connection {} not found or unresolvable",
                w.ros_uuid
            ))
        }
        Err(e) => return DeliverOutcome::Retry(format!("resolving ros connection: {e}")),
    };

    match api
        .write_schedule(w.host_uuid.as_deref(), &w.schedule_uuid, w.schedule)
        .await
    {
        Ok(_) => DeliverOutcome::Delivered,
        Err(e @ RosApiError::Unreachable(_)) => DeliverOutcome::Retry(e.to_string()),
        Err(e) => DeliverOutcome::Fail(e.to_string()),
    }
}
