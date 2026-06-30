//! List the caller's own assets in a workspace — the asset analog of `list_docs` (document-store
//! scope). Requires `store:asset/*:read`, workspace-first. Lists assets the principal *owns*;
//! assets readable via embed/share are fetched by id through `get_asset` (which re-gates).

use lb_assets::{list_assets as store_list_assets, Asset};
use lb_auth::Principal;
use lb_caps::Action;
use lb_store::Store;

use super::authorize::authorize_asset;
use super::error::AssetError;

/// Return the assets `principal` owns in workspace `ws`, oldest→newest.
pub async fn list_assets(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<Asset>, AssetError> {
    authorize_asset(principal, ws, "*", Action::Read)?;
    Ok(store_list_assets(store, ws, principal.sub()).await?)
}
