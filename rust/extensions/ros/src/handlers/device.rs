//! The `device` verbs — `list`/`get`, proxied live from the box under a network. `list` takes
//! `{ros_uuid, network_uuid, host_uuid?}`; `get` adds `device_uuid`. The box is authority (no device
//! shadow). `host_uuid` (ros-location-group scope) is OPTIONAL but load-bearing in practice: the box
//! resolves `network_uuid` against its implicit default Host without it, so a network that belongs to
//! a non-default Host comes back with zero devices — confirmed live.

use serde_json::{json, Value};

use super::{page_args, req_str};
use crate::host::{HostCtx, HostError};
use crate::paging::keyset_page;
use crate::resolve::{resolve_api, RosApiFactory};

/// `device.list {ros_uuid, network_uuid}` — keyset-paged devices under a network.
pub async fn list(
    host: &HostCtx,
    factory: &dyn RosApiFactory,
    input: &Value,
) -> Result<Value, HostError> {
    host.require("ros.device.list")?;
    let ros_uuid = req_str(input, "ros_uuid")?;
    let network_uuid = req_str(input, "network_uuid")?;
    let host_uuid = input.get("host_uuid").and_then(|v| v.as_str());
    let (cursor, limit) = page_args(input);
    let api = match resolve_api(host, factory, &ros_uuid).await? {
        Some(api) => api,
        None => return Ok(json!({ "error": "not_found", "ros_uuid": ros_uuid })),
    };
    let devices = api
        .list_devices(host_uuid, &network_uuid)
        .await
        .map_err(|e| HostError::Callback(e.to_string()))?;
    let items: Vec<Value> = devices
        .iter()
        .map(|d| {
            json!({
                "uuid": d.uuid, "name": d.name, "enable": d.enable, "network_uuid": d.network_uuid,
            })
        })
        .collect();
    Ok(keyset_page(items, cursor.as_deref(), limit, |v| {
        v["uuid"].as_str().unwrap_or_default().to_string()
    }))
}

/// `device.get {ros_uuid, network_uuid, device_uuid}` — one device by uuid.
pub async fn get(
    host: &HostCtx,
    factory: &dyn RosApiFactory,
    input: &Value,
) -> Result<Value, HostError> {
    host.require("ros.device.get")?;
    let ros_uuid = req_str(input, "ros_uuid")?;
    let network_uuid = req_str(input, "network_uuid")?;
    let device_uuid = req_str(input, "device_uuid")?;
    let host_uuid = input.get("host_uuid").and_then(|v| v.as_str());
    let api = match resolve_api(host, factory, &ros_uuid).await? {
        Some(api) => api,
        None => return Ok(json!({ "error": "not_found", "ros_uuid": ros_uuid })),
    };
    let devices = api
        .list_devices(host_uuid, &network_uuid)
        .await
        .map_err(|e| HostError::Callback(e.to_string()))?;
    match devices.into_iter().find(|d| d.uuid == device_uuid) {
        Some(d) => Ok(json!({
            "uuid": d.uuid, "name": d.name, "enable": d.enable, "network_uuid": d.network_uuid,
        })),
        None => Ok(json!({ "error": "not_found", "device_uuid": device_uuid })),
    }
}
