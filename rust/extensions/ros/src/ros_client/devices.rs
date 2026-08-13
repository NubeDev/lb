//! The `device` model + the `/api/devices` list call. Model verbatim from the vendored client; the
//! `get_devices` call is `async`. Carries `enable` (poll-gating) and the `*_poll_rate` fields the
//! platform-side poll interval is seeded from (ros-scope resolved decision).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use super::{client::Client, error::RosClientError, points::Point};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub uuid: String,
    pub name: String,
    pub enable: Option<bool>,
    pub last_ok: Option<String>,
    pub last_fail: Option<String>,
    pub created_on: Option<String>,
    pub updated_on: Option<String>,
    pub last_write: Option<String>,
    pub thing_class: Option<String>,
    pub address_uuid: Option<String>,
    pub network_uuid: Option<String>,
    pub points: Option<Vec<Point>>,
    pub fast_poll_rate: Option<i64>,
    pub normal_poll_rate: Option<i64>,
    pub slow_poll_rate: Option<i64>,
    pub connection: Option<String>,
    pub connection_message: Option<String>,
    pub source_uuid: Option<String>,
    pub history_enable: Option<bool>,
    pub config: Option<Value>,
    pub is_clone: Option<bool>,
    pub disable_grouping: Option<bool>,
    pub enable_concurrency: Option<bool>,
    pub concurrency_limit: Option<i64>,
    pub tags: Option<Vec<String>>,
    pub meta_tags: Option<HashMap<String, String>>,
}

impl Client {
    // No standalone devices-list endpoint — matches the reference client (`device_service.ts` has
    // no list method either): devices come nested off a network fetch (`get_networks` with
    // `with_devices: true`). This is a single fetch-by-id instead, `GET /api/devices/{uuid}
    // ?with_points={with_points}`, `host_uuid` as the `X-Host` header.
    pub async fn get_device_by_uuid(
        &self,
        uuid: &str,
        host_uuid: Option<&str>,
        with_points: bool,
    ) -> Result<Device, RosClientError> {
        let path = format!("/api/devices/{uuid}");
        let query = [("with_points", with_points.to_string())];
        self.get_json(&path, &query, host_uuid).await
    }
}
