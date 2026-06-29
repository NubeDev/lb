//! [`ApiKeyCache`] — the small hash→`Principal` cache that keeps the per-request auth path cheap
//! (api-keys scope, decision 1). On a cache HIT (same id, same presented hash, within the TTL, and
//! not past the record's `expires_at`) the auth path skips the store read + cap resolution entirely.
//! Revoke/rotate [`bust`](ApiKeyCache::bust) the entry by id so a tombstone bites on the very next
//! request **on this node** — instant local revoke, the mandatory integration property.
//!
//! Keyed by `id` and storing the verified hash + the built `Principal`: a WRONG secret for a valid
//! id produces a different hash, so the constant-time hash compare fails → cache miss → full verify
//! path → refuse. A correct secret is cached; a different secret is never served the cached
//! principal. The TTL is a small fixed constant (5s) + explicit bust-on-revoke; not configurable in
//! v1 (a resolved open question of the scope).

use std::collections::HashMap;

use lb_auth::Principal;
use tokio::sync::RwLock;

/// The cache TTL (seconds). Deliberately small + not configurable in v1: it bounds how long a
/// revoked key stays live on a peer that missed the local bust (sync + this TTL is the multi-node
/// correctness floor). On the revoking node the explicit [`bust`](ApiKeyCache::bust) is instant.
pub const TTL_SECS: u64 = 5;

/// One cached verification: the hash the secret hashed to, the resulting principal, when it was
/// inserted, and the record's `expires_at` (so a cached entry does not outlive the key's expiry —
/// the lazy-expiry check stays authoritative even across a cache hit).
struct Entry {
    hash: String,
    principal: Principal,
    inserted_at: u64,
    expires_at: u64,
}

/// The per-node API-key verification cache. Held on [`Node`](crate::Node) so the gateway's auth path
/// and the host's revoke/rotate verbs share one cache (revoke busts the entry the auth path reads).
#[derive(Default)]
pub struct ApiKeyCache {
    inner: RwLock<HashMap<String, Entry>>,
}

impl ApiKeyCache {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }

    /// Return the cached principal for `id` iff an entry exists, its hash matches `presented_hash`
    /// (constant-time), it is within the TTL at `now`, AND `now` has not passed the record's expiry.
    /// A wrong secret, a stale entry, or a since-expired key all miss.
    pub async fn get(&self, id: &str, presented_hash: &str, now: u64) -> Option<Principal> {
        let map = self.inner.read().await;
        let entry = map.get(id)?;
        if now.saturating_sub(entry.inserted_at) >= TTL_SECS {
            return None;
        }
        // A cached entry must NOT outlive the key's expiry — the lazy check stays authoritative.
        if entry.expires_at != 0 && now >= entry.expires_at {
            return None;
        }
        if lb_apikey::hash_matches(&entry.hash, presented_hash) {
            Some(entry.principal.clone())
        } else {
            None
        }
    }

    /// Insert/replace the cached principal for `id` (called after a successful full verification).
    pub async fn put(
        &self,
        id: String,
        hash: String,
        principal: Principal,
        inserted_at: u64,
        expires_at: u64,
    ) {
        let mut map = self.inner.write().await;
        map.insert(
            id,
            Entry {
                hash,
                principal,
                inserted_at,
                expires_at,
            },
        );
    }

    /// Drop the cached entry for `id` — the instant local revoke. Idempotent (a missing entry is a
    /// no-op). Called by `apikey.revoke` and `apikey.rotate`.
    pub async fn bust(&self, id: &str) {
        let mut map = self.inner.write().await;
        map.remove(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn principal(sub: &str) -> Principal {
        Principal::for_key(
            sub.to_string(),
            "acme".to_string(),
            vec!["store:*:read".into()],
        )
    }

    #[tokio::test]
    async fn a_put_then_get_within_ttl_hits() {
        let cache = ApiKeyCache::new();
        let hash = lb_apikey::key_hash(b"p", "secret");
        cache
            .put("k1".into(), hash.clone(), principal("key:k1"), 100, 0)
            .await;
        assert!(cache.get("k1", &hash, 100).await.is_some());
        assert!(cache.get("k1", &hash, 104).await.is_some()); // within 5s
    }

    #[tokio::test]
    async fn a_wrong_secret_misses_even_with_a_cached_entry() {
        let cache = ApiKeyCache::new();
        let good = lb_apikey::key_hash(b"p", "secret");
        let bad = lb_apikey::key_hash(b"p", "other");
        cache
            .put("k1".into(), good, principal("key:k1"), 100, 0)
            .await;
        assert!(cache.get("k1", &bad, 100).await.is_none());
    }

    #[tokio::test]
    async fn bust_removes_the_entry_immediately() {
        let cache = ApiKeyCache::new();
        let hash = lb_apikey::key_hash(b"p", "secret");
        cache
            .put("k1".into(), hash.clone(), principal("key:k1"), 100, 0)
            .await;
        assert!(cache.get("k1", &hash, 100).await.is_some());
        cache.bust("k1").await;
        assert!(cache.get("k1", &hash, 100).await.is_none()); // not after the TTL — immediately
    }

    #[tokio::test]
    async fn a_cached_entry_expires_at_the_record_expiry() {
        let cache = ApiKeyCache::new();
        let hash = lb_apikey::key_hash(b"p", "secret");
        // inserted at 100, expires at 103 — well within the 5s TTL so staleness is NOT the cause.
        cache
            .put("k1".into(), hash.clone(), principal("key:k1"), 100, 103)
            .await;
        assert!(cache.get("k1", &hash, 102).await.is_some());
        assert!(cache.get("k1", &hash, 103).await.is_none()); // now == expires_at → miss → re-verify → refuse
    }

    #[tokio::test]
    async fn an_entry_stales_after_the_ttl() {
        let cache = ApiKeyCache::new();
        let hash = lb_apikey::key_hash(b"p", "secret");
        cache
            .put("k1".into(), hash.clone(), principal("key:k1"), 100, 0)
            .await;
        assert!(cache.get("k1", &hash, 105).await.is_none()); // >= 5s → stale
    }
}
