//! `cached_connect` — a process-local warm-pool cache in front of `source::connect`
//! (federation-pool-cache scope). `connect()` builds a full connection pool (for Postgres/Timescale:
//! a TLS handshake + auth, measured at ~2,500 ms against a remote Timescale with a 137 ms RTT); the
//! query path called it **per query** and dropped the pool at the end of the call, so 98% of every
//! federated read's wall time was connect overhead, paid again for every tile on a dashboard.
//!
//! This caches the connected `Source` keyed on `(kind, dsn_hash)` for the child's lifetime.
//!
//! **Why this does not break "the child is stateless" (`main.rs` §3.4).** §3.4 forbids *durable*
//! state — anything a kill + respawn would lose that a caller depends on. A warm pool is not that:
//! every entry is reconstructible from the next call's own input (the host hands the DSN in every
//! call), it is invisible in results, and losing it on restart costs one slow query, never
//! correctness. The child stays restart-transparent.
//!
//! **The DSN is hashed, never stored raw.** A connection string in a long-lived map is exactly the
//! leak `datasources-scope.md` §155 forbids. The key is a SHA-256 of the DSN, so a changed DSN
//! misses naturally (and connects fresh) while the password never sits in process memory in a
//! readable form. `sha2` directly rather than `lb_telemetry::params_digest`: that crate pulls
//! `lb-store` (SurrealDB) + `lb-bus` (Zenoh), and a supervised sidecar must not link the datastore
//! to hash a string. The digest discipline is the same.
//!
//! **Concurrency (scope Risk 4).** The map `Mutex` is NEVER held across an `.await`. A cold key
//! installs a per-key `OnceCell` under the lock, releases it, and awaits the connect on that cell —
//! so two racers on one key build a single pool, and a slow connect to source A never blocks a
//! query to source B.
//!
//! **Eviction (scope Risk 3).** A cached pool that half-breaks (server restart, network partition)
//! would otherwise serve errors forever where per-call connect self-healed. `evict` is the explicit
//! escape hatch the timeout path and failed probes call; without it, caching is *worse* than the
//! per-call connect it replaces.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use sha2::{Digest, Sha256};
use tokio::sync::OnceCell;

use crate::source::{connect, Source, SourceError};

/// Max distinct sources held warm (scope Risk 2: unbounded growth retains sockets). A node with 16
/// live datasources is already unusual; past the cap the oldest-inserted entry is dropped, which
/// costs that source one slow reconnect and nothing else.
const MAX_ENTRIES: usize = 16;

/// A cache key: the source kind plus a SHA-256 of the DSN. Never the DSN itself.
type CacheKey = (String, String);

/// One slot. The `OnceCell` is installed under the map lock and awaited outside it, so the connect
/// happens exactly once per key without serialising unrelated sources.
type Slot = Arc<OnceCell<Result<Arc<dyn Source>, String>>>;

struct Cache {
    slots: HashMap<CacheKey, Slot>,
    /// Insertion order, for the cap eviction. Front is oldest.
    order: Vec<CacheKey>,
}

fn cache() -> &'static Mutex<Cache> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    CACHE.get_or_init(|| {
        Mutex::new(Cache {
            slots: HashMap::new(),
            order: Vec::new(),
        })
    })
}

/// Hash a DSN for use as a cache key. SHA-256 hex — a changed DSN yields a different key and so
/// misses naturally; no invalidation hook is needed (and none would be right: the child does not
/// serve `datasource.save`, so coupling it to that host verb would be a layering break).
fn dsn_hash(dsn: &str) -> String {
    let digest = Sha256::digest(dsn.as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// Whether `(kind, dsn)` is currently warm. Used by the query path to report `cache: hit|miss`
/// **before** the connect, so the event says what actually happened rather than what is true after.
pub fn is_warm(kind: &str, dsn: &str) -> bool {
    let key = (kind.to_string(), dsn_hash(dsn));
    let guard = cache().lock().expect("pool cache mutex");
    guard
        .slots
        .get(&key)
        .and_then(|s| s.get())
        .is_some_and(|r| r.is_ok())
}

/// Get a warm `Source` for `(kind, dsn)`, connecting once on a miss.
///
/// A connect that FAILS is not retained: the slot is evicted before returning, so a transient
/// outage does not pin an error into the cache for the child's lifetime.
pub async fn cached_connect(kind: &str, dsn: &str) -> Result<Arc<dyn Source>, SourceError> {
    let key = (kind.to_string(), dsn_hash(dsn));

    // Phase 1 — install-or-take the slot under the lock. No `.await` inside this scope.
    let slot: Slot = {
        let mut guard = cache().lock().expect("pool cache mutex");
        if let Some(existing) = guard.slots.get(&key) {
            existing.clone()
        } else {
            // Cap before insert so the map never exceeds MAX_ENTRIES.
            while guard.order.len() >= MAX_ENTRIES {
                let oldest = guard.order.remove(0);
                guard.slots.remove(&oldest);
            }
            let slot: Slot = Arc::new(OnceCell::new());
            guard.slots.insert(key.clone(), slot.clone());
            guard.order.push(key.clone());
            slot
        }
    };

    // Phase 2 — await the connect OUTSIDE the map lock. Racers on the same key share one connect;
    // a different key proceeds fully in parallel.
    let result = slot
        .get_or_init(|| async {
            connect(kind, dsn)
                .await
                .map(Arc::from)
                .map_err(|e| e.to_string())
        })
        .await;

    match result {
        Ok(source) => Ok(source.clone()),
        Err(e) => {
            // Don't cache a failure — the next call gets a fresh attempt.
            evict(kind, dsn);
            Err(SourceError(e.clone()))
        }
    }
}

/// Drop the cached entry for `(kind, dsn)`, so the next call reconnects.
///
/// Called on a query timeout (a pool that hung is suspect) and on a failed probe. This is the path
/// that keeps a poisoned pool from outliving the fault — scope Risk 3.
pub fn evict(kind: &str, dsn: &str) {
    let key = (kind.to_string(), dsn_hash(dsn));
    let mut guard = cache().lock().expect("pool cache mutex");
    guard.slots.remove(&key);
    guard.order.retain(|k| k != &key);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Distinct DSNs hash to distinct keys, and the same DSN is stable across calls — the property
    /// the whole cache rests on (a changed DSN must MISS, not serve the old pool).
    #[test]
    fn dsn_hash_is_stable_and_distinct() {
        assert_eq!(dsn_hash("postgres://a"), dsn_hash("postgres://a"));
        assert_ne!(dsn_hash("postgres://a"), dsn_hash("postgres://b"));
    }

    /// The hash never contains the DSN (nor a password inside it) — secret mediation, §155.
    #[test]
    fn dsn_hash_leaks_nothing() {
        let h = dsn_hash("postgres://user:hunter2@host:5432/db");
        assert!(!h.contains("hunter2"));
        assert!(!h.contains("host"));
        assert!(!h.contains("postgres"));
        assert_eq!(h.len(), 64, "sha-256 hex");
    }
}
