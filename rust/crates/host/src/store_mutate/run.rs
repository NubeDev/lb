//! `store.write` / `store.delete` — the generic per-table mutation verbs. Each authorizes the
//! per-table store gate (`store:<table>:write`), then delegates to the raw store verb inside the
//! **caller's workspace namespace** (selected host-side from the token — never named in the args), so
//! a ws-B caller can only ever land in ws-B's namespace (README §7, structural isolation).
//!
//! `store.write` UPSERTs `value` at `table:id` (bumping the record's monotonic `rev`, see
//! `lb_store::write`). `store.delete` erases `table:id` (idempotent — deleting an absent record is a
//! success). The value is stored under the same `{ data: … }` envelope the whole store uses, so a
//! record written here reads back identically through `lb_store::read` / `store.query`.

use lb_auth::Principal;
use lb_store::Store;
use serde_json::Value;

use super::authorize::authorize_store_mutate;
use super::error::StoreMutateError;

/// UPSERT `value` at `table:id` in `ws`. Gated `store:<table>:write`; workspace-walled. Returns the
/// `(table, id)` written so a caller that let the record id default still learns the key.
pub async fn store_write_run(
    store: &Store,
    principal: &Principal,
    ws: &str,
    table: &str,
    id: &str,
    value: &Value,
) -> Result<(String, String), StoreMutateError> {
    validate_key(table, id)?;
    authorize_store_mutate(principal, ws, table)?;
    lb_store::write(store, ws, table, id, value).await?;
    Ok((table.to_string(), id.to_string()))
}

/// Erase `table:id` from `ws`. Gated `store:<table>:write` (a delete is a table mutation — module
/// doc); workspace-walled; idempotent. Returns the `(table, id)` targeted.
pub async fn store_delete_run(
    store: &Store,
    principal: &Principal,
    ws: &str,
    table: &str,
    id: &str,
) -> Result<(String, String), StoreMutateError> {
    validate_key(table, id)?;
    authorize_store_mutate(principal, ws, table)?;
    lb_store::delete(store, ws, table, id).await?;
    Ok((table.to_string(), id.to_string()))
}

/// Reject an empty `table` or `id`. Both are bound as `type::thing` params by the store (never string
/// interpolation), so there is no injection surface — this is a shape check for a clean `BadInput`
/// rather than a `Denied` (an empty table would gate on `store::write`, an odd signal).
fn validate_key(table: &str, id: &str) -> Result<(), StoreMutateError> {
    if table.trim().is_empty() {
        return Err(StoreMutateError::BadInput("empty arg: table".into()));
    }
    if id.trim().is_empty() {
        return Err(StoreMutateError::BadInput("empty arg: id".into()));
    }
    Ok(())
}
