//! The vendored `rust-ros` REST client, ported to **async `reqwest`** and with its `sqlx`/Postgres
//! dependency dropped (the ROS box owns its own DB; we speak REST only — ros-scope non-goals). This is
//! the raw HTTP surface for a ROS (Rubix) appliance: the connection/network/device/point tree, point
//! priority writes, and a `ping` health check. It is wrapped one level up by the `RosApi` trait
//! (`src/ros_api.rs`) — the ONE external-fake seam (testing-scope §0) — so nothing above this file
//! knows it is talking HTTP vs a canned fake box.
//!
//! `allow(dead_code)`: this is a faithful, complete vendoring of the box's REST surface. Some models
//! and calls (users, histories, some params) are not on the driver's slice-1 critical path but are
//! kept so the client stays a true copy — the CRUD/poller/write slices wire the rest.
#![allow(dead_code, unused_imports)]

mod client;
mod devices;
mod error;
mod histories;
mod networks;
mod points;
mod system;
mod users;

pub use client::{Client, Config};
pub use devices::{Device, GetDevicesParams};
pub use error::RosClientError;
pub use histories::{GetHistoriesParams, HistoryRecord};
pub use networks::{GetNetworksParams, Network};
pub use points::{
    GetPointsParams, MetaTag, Point, Priority, Tag, UpdatePointPayload, WritePointPriorityPayload,
};
pub use system::{AppsInstalled, DeviceInfo, InstalledApp, PingResponse, ROSInfo, VPNStatus};
pub use users::User;
