//! `brand.get(id)` — the read verb (reports scope, "Brand profiles"). Gates 1+2 (`authorize_brand`)
//! before any fetch (no existence signal to an outsider), then fetch. A brand is workspace-shared,
//! so there is no gate-3 membership check (any member with `brand.get` reads any brand). A
//! tombstoned brand reads as `NotFound`.

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_brand;
use super::error::BrandError;
use super::model::Brand;
use super::store::read_brand;

/// Read brand `id` in `ws` for `principal`, if gates 1+2 pass.
pub async fn brand_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<Brand, BrandError> {
    authorize_brand(principal, ws, "brand.get")?;
    read_brand(store, ws, id)
        .await?
        .filter(|b| !b.deleted)
        .ok_or(BrandError::NotFound)
}
