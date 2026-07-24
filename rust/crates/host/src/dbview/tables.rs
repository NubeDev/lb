//! `store.tables` — authorize (admin cap), then list the workspace's tables + row counts. The raw
//! read is `lb_store::tables`; this layer adds the gate (data-console scope). Read-only.

use lb_auth::Principal;
use lb_store::{tables as store_tables, Store};
use serde::{Deserialize, Serialize};

use super::authorize::authorize_dbview;
use super::error::DbViewError;

/// One table row in the picker: name, exact row count, and whether the table is **host-owned**
/// (`system: true` ⇔ `lb_store::reserved::is_reserved` — the ext-store-nodes reserved-table wall).
/// The writable-table picker excludes `system` rows; the read picker shows them with a badge. The
/// flag is a global const property, identical across workspaces.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TableInfo {
    pub table: String,
    pub count: u64,
    pub system: bool,
}

/// List every table in `ws` with its row count + `system` flag, for the DB-browser / flow-editor
/// table pickers. Gated by `mcp:store.tables:call` (workspace-admin AND editor/member since the
/// ext-store-nodes scope opened it to flow authors — it reveals table names + counts only).
/// Namespace-scoped — a ws-B caller sees only ws-B's tables.
pub async fn store_tables_view(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<TableInfo>, DbViewError> {
    authorize_dbview(principal, ws, "store.tables")?;
    let rows = store_tables(store, ws).await?;
    Ok(rows
        .into_iter()
        .map(|t| TableInfo {
            system: lb_store::reserved::is_reserved(&t.table),
            table: t.table,
            count: t.count,
        })
        .collect())
}
