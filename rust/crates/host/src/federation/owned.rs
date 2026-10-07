//! **Extension-owned tables** — the tables a native extension created in a datasource with
//! `federation.migrate`, and the one exception they make to the entity row policy.
//!
//! An extension's backend acts as itself (`ext:<id>`, `native::spec::mint_child_token`): it holds no
//! menus and no grants, so under an enforced row policy its entity scope is "nothing" and every
//! verb that cannot be narrowed refuses it. That is right for the estate's data, and wrong for the
//! extension's own housekeeping: it could not create, write or read the tables it keeps its records
//! in. The extension already narrows PEOPLE itself (`authz.entity_scope` with `subject`).
//!
//! So a table an extension created is recorded here as ITS table, and on a restricted source that
//! extension (and nobody else) may migrate, write, delete and read it unfiltered. Everything else in
//! the source stays under the policy. The rules that keep this narrow:
//! - a table is claimed only when the migrate plan CREATES it — an existing table, including every
//!   estate table, can never be claimed ([`super::migrate_owned`]);
//! - the claim is an atomic first-write (`lb_store::create`), so two extensions cannot both own one;
//! - the record table is reserved (`lb_store::reserved`), so `store.write` cannot forge a claim;
//! - only an extension's OWN principal counts — a derived actor (`Principal::derive`) acts for its
//!   owner and is judged by the owner's scope as before.

use lb_auth::Principal;
use lb_store::{Store, StoreError};
use serde_json::{json, Value};

/// The store table holding one record per owned `(source, table)`.
pub const OWNED_TABLE: &str = "federation_table_owner";

/// The extension id when `principal` is an extension acting as itself; `None` for people and for
/// derived (on-behalf-of) actors.
pub fn extension_of(principal: &Principal) -> Option<&str> {
    if principal.owner_sub() != principal.sub() {
        return None;
    }
    principal
        .sub()
        .strip_prefix("ext:")
        .filter(|id| !id.is_empty())
}

fn record_id(source: &str, table: &str) -> String {
    format!("{source}/{table}")
}

/// The extension that owns `table` in `source`, if any.
pub async fn owner(
    store: &Store,
    ws: &str,
    source: &str,
    table: &str,
) -> Result<Option<String>, StoreError> {
    let rec = lb_store::read(store, ws, OWNED_TABLE, &record_id(source, table)).await?;
    Ok(rec.and_then(|v| v.get("ext").and_then(Value::as_str).map(str::to_string)))
}

/// Does extension `ext` own `table` in `source`?
pub async fn owns(
    store: &Store,
    ws: &str,
    ext: &str,
    source: &str,
    table: &str,
) -> Result<bool, StoreError> {
    Ok(owner(store, ws, source, table).await?.as_deref() == Some(ext))
}

/// Every table extension `ext` owns in `source`.
pub async fn owned_tables(
    store: &Store,
    ws: &str,
    ext: &str,
    source: &str,
) -> Result<Vec<String>, StoreError> {
    let rows = lb_store::list(store, ws, OWNED_TABLE, "ext", ext).await?;
    let mut tables: Vec<String> = rows
        .iter()
        .filter(|r| r.get("source").and_then(Value::as_str) == Some(source))
        .filter_map(|r| r.get("table").and_then(Value::as_str).map(str::to_string))
        .collect();
    tables.sort();
    tables.dedup();
    Ok(tables)
}

/// Claim `table` in `source` for `ext`. `Ok(true)` when it is now (or already was) `ext`'s;
/// `Ok(false)` when another extension holds it. Atomic: of two racing claims exactly one wins.
pub async fn claim(
    store: &Store,
    ws: &str,
    ext: &str,
    source: &str,
    table: &str,
    ts: u64,
) -> Result<bool, StoreError> {
    let value = json!({ "ext": ext, "source": source, "table": table, "ts": ts });
    match lb_store::create(store, ws, OWNED_TABLE, &record_id(source, table), &value).await {
        Ok(()) => Ok(true),
        Err(StoreError::Conflict) => owns(store, ws, ext, source, table).await,
        Err(e) => Err(e),
    }
}

/// Drop `ext`'s claim on `table` — the rollback when the migrate that claimed it fails.
pub async fn release(
    store: &Store,
    ws: &str,
    ext: &str,
    source: &str,
    table: &str,
) -> Result<(), StoreError> {
    if owns(store, ws, ext, source, table).await? {
        lb_store::delete(store, ws, OWNED_TABLE, &record_id(source, table)).await?;
    }
    Ok(())
}
