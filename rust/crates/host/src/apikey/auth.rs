//! `apikey.authenticate` — verify a presented bearer credential and build the verified `Principal`
//! (api-keys scope, decision 1). The gateway's auth path calls this when it sees the `lbk_` prefix.
//! It is the **per-request** verification (not a token exchange): parse → O(1) ws-scoped lookup →
//! constant-time HMAC compare → status + lazy-expiry check → resolve caps → `Principal::for_key`,
//! with a small hash→principal cache (busted on revoke) keeping the hot path cheap.
//!
//! Security never depends on the cache: a wrong secret, a tombstone, and an expired key are ALL
//! refused here on every request the cache does not hit, and the cache itself refuses an entry past
//! the record's expiry (the lazy check stays authoritative). The secret is **never logged**; only the
//! hash is compared. The resulting principal's `ws` is the bearer's ws — the hard wall — so a key
//! minted in ws A cannot authenticate against ws B (the store namespace wall makes it so).

use std::collections::BTreeSet;

use lb_auth::Principal;
use lb_authz::Subject;
use lb_store::{read, Store};

use super::cache::ApiKeyCache;
use super::error::ApiKeyError;
use super::model::{ApiKeyRecord, TABLE};
use crate::authz::resolve_subject_caps_live as resolve_subject_caps;

/// Verify `secret` against key `id` in workspace `ws` at logical time `now`, resolving the key's
/// caps and building a `Principal::for_key`. `cache` is the node's shared verification cache.
///
/// Outcomes `NotFound` / `Revoked` / `Expired` / `Invalid` are auth failures the gateway collapses
/// to the same opaque `401` (no oracle). Returns the verified principal on success.
pub async fn apikey_authenticate(
    store: &Store,
    cache: &ApiKeyCache,
    pepper: &[u8],
    ws: &str,
    id: &str,
    secret: &str,
    now: u64,
) -> Result<Principal, ApiKeyError> {
    let presented_hash = lb_apikey::key_hash(pepper, secret);

    // 1. Hot path: a fresh cache entry with a matching hash is served without a store read.
    if let Some(principal) = cache.get(id, &presented_hash, now).await {
        return Ok(principal);
    }

    // 2. O(1) ws-scoped lookup. A key minted in another workspace simply is not in this namespace.
    let value = read(store, ws, TABLE, id)
        .await?
        .ok_or(ApiKeyError::NotFound)?;
    let record: ApiKeyRecord = serde_json::from_value(value).map_err(unexpected)?;

    // 3. Status + expiry BEFORE the secret compare would leak — but compare the secret constant-time
    //    regardless so a revoked/expired key reveals nothing about whether the secret was right.
    if record.is_revoked() {
        return Err(ApiKeyError::Revoked);
    }
    if record.is_expired(now) {
        return Err(ApiKeyError::Expired);
    }

    // 4. Constant-time HMAC verify of the secret field alone.
    if !lb_apikey::verify_hash(pepper, secret, &record.key_hash) {
        return Err(ApiKeyError::Invalid);
    }

    // 5. Resolve the key's caps (direct grants + role expansion — NO team edge) and build the
    //    verified principal via the dedicated for_key constructor (not the co-trust `routed` path).
    let mut caps: BTreeSet<String> = BTreeSet::new();
    resolve_subject_caps(store, ws, &Subject::Key(id.to_string()), &mut caps).await?;
    let principal = Principal::for_key(
        format!("key:{id}"),
        ws.to_string(),
        caps.into_iter().collect(),
    );

    // 6. Cache the verified principal (carrying the record's expiry so a cached entry cannot outlive
    //    the key). A wrong secret never reaches here.
    cache
        .put(
            id.to_string(),
            presented_hash,
            principal.clone(),
            now,
            record.expires_at,
        )
        .await;

    Ok(principal)
}

fn unexpected(e: serde_json::Error) -> ApiKeyError {
    ApiKeyError::Store(lb_store::StoreError::Decode(e.to_string()))
}
