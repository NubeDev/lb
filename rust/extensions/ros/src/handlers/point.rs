//! The `point` verbs — `list`/`get` (proxied live from the box under a device) and `write` (the
//! must-deliver setpoint). `list` takes `{ros_uuid, device_uuid}`; `get` reads one point by uuid; the
//! box is authority (no point shadow).
//!
//! **`point.write` is must-deliver → outbox, never inline.** A dropped setpoint is a physical-world
//! safety bug, so `write` does NOT PATCH the box in-handler: it cap-checks, validates the slot, and
//! stages an **outbox effect** (`outbox.enqueue` via the callback) that the sidecar's relay loop
//! (`poller/relay.rs`) delivers through `RosTarget` with at-least-once retry. No REST write leaves the
//! node here — the deny test asserts exactly that. The effect id is stable
//! (`ros/{ros_uuid}/{point_uuid}/{slot}`) so re-writing the same slot upserts the same effect
//! (idempotent at the priority slot — the ROS priority-array model already is).

use serde_json::{json, Value};

use super::{page_args, req_str};
use crate::host::{HostCtx, HostError};
use crate::paging::keyset_page;
use crate::resolve::{resolve_api, RosApiFactory};

/// The outbox `target` string ROS effects carry — the sidecar relay's `outbox.due` filter matches it.
pub const ROS_TARGET: &str = "ros";
/// The outbox `action` for a setpoint write.
pub const WRITE_ACTION: &str = "point.write";

/// `point.list {ros_uuid, device_uuid}` — keyset-paged points under a device.
pub async fn list(
    host: &HostCtx,
    factory: &dyn RosApiFactory,
    input: &Value,
) -> Result<Value, HostError> {
    host.require("point.list")?;
    let ros_uuid = req_str(input, "ros_uuid")?;
    let device_uuid = req_str(input, "device_uuid")?;
    let (cursor, limit) = page_args(input);
    let api = match resolve_api(host, factory, &ros_uuid).await? {
        Some(api) => api,
        None => return Ok(json!({ "error": "not_found", "ros_uuid": ros_uuid })),
    };
    let points = api
        .list_points(&device_uuid)
        .await
        .map_err(|e| HostError::Callback(e.to_string()))?;
    let items: Vec<Value> = points
        .iter()
        .map(|p| {
            json!({
                "uuid": p.uuid, "name": p.name, "enable": p.enable,
                "present_value": p.present_value, "device_uuid": p.device_uuid,
            })
        })
        .collect();
    Ok(keyset_page(items, cursor.as_deref(), limit, |v| {
        v["uuid"].as_str().unwrap_or_default().to_string()
    }))
}

/// `point.get {ros_uuid, point_uuid}` — one point (its live present_value).
pub async fn get(
    host: &HostCtx,
    factory: &dyn RosApiFactory,
    input: &Value,
) -> Result<Value, HostError> {
    host.require("point.get")?;
    let ros_uuid = req_str(input, "ros_uuid")?;
    let point_uuid = req_str(input, "point_uuid")?;
    let api = match resolve_api(host, factory, &ros_uuid).await? {
        Some(api) => api,
        None => return Ok(json!({ "error": "not_found", "ros_uuid": ros_uuid })),
    };
    match api.get_point(&point_uuid).await {
        Ok(p) => Ok(json!({
            "uuid": p.uuid, "name": p.name, "enable": p.enable,
            "present_value": p.present_value, "device_uuid": p.device_uuid,
        })),
        Err(crate::ros_api::RosApiError::NotFound(_)) => {
            Ok(json!({ "error": "not_found", "point_uuid": point_uuid }))
        }
        Err(e) => Err(HostError::Callback(e.to_string())),
    }
}

/// The stable outbox effect id for a setpoint on `(ros_uuid, point_uuid, slot)`. Slot-scoped so two
/// writes to the SAME slot upsert the same effect (idempotent), while writes to different slots are
/// distinct pending effects. `/`-separated to match the cap-resource grammar (no `:` collisions).
pub fn write_effect_id(ros_uuid: &str, point_uuid: &str, slot: u8) -> String {
    format!("ros/{ros_uuid}/{point_uuid}/{slot}")
}

/// `point.write {ros_uuid, point_uuid, slot: 1..=16, value: number|null}` — stage a must-deliver
/// setpoint as an outbox effect. Cap-checks `mcp:point.write:call` FIRST, validates the slot, confirms
/// the connection exists, then `outbox.enqueue`s the effect. NO REST write happens here — the relay
/// delivers it. Returns the staged effect id so a caller/UI can watch it via `outbox.status`.
pub async fn write(
    host: &HostCtx,
    factory: &dyn RosApiFactory,
    input: &Value,
    ts: u64,
) -> Result<Value, HostError> {
    host.require("point.write")?;
    let ros_uuid = req_str(input, "ros_uuid")?;
    let point_uuid = req_str(input, "point_uuid")?;

    let slot = input
        .get("slot")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| HostError::BadResponse("missing integer arg: slot".into()))?;
    if !(1..=16).contains(&slot) {
        return Err(HostError::BadResponse(format!(
            "priority slot out of range (1-16): {slot}"
        )));
    }
    let slot = slot as u8;
    // `value` is optional: a JSON null (or omitted) releases the slot; a number sets it. Anything else
    // is a bad input (a string setpoint is a bug, not a release).
    let value: Option<f64> = match input.get("value") {
        None | Some(Value::Null) => None,
        Some(v) => Some(
            v.as_f64()
                .ok_or_else(|| HostError::BadResponse("value must be a number or null".into()))?,
        ),
    };

    // Confirm the connection exists before staging (a write to an unknown box is a not_found, not a
    // silently-pending effect). Resolving also proves the shadow+token are present.
    if resolve_api(host, factory, &ros_uuid).await?.is_none() {
        return Ok(json!({ "error": "not_found", "ros_uuid": ros_uuid }));
    }

    let id = write_effect_id(&ros_uuid, &point_uuid, slot);
    let payload = json!({
        "ros_uuid": ros_uuid, "point_uuid": point_uuid, "slot": slot, "value": value,
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
