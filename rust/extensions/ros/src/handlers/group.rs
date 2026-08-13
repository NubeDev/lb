//! `ros.group.list` — the box's OWN Group tier, proxied live (ros-location-group scope). There is no
//! `/api/groups?location_uuid=X` list endpoint on the box — Groups only come embedded on the Location
//! list response (`GET /api/locations?with_groups=true`). So this handler reads the SAME call
//! `ros.location.list` makes, finds the matching Location, and returns its embedded `groups`.

use serde_json::{json, Value};

use super::req_str;
use crate::host::{HostCtx, HostError};
use crate::resolve::{resolve_api, RosApiFactory};

/// `ros.group.list {ros_uuid, location_uuid}` — the Groups under one Location.
pub async fn list(
    host: &HostCtx,
    factory: &dyn RosApiFactory,
    input: &Value,
) -> Result<Value, HostError> {
    host.require("ros.group.list")?;
    let ros_uuid = req_str(input, "ros_uuid")?;
    let location_uuid = req_str(input, "location_uuid")?;
    let api = match resolve_api(host, factory, &ros_uuid).await? {
        Some(api) => api,
        None => return Ok(json!({ "error": "not_found", "ros_uuid": ros_uuid })),
    };
    let locations = api
        .list_locations(true)
        .await
        .map_err(|e| HostError::Callback(e.to_string()))?;
    let groups = locations
        .into_iter()
        .find(|l| l.uuid == location_uuid)
        .and_then(|l| l.groups)
        .unwrap_or_default();
    let items: Vec<Value> = groups
        .iter()
        .map(|g| json!({ "uuid": g.uuid, "name": g.name }))
        .collect();
    Ok(json!({ "items": items }))
}
