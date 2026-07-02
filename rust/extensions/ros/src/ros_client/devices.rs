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

#[derive(Debug, Clone, Default)]
pub struct GetDevicesParams {
    pub with_points: Option<bool>,
    pub with_networks: Option<bool>,
    pub offset: Option<i64>,
    pub limit: Option<i64>,
    pub name: Option<String>,
    pub network_uuid: Option<String>,
}

impl Client {
    pub async fn get_devices(
        &self,
        params: Option<&GetDevicesParams>,
    ) -> Result<Vec<Device>, RosClientError> {
        let mut query = Vec::new();

        if let Some(params) = params {
            if let Some(v) = params.with_points {
                query.push(("with_points", v.to_string()));
            }
            if let Some(v) = params.with_networks {
                query.push(("with_networks", v.to_string()));
            }
            if let Some(v) = params.offset {
                query.push(("offset", v.to_string()));
            }
            if let Some(v) = params.limit {
                query.push(("limit", v.to_string()));
            }
            if let Some(v) = &params.name {
                if !v.is_empty() {
                    query.push(("name", v.clone()));
                }
            }
            if let Some(v) = &params.network_uuid {
                if !v.is_empty() {
                    query.push(("network_uuid", v.clone()));
                }
            }
        }

        query.push(("show_clones", "false".to_string()));
        self.get_json("/api/devices", &query).await
    }
}
