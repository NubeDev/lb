//! `datasource.remove {name}` (admin) — deregister a source (datasources scope). Admin-gated
//! (`mcp:datasource.remove:call`, workspace-first). Tombstones the record (the store has no delete
//! verb; a tombstone keeps the id stable + idempotent) so it reads as absent on resolve/list, AND
//! forgets the DSN secret so a later re-add starts clean (the secret is owned by `ext:federation`,
//! so the delete always passes the owner wall — see `secret::forget_dsn`). Leaving the secret behind
//! was the CRUD trap: a re-add with a new DSN would hit the owner wall on the stale record.

use lb_auth::Principal;

use super::authorize::authorize;
use super::error::FederationError;
use super::record::{put, resolve, Datasource};
use super::secret::forget_dsn;
use crate::boot::Node;

/// Remove the registered source `name` in `ws`. Idempotent: removing an absent source is a no-op
/// success (the tombstone is upserted regardless).
pub async fn datasource_remove(
    node: &Node,
    caller: &Principal,
    ws: &str,
    name: &str,
    ts: u64,
) -> Result<(), FederationError> {
    authorize(caller, ws, "datasource.remove")?;

    // Preserve the kind/endpoint/secret_ref on the tombstone (audit), just flag it removed.
    let mut ds = match resolve(&node.store, ws, name).await? {
        Some(ds) => ds,
        None => Datasource::new(name, "", "", format!("federation/{name}"), ts),
    };
    ds.removed = true;
    ds.ts = ts;
    put(&node.store, ws, &ds).await?;

    // Forget the DSN so a future re-add of this name writes a fresh secret without colliding with a
    // stale record. Owner is `ext:federation`, so this never denies; a source that never had a DSN
    // is a benign no-op.
    forget_dsn(node, ws, &ds.secret_ref).await?;
    Ok(())
}
