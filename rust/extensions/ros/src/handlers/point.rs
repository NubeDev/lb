//! The `point` verbs — `list`/`get`, proxied live from the box under a device. `list` takes
//! `{ros_uuid, device_uuid}`; `get` reads one point by uuid (its `present_value`, priority, …). The
//! box is authority (no point shadow). `point.write` (the setpoint) is slice 4, not here.

use serde_json::{json, Value};

use super::{page_args, req_str};
use crate::host::{HostCtx, HostError};
use crate::paging::keyset_page;
use crate::resolve::{resolve_api, RosApiFactory};

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
