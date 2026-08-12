//! The `network` verbs — `list`/`get`, proxied live from the ROS box (the box is authority for the
//! tree; scope non-goal: no network/device/point shadow). A caller supplies `ros_uuid` (which
//! connection) and, for `get`, `network_uuid`. `host_uuid` (which supervised Host, ros-location-group
//! scope — the box scopes network listing per-Host) is OPTIONAL: absent falls back to the unscoped
//! list (today's exact behavior, what the dashboard widgets below still call without it); given, only
//! that Host's networks come back — what the panel query-picker's Location→Group→Host chain now
//! supplies. Create/update/delete land in a later slice (write-back to the box); this slice ships the
//! reads the UI drill-down needs.

use serde_json::{json, Value};

use super::{page_args, req_str};
use crate::host::{HostCtx, HostError};
use crate::paging::keyset_page;
use crate::resolve::{resolve_api, RosApiFactory};

/// Resolve networks for `input`: `host_uuid`-scoped if given, unscoped otherwise (see file doc).
async fn resolve_networks(
    api: &dyn crate::ros_api::RosApi,
    input: &Value,
) -> Result<Vec<crate::ros_client::Network>, HostError> {
    match input.get("host_uuid").and_then(|v| v.as_str()) {
        Some(host_uuid) if !host_uuid.is_empty() => api
            .list_networks_for_host(host_uuid, false)
            .await
            .map_err(|e| HostError::Callback(e.to_string())),
        _ => api
            .list_networks(false)
            .await
            .map_err(|e| HostError::Callback(e.to_string())),
    }
}

/// `network.list {ros_uuid, host_uuid?}` — keyset-paged networks, scoped to one supervised Host when
/// `host_uuid` is given.
pub async fn list(
    host: &HostCtx,
    factory: &dyn RosApiFactory,
    input: &Value,
) -> Result<Value, HostError> {
    host.require("ros.network.list")?;
    let ros_uuid = req_str(input, "ros_uuid")?;
    let (cursor, limit) = page_args(input);
    let api = match resolve_api(host, factory, &ros_uuid).await? {
        Some(api) => api,
        None => return Ok(json!({ "error": "not_found", "ros_uuid": ros_uuid })),
    };
    let networks = resolve_networks(api.as_ref(), input).await?;
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

/// `network.get {ros_uuid, host_uuid?, network_uuid}` — one network (from the resolved list, matched
/// by uuid).
pub async fn get(
    host: &HostCtx,
    factory: &dyn RosApiFactory,
    input: &Value,
) -> Result<Value, HostError> {
    host.require("ros.network.get")?;
    let ros_uuid = req_str(input, "ros_uuid")?;
    let network_uuid = req_str(input, "network_uuid")?;
    let api = match resolve_api(host, factory, &ros_uuid).await? {
        Some(api) => api,
        None => return Ok(json!({ "error": "not_found", "ros_uuid": ros_uuid })),
    };
    let networks = resolve_networks(api.as_ref(), input).await?;
    match networks.into_iter().find(|n| n.uuid == network_uuid) {
        Some(n) => Ok(json!({ "uuid": n.uuid, "name": n.name, "enable": n.enable })),
        None => Ok(json!({ "error": "not_found", "network_uuid": network_uuid })),
    }
}
