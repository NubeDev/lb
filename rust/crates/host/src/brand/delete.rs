//! `brand.delete(id)` — tombstone-upsert (reports scope; §6.8 idempotent). Owner-only, like update.
//! Gated by `mcp:brand.delete:call`. Plain soft-delete: a report referencing a deleted brand falls
//! back to the neutral default at export (no in-use check — brands are cheap and the report degrades
//! gracefully).

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_brand;
use super::error::BrandError;
use super::seed::SYSTEM_OWNER;
use super::store::{read_brand, write_brand};

/// Soft-delete brand `id` in `ws` as `principal`, at logical time `now`. Idempotent: an absent or
/// already-tombstoned brand is a no-op. Only the owner may delete.
pub async fn brand_delete(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    now: u64,
) -> Result<(), BrandError> {
    authorize_brand(principal, ws, "brand.delete")?;
    match read_brand(store, ws, id).await? {
        None => Ok(()),
        Some(b) if b.deleted => Ok(()),
        Some(mut b) => {
            // Owner-only, save for the SYSTEM_OWNER seed which any writer with the cap may remove
            // (a workspace that deliberately deletes every brand is not re-seeded — see seed.rs).
            if b.owner != SYSTEM_OWNER && b.owner != principal.owner_sub() {
                return Err(BrandError::Denied);
            }
            b.deleted = true;
            b.updated_ts = now;
            write_brand(store, ws, &b).await?;
            Ok(())
        }
    }
}
