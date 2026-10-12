//! `member_counts` — how many detections each of several cases cites, in ONE read.
//!
//! A lane page ships up to [`crate::list::MAX_CASE_PAGE`] rows and every row carries its member
//! count. Asking per row ([`crate::member_count`]) is one query each, and `case_member` has no index
//! on `case_id`, so each of those queries walks the whole membership table: 200 rows was 200 walks.
//! This asks once for all of them and lets the engine group.
//!
//! One responsibility: member counts for a set of case ids.

use std::collections::HashMap;

use lb_store::Store;
use serde::Deserialize;

use crate::case_member::TABLE;
use crate::error::CasesError;

#[derive(Deserialize)]
struct CountRow {
    case_id: Option<String>,
    n: Option<usize>,
}

/// The member count of every case in `ids`. A case with no members is absent from the map (its
/// count is 0).
pub async fn member_counts(
    store: &Store,
    ws: &str,
    ids: &[String],
) -> Result<HashMap<String, usize>, CasesError> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let sql = "SELECT data.case_id AS case_id, count() AS n FROM type::table($tb) \
               WHERE data.case_id IN $ids GROUP BY case_id";
    let mut resp = store
        .query_ws(
            ws,
            sql,
            vec![
                ("tb".into(), serde_json::Value::String(TABLE.into())),
                ("ids".into(), serde_json::json!(ids)),
            ],
        )
        .await?;
    let rows: Vec<serde_json::Value> = resp
        .take(0)
        .map_err(|e| lb_store::StoreError::Decode(e.to_string()))?;
    Ok(rows
        .into_iter()
        .filter_map(|v| serde_json::from_value::<CountRow>(v).ok())
        .filter_map(|r| Some((r.case_id?, r.n?)))
        .collect())
}
