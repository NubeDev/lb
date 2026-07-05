//! `scan_all` — page a whole workspace table's rows into memory (insights internal).
//!
//! `lb_store::list` filters by a single `data.<field>` equality; some insight reads need EVERY
//! row of a `write`-based ws table (an unfiltered `insight.list`, the admin sub lens, the digest
//! reactor's `insight_notify` scan). This walks `lb_store::scan`'s id-cursor pages to the end and
//! returns the **unwrapped host value** of each row — the workspace wall is structural (each page
//! is `use_ws(ws)`, README §7), and the tables are bounded in practice by dedup + the sub/ring caps.
//!
//! **Envelope note:** `write`-based rows are stored under a `data` field (see `store::record`), so
//! `scan` (which selects the whole record) returns `{ data: {...}, rev }`. This helper unwraps the
//! inner `data` so callers get the same shape `store::list`/`read` return. (Capped rows — the
//! occurrence ring — are stored FLAT by `capped_insert` and are read by their own direct query, not
//! here.)
//!
//! A hard page-budget backstop stops a pathological table from unbounded growth (the retention
//! follow-up sweeps these tables; this is the read-side guard until then).

use lb_store::{scan, Store, StoreError, MAX_SCAN_LIMIT};
use serde_json::Value;

/// The most rows a single `scan_all` will return before it stops paging (a read-side backstop;
/// the retention follow-up is the real bound). Generous vs the 1000/ws sub cap + typical insight
/// counts, small enough that one call never runs away.
pub const MAX_ROWS: usize = 10_000;

/// Return the unwrapped host value of every row in `table` within workspace `ws`, paging the
/// id-cursor `scan` to the end (or [`MAX_ROWS`], whichever comes first).
pub async fn scan_all(store: &Store, ws: &str, table: &str) -> Result<Vec<Value>, StoreError> {
    let mut out = Vec::new();
    let mut after: Option<String> = None;
    loop {
        let page = scan(store, ws, table, MAX_SCAN_LIMIT, after.as_deref()).await?;
        for row in page.rows {
            // `scan` returns the whole record; a `write`-based row wraps the host value under
            // `data`. Unwrap it so callers decode the record shape directly.
            let value = match row.data {
                Value::Object(mut obj) => obj.remove("data").unwrap_or(Value::Object(obj)),
                other => other,
            };
            out.push(value);
            if out.len() >= MAX_ROWS {
                return Ok(out);
            }
        }
        match page.next {
            Some(cursor) => after = Some(cursor),
            None => break,
        }
    }
    Ok(out)
}
