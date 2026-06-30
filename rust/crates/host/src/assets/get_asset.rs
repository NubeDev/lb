//! Read a binary asset — the three-gate read verb (document-store scope), the asset analog of
//! `get_doc`. Gates run in exact order:
//!   1. workspace + 2. capability — `authorize_asset(..., Read)` (`store:asset/{id}:read`);
//!   3. membership — `may_read_asset` (owner / shared-team-or-user / embedded-by-a-readable-doc).
//!
//! The embed path re-gates the embedding doc through the FULL doc gate — an embed never widens
//! access (the load-bearing "link/embed never widens" deny test).

use lb_assets::{get_asset as store_get_asset, Asset};
use lb_auth::Principal;
use lb_caps::Action;
use lb_store::Store;

use super::authorize::authorize_asset;
use super::error::AssetError;
use super::visibility::may_read_asset;

/// Read asset `id` in workspace `ws` for `principal`, if all three gates pass. `NotFound` only
/// reaches a caller who passed the capability gate (so it leaks nothing to an outsider).
pub async fn get_asset(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<Asset, AssetError> {
    // Gates 1 + 2 before any fetch.
    authorize_asset(principal, ws, id, Action::Read)?;
    let asset = store_get_asset(store, ws, id)
        .await?
        .ok_or(AssetError::NotFound)?;
    // Gate 3: membership — owner / shared / embedded-by-a-readable-doc.
    may_read_asset(store, principal, ws, &asset).await?;
    Ok(asset)
}
