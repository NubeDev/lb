//! `RosSource` — the **only** file where ROS vocabulary meets the reusable poll engine. It adapts a
//! `RosApi` (the ROS REST seam) into the driver-agnostic `Source` trait: it walks the box's
//! network → device → point tree, flattens it into `PollTarget`s carrying the four enable flags, mints
//! the series id, and reads a point's `present_value` per tick. Everything above it (`Poller`,
//! `gating`, `Sink`) never names ROS — swap this file for a `BacnetSource` and the engine is unchanged.
//!
//! **Series id (resolved scope decision):** `ros.{ws}.{ros}.{net}.{dev}.{point}` — workspace first (the
//! isolation prefix), then the connection, then each tree level down to the point. Two workspaces' or
//! two connections' identical point uuids never collide.
//!
//! **The connection-level enable** is NOT in the box tree (the box knows nothing of our shadow) — it is
//! the `ros` connection shadow's `enable`, passed in at construction. So `ros.update {enable:false}` on
//! the connection silences the whole box on the next tick's `targets()`, exactly like a network/device/
//! point toggle silences its branch — the four-level AND the gating resolver enforces.

use async_trait::async_trait;

use super::source::{PollTarget, Reading, Source, SourceError};
use crate::ros_api::{RosApi, RosApiError};

/// A `Source` over one ROS connection. Holds the API client, the identity needed to build series ids
/// (workspace + connection uuid), and the connection-level enable flag (from the shadow — the box
/// doesn't carry it).
pub struct RosSource {
    api: Box<dyn RosApi>,
    ws: String,
    ros_uuid: String,
    connection_enable: bool,
}

impl RosSource {
    pub fn new(
        api: Box<dyn RosApi>,
        ws: impl Into<String>,
        ros_uuid: impl Into<String>,
        connection_enable: bool,
    ) -> Self {
        Self {
            api,
            ws: ws.into(),
            ros_uuid: ros_uuid.into(),
            connection_enable,
        }
    }

    /// `ros.{ws}.{ros}.{net}.{dev}.{point}` — the resolved series id scheme.
    fn series_id(&self, net: &str, dev: &str, point: &str) -> String {
        format!(
            "ros.{}.{}.{}.{}.{}",
            self.ws, self.ros_uuid, net, dev, point
        )
    }
}

#[async_trait]
impl Source for RosSource {
    async fn targets(&self) -> Result<Vec<PollTarget>, SourceError> {
        // One tree fetch per tick (`with_tree` nests devices+points), so the target walk is a single
        // REST round-trip, not N+1 — the poll-storm mitigation the scope calls for.
        let networks = self.api.list_networks(true).await.map_err(map_err)?;
        let mut targets = Vec::new();
        for net in &networks {
            let net_enable = net.enable.unwrap_or(true);
            for dev in net.devices.as_deref().unwrap_or(&[]) {
                let dev_enable = dev.enable.unwrap_or(true);
                for point in dev.points.as_deref().unwrap_or(&[]) {
                    let point_enable = point.enable.unwrap_or(true);
                    targets.push(PollTarget {
                        id: point.uuid.clone(),
                        series: self.series_id(&net.uuid, &dev.uuid, &point.uuid),
                        connection_enable: self.connection_enable,
                        network_enable: net_enable,
                        device_enable: dev_enable,
                        point_enable,
                    });
                }
            }
        }
        Ok(targets)
    }

    async fn read(&self, target: &PollTarget, ts: u64) -> Result<Reading, SourceError> {
        let point = self.api.get_point(&target.id).await.map_err(map_err)?;
        // A point with no present_value this tick is a transient miss, not a fatal error — drop it
        // (per-target) so one un-read point doesn't abandon the whole batch.
        let value = point
            .present_value
            .ok_or_else(|| SourceError::NotFound(format!("{}: no present_value", target.id)))?;
        Ok(Reading {
            series: target.series.clone(),
            value: serde_json::json!(value),
            ts,
        })
    }
}

/// Map the ROS seam error onto the engine's `SourceError`: a down box is `Unreachable` (tick-level
/// backoff); a bad uuid is `NotFound` (per-target drop); anything else is `Other` (per-target drop).
fn map_err(e: RosApiError) -> SourceError {
    match e {
        RosApiError::Unreachable(m) => SourceError::Unreachable(m),
        RosApiError::NotFound(m) => SourceError::NotFound(m),
        RosApiError::Api { status, body } => SourceError::Other(format!("status {status}: {body}")),
        RosApiError::InvalidInput(m) => SourceError::Other(m),
    }
}
