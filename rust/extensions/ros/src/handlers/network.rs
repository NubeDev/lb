//! The `network` verbs — `list`/`get`, proxied live from the ROS box (the box is authority for the
//! tree; scope non-goal: no network/device/point shadow). A caller supplies the `ros_uuid` (which
//! connection) plus, for `get`, the `network_uuid`. Create/update/delete land in a later slice
//! (write-back to the box); this slice ships the reads the UI drill-down needs.

use serde_json::{json, Value};

use super::{page_args, req_str};
use crate::host::{HostCtx, HostError};
use crate::paging::keyset_page;
use crate::resolve::{resolve_api, RosApiFactory};

/// `network.list {ros_uuid}` — keyset-paged networks under a connection.
pub async fn list(
    host: &HostCtx,
    factory: &dyn RosApiFactory,
    input: &Value,
) -> Result<Value, HostError> {
    host.require("network.list")?;
    let ros_uuid = req_str(input, "ros_uuid")?;
    let (cursor, limit) = page_args(input);
    let api = match resolve_api(host, factory, &ros_uuid).await? {
        Some(api) => api,
        None => return Ok(json!({ "error": "not_found", "ros_uuid": ros_uuid })),
    };
    let networks = api
        .list_networks(false)
        .await
        .map_err(|e| HostError::Callback(e.to_string()))?;
    let items: Vec<Value> = networks
        .iter()
        .map(|n| {
            json!({
                "uuid": n.uuid, "name": n.name, "enable": n.enable,
            })
        })
        .collect();
    Ok(keyset_page(items, cursor.as_deref(), limit, |v| {
        v["uuid"].as_str().unwrap_or_default().to_string()
    }))
}

/// `network.get {ros_uuid, network_uuid}` — one network (from the box's list, matched by uuid).
pub async fn get(
    host: &HostCtx,
    factory: &dyn RosApiFactory,
    input: &Value,
) -> Result<Value, HostError> {
    host.require("network.get")?;
    let ros_uuid = req_str(input, "ros_uuid")?;
    let network_uuid = req_str(input, "network_uuid")?;
    let api = match resolve_api(host, factory, &ros_uuid).await? {
        Some(api) => api,
        None => return Ok(json!({ "error": "not_found", "ros_uuid": ros_uuid })),
    };
    let networks = api
        .list_networks(false)
        .await
        .map_err(|e| HostError::Callback(e.to_string()))?;
    match networks.into_iter().find(|n| n.uuid == network_uuid) {
        Some(n) => Ok(json!({ "uuid": n.uuid, "name": n.name, "enable": n.enable })),
        None => Ok(json!({ "error": "not_found", "network_uuid": network_uuid })),
    }
}
