//! `apikey.revoke` — tombstone a key + bust the cache + revoke its grants (api-keys scope). Gated by
//! `mcp:apikey.manage:call`, workspace-first. The tombstone is a `status = "__revoked__"` upsert —
//! **idempotent** (re-revoking writes the same tombstone harmlessly), so it replays cleanly under
//! sync (§6.8). The cache [`bust`](crate::apikey::ApiKeyCache::bust) makes the revoke bite on the
//! very next request **on this node** (instant local revoke — the mandatory integration property).
//!
//! The key's grants are revoked too (via [`revoke_subject`]) so a key re-created with the same id
//! cannot inherit the old caps — the scope's `revoke_subject` ripple for keys.

use lb_auth::Principal;
use lb_authz::revoke_subject;
use lb_mcp::authorize_tool;
use lb_store::{read, write, Store};

use super::cache::ApiKeyCache;
use super::error::ApiKeyError;
use super::model::{key_subject, ApiKeyRecord, TABLE, TOMBSTONE_STATUS};

/// Revoke key `id` in `ws` as `principal`. Idempotent. `cache` is the node's shared verification
/// cache (busted so the next auth on this node refuses immediately).
pub async fn apikey_revoke(
    store: &Store,
    cache: &ApiKeyCache,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<(), ApiKeyError> {
    authorize_tool(principal, ws, "apikey.manage").map_err(|_| ApiKeyError::Denied)?;

    // Read-modify-write to flip status → tombstone (preserves label/kind for the list/audit view).
    // Idempotent: re-revoking an already-tombstoned record writes the same status back.
    let value = read(store, ws, TABLE, id)
        .await?
        .ok_or(ApiKeyError::NotFound)?;
    let mut record: ApiKeyRecord = serde_json::from_value(value).map_err(unexpected)?;
    record.status = TOMBSTONE_STATUS.to_string();
    let value =
        serde_json::to_value(&record).map_err(|e| lb_store::StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, id, &value).await?;

    // Instant local revoke: drop the cached principal so the next auth on this node misses and reads
    // the tombstone. (Peers are bounded by sync + the cache TTL — the multi-node floor.)
    cache.bust(id).await;

    // Strip the key's grants so a re-created id can't inherit stale caps.
    revoke_subject(store, ws, &key_subject(id)).await?;
    Ok(())
}

fn unexpected(e: serde_json::Error) -> ApiKeyError {
    ApiKeyError::Store(lb_store::StoreError::Decode(e.to_string()))
}
