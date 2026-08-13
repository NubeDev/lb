//! `RosApi` — the ONE external-fake seam (testing-scope §0). A live ROS appliance is a true external
//! we cannot run in CI, so ALL access to it goes behind this single trait, in this single file, with
//! exactly two impls: the real `rust-ros`-backed [`RealRosApi`] and the canned [`RosFake`]
//! (`src/ros_fake.rs`). Everything above — the MCP handlers, the poller — is written against this
//! trait and is exercised *for real* against the store/bus/ingest/outbox/gateway with only the ROS box
//! faked. No `*.fake.ts`, no re-implemented host behavior (CLAUDE.md rule 9).
//!
//! The trait speaks the ROS tree (network → device → point), the point present-value read the poller
//! needs, and the priority-slot write the outbox effect delivers. It is deliberately ROS-shaped (not
//! a generic "driver" trait): the poller's *reusable* seam is `poller::Source`, and `RosSource`
//! adapts a `RosApi` to it. That keeps the ROS vocabulary here and out of the reusable engine.

// `allow(dead_code)`: the trait + real impl land in slice 1; the CRUD handlers (slice 2) and the
// poller (slice 3) are their first callers. Complete-ahead-of-use, not unreachable.
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde_json::Value;

use crate::ros_client::{Device, Group, Location, Network, PingResponse, Point, Schedule};

/// The typed error surface the handlers map onto MCP/tool errors. `Denied`/`NotFound`/`Unreachable`
/// are distinct so a handler can react (a box-unreachable poll backs off; a bad uuid is a 404, not a
/// retry). Never carries the `External` token.
#[derive(Debug, thiserror::Error)]
pub enum RosApiError {
    #[error("ros box unreachable: {0}")]
    Unreachable(String),
    #[error("ros resource not found: {0}")]
    NotFound(String),
    #[error("ros box refused: status {status}: {body}")]
    Api { status: u16, body: String },
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

/// The seam every ROS REST interaction crosses. One connection's worth of API — a `RosApi` is bound
/// to a single appliance (`base_url` + token), constructed per connection from the config record +
/// the `lb-secrets`-held token.
#[async_trait]
pub trait RosApi: Send + Sync {
    /// Health-check the appliance (`ros.ping`).
    async fn ping(&self) -> Result<PingResponse, RosApiError>;

    /// List networks on the box (optionally with their devices/points nested — the poll-target walk
    /// asks for the full tree in one call). UNSCOPED — every network across every supervised Host.
    /// The poller's own tree-walk uses this (host-scoping the poll target set is a separate, later
    /// concern); the panel query-picker uses [`list_networks_for_host`] instead (ros-location-group
    /// scope), since an operator picks a specific Host to author against.
    async fn list_networks(&self, with_tree: bool) -> Result<Vec<Network>, RosApiError>;

    /// List networks under ONE supervised Host (ros-location-group scope) — the box's own
    /// `host_uuid`-scoped network listing, what the panel query-picker's Network step needs once a
    /// Host is chosen via Location → Group → Host.
    async fn list_networks_for_host(
        &self,
        host_uuid: &str,
        with_tree: bool,
    ) -> Result<Vec<Network>, RosApiError>;

    /// List every Location on the box, each with its Groups embedded (ros-location-group scope) —
    /// `GET /api/locations?with_groups=true`. The box's own top-level organizational tier; a
    /// connection reaches many Locations, each with Groups, each with supervised Hosts.
    async fn list_locations(&self, with_groups: bool) -> Result<Vec<Location>, RosApiError>;

    /// Read one Group, with its Hosts embedded when `with_hosts` (ros-location-group scope) —
    /// `GET /api/groups/{uuid}?with_hosts=true`. There is no separate host-list endpoint; a Group's
    /// Hosts are only reachable this way.
    async fn get_group(&self, group_uuid: &str, with_hosts: bool) -> Result<Group, RosApiError>;

    /// List devices under a network. `host_uuid` (ros-location-group scope) is OPTIONAL but the box
    /// needs it in practice: `/api/devices?network_uuid=X` alone resolves against the box's implicit
    /// default Host, so a `network_uuid` that belongs to a DIFFERENT (non-default) Host comes back
    /// empty without it — confirmed live (a real, non-empty network's devices only appeared once its
    /// own Host's uuid was passed). Absent falls back to the unscoped call, same posture as
    /// `list_networks_for_host`'s network-level fix.
    async fn list_devices(
        &self,
        host_uuid: Option<&str>,
        network_uuid: &str,
    ) -> Result<Vec<Device>, RosApiError>;

    /// List points under a device — same optional `host_uuid` scoping as `list_devices`, for the same
    /// reason (`GetPointsParams.host_uuid` already existed in the vendored client, unused until now).
    async fn list_points(
        &self,
        host_uuid: Option<&str>,
        device_uuid: &str,
    ) -> Result<Vec<Point>, RosApiError>;

    /// Read one point (its `present_value`, priority, …) — the per-tick poll read.
    async fn get_point(&self, point_uuid: &str) -> Result<Point, RosApiError>;

    /// Write a priority slot on a point (the setpoint the outbox delivers). Reads the current
    /// priority, sets `slot` to `value` (None releases), and PATCHes it back — idempotent at the slot.
    async fn write_point_slot(
        &self,
        point_uuid: &str,
        slot: u8,
        value: Option<f64>,
    ) -> Result<Point, RosApiError>;

    /// List schedules on the box (flat, no network/device nesting).
    async fn list_schedules(&self) -> Result<Vec<Schedule>, RosApiError>;

    /// Read one schedule.
    async fn get_schedule(&self, schedule_uuid: &str) -> Result<Schedule, RosApiError>;

    /// Write a schedule's weekly/exception/event payload back (must-deliver, same as a point write).
    async fn write_schedule(
        &self,
        schedule_uuid: &str,
        schedule: Value,
    ) -> Result<Schedule, RosApiError>;
}

/// How long a fetched Locations+Groups tree stays fresh (see `list_locations`'s dedup cache). Short
/// enough that a box-side edit shows up within one dashboard refresh cycle; long enough to collapse
/// the `location.list` + `group.list` pair a picker chain fires within the same interaction.
const LOCATIONS_CACHE_TTL: Duration = Duration::from_secs(10);

fn locations_cache() -> &'static Mutex<HashMap<String, (Instant, Vec<Location>)>> {
    static CACHE: OnceLock<Mutex<HashMap<String, (Instant, Vec<Location>)>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn locations_cache_get(base_url: &str) -> Option<Vec<Location>> {
    let cache = locations_cache().lock().expect("locations cache poisoned");
    let (fetched_at, locations) = cache.get(base_url)?;
    (fetched_at.elapsed() < LOCATIONS_CACHE_TTL).then(|| locations.clone())
}

fn locations_cache_put(base_url: &str, locations: Vec<Location>) {
    let mut cache = locations_cache().lock().expect("locations cache poisoned");
    cache.insert(base_url.to_string(), (Instant::now(), locations));
}

/// The real, `rust-ros`-backed impl. One `Client` (async `reqwest`) per connection. Nothing here is
/// mocked — against a live box this is the genuine REST path; in tests `RosFake` stands in its place.
pub struct RealRosApi {
    client: crate::ros_client::Client,
}

impl RealRosApi {
    pub fn new(base_url: impl Into<String>, token: impl Into<String>) -> Result<Self, RosApiError> {
        let client = crate::ros_client::Client::new(crate::ros_client::Config {
            base_url: base_url.into(),
            token: token.into(),
        })
        .map_err(map_client_err)?;
        Ok(Self { client })
    }
}

#[async_trait]
impl RosApi for RealRosApi {
    async fn ping(&self) -> Result<PingResponse, RosApiError> {
        self.client.ping().await.map_err(map_client_err)
    }

    async fn list_networks(&self, with_tree: bool) -> Result<Vec<Network>, RosApiError> {
        let params = crate::ros_client::GetNetworksParams {
            with_devices: Some(with_tree),
            with_points: Some(with_tree),
            ..Default::default()
        };
        self.client
            .get_networks(Some(&params))
            .await
            .map_err(map_client_err)
    }

    async fn list_networks_for_host(
        &self,
        host_uuid: &str,
        with_tree: bool,
    ) -> Result<Vec<Network>, RosApiError> {
        let params = crate::ros_client::GetNetworksParams {
            with_devices: Some(with_tree),
            with_points: Some(with_tree),
            host_uuid: Some(host_uuid.to_string()),
            ..Default::default()
        };
        self.client
            .get_networks(Some(&params))
            .await
            .map_err(map_client_err)
    }

    async fn list_locations(&self, with_groups: bool) -> Result<Vec<Location>, RosApiError> {
        // Short TTL cache, keyed by base_url (ros-location-group scope): `ros.location.list` and
        // `ros.group.list` both need this SAME `/api/locations?with_groups=true` tree (there is no
        // separate group-list endpoint — see `handlers/group.rs`), and a `resolve_api()` call rebuilds
        // a fresh `RealRosApi` on every single tool call, so nothing upstream already dedupes this. A
        // Location→Group→Host chain resolve was paying for the full box round trip twice. Locations
        // are box-admin-managed, pre-existing site config — not something a dashboard author edits
        // through this panel — so a few seconds of staleness is the right trade against halving the
        // round trips for the two heaviest tool calls in the chain. `with_groups=false` is unused by
        // any caller today (grep confirms only `true`), so caching only that shape is deliberately not
        // a generality claim — it is what every real caller needs.
        if with_groups {
            if let Some(hit) = locations_cache_get(self.client.base_url()) {
                return Ok(hit);
            }
        }
        let locations = self
            .client
            .get_locations(with_groups)
            .await
            .map_err(map_client_err)?;
        if with_groups {
            locations_cache_put(self.client.base_url(), locations.clone());
        }
        Ok(locations)
    }

    async fn get_group(&self, group_uuid: &str, with_hosts: bool) -> Result<Group, RosApiError> {
        self.client
            .get_group(group_uuid, with_hosts)
            .await
            .map_err(map_client_err)
    }

    async fn list_devices(
        &self,
        host_uuid: Option<&str>,
        network_uuid: &str,
    ) -> Result<Vec<Device>, RosApiError> {
        let params = crate::ros_client::GetDevicesParams {
            network_uuid: Some(network_uuid.to_string()),
            host_uuid: host_uuid.map(|h| h.to_string()),
            ..Default::default()
        };
        self.client
            .get_devices(Some(&params))
            .await
            .map_err(map_client_err)
    }

    async fn list_points(
        &self,
        host_uuid: Option<&str>,
        device_uuid: &str,
    ) -> Result<Vec<Point>, RosApiError> {
        let params = crate::ros_client::GetPointsParams {
            device_uuid: Some(device_uuid.to_string()),
            host_uuid: host_uuid.map(|h| h.to_string()),
            with_priority: Some(true),
            ..Default::default()
        };
        self.client
            .get_points(Some(&params))
            .await
            .map_err(map_client_err)
    }

    async fn get_point(&self, point_uuid: &str) -> Result<Point, RosApiError> {
        let params = crate::ros_client::GetPointsParams {
            with_priority: Some(true),
            ..Default::default()
        };
        self.client
            .get_point_by_uuid(point_uuid, Some(&params))
            .await
            .map_err(map_client_err)
    }

    async fn write_point_slot(
        &self,
        point_uuid: &str,
        slot: u8,
        value: Option<f64>,
    ) -> Result<Point, RosApiError> {
        let mut priority = self
            .client
            .get_point_priority(point_uuid)
            .await
            .map_err(map_client_err)?
            .unwrap_or_default();
        priority.set_slot(slot, value).map_err(map_client_err)?;
        self.client
            .write_point_priority(point_uuid, &priority)
            .await
            .map_err(map_client_err)
    }

    async fn list_schedules(&self) -> Result<Vec<Schedule>, RosApiError> {
        self.client.get_schedules().await.map_err(map_client_err)
    }

    async fn get_schedule(&self, schedule_uuid: &str) -> Result<Schedule, RosApiError> {
        self.client
            .get_schedule(schedule_uuid)
            .await
            .map_err(map_client_err)
    }

    async fn write_schedule(
        &self,
        schedule_uuid: &str,
        schedule: Value,
    ) -> Result<Schedule, RosApiError> {
        self.client
            .write_schedule(schedule_uuid, schedule)
            .await
            .map_err(map_client_err)
    }
}

/// Map the low-level client error onto the seam's typed error. A `404` becomes `NotFound`; a
/// transport failure becomes `Unreachable` (the poll-backoff signal); other statuses stay `Api`.
fn map_client_err(e: crate::ros_client::RosClientError) -> RosApiError {
    use crate::ros_client::RosClientError as E;
    match e {
        E::Http(err) => RosApiError::Unreachable(err.to_string()),
        E::Api { status: 404, body } => RosApiError::NotFound(body),
        E::Api { status, body } => RosApiError::Api { status, body },
        E::InvalidInput(m) => RosApiError::InvalidInput(m),
    }
}
