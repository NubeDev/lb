//! `store.scan` — authorize (admin cap), then read a bounded, id-cursor-paged page of raw rows from
//! one table. The raw read is `lb_store::scan` (hard `limit` cap + stable id cursor); this layer adds
//! the gate (data-console scope). Read-only.

use lb_auth::Principal;
use lb_store::{scan as store_scan, Page, Store};

use super::authorize::authorize_dbview;
use super::error::DbViewError;

/// Scan a bounded page of `table`'s rows in `ws`, starting after the `after` cursor. Gated by
/// `mcp:store.scan:call` (admin-only). The `limit` is hard-capped server-side (`lb_store`); the
/// returned page carries the next cursor (or `None` at the end). Namespace-scoped.
pub async fn store_scan_view(
    store: &Store,
    principal: &Principal,
    ws: &str,
    table: &str,
    limit: usize,
    after: Option<&str>,
) -> Result<Page, DbViewError> {
    authorize_dbview(principal, ws, "store.scan")?;
    // The secret-plane wall (node-update scope, decision 9): a raw scan bypasses the owner gate on
    // `secret.get` entirely, so the table itself is refused — for every principal, no override cap.
    // A scan names its table literally, so there is nothing dynamic to prove here.
    if let Some(t) = lb_store::secret_table_of(table) {
        return Err(DbViewError::SecretTable(t));
    }
    Ok(store_scan(store, ws, table, limit, after).await?)
}
