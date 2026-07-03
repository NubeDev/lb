//! `nav.delete(id)` — tombstone-upsert (nav scope, "MCP surface"; §6.8 idempotent). A re-delete is a
//! no-op, and a delete of an absent nav is a no-op (not an error) — the idempotency the sync path
//! relies on. Owner-only, like update. Gated by `mcp:nav.delete:call`.

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_nav;
use super::error::NavError;
use super::store::{read_nav, write_nav};

/// Soft-delete nav `id` in `ws` as `principal`, at logical time `now`. Idempotent: an absent or
/// already-tombstoned nav is a no-op. Only the owner may delete.
pub async fn nav_delete(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    now: u64,
) -> Result<(), NavError> {
    authorize_nav(principal, ws, "nav.delete")?;

    match read_nav(store, ws, id).await? {
        None => Ok(()),
        Some(n) if n.deleted => Ok(()),
        Some(mut n) => {
            if n.owner != principal.sub() {
                return Err(NavError::Denied);
            }
            n.deleted = true;
            n.updated_ts = now;
            write_nav(store, ws, &n).await?;
            Ok(())
        }
    }
}
