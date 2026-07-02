//! The `network` model + the `/api/networks` list call. Model is verbatim from the vendored client;
//! the `get_networks` call is `async` (awaits `get_json`). A network carries the `enable` flag the
//! poll-gating ANDs up the tree.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use super::{client::Client, devices::Device, error::RosClientError, points::Point};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Network {
    pub uuid: String,
    pub name: String,
    pub enable: Option<bool>,
    pub last_ok: Option<String>,
    pub last_fail: Option<String>,
    pub created_on: Option<String>,
    pub updated_on: Option<String>,
    pub last_write: Option<String>,
    pub thing_class: Option<String>,
    pub transport_type: Option<String>,
    pub plugin_uuid: Option<String>,
    pub plugin_name: Option<String>,
    pub network_interface: Option<String>,
    pub ip: Option<String>,
    pub port: Option<i64>,
    pub network_mask: Option<String>,
    pub address_id: Option<String>,
    pub address_uuid: Option<String>,
    pub devices: Option<Vec<Device>>,
    pub points: Option<Vec<Point>>,
    pub has_polling_statistics: Option<bool>,
    pub global_uuid: Option<String>,
    pub connection: Option<String>,
    pub connection_message: Option<String>,
    pub supports_device_ping: Option<bool>,
    pub source_uuid: Option<String>,
    pub source_plugin_name: Option<String>,
    pub is_clone: Option<bool>,
    pub host_uuid: Option<String>,
    pub history_enable: Option<bool>,
    pub config: Option<Value>,
    pub tags: Option<Vec<String>>,
    pub meta_tags: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Default)]
pub struct GetNetworksParams {
    pub with_devices: Option<bool>,
    pub with_points: Option<bool>,
    pub offset: Option<i64>,
    pub limit: Option<i64>,
    pub name: Option<String>,
    pub host_uuid: Option<String>,
}

impl Client {
    pub async fn get_networks(
        &self,
        params: Option<&GetNetworksParams>,
    ) -> Result<Vec<Network>, RosClientError> {
        let mut query = Vec::new();

        if let Some(params) = params {
            if let Some(v) = params.with_devices {
                query.push(("with_devices", v.to_string()));
            }
            if let Some(v) = params.with_points {
                query.push(("with_points", v.to_string()));
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
            if let Some(v) = &params.host_uuid {
                if !v.is_empty() {
                    query.push(("host_uuid", v.clone()));
                }
            }
        }

        query.push(("show_clones", "false".to_string()));
        self.get_json("/api/networks", &query).await
    }
}
