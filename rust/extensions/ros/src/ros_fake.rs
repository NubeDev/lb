//! `RosFake` — the single test double for the one allowed external (testing-scope §0). It serves a
//! canned network → device → point tree and accepts priority-slot writes into an in-memory store,
//! standing in for a live ROS appliance behind the `RosApi` trait. Everything else in a test — the
//! store, bus, ingest, outbox, gateway — is the REAL thing; only the box is faked, and only here.
//!
//! It is deliberately small and inspectable: a test seeds a tree, drives the poller/handlers for real,
//! and asserts on `writes()` (what setpoints reached the "box") and on the values it returns. An
//! `unreachable` toggle lets a test exercise the poll-backoff / outbox-retry path (a box that is down)
//! without any network flakiness.
//!
//! `allow(dead_code)`: the seeders/inspectors are the fixture surface the slice-2+ tests drive; slice
//! 1 exercises only construction + the trait impl, so some helpers are not called yet.
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;

use crate::ros_api::{RosApi, RosApiError};
use crate::ros_client::{Device, Network, PingResponse, Point, Priority};

/// A recorded setpoint write — what a test asserts reached the box.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordedWrite {
    pub point_uuid: String,
    pub slot: u8,
    pub value: Option<f64>,
}

/// The canned box. Interior-mutable so a `&self` trait call can record writes / update present values
/// (the real API is also `&self` over an HTTP client). `unreachable` simulates a down box.
pub struct RosFake {
    networks: Vec<Network>,
    devices: Mutex<HashMap<String, Vec<Device>>>, // network_uuid -> devices
    points: Mutex<HashMap<String, Vec<Point>>>,   // device_uuid  -> points
    values: Mutex<HashMap<String, f64>>,          // point_uuid   -> present_value
    writes: Mutex<Vec<RecordedWrite>>,
    unreachable: Mutex<bool>,
}

impl RosFake {
    /// An empty box (no tree). Build a tree with the `with_*` seeders.
    pub fn empty() -> Self {
        Self {
            networks: Vec::new(),
            devices: Mutex::new(HashMap::new()),
            points: Mutex::new(HashMap::new()),
            values: Mutex::new(HashMap::new()),
            writes: Mutex::new(Vec::new()),
            unreachable: Mutex::new(false),
        }
    }

    /// A minimal one-network / one-device / one-point tree, all enabled, the point at `value`. The
    /// default fixture most tests start from; extend via the seeders for gating tests.
    pub fn seeded(point_uuid: &str, value: f64) -> Self {
        let mut fake = Self::empty();
        fake.add_network("net-1", "Network 1", true);
        fake.add_device("net-1", "dev-1", "Device 1", true);
        fake.add_point("dev-1", point_uuid, "Point 1", true, value);
        fake
    }

    pub fn add_network(&mut self, uuid: &str, name: &str, enable: bool) {
        self.networks.push(mk_network(uuid, name, enable));
    }

    pub fn add_device(&mut self, network_uuid: &str, uuid: &str, name: &str, enable: bool) {
        self.devices
            .lock()
            .unwrap()
            .entry(network_uuid.to_string())
            .or_default()
            .push(mk_device(uuid, name, network_uuid, enable));
    }

    pub fn add_point(
        &mut self,
        device_uuid: &str,
        uuid: &str,
        name: &str,
        enable: bool,
        value: f64,
    ) {
        self.points
            .lock()
            .unwrap()
            .entry(device_uuid.to_string())
            .or_default()
            .push(mk_point(uuid, name, device_uuid, enable, value));
        self.values.lock().unwrap().insert(uuid.to_string(), value);
    }

    /// Flip the box to unreachable/reachable — drives the poll-backoff / outbox-retry paths.
    pub fn set_unreachable(&self, down: bool) {
        *self.unreachable.lock().unwrap() = down;
    }

    /// Set a point's present value (a test simulating the physical value changing between ticks).
    pub fn set_value(&self, point_uuid: &str, value: f64) {
        self.values
            .lock()
            .unwrap()
            .insert(point_uuid.to_string(), value);
    }

    /// The setpoint writes that reached the box, in order — what a `point.write` test asserts on.
    pub fn writes(&self) -> Vec<RecordedWrite> {
        self.writes.lock().unwrap().clone()
    }

    fn guard(&self) -> Result<(), RosApiError> {
        if *self.unreachable.lock().unwrap() {
            Err(RosApiError::Unreachable("fake box is down".into()))
        } else {
            Ok(())
        }
    }
}

/// A `RosApiFactory` that hands out a single shared fake box for every connection — the test seam that
/// stands in for `RealFactory` (which would build a live client from base_url + token). A test seeds
/// the `Arc<RosFake>`, drives the handlers for real against the store/secrets/gateway, and asserts on
/// the fake's recorded writes / returned values. One fake for all connections is enough for the CRUD +
/// isolation tests; a per-uuid map can be added if a test needs two distinct trees.
pub struct FakeFactory {
    fake: std::sync::Arc<RosFake>,
}

impl FakeFactory {
    pub fn new(fake: std::sync::Arc<RosFake>) -> Self {
        Self { fake }
    }
}

#[async_trait]
impl crate::resolve::RosApiFactory for FakeFactory {
    async fn build(
        &self,
        _ros_uuid: &str,
        _base_url: &str,
        _token: &str,
    ) -> Result<Box<dyn RosApi>, crate::host::HostError> {
        Ok(Box::new(self.fake.clone()))
    }
}

/// Delegate the trait through an `Arc` so `FakeFactory` can return a shared box as a `Box<dyn RosApi>`.
#[async_trait]
impl RosApi for std::sync::Arc<RosFake> {
    async fn ping(&self) -> Result<PingResponse, RosApiError> {
        (**self).ping().await
    }
    async fn list_networks(&self, with_tree: bool) -> Result<Vec<Network>, RosApiError> {
        (**self).list_networks(with_tree).await
    }
    async fn list_devices(&self, network_uuid: &str) -> Result<Vec<Device>, RosApiError> {
        (**self).list_devices(network_uuid).await
    }
    async fn list_points(&self, device_uuid: &str) -> Result<Vec<Point>, RosApiError> {
        (**self).list_points(device_uuid).await
    }
    async fn get_point(&self, point_uuid: &str) -> Result<Point, RosApiError> {
        (**self).get_point(point_uuid).await
    }
    async fn write_point_slot(
        &self,
        point_uuid: &str,
        slot: u8,
        value: Option<f64>,
    ) -> Result<Point, RosApiError> {
        (**self).write_point_slot(point_uuid, slot, value).await
    }
}

#[async_trait]
impl RosApi for RosFake {
    async fn ping(&self) -> Result<PingResponse, RosApiError> {
        self.guard()?;
        Ok(mk_ping())
    }

    async fn list_networks(&self, with_tree: bool) -> Result<Vec<Network>, RosApiError> {
        self.guard()?;
        let mut networks = self.networks.clone();
        if with_tree {
            let devices = self.devices.lock().unwrap();
            let points = self.points.lock().unwrap();
            for net in &mut networks {
                let mut devs = devices.get(&net.uuid).cloned().unwrap_or_default();
                for dev in &mut devs {
                    dev.points = Some(points.get(&dev.uuid).cloned().unwrap_or_default());
                }
                net.devices = Some(devs);
            }
        }
        Ok(networks)
    }

    async fn list_devices(&self, network_uuid: &str) -> Result<Vec<Device>, RosApiError> {
        self.guard()?;
        Ok(self
            .devices
            .lock()
            .unwrap()
            .get(network_uuid)
            .cloned()
            .unwrap_or_default())
    }

    async fn list_points(&self, device_uuid: &str) -> Result<Vec<Point>, RosApiError> {
        self.guard()?;
        Ok(self
            .points
            .lock()
            .unwrap()
            .get(device_uuid)
            .cloned()
            .unwrap_or_default())
    }

    async fn get_point(&self, point_uuid: &str) -> Result<Point, RosApiError> {
        self.guard()?;
        let value = *self
            .values
            .lock()
            .unwrap()
            .get(point_uuid)
            .ok_or_else(|| RosApiError::NotFound(point_uuid.to_string()))?;
        Ok(mk_point(point_uuid, point_uuid, "", true, value))
    }

    async fn write_point_slot(
        &self,
        point_uuid: &str,
        slot: u8,
        value: Option<f64>,
    ) -> Result<Point, RosApiError> {
        self.guard()?;
        if !(1..=16).contains(&slot) {
            return Err(RosApiError::InvalidInput(format!(
                "priority slot out of range (1-16): {slot}"
            )));
        }
        self.writes.lock().unwrap().push(RecordedWrite {
            point_uuid: point_uuid.to_string(),
            slot,
            value,
        });
        // A written value becomes the present value on the next read (the box applies it), so a
        // poll after a write reflects the setpoint — the round-trip the UI test observes.
        if let Some(v) = value {
            self.values
                .lock()
                .unwrap()
                .insert(point_uuid.to_string(), v);
        }
        let present = self
            .values
            .lock()
            .unwrap()
            .get(point_uuid)
            .copied()
            .unwrap_or(0.0);
        Ok(mk_point(point_uuid, point_uuid, "", true, present))
    }
}

fn mk_network(uuid: &str, name: &str, enable: bool) -> Network {
    Network {
        uuid: uuid.into(),
        name: name.into(),
        enable: Some(enable),
        last_ok: None,
        last_fail: None,
        created_on: None,
        updated_on: None,
        last_write: None,
        thing_class: None,
        transport_type: None,
        plugin_uuid: None,
        plugin_name: None,
        network_interface: None,
        ip: None,
        port: None,
        network_mask: None,
        address_id: None,
        address_uuid: None,
        devices: None,
        points: None,
        has_polling_statistics: None,
        global_uuid: None,
        connection: None,
        connection_message: None,
        supports_device_ping: None,
        source_uuid: None,
        source_plugin_name: None,
        is_clone: None,
        host_uuid: None,
        history_enable: None,
        config: None,
        tags: None,
        meta_tags: None,
    }
}

fn mk_device(uuid: &str, name: &str, network_uuid: &str, enable: bool) -> Device {
    Device {
        uuid: uuid.into(),
        name: name.into(),
        enable: Some(enable),
        last_ok: None,
        last_fail: None,
        created_on: None,
        updated_on: None,
        last_write: None,
        thing_class: None,
        address_uuid: None,
        network_uuid: Some(network_uuid.into()),
        points: None,
        fast_poll_rate: None,
        normal_poll_rate: None,
        slow_poll_rate: None,
        connection: None,
        connection_message: None,
        source_uuid: None,
        history_enable: None,
        config: None,
        is_clone: None,
        disable_grouping: None,
        enable_concurrency: None,
        concurrency_limit: None,
        tags: None,
        meta_tags: None,
    }
}

fn mk_point(uuid: &str, name: &str, device_uuid: &str, enable: bool, value: f64) -> Point {
    Point {
        uuid: uuid.into(),
        name: name.into(),
        enable: Some(enable),
        created_on: None,
        updated_on: None,
        last_write: None,
        thing_class: None,
        last_ok: None,
        last_fail: None,
        present_value: Some(value),
        original_value: None,
        display_value: None,
        write_value: None,
        write_value_original: None,
        current_priority: None,
        is_output: None,
        is_type_bool: None,
        in_sync: None,
        fallback: None,
        device_uuid: Some(device_uuid.into()),
        writeable: Some(true),
        cov: None,
        data_type: None,
        address_id: None,
        address_length: None,
        decimal: None,
        multiplication_factor: None,
        scale_enable: None,
        scale_in_min: None,
        scale_in_max: None,
        scale_out_min: None,
        scale_out_max: None,
        offset: None,
        unit: None,
        poll_priority: None,
        poll_rate: None,
        history_enable: None,
        history_type: None,
        history_interval: None,
        history_cov_threshold: None,
        connection: None,
        connection_message: None,
        source_uuid: None,
        last_history_value: None,
        config: None,
        is_clone: None,
        address_uuid: None,
        io_number: None,
        object_type: None,
        point_source_uuid: None,
        host_uuid: None,
        priority: Some(Priority::default()),
        tags: None,
        meta_tags: None,
    }
}

fn mk_ping() -> PingResponse {
    PingResponse {
        health: "ok".into(),
        database: "ok".into(),
        ros: true,
        memory_usage_percentage: "10".into(),
        swap_usage_percentage: "0".into(),
        local_time: "".into(),
        utc_time: "".into(),
        device_info: None,
        apps_installed: None,
        vpn_status: None,
        error: None,
        message: None,
        status: Some("ok".into()),
    }
}
