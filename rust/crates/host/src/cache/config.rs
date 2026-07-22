//! `CacheConfig` — the additive, feature-independent knobs an embedder sets on
//! `BootConfig.cache` to turn the optional server-side response cache on and bound its memory
//! (response-cache scope). This type is **always compiled**, whether or not the `page-cache`
//! cargo feature is on: it is plain data (no `moka`), so `BootConfig.cache: Option<CacheConfig>`
//! stays a legal field in a feature-off build. Only the LIVE cache (`super::ResponseCache`) is
//! feature-gated — a feature-off node simply ignores this config (`install_response_cache` is a
//! no-op), which is exactly the zero-cost seam the scope requires.

/// Runtime configuration for the optional response cache.
///
/// `#[non_exhaustive]` so future knobs (per-class TTL overrides, the v2 warm-tier budget) are
/// additive for every embedder — construct with [`CacheConfig::default`] then mutate, never a
/// cross-crate struct literal, exactly as `BootConfig` itself is constructed.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct CacheConfig {
    /// The master **runtime** switch. `false` ⇒ the cache is not built and every call passes
    /// straight through (this is where the `RUBIX_CACHE_ENABLED=0` kill-switch lands). Distinct
    /// from the compile-time `page-cache` feature: feature-off removes the code entirely;
    /// `enabled:false` is the runtime off on a build that DID compile it in.
    pub enabled: bool,
    /// The byte budget for the in-process hot tier — moka's weighted capacity. The weigher counts
    /// the serialised response bytes (plus the key), so eviction (TinyLFU) keeps RSS bounded on a
    /// small-RAM edge node. Default 32 MiB (the Pi posture).
    pub memory_budget_bytes: u64,
    /// TTL for the list class (`datasource.list`, `series.list`, `flows.list`, `flows.get`,
    /// `ext.list`) in seconds — the backstop bound on staleness for a datasource-backed read that
    /// no generation bump covers (an external sqlite writer, sidecar liveness). Default 60 s.
    pub list_ttl_secs: u64,
}

impl Default for CacheConfig {
    /// The shipped-on posture a product host (rubix-ai) wants: enabled, 32 MiB, 60 s lists. An
    /// embedder that leaves `BootConfig.cache = None` gets no cache at all; setting
    /// `Some(CacheConfig::default())` turns it on with these bounds.
    fn default() -> Self {
        Self {
            enabled: true,
            memory_budget_bytes: 32 * 1024 * 1024,
            list_ttl_secs: 60,
        }
    }
}
