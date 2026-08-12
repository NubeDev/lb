//! The `schedule` verbs — `list`/`get` (proxied live from the box, flat, no network/device nesting)
//! and `write` (must-deliver, same outbox-staged pattern as `point::write` — a schedule write is
//! exactly as consequential as a setpoint write, so it gets the same at-least-once delivery guarantee).

use serde_json::{json, Value};

use super::{page_args, req_str};
use crate::host::{HostCtx, HostError};
use crate::paging::keyset_page;
use crate::resolve::{resolve_api, RosApiFactory};

/// The outbox `target` string ROS effects carry — shared with `point::write`'s.
const ROS_TARGET: &str = "ros";
/// The outbox `action` for a schedule write.
const WRITE_ACTION: &str = "schedule.write";

/// `schedule.list {ros_uuid}` — keyset-paged schedules under a connection (flat).
pub async fn list(
    host: &HostCtx,
    factory: &dyn RosApiFactory,
    input: &Value,
) -> Result<Value, HostError> {
    host.require("ros.schedule.list")?;
    let ros_uuid = req_str(input, "ros_uuid")?;
    let (cursor, limit) = page_args(input);
    let api = match resolve_api(host, factory, &ros_uuid).await? {
        Some(api) => api,
        None => return Ok(json!({ "error": "not_found", "ros_uuid": ros_uuid })),
    };
    let schedules = api
        .list_schedules()
        .await
        .map_err(|e| HostError::Callback(e.to_string()))?;
    let items: Vec<Value> = schedules
        .iter()
        .map(|s| {
            json!({
                "uuid": s.uuid, "name": s.name, "enable": s.enable, "is_active": s.is_active,
            })
        })
        .collect();
    Ok(keyset_page(items, cursor.as_deref(), limit, |v| {
        v["uuid"].as_str().unwrap_or_default().to_string()
    }))
}

/// `schedule.get {ros_uuid, schedule_uuid}` — one schedule (its full weekly/exception/event payload).
pub async fn get(
    host: &HostCtx,
    factory: &dyn RosApiFactory,
    input: &Value,
) -> Result<Value, HostError> {
    host.require("ros.schedule.get")?;
    let ros_uuid = req_str(input, "ros_uuid")?;
    let schedule_uuid = req_str(input, "schedule_uuid")?;
    let api = match resolve_api(host, factory, &ros_uuid).await? {
        Some(api) => api,
        None => return Ok(json!({ "error": "not_found", "ros_uuid": ros_uuid })),
    };
    match api.get_schedule(&schedule_uuid).await {
        Ok(s) => Ok(json!({
            "uuid": s.uuid, "name": s.name, "enable": s.enable, "is_active": s.is_active,
            "schedule": s.schedule,
        })),
        Err(crate::ros_api::RosApiError::NotFound(_)) => {
            Ok(json!({ "error": "not_found", "schedule_uuid": schedule_uuid }))
        }
        Err(e) => Err(HostError::Callback(e.to_string())),
    }
}

/// The stable outbox effect id for a schedule write on `(ros_uuid, schedule_uuid)`. Schedule-scoped so
/// two writes to the SAME schedule upsert the same effect (idempotent), matching `point::write_effect_id`.
pub fn write_effect_id(ros_uuid: &str, schedule_uuid: &str) -> String {
    format!("ros/{ros_uuid}/schedule/{schedule_uuid}")
}

/// `schedule.write {ros_uuid, schedule_uuid, schedule: object}` — stage a must-deliver schedule payload
/// as an outbox effect. Cap-checks `ros.schedule.write` FIRST, confirms the connection exists, then
/// `outbox.enqueue`s the effect. NO REST write happens here — the relay delivers it (same contract as
/// `point::write`).
pub async fn write(
    host: &HostCtx,
    factory: &dyn RosApiFactory,
    input: &Value,
    ts: u64,
) -> Result<Value, HostError> {
    host.require("ros.schedule.write")?;
    let ros_uuid = req_str(input, "ros_uuid")?;
    let schedule_uuid = req_str(input, "schedule_uuid")?;
    let schedule = input
        .get("schedule")
        .cloned()
        .ok_or_else(|| HostError::BadResponse("missing object arg: schedule".into()))?;
    if !schedule.is_object() {
        return Err(HostError::BadResponse("schedule must be an object".into()));
    }

    // Confirm the connection exists before staging (a write to an unknown box is a not_found, not a
    // silently-pending effect).
    if resolve_api(host, factory, &ros_uuid).await?.is_none() {
        return Ok(json!({ "error": "not_found", "ros_uuid": ros_uuid }));
    }

    let id = write_effect_id(&ros_uuid, &schedule_uuid);
    let payload = json!({
        "ros_uuid": ros_uuid, "schedule_uuid": schedule_uuid, "schedule": schedule,
    });
    host.client()
        .call_tool(
            "outbox.enqueue",
            json!({
                "id": id,
                "target": ROS_TARGET,
                "action": WRITE_ACTION,
                "payload": payload.to_string(),
                "ts": ts,
            }),
        )
        .await?;
    Ok(json!({ "effect_id": id, "status": "pending" }))
}
