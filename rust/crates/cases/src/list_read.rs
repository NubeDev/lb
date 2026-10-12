//! `read_cases` — the case rows a lane read starts from, narrowed by the engine.
//!
//! A lane used to page EVERY case record through `scan_all` and decode it before dropping the ones
//! it did not want. Most of a workspace's cases are closed (ESR: ~700 open of ~5,200), and the
//! default lane read never shows them, so the engine is asked for open cases only unless the caller
//! wants closed ones, and for just the listed ids when the caller names them (`filter.ids`). Every
//! other axis is still checked in memory by [`crate::list`]'s `matches`, so the result is the same
//! set the full scan gave — only the work to get there shrinks.
//!
//! One responsibility: the narrowed read of the case table.

use lb_store::Store;

use crate::case::{Case, TABLE};
use crate::error::CasesError;
use crate::list::unwrap_case;

/// Every case in `ws`, closed ones only when `include_closed`, only `ids` when given.
pub(crate) async fn read_cases(
    store: &Store,
    ws: &str,
    include_closed: bool,
    ids: Option<&[String]>,
) -> Result<Vec<Case>, CasesError> {
    let mut conditions = Vec::new();
    // `!= true` rather than `= false`, so a record written before `closed` existed still reads as
    // open, as its decode (`closed` defaults to false) says it is.
    if !include_closed {
        conditions.push("data.closed != true");
    }
    if ids.is_some() {
        conditions.push("data.id IN $ids");
    }
    let mut sql = String::from("SELECT data FROM type::table($tb)");
    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }
    let mut bindings = vec![("tb".into(), serde_json::Value::String(TABLE.into()))];
    if let Some(ids) = ids {
        bindings.push(("ids".into(), serde_json::json!(ids)));
    }
    let mut resp = store.query_ws(ws, &sql, bindings).await?;
    let rows: Vec<serde_json::Value> = resp
        .take(0)
        .map_err(|e| lb_store::StoreError::Decode(e.to_string()))?;
    Ok(rows.into_iter().filter_map(unwrap_case).collect())
}
