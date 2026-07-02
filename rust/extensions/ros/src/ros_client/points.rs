//! The `point` model, the 16-slot `Priority` array, and the point read/update/write calls. Models are
//! verbatim from the vendored client; every `impl Client` call is now `async`. `present_value` is what
//! the poller reads each tick; `write_point_priority` (PATCH `/api/points/{uuid}/write`) is the
//! setpoint the outbox effect delivers. The priority array is idempotent at the slot level (writing
//! slot 8 twice == once) — the property the must-deliver outbox relies on (ros-scope setpoint safety).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{client::Client, error::RosClientError};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Priority {
    pub point_uuid: Option<String>,
    #[serde(rename = "_1")]
    pub p1: Option<f64>,
    #[serde(rename = "_2")]
    pub p2: Option<f64>,
    #[serde(rename = "_3")]
    pub p3: Option<f64>,
    #[serde(rename = "_4")]
    pub p4: Option<f64>,
    #[serde(rename = "_5")]
    pub p5: Option<f64>,
    #[serde(rename = "_6")]
    pub p6: Option<f64>,
    #[serde(rename = "_7")]
    pub p7: Option<f64>,
    #[serde(rename = "_8")]
    pub p8: Option<f64>,
    #[serde(rename = "_9")]
    pub p9: Option<f64>,
    #[serde(rename = "_10")]
    pub p10: Option<f64>,
    #[serde(rename = "_11")]
    pub p11: Option<f64>,
    #[serde(rename = "_12")]
    pub p12: Option<f64>,
    #[serde(rename = "_13")]
    pub p13: Option<f64>,
    #[serde(rename = "_14")]
    pub p14: Option<f64>,
    #[serde(rename = "_15")]
    pub p15: Option<f64>,
    #[serde(rename = "_16")]
    pub p16: Option<f64>,
}

impl Priority {
    pub fn highest_priority_value(&self) -> Option<f64> {
        self.p1
            .or(self.p2)
            .or(self.p3)
            .or(self.p4)
            .or(self.p5)
            .or(self.p6)
            .or(self.p7)
            .or(self.p8)
            .or(self.p9)
            .or(self.p10)
            .or(self.p11)
            .or(self.p12)
            .or(self.p13)
            .or(self.p14)
            .or(self.p15)
            .or(self.p16)
    }

    /// Set slot `n` (1-16) to `value` (`None` clears/releases it). Returns an error for an
    /// out-of-range slot so a bad `point.write` is rejected before any REST call. The one place the
    /// `{slot, value|null}` write ergonomics (ros-scope resolved decision) maps onto the array.
    pub fn set_slot(&mut self, slot: u8, value: Option<f64>) -> Result<(), RosClientError> {
        match slot {
            1 => self.p1 = value,
            2 => self.p2 = value,
            3 => self.p3 = value,
            4 => self.p4 = value,
            5 => self.p5 = value,
            6 => self.p6 = value,
            7 => self.p7 = value,
            8 => self.p8 = value,
            9 => self.p9 = value,
            10 => self.p10 = value,
            11 => self.p11 = value,
            12 => self.p12 = value,
            13 => self.p13 = value,
            14 => self.p14 = value,
            15 => self.p15 = value,
            16 => self.p16 = value,
            _ => {
                return Err(RosClientError::InvalidInput(format!(
                    "priority slot out of range (1-16): {slot}"
                )))
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub tag: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaTag {
    pub point_uuid: String,
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Point {
    pub uuid: String,
    pub name: String,
    pub enable: Option<bool>,
    pub created_on: Option<String>,
    pub updated_on: Option<String>,
    pub last_write: Option<String>,
    pub thing_class: Option<String>,
    pub last_ok: Option<String>,
    pub last_fail: Option<String>,
    pub present_value: Option<f64>,
    pub original_value: Option<f64>,
    pub display_value: Option<String>,
    pub write_value: Option<f64>,
    pub write_value_original: Option<f64>,
    pub current_priority: Option<i64>,
    pub is_output: Option<bool>,
    pub is_type_bool: Option<bool>,
    pub in_sync: Option<bool>,
    pub fallback: Option<f64>,
    pub device_uuid: Option<String>,
    pub writeable: Option<bool>,
    pub cov: Option<f64>,
    pub data_type: Option<String>,
    pub address_id: Option<Value>,
    pub address_length: Option<i64>,
    pub decimal: Option<i64>,
    pub multiplication_factor: Option<f64>,
    pub scale_enable: Option<bool>,
    pub scale_in_min: Option<f64>,
    pub scale_in_max: Option<f64>,
    pub scale_out_min: Option<f64>,
    pub scale_out_max: Option<f64>,
    pub offset: Option<f64>,
    pub unit: Option<String>,
    pub poll_priority: Option<String>,
    pub poll_rate: Option<String>,
    pub history_enable: Option<bool>,
    pub history_type: Option<String>,
    pub history_interval: Option<i64>,
    pub history_cov_threshold: Option<f64>,
    pub connection: Option<String>,
    pub connection_message: Option<String>,
    pub source_uuid: Option<String>,
    pub last_history_value: Option<f64>,
    pub config: Option<Value>,
    pub is_clone: Option<bool>,
    pub address_uuid: Option<String>,
    pub io_number: Option<Value>,
    pub object_type: Option<String>,
    pub point_source_uuid: Option<String>,
    pub host_uuid: Option<String>,
    pub priority: Option<Priority>,
    pub tags: Option<Vec<Tag>>,
    pub meta_tags: Option<Vec<MetaTag>>,
}

#[derive(Debug, Clone, Default)]
pub struct GetPointsParams {
    pub with_priority: Option<bool>,
    pub with_tags: Option<bool>,
    pub with_meta_tags: Option<bool>,
    pub name: Option<String>,
    pub address_uuid: Option<String>,
    pub io_number: Option<Value>,
    pub address_id: Option<Value>,
    pub object_type: Option<String>,
    pub meta_tags: Option<String>,
    pub point_source_uuid: Option<String>,
    pub host_uuid: Option<String>,
    pub device_uuid: Option<String>,
    pub offset: Option<i64>,
    pub limit: Option<i64>,
    pub search_keyword: Option<String>,
    pub tag: Option<String>,
    pub meta_tag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdatePointPayload {
    pub name: Option<String>,
    pub cov: Option<f64>,
    pub history_enable: Option<bool>,
    pub history_type: Option<String>,
    pub history_interval: Option<i64>,
    pub history_cov_threshold: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WritePointPriorityPayload {
    pub priority: Priority,
}

impl Client {
    pub async fn get_points(
        &self,
        params: Option<&GetPointsParams>,
    ) -> Result<Vec<Point>, RosClientError> {
        let mut query = Vec::new();

        if let Some(params) = params {
            if let Some(v) = params.with_priority {
                query.push(("with_priority", v.to_string()));
            }
            if let Some(v) = params.with_tags {
                query.push(("with_tags", v.to_string()));
            }
            if let Some(v) = params.with_meta_tags {
                query.push(("with_meta_tags", v.to_string()));
            }
            push_opt_string(&mut query, "name", params.name.as_ref());
            push_opt_string(&mut query, "address_uuid", params.address_uuid.as_ref());
            push_opt_json_value(&mut query, "io_number", params.io_number.as_ref());
            push_opt_json_value(&mut query, "address_id", params.address_id.as_ref());
            push_opt_string(&mut query, "object_type", params.object_type.as_ref());
            push_opt_string(&mut query, "meta_tags", params.meta_tags.as_ref());
            push_opt_string(
                &mut query,
                "point_source_uuid",
                params.point_source_uuid.as_ref(),
            );
            push_opt_string(&mut query, "host_uuid", params.host_uuid.as_ref());
            push_opt_string(&mut query, "device_uuid", params.device_uuid.as_ref());
            if let Some(v) = params.offset {
                query.push(("offset", v.to_string()));
            }
            if let Some(v) = params.limit {
                query.push(("limit", v.to_string()));
            }
            push_opt_string(&mut query, "search_keyword", params.search_keyword.as_ref());
            push_opt_string(&mut query, "tag", params.tag.as_ref());
            push_opt_string(&mut query, "meta_tag", params.meta_tag.as_ref());
        }

        query.push(("show_clones", "false".to_string()));
        self.get_json("/api/points", &query).await
    }

    pub async fn get_point_by_uuid(
        &self,
        uuid: &str,
        params: Option<&GetPointsParams>,
    ) -> Result<Point, RosClientError> {
        let mut query = Vec::new();

        if let Some(params) = params {
            if let Some(v) = params.with_priority {
                query.push(("with_priority", v.to_string()));
            }
            if let Some(v) = params.with_tags {
                query.push(("with_tags", v.to_string()));
            }
            if let Some(v) = params.with_meta_tags {
                query.push(("with_meta_tags", v.to_string()));
            }
            push_opt_string(&mut query, "tag", params.tag.as_ref());
            push_opt_string(&mut query, "meta_tag", params.meta_tag.as_ref());
        }

        let path = format!("/api/points/{uuid}");
        self.get_json(&path, &query).await
    }

    pub async fn update_point(&self, uuid: &str, point: &Point) -> Result<Point, RosClientError> {
        let path = format!("/api/points/{uuid}");
        self.patch_json(&path, point).await
    }

    pub async fn get_point_priority(&self, uuid: &str) -> Result<Option<Priority>, RosClientError> {
        let params = GetPointsParams {
            with_priority: Some(true),
            ..Default::default()
        };
        let point = self.get_point_by_uuid(uuid, Some(&params)).await?;
        Ok(point.priority)
    }

    pub async fn write_point_priority(
        &self,
        uuid: &str,
        priority: &Priority,
    ) -> Result<Point, RosClientError> {
        let path = format!("/api/points/{uuid}/write");
        let payload = WritePointPriorityPayload {
            priority: priority.clone(),
        };
        self.patch_json(&path, &payload).await
    }

    pub async fn write_point_priority_by_name(
        &self,
        network_name: &str,
        device_name: &str,
        point_name: &str,
        priority: &Priority,
    ) -> Result<Point, RosClientError> {
        let path = format!("/api/points/name/{network_name}/{device_name}/{point_name}/write");
        let payload = WritePointPriorityPayload {
            priority: priority.clone(),
        };
        self.patch_json(&path, &payload).await
    }
}

fn push_opt_string(query: &mut Vec<(&str, String)>, key: &'static str, value: Option<&String>) {
    if let Some(value) = value {
        if !value.is_empty() {
            query.push((key, value.clone()));
        }
    }
}

fn push_opt_json_value(query: &mut Vec<(&str, String)>, key: &'static str, value: Option<&Value>) {
    if let Some(value) = value {
        let query_value = match value {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(v) => v.to_string(),
            _ => value.to_string(),
        };
        if !query_value.is_empty() {
            query.push((key, query_value));
        }
    }
}
