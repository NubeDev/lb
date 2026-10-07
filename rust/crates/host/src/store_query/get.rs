//! `store.get {table, id}` → `{table, id, value}` — read ONE record by id, gated on the table.
//!
//! The read half of `store.write`'s per-table grammar: `store:<table>:read` names the table, so a
//! holder reads that table and nothing else. `store.query` cannot be gated that way — a SurrealQL
//! statement can reach other tables without naming them (record links, graph edges) — which is why
//! the entity limit refuses it outright for a restricted caller.
//!
//! The entity limit here is the same, with one exception: an EXTENSION acting as itself reads a
//! table its approved capabilities name EXACTLY (`store:waste_config:read`, not `store:*:read`) —
//! its own records. Without it, an extension in a workspace with an enforced row policy could not
//! read its own settings (`ext:<id>` holds no menus, so its entity scope is "nothing").
//! A person is limited exactly as on `store.query`.

use lb_auth::Principal;
use lb_caps::{check, Action, Decision, Request, Surface};
use lb_store::Store;
use serde_json::{json, Value};

use super::authorize::authorize_store_query;
use super::error::StoreQueryError;
use super::secret_wall::ensure_no_secret_tables;

/// Read `table:id` in `ws` as `principal`. `value` is `null` when no such record exists.
pub async fn store_get_run(
    store: &Store,
    principal: &Principal,
    ws: &str,
    table: &str,
    id: &str,
) -> Result<Value, StoreQueryError> {
    authorize_store_query(principal, ws, "store.get")?;
    if table.trim().is_empty() || id.trim().is_empty() {
        return Err(StoreQueryError::Rejected(
            "table and id are required".into(),
        ));
    }
    ensure_no_secret_tables(table, &[])?;
    let req = Request::new(ws, Surface::Store, table, Action::Read);
    if let Decision::Denied(_) = check(principal, &req) {
        return Err(StoreQueryError::Denied);
    }
    if !own_records(principal, table)
        && crate::insight::entity_limit(store, principal, ws)
            .await
            .map_err(|_| StoreQueryError::Denied)?
            .is_some()
    {
        return Err(StoreQueryError::Denied);
    }
    let value = lb_store::read(store, ws, table, id).await?;
    Ok(json!({ "table": table, "id": id, "value": value }))
}

/// An extension acting as itself, reading a table its approved caps name exactly.
fn own_records(principal: &Principal, table: &str) -> bool {
    let exact = format!("store:{table}:read");
    crate::federation::extension_of(principal).is_some() && principal.caps().contains(&exact)
}
