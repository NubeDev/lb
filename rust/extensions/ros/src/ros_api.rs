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

use async_trait::async_trait;

use crate::ros_client::{Device, Network, PingResponse, Point};

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
    /// asks for the full tree in one call).
    async fn list_networks(&self, with_tree: bool) -> Result<Vec<Network>, RosApiError>;

    /// List devices under a network.
    async fn list_devices(&self, network_uuid: &str) -> Result<Vec<Device>, RosApiError>;

    /// List points under a device.
    async fn list_points(&self, device_uuid: &str) -> Result<Vec<Point>, RosApiError>;

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

    async fn list_devices(&self, network_uuid: &str) -> Result<Vec<Device>, RosApiError> {
        let params = crate::ros_client::GetDevicesParams {
            network_uuid: Some(network_uuid.to_string()),
            ..Default::default()
        };
        self.client
            .get_devices(Some(&params))
            .await
            .map_err(map_client_err)
    }

    async fn list_points(&self, device_uuid: &str) -> Result<Vec<Point>, RosApiError> {
        let params = crate::ros_client::GetPointsParams {
            device_uuid: Some(device_uuid.to_string()),
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
