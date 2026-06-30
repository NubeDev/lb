//! Create/update a binary asset — the image/attachment write verb (document-store scope move
//! 2). Requires `store:asset/{id}:write` (capability-first, §3.5), workspace-first. The asset
//! is owned by the caller. Bytes are an inline record value at v1 (no `DEFINE BUCKET`), so the
//! put is **size-bounded**: an over-bound payload is rejected with a clear error, never
//! silently truncated (document-store scope risk). The bound is the documented v1 ceiling;
//! streaming + real buckets is the deferred slice.

use lb_assets::{put_asset as store_put_asset, Asset};
use lb_auth::Principal;
use lb_caps::Action;
use lb_store::Store;

use super::authorize::authorize_asset;
use super::error::AssetError;

/// The v1 inline-asset size ceiling (8 MiB). Stated explicitly per the scope's "state the bound
/// explicitly" risk. A larger payload is rejected; streaming put is the deferred bucket slice.
pub const MAX_ASSET_BYTES: usize = 8 * 1024 * 1024;

/// Create or update asset `id` in workspace `ws` as `principal`, with caller-supplied `mime`
/// and raw `bytes`. `owner` is forced to `principal.sub`. Rejects payloads over
/// [`MAX_ASSET_BYTES`]. Returns the stored asset.
pub async fn put_asset(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    mime: &str,
    bytes: Vec<u8>,
    ts: u64,
) -> Result<Asset, AssetError> {
    authorize_asset(principal, ws, id, Action::Write)?;
    if bytes.len() > MAX_ASSET_BYTES {
        // A clear, honest error — never a silent truncation (scope risk: "reject over-bound
        // puts with a clear error").
        return Err(AssetError::TooLarge);
    }
    let asset = Asset::new(id, principal.sub(), mime, bytes, ts);
    store_put_asset(store, ws, &asset).await?;
    Ok(asset)
}
