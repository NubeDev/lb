//! `store.list {table, after?, limit?}` → `{table, rows: [{id, value}], next}` — read one table a
//! page at a time, gated exactly like `store.get` (`get.rs`: `store:<table>:read`, the entity limit,
//! and the exception for an extension reading a table its caps name exactly).
//!
//! Why it exists: under an enforced row policy an extension is refused `store.query` outright (a
//! SurrealQL statement can reach other tables through record links), and `store.get` needs the id up
//! front. An extension that keeps its settings and state in its own tables also has to enumerate
//! them; this is that read, bound to the one named table, with no query text at all.
//!
//! Paging is `lb_store::scan`'s record-id cursor (stable under concurrent writes) and its hard page
//! ceiling (`MAX_SCAN_LIMIT`). `next` is opaque: pass it back as `after`; `null` means the end.

use lb_auth::Principal;
use lb_store::Store;
use serde_json::{json, Value};

use super::error::StoreQueryError;
use super::get::authorize_table_read;

/// One page of `table` in `ws` as `principal`, in record-id order, starting after `after`.
pub async fn store_list_run(
    store: &Store,
    principal: &Principal,
    ws: &str,
    table: &str,
    after: Option<&str>,
    limit: usize,
) -> Result<Value, StoreQueryError> {
    authorize_table_read(store, principal, ws, "store.list", table).await?;
    let page = lb_store::scan(store, ws, table, limit, after).await?;
    let prefix = format!("{table}:");
    let rows: Vec<Value> = page
        .rows
        .into_iter()
        .map(|row| {
            // `scan` hands back the whole stored record; the host value is its `data` envelope,
            // the same thing `store.get` returns as `value`.
            let id = row.id.strip_prefix(&prefix).unwrap_or(&row.id).to_string();
            let value = row.data.get("data").cloned().unwrap_or(Value::Null);
            json!({ "id": id, "value": value })
        })
        .collect();
    Ok(json!({ "table": table, "rows": rows, "next": page.next }))
}
