//! `brand.save(id, ...)` — one idempotent UPSERT for create+update (reports scope). A fresh id
//! creates (owner = principal), an existing id updates (owner-only — a non-owner with the save cap
//! cannot overwrite someone else's brand). Gated by `mcp:brand.save:call`.
//!
//! Exception: the seeded default carries the [`SYSTEM_OWNER`] sentinel, which no real principal owns.
//! A save against it ADOPTS it (owner becomes the writer) rather than denying — so an admin can brand
//! the workspace default in place, no reseed. This is the only owner that transfers on write.

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_brand;
use super::error::BrandError;
use super::model::{Brand, Colors, Fonts, SCHEMA_VERSION};
use super::seed::SYSTEM_OWNER;

/// Upsert brand `id` in `ws` as `principal`, at logical time `now`. Creates on a fresh id
/// (owner = principal); updates an existing one (owner-only). Returns the persisted record.
#[allow(clippy::too_many_arguments)]
pub async fn brand_save(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    name: &str,
    logo_asset_id: &str,
    colors: Colors,
    fonts: Fonts,
    header_text: &str,
    footer_text: &str,
    now: u64,
) -> Result<Brand, BrandError> {
    authorize_brand(principal, ws, "brand.save")?;
    if id.is_empty() {
        return Err(BrandError::BadInput("empty brand id".into()));
    }

    // Preserve owner across an update; only the owner may update. A tombstoned record is treated as
    // absent — a save with that id resurrects it under the new owner (create). The SYSTEM_OWNER seed
    // is the exception: any writer with the cap adopts it (owner becomes them), so the workspace
    // default is brandable in place.
    let owner = match super::store::read_brand(store, ws, id)
        .await?
        .filter(|b| !b.deleted)
    {
        Some(existing) if existing.owner == SYSTEM_OWNER => principal.owner_sub().to_string(),
        Some(existing) => {
            if existing.owner != principal.owner_sub() {
                return Err(BrandError::Denied);
            }
            existing.owner
        }
        None => principal.owner_sub().to_string(),
    };

    let brand = Brand {
        id: id.to_string(),
        name: name.to_string(),
        owner,
        logo_asset_id: logo_asset_id.to_string(),
        colors,
        fonts,
        header_text: header_text.to_string(),
        footer_text: footer_text.to_string(),
        schema_version: SCHEMA_VERSION,
        updated_ts: now,
        deleted: false,
    };
    super::store::write_brand(store, ws, &brand).await?;
    Ok(brand)
}
