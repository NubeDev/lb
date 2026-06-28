//! `datasource.remove {name}` (admin) — deregister a source (datasources scope). Admin-gated
//! (`mcp:datasource.remove:call`, workspace-first). Tombstones the record (the store has no delete
//! verb; a tombstone keeps the id stable + idempotent) so it reads as absent on resolve/list. The
//! DSN secret is left in `lb-secrets` (its own lifecycle/rotation surface, secrets scope) — removing
//! the source just makes it unresolvable.

use lb_auth::Principal;

use super::authorize::authorize;
use super::error::FederationError;
use super::record::{put, resolve, Datasource};
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
    Ok(())
}
