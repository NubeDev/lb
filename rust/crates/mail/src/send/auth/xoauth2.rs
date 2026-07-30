//! [`TokenCache`] + [`access_token`] — a *fresh* bearer at send time, without a refresh per message.
//!
//! An access token lives about an hour; the outbox may relay a dozen effects a minute. So the token is
//! cached per grant (endpoint + client + refresh-token fingerprint) and re-minted only when it is
//! within [`EXPIRY_SKEW_SECS`] of expiring — the skew exists because a token that expires *between*
//! our check and the server's check produces a baffling `535` instead of a refresh.
//!
//! The cache holds a token VALUE in memory, which is the one place this crate keeps a secret at all.
//! It is process-local, never serialized, and keyed by a fingerprint rather than the refresh token
//! itself (see [`RefreshRequest::cache_key`]) — so rotating the sealed refresh token invalidates the
//! cached access token instead of silently reusing one minted from the old grant.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::refresh::{refresh_access_token, RefreshRequest};
use crate::error::MailResult;

/// Refresh this many seconds before the stated expiry (see the module note).
pub const EXPIRY_SKEW_SECS: u64 = 60;

/// The lifetime assumed when the token endpoint states none. Conservative on purpose: an extra
/// refresh is cheap, a stale token is a failed send.
pub const DEFAULT_TOKEN_TTL_SECS: u64 = 300;

/// A process-local access-token cache, one per provider instance.
#[derive(Default)]
pub struct TokenCache {
    entries: Mutex<HashMap<String, Entry>>,
}

struct Entry {
    access_token: String,
    /// When this token stops being usable (already skew-adjusted).
    good_until: Instant,
}

impl TokenCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// The cached token for `key`, if still fresh.
    fn fresh(&self, key: &str) -> Option<String> {
        let entries = self.entries.lock().ok()?;
        let entry = entries.get(key)?;
        (entry.good_until > Instant::now()).then(|| entry.access_token.clone())
    }

    fn store(&self, key: String, access_token: String, ttl_secs: u64) {
        // Skew-adjust, floored at zero: a token whose stated lifetime is under the skew is cached as
        // already-expired rather than as "valid forever" (a saturating_sub bug here would pin a dead
        // token in the cache for the life of the process).
        let usable = ttl_secs.saturating_sub(EXPIRY_SKEW_SECS);
        let good_until = Instant::now() + Duration::from_secs(usable);
        if let Ok(mut entries) = self.entries.lock() {
            entries.insert(
                key,
                Entry {
                    access_token,
                    good_until,
                },
            );
        }
    }

    /// Drop the cached token for this grant — used when the relay rejects a token we believed fresh.
    pub fn invalidate(&self, req: &RefreshRequest) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.remove(&req.cache_key());
        }
    }
}

/// A fresh access token for `req`: the cached one when it is still good, else one refresh exchange.
///
/// A refresh failure propagates its classification unchanged — `invalid_grant` is permanent (an
/// operator must redo the consent ceremony), a `5xx`/network failure is transient (the outbox backs
/// off and the mail is still deliverable).
pub async fn access_token(
    cache: &TokenCache,
    client: &reqwest::Client,
    req: &RefreshRequest,
) -> MailResult<String> {
    let key = req.cache_key();
    if let Some(token) = cache.fresh(&key) {
        return Ok(token);
    }
    let response = refresh_access_token(client, req).await?;
    let ttl = response.expires_in.unwrap_or(DEFAULT_TOKEN_TTL_SECS);
    cache.store(key, response.access_token.clone(), ttl);
    tracing::debug!(
        endpoint = %req.token_endpoint,
        ttl_secs = ttl,
        "mail: minted a fresh access token"
    );
    Ok(response.access_token)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req() -> RefreshRequest {
        RefreshRequest {
            token_endpoint: "https://example/token".into(),
            client_id: "cid".into(),
            client_secret: "csecret".into(),
            refresh_token: "refresh-token-value".into(),
        }
    }

    #[test]
    fn a_stored_token_is_reused_until_the_skew_window() {
        let cache = TokenCache::new();
        let key = req().cache_key();
        cache.store(key.clone(), "at-1".into(), 3600);
        assert_eq!(cache.fresh(&key).as_deref(), Some("at-1"));
    }

    #[test]
    fn a_token_shorter_than_the_skew_is_never_considered_fresh() {
        // The trap this guards: a saturating_sub that floors at 0 must yield an EXPIRED entry, not one
        // that lives forever. `expires_in: 30` with a 60s skew has no usable window at all.
        let cache = TokenCache::new();
        let key = req().cache_key();
        cache.store(key.clone(), "at-2".into(), 30);
        assert_eq!(cache.fresh(&key), None);
    }

    #[test]
    fn invalidate_drops_the_grants_token() {
        let cache = TokenCache::new();
        let r = req();
        cache.store(r.cache_key(), "at-3".into(), 3600);
        cache.invalidate(&r);
        assert_eq!(cache.fresh(&r.cache_key()), None);
    }
}
