//! The `schedule` model and read/write calls. Faithful vendoring of the box's REST surface, same as
//! `networks`/`points`. The nested weekly/exception/event payload (`schedule.schedule`) stays an
//! opaque `Value` — same call as `Network.config`/`Point.address_id` for nested-but-opaque shape —
//! since no slice on our side inspects it, only round-trips it for a write.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{client::Client, error::RosClientError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schedule {
    pub uuid: String,
    pub name: String,
    pub enable: Option<bool>,
    pub thing_class: Option<String>,
    pub thing_type: Option<String>,
    pub timezone: Option<String>,
    pub is_active: Option<bool>,
    pub active_weekly: Option<bool>,
    pub active_exception: Option<bool>,
    pub active_event: Option<bool>,
    pub enable_payload: Option<bool>,
    pub min_payload: Option<f64>,
    pub max_payload: Option<f64>,
    pub payload: Option<f64>,
    pub period_start_string: Option<String>,
    pub period_stop_string: Option<String>,
    pub next_start_string: Option<String>,
    pub next_stop_string: Option<String>,
    pub schedule: Option<Value>,
    pub created_on: Option<String>,
    pub updated_on: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteSchedulePayload {
    pub schedule: Value,
}

impl Client {
    pub async fn get_schedules(&self) -> Result<Vec<Schedule>, RosClientError> {
        self.get_json("/api/schedules", &[]).await
    }

    pub async fn get_schedule(&self, uuid: &str) -> Result<Schedule, RosClientError> {
        let path = format!("/api/schedules/{uuid}");
        self.get_json(&path, &[]).await
    }

    pub async fn write_schedule(
        &self,
        uuid: &str,
        schedule: Value,
    ) -> Result<Schedule, RosClientError> {
        let path = format!("/api/schedules/{uuid}/write");
        let payload = WriteSchedulePayload { schedule };
        self.patch_json(&path, &payload).await
    }
}
