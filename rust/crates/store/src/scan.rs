//! Scan a bounded page of raw rows from one table in a workspace's namespace — the generic read
//! behind the admin DB-browser's row grid (data-console scope). Read-only; **id-cursor** paged so a
//! huge table never returns unbounded, and the cursor is stable under concurrent writes (an offset
//! drifts when rows are inserted/removed mid-paging; a record-id cursor does not).
//!
//! Namespace-bound (`use_ws(ws)`): a ws-A scan physically sees A's rows only (the hard wall, §7).
//! The `limit` is **hard-capped server-side** ([`MAX_SCAN_LIMIT`]) — a caller asking for a million
//! rows still gets at most one bounded page. The capability gate (admin-only) is one layer up in the
//! host `dbview` service; this is the raw read.
//!
//! Unlike `list`/`read`, a scan returns each record's **id alongside its data**, because the grid
//! shows heterogeneous rows (the id is how a row is identified and how graph-expand starts from it),
//! and the id is the paging cursor.

use serde::Deserialize;
use serde_json::Value;

use crate::open::{Store, StoreError};

// `Deserialize` is used by the derive on `Row`/`Page`.

/// The hard server-side ceiling on a single scan page. A caller's `limit` is clamped to this, so one
/// call can never return more than a bounded page regardless of what it asks for.
pub const MAX_SCAN_LIMIT: usize = 200;

/// One scanned record: its full record id (`table:id`) and its stored `data` value. The grid renders
/// the data; the id labels the row and seeds graph-expand and the next-page cursor.
#[derive(Debug, Clone, serde::Serialize, Deserialize, PartialEq)]
pub struct Row {
    pub id: String,
    pub data: Value,
}

/// A bounded page of a scan: the rows plus the cursor to pass as `after` for the next page (`None`
/// when the page was not full — i.e. the end was reached).
#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub struct Page {
    pub rows: Vec<Row>,
    pub next: Option<String>,
}

/// Scan up to `limit` rows of `table` in `ws`, ordered by record id ascending, starting strictly
/// after the `after` cursor (a record id from a prior page; `None` for the first page). `limit` is
/// clamped to [`MAX_SCAN_LIMIT`]. The returned `next` cursor is the last row's id when the page was
/// full, else `None`.
pub async fn scan(
    store: &Store,
    ws: &str,
    table: &str,
    limit: usize,
    after: Option<&str>,
) -> Result<Page, StoreError> {
    let n = limit.clamp(1, MAX_SCAN_LIMIT);

    // Select id + the stored value, ordered by id so the cursor is total + stable. `type::table($tb)`
    // binds the table name (never raw text); the cursor, when present, is `id > type::thing($tb,
    // $after)` so paging resumes strictly after the last seen id within THIS table.
    // Project the displayable id (`meta::id`), keep the real `id` under `_oid` to satisfy ORDER BY
    // (SurrealDB requires the order idiom in the selection — debugging/store/
    // order-by-needs-selected-idiom.md), and the **whole record** via `* OMIT id, in, out`: a record
    // stores its fields directly (a `series` row is `{series, producer, seq, …}`), and the generic
    // grid renders the full object. We OMIT `id` (a Thing — breaks JSON) and the relation links
    // `in`/`out` (Things on edge tables); `OMIT` of an absent field on a normal table is a no-op.
    let (sql, bindings): (String, Vec<(String, Value)>) = match after {
        Some(cursor) => (
            format!(
                "SELECT meta::id(id) AS rid, <string>id AS _oid, * OMIT id, in, out FROM type::table($tb) \
                 WHERE <string>id > $after ORDER BY _oid ASC LIMIT {n}"
            ),
            vec![
                ("tb".into(), Value::String(table.to_string())),
                ("after".into(), Value::String(cursor.to_string())),
            ],
        ),
        None => (
            format!(
                "SELECT meta::id(id) AS rid, <string>id AS _oid, * OMIT id, in, out FROM type::table($tb) \
                 ORDER BY _oid ASC LIMIT {n}"
            ),
            vec![("tb".into(), Value::String(table.to_string()))],
        ),
    };

    let mut resp = store.query_ws(ws, &sql, bindings).await?;
    let raw: Vec<Value> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;

    // The cursor is the `<string>id` of the last row (the `_oid` projection) — the exact value the
    // `WHERE <string>id > $after` comparison uses, so paging resumes strictly after it. We keep it
    // separate from the displayable `table:id` the grid shows.
    let mut last_oid: Option<String> = None;
    let rows: Vec<Row> = raw
        .into_iter()
        .filter_map(|v| v.as_object().cloned())
        .map(|mut obj| {
            let rid = obj.remove("rid").unwrap_or(Value::Null);
            if let Some(Value::String(oid)) = obj.remove("_oid") {
                last_oid = Some(oid);
            }
            Row {
                id: format!("{table}:{}", render_id(&rid)),
                data: Value::Object(obj),
            }
        })
        .collect();

    // A full page means there may be more — hand back the last row's id-string as the cursor. A short
    // page is the end (`None`), so the grid knows to stop.
    let next = if rows.len() == n { last_oid } else { None };
    Ok(Page { rows, next })
}

/// Render a record id (which may be a string, a number, or a **composite array** — `series` is keyed
/// on `[series, producer, seq]`) into the displayable id half. A bare string is verbatim; anything
/// structured (array/number) is its compact JSON, so the `table:id` round-trips for the UI/cursor.
fn render_id(rid: &Value) -> String {
    match rid {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}
