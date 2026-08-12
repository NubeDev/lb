//! `ros.host.list` — the box's OWN Host tier, proxied live (ros-location-group scope). A Host is a
//! supervised remote device the box manages (its own `ip`/`port`/online state) — NOT the box
//! connection itself, which is what `ros_uuid` already names. There is no `/api/hosts?group_uuid=X`
//! list endpoint — Hosts only come embedded on one Group's own read (`GET /api/groups/{uuid}?
//! with_hosts=true`).

use serde_json::{json, Value};

use super::req_str;
use crate::host::{HostCtx, HostError};
use crate::resolve::{resolve_api, RosApiFactory};

/// `ros.host.list {ros_uuid, group_uuid}` — the Hosts under one Group.
pub async fn list(
    host: &HostCtx,
    factory: &dyn RosApiFactory,
    input: &Value,
) -> Result<Value, HostError> {
    host.require("ros.host.list")?;
    let ros_uuid = req_str(input, "ros_uuid")?;
    let group_uuid = req_str(input, "group_uuid")?;
    let api = match resolve_api(host, factory, &ros_uuid).await? {
        Some(api) => api,
        None => return Ok(json!({ "error": "not_found", "ros_uuid": ros_uuid })),
    };
    let group = api
        .get_group(&group_uuid, true)
        .await
        .map_err(|e| HostError::Callback(e.to_string()))?;
    let items: Vec<Value> = group
        .hosts
        .unwrap_or_default()
        .iter()
        .map(|h| json!({ "uuid": h.uuid, "name": h.name }))
        .collect();
    Ok(json!({ "items": items }))
}
