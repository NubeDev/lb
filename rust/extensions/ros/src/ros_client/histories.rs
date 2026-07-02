//! The `history` record + the per-point histories fetch (`/api/histories/...`). Vendored for
//! completeness; a `ros-histories` backfill is a follow-up slice (ros-scope non-goals), so nothing
//! above the client wires it yet. `async` port only.

use serde::{Deserialize, Serialize};

use super::{client::Client, error::RosClientError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryRecord {
    pub id: i64,
    pub point_uuid: String,
    pub present_value: f64,
    pub timestamp: String,
}

#[derive(Debug, Clone, Default)]
pub struct GetHistoriesParams {
    pub id_gt: Option<i64>,
    pub timestamp_gt: Option<String>,
    pub timestamp_lt: Option<String>,
    pub order: Option<String>,
    pub limit: Option<i64>,
}

impl Client {
    pub async fn get_histories_for_point(
        &self,
        uuid: &str,
        params: Option<&GetHistoriesParams>,
    ) -> Result<Vec<HistoryRecord>, RosClientError> {
        let mut query = Vec::new();

        if let Some(params) = params {
            if let Some(v) = params.id_gt {
                query.push(("id_gt", v.to_string()));
            }
            if let Some(v) = &params.timestamp_gt {
                if !v.is_empty() {
                    query.push(("timestamp_gt", v.clone()));
                }
            }
            if let Some(v) = &params.timestamp_lt {
                if !v.is_empty() {
                    query.push(("timestamp_lt", v.clone()));
                }
            }
            if let Some(v) = &params.order {
                if !v.is_empty() {
                    query.push(("order", v.clone()));
                }
            }
            if let Some(v) = params.limit {
                query.push(("limit", v.to_string()));
            }
        }

        let path = format!("/api/histories/points/point-uuid/{uuid}");
        self.get_json(&path, &query).await
    }
}
