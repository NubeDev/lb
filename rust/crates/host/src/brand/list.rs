//! `brand.list()` — the roster verb (reports scope). Returns every non-tombstoned brand in the
//! workspace as cheap summaries (id/name/updated_ts). A brand is workspace-shared, so any member
//! with `brand.list` sees all brands (no per-row membership filter) — the picker is never empty.

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_brand;
use super::error::BrandError;
use super::model::BrandSummary;
use super::store::scan_brands;

/// List the brands in `ws` that `principal` may read (all of them, given the cap). Tombstoned
/// brands are excluded.
pub async fn brand_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<BrandSummary>, BrandError> {
    authorize_brand(principal, ws, "brand.list")?;
    let all = scan_brands(store, ws).await?;
    Ok(all
        .iter()
        .filter(|b| !b.deleted)
        .map(BrandSummary::from)
        .collect())
}
