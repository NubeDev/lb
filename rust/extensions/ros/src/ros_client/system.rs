//! The `ping` health check + its response models (`/api/system/ping`). Models verbatim; `ping` is
//! `async`. This is the read-only health probe behind `ros.ping` — VPN/system control is out of scope
//! for this slice (ros-scope non-goals).

use serde::{Deserialize, Serialize};

use super::{client::Client, error::RosClientError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ROSInfo {
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub global_uuid: String,
    pub version: String,
    #[serde(rename = "type")]
    pub device_type: String,
    pub timezone: String,
    pub ros: Option<ROSInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledApp {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppsInstalled {
    pub ros_version: String,
    pub installed_count: i64,
    pub installed_apps: Vec<InstalledApp>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VPNStatus {
    pub active: bool,
    pub dead: bool,
    pub status_ok: bool,
    pub process_restarting: bool,
    pub could_not_determine_protocol: bool,
    pub cannot_resolve_host_address: bool,
    pub messages: Vec<String>,
    pub ip: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingResponse {
    pub health: String,
    pub database: String,
    pub ros: bool,
    pub memory_usage_percentage: String,
    pub swap_usage_percentage: String,
    pub local_time: String,
    pub utc_time: String,
    pub device_info: Option<DeviceInfo>,
    pub apps_installed: Option<AppsInstalled>,
    pub vpn_status: Option<VPNStatus>,
    #[serde(rename = "err")]
    pub error: Option<String>,
    pub message: Option<String>,
    pub status: Option<String>,
}

impl Client {
    pub async fn ping(&self) -> Result<PingResponse, RosClientError> {
        self.get_json("/api/system/ping", &[]).await
    }
}
