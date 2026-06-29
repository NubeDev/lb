//! `apikey.rotate` — replace a key's secret with a fresh one, killing the old secret instantly
//! (api-keys scope, resolved decision: INSTANT rotation, no grace/overlap). Gated by
//! `mcp:apikey.manage:call`, workspace-first. The grants are untouched (same authority, new
//! credential); the hash is replaced and the cache busted, so the old secret dies on the next
//! request on this node. The new secret is returned once — the same single-egress rule as create.

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::{read, write, Store};

use super::cache::ApiKeyCache;
use super::error::ApiKeyError;
use super::model::{ApiKeyRecord, TABLE};

/// Rotate key `id`'s secret in `ws` as `principal`, returning the one-time new bearer string. The
/// old secret is dead immediately (hash replaced + cache busted); the key's grants are unchanged.
pub async fn apikey_rotate(
    store: &Store,
    cache: &ApiKeyCache,
    principal: &Principal,
    ws: &str,
    id: &str,
    pepper: &[u8],
) -> Result<String, ApiKeyError> {
    authorize_tool(principal, ws, "apikey.manage").map_err(|_| ApiKeyError::Denied)?;

    let value = read(store, ws, TABLE, id)
        .await?
        .ok_or(ApiKeyError::NotFound)?;
    let mut record: ApiKeyRecord = serde_json::from_value(value).map_err(unexpected)?;
    if record.is_revoked() {
        return Err(ApiKeyError::Revoked);
    }

    // Fresh secret; rehash under the same pepper. The old secret's hash is overwritten, so it no
    // longer verifies — dead, no overlap.
    let new_secret = lb_apikey::generate_secret();
    record.key_hash = lb_apikey::key_hash(pepper, &new_secret);
    let value =
        serde_json::to_value(&record).map_err(|e| lb_store::StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, id, &value).await?;

    // Drop the cached principal (built from the OLD hash) so the next auth re-verifies under the new
    // hash. The old secret's hash no longer matches the stored hash → refuses.
    cache.bust(id).await;

    Ok(lb_apikey::format_bearer(ws, id, &new_secret))
}

fn unexpected(e: serde_json::Error) -> ApiKeyError {
    ApiKeyError::Store(lb_store::StoreError::Decode(e.to_string()))
}
