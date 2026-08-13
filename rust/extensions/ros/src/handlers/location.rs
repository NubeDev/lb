//! `ros.location.list` — the box's OWN Location tier, proxied live (ros-location-group scope). A
//! Location is real, pre-existing data on the ROS appliance itself (its own admin UI manages these) —
//! read-only here, same "box stays authority, never shadowed" posture `network.rs`/`device.rs`/
//! `point.rs` already have. `GET /api/locations?with_groups=true` returns every Location with its
//! Groups embedded — there is no separate group-list endpoint (see `ros.group.list`, which reads the
//! embedded field off this same call).

use serde_json::{json, Value};

use super::req_str;
use crate::host::{HostCtx, HostError};
use crate::resolve::{resolve_api, RosApiFactory};

/// `ros.location.list {ros_uuid}` — every Location on the box.
pub async fn list(
    host: &HostCtx,
    factory: &dyn RosApiFactory,
    input: &Value,
) -> Result<Value, HostError> {
    host.require("ros.location.list")?;
    let ros_uuid = req_str(input, "ros_uuid")?;
    let api = match resolve_api(host, factory, &ros_uuid).await? {
        Some(api) => api,
        None => return Ok(json!({ "error": "not_found", "ros_uuid": ros_uuid })),
    };
    let locations = api
        .list_locations(true)
        .await
        .map_err(|e| HostError::Callback(e.to_string()))?;
    let items: Vec<Value> = locations
        .iter()
        .map(|l| json!({ "uuid": l.uuid, "name": l.name }))
        .collect();
    Ok(json!({ "items": items }))
}
