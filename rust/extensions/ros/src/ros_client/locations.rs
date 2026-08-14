//! The `location`/`group`/`host` models + the box's own `/api/locations`/`/api/groups/{uuid}` calls
//! (ros-location-group scope) — the box's REAL Location → Group → Host hierarchy (confirmed against
//! the reference Grafana `Rubix OS Data Source` plugin's own REST client), reached through the SAME
//! `base_url`+token every other resource in this crate already uses. `/api/locations?with_groups=true`
//! embeds every Location's Groups in one call — there is no separate group-list endpoint.
//! `/api/groups/{uuid}?with_hosts=true` embeds one Group's Hosts — there is no separate host-list
//! endpoint either. A Host is a supervised remote device the box manages (its own `ip`/`port`/
//! `is_online`), not the box connection itself.

use serde::{Deserialize, Serialize};

use super::client::Client;
use super::error::RosClientError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub uuid: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub groups: Option<Vec<Group>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub uuid: String,
    pub name: String,
    pub location_uuid: String,
    pub description: Option<String>,
    #[serde(default)]
    pub hosts: Option<Vec<Host>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Host {
    pub uuid: String,
    pub name: String,
    pub group_uuid: String,
    pub enable: Option<bool>,
    pub is_online: Option<bool>,
}

impl Client {
    /// `GET /api/locations?with_groups={with_groups}` — every Location, each with its `groups`
    /// embedded when `with_groups` is true.
    pub async fn get_locations(&self, with_groups: bool) -> Result<Vec<Location>, RosClientError> {
        self.get_json(
            "/api/locations",
            &[("with_groups", with_groups.to_string())],
            None,
        )
        .await
    }

    /// `GET /api/groups/{uuid}?with_hosts={with_hosts}` — one Group, with its `hosts` embedded when
    /// `with_hosts` is true.
    pub async fn get_group(&self, uuid: &str, with_hosts: bool) -> Result<Group, RosClientError> {
        self.get_json(
            &format!("/api/groups/{uuid}"),
            &[("with_hosts", with_hosts.to_string())],
            None,
        )
        .await
    }
}
