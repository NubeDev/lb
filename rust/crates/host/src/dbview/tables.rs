//! `store.tables` — authorize (admin cap), then list the workspace's tables + row counts. The raw
//! read is `lb_store::tables`; this layer adds the gate (data-console scope). Read-only.

use lb_auth::Principal;
use lb_store::{tables as store_tables, Store, TableCount};

use super::authorize::authorize_dbview;
use super::error::DbViewError;

/// List every table in `ws` with its row count, for the DB-browser table picker. Gated by
/// `mcp:store.tables:call` (admin-only). Namespace-scoped — a ws-B admin sees only ws-B's tables.
pub async fn store_tables_view(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<TableCount>, DbViewError> {
    authorize_dbview(principal, ws, "store.tables")?;
    Ok(store_tables(store, ws).await?)
}
