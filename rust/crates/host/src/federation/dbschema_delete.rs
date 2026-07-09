//! `dbschema.delete {name}` (member) — remove a designed-schema record (schema-designer scope).
//! Member-gated (`mcp:dbschema.delete:call`, workspace-first, covered by the author delete
//! wildcard). Tombstones the record (the store has no delete verb; a tombstone keeps the id stable
//! + idempotent, mirroring `datasource.remove`). The record's tables/columns are documentation —
//! deleting it touches NO live database (a live schema is dropped via a future destructive-migrate
//! verb, never here).

use lb_auth::Principal;

use super::authorize::authorize;
use super::dbschema_record::{put, resolve, DbSchemaRecord};
use super::error::FederationError;
use crate::boot::Node;

/// Tombstone the `db_schema:{ws}:{name}` record. Idempotent: deleting an absent schema is a no-op
/// success (the tombstone is upserted regardless). Never touches any external/live database.
pub async fn dbschema_delete(
    node: &Node,
    caller: &Principal,
    ws: &str,
    name: &str,
    ts: u64,
) -> Result<(), FederationError> {
    authorize(caller, ws, "dbschema.delete")?;

    let mut rec = match resolve(&node.store, ws, name).await? {
        Some(r) => r,
        None => DbSchemaRecord::new(name, ts),
    };
    rec.removed = true;
    rec.ts = ts;
    put(&node.store, ws, &rec).await?;
    Ok(())
}
