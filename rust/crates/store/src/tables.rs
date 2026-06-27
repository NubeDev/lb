//! List the tables in a workspace's namespace and count their rows — the generic store read behind
//! the admin DB-browser's table picker (data-console scope). `INFO FOR DB` enumerates the defined
//! tables; a `count()` per table gives the row count a non-SQL user reads as "what's in here".
//!
//! Namespace-bound like every other store read: `use_ws(ws)` selects workspace A's namespace first,
//! so the list a ws-A admin sees is physically A's tables only — never B's (the hard wall, README
//! §7). This is a *raw* read; the capability gate (admin-only, gate-3-relaxed) lives one layer up in
//! the host `dbview` service, never here.
//!
//! Cost note (scope open question, resolved): we take an **exact** `count()` per table. At the
//! dev/admin scale this console targets that is cheap; on a million-row table it is not free, but an
//! exact count is the honest answer for "how many rows", and the admin-only gate bounds who pays it.
//! A cheaper estimate is a documented follow-up, not a v1 need.

use serde::Deserialize;

use crate::open::{Store, StoreError};

/// One table and its row count in a workspace.
#[derive(Debug, Clone, serde::Serialize, Deserialize, PartialEq)]
pub struct TableCount {
    pub table: String,
    pub count: u64,
}

/// Every table defined in `ws`'s namespace with its exact row count, sorted by table name. Empty if
/// the namespace has no tables — never another workspace's tables.
pub async fn tables(store: &Store, ws: &str) -> Result<Vec<TableCount>, StoreError> {
    // `INFO FOR DB` returns a structured object; its `tables` field is a map of table-name → DEFINE
    // statement. We only need the keys (the table names).
    let mut resp = store.query_ws(ws, "INFO FOR DB", vec![]).await?;
    let info: Option<DbInfo> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;

    let mut names: Vec<String> = info
        .map(|i| i.tables)
        .unwrap_or_default()
        .into_keys()
        .collect();
    names.sort();

    let mut out = Vec::with_capacity(names.len());
    for table in names {
        // `count()` over the whole table, grouped to a single row (`GROUP ALL`). Bound the table
        // name through `type::table` so the identifier never reaches the query text as raw input.
        let mut cresp = store
            .query_ws(
                ws,
                "SELECT count() AS c FROM type::table($tb) GROUP ALL",
                vec![("tb".into(), serde_json::Value::String(table.clone()))],
            )
            .await?;
        let rows: Vec<CountRow> = cresp
            .take(0)
            .map_err(|e| StoreError::Decode(e.to_string()))?;
        let count = rows.first().map(|r| r.c).unwrap_or(0);
        out.push(TableCount { table, count });
    }
    Ok(out)
}

/// The slice of `INFO FOR DB` we read — the `tables` map (table-name → DEFINE text). We ignore every
/// other field (analyzers, functions, params, …).
#[derive(Deserialize)]
struct DbInfo {
    #[serde(default)]
    tables: std::collections::BTreeMap<String, serde_json::Value>,
}

#[derive(Deserialize)]
struct CountRow {
    c: u64,
}
