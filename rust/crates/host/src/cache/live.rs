//! The live hot-tier cache — moka in-process, byte-weighted, TTL-bounded, single-flight
//! (response-cache scope, Intent §5). Compiled only under the `page-cache` feature; a feature-off
//! build never sees `moka`.
//!
//! A lookup goes through moka's `entry(...).or_try_insert_with(init)`: the first caller of a cold
//! key computes (the `init` future runs the real dispatch); concurrent identical callers COALESCE
//! onto that one computation and share its result — one engine dispatch under any fan-in (the
//! many-viewers open and the post-write recompute burst both collapse to one). The key carries the
//! `{ws, class}` generation, so a write that bumps the generation makes every prior entry
//! unreachable without a scan.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use lb_mcp::ToolError;
use moka::notification::RemovalCause;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::config::CacheConfig;
use super::generation::Generations;
use super::policy::Class;

/// The node's response cache: the moka hot tier + the generation map + the observability counters.
/// Held as `Arc<ResponseCache>` on the `Node` and shared across every dispatch.
pub struct ResponseCache {
    moka: moka::future::Cache<String, Arc<str>>,
    generations: Generations,
    stats: Stats,
}

impl ResponseCache {
    /// Build the cache from config. `max_capacity` is the WEIGHTED budget (bytes) because a weigher
    /// is set; TinyLFU eviction keeps the weighted size under it. A single global TTL is correct for
    /// v1 (every cached class — the five lists — shares the list TTL); a per-entry `Expiry` arrives
    /// with the time-windowed `viz.query` follow-up.
    pub fn new(cfg: &CacheConfig) -> Self {
        let evictions = Arc::new(AtomicU64::new(0));
        let ev = evictions.clone();
        let moka = moka::future::Cache::builder()
            .max_capacity(cfg.memory_budget_bytes)
            // Weigh key + serialised value bytes so the budget is HONEST about real footprint
            // (Risks: "a weigher that under-counts blows the small-RAM budget").
            .weigher(|k: &String, v: &Arc<str>| (k.len() + v.len()).min(u32::MAX as usize) as u32)
            .time_to_live(Duration::from_secs(cfg.list_ttl_secs.max(1)))
            .eviction_listener(move |_k: Arc<String>, _v: Arc<str>, cause: RemovalCause| {
                // Count only capacity/TTL evictions — an explicit `invalidate` (none in v1) or a
                // replace is not an eviction the operator cares about for budget tuning.
                if matches!(cause, RemovalCause::Size | RemovalCause::Expired) {
                    ev.fetch_add(1, Ordering::Relaxed);
                }
            })
            .build();
        Self {
            moka,
            generations: Generations::default(),
            stats: Stats::new(evictions),
        }
    }

    /// Bump every class this write dirties. Called by the seam AFTER a write returns `Ok` — the
    /// invalidation lands the moment the write does.
    pub fn bump(&self, ws: &str, class: Class) {
        self.generations.bump(ws, class);
    }

    /// Purge a workspace — bump ALL of its class generations (the operator escape hatch,
    /// `cache.purge`). Existing entries become unreachable immediately and free on TTL/eviction;
    /// other workspaces are untouched.
    pub fn purge(&self, ws: &str) {
        self.generations.bump_all(ws);
    }

    /// The read-through, single-flight lookup for a cacheable read. `init` computes the serialised
    /// response on a miss; concurrent identical misses coalesce onto one `init`.
    pub async fn get_or_compute<F>(
        &self,
        ws: &str,
        verb: &str,
        args: &Value,
        class: Class,
        init: F,
    ) -> Result<String, ToolError>
    where
        F: std::future::Future<Output = Result<String, ToolError>>,
    {
        let generation = self.generations.current(ws, class);
        let key = build_key(ws, verb, args, generation);
        let init = async move { init.await.map(|s| Arc::<str>::from(s.into_boxed_str())) };
        match self.moka.entry(key).or_try_insert_with(init).await {
            Ok(entry) => {
                if entry.is_fresh() {
                    self.stats.record_miss(class);
                } else {
                    self.stats.record_hit(class);
                }
                Ok(entry.into_value().as_ref().to_owned())
            }
            // moka wraps the init error in an `Arc`; unwrap it back to the plain `ToolError` (errors
            // are never stored — only `Ok` values populate the cache, so a failing read never sticks).
            Err(arc) => Err((*arc).clone()),
        }
    }

    /// A snapshot of the cache's observability counters — the `cache.stats` payload. `run_pending_tasks`
    /// first so `entry_count`/`weighted_size` reflect just-applied inserts/evictions.
    pub async fn stats_snapshot(&self) -> Value {
        self.moka.run_pending_tasks().await;
        self.stats
            .snapshot(self.moka.entry_count(), self.moka.weighted_size())
    }
}

/// Canonicalise + hash a call into a stable cache key: `{ws}\x1f{verb}\x1f{generation}\x1f{hash}`.
/// The hash is over the canonical (recursively key-sorted) JSON of the args, so an argument object
/// with a different field ORDER — or the `preserve_order` serde feature toggled anywhere — produces
/// the SAME key (Intent §4: "key-by-JSON with unstable field order is a silent 100% miss").
fn build_key(ws: &str, verb: &str, args: &Value, generation: u64) -> String {
    let canon = canonical_json(args);
    let mut h = Sha256::new();
    h.update(canon.as_bytes());
    format!("{ws}\u{1f}{verb}\u{1f}{generation}\u{1f}{:x}", h.finalize())
}

/// The canonical JSON string of `v` — object keys sorted recursively. `null`-vs-absent is preserved
/// (they are distinct args and must key distinctly); only ORDER is normalised.
fn canonical_json(v: &Value) -> String {
    canonicalize(v).to_string()
}

fn canonicalize(v: &Value) -> Value {
    match v {
        Value::Object(m) => {
            // Collect into a BTreeMap so iteration (and thus the rebuilt `Map`) is key-sorted,
            // regardless of whether serde_json's `Map` is a BTreeMap or an insertion-ordered IndexMap.
            let sorted: std::collections::BTreeMap<&String, Value> =
                m.iter().map(|(k, val)| (k, canonicalize(val))).collect();
            Value::Object(
                sorted
                    .into_iter()
                    .map(|(k, val)| (k.clone(), val))
                    .collect(),
            )
        }
        Value::Array(a) => Value::Array(a.iter().map(canonicalize).collect()),
        other => other.clone(),
    }
}

/// The observability counters behind `cache.stats`. `evictions` is an `Arc` shared with moka's
/// eviction listener; the rest are updated on the lookup path.
struct Stats {
    hits: AtomicU64,
    misses: AtomicU64,
    evictions: Arc<AtomicU64>,
    per_class: DashMap<Class, (u64, u64)>,
}

impl Stats {
    fn new(evictions: Arc<AtomicU64>) -> Self {
        Self {
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            evictions,
            per_class: DashMap::new(),
        }
    }

    fn record_hit(&self, class: Class) {
        self.hits.fetch_add(1, Ordering::Relaxed);
        self.per_class.entry(class).or_insert((0, 0)).0 += 1;
    }

    fn record_miss(&self, class: Class) {
        self.misses.fetch_add(1, Ordering::Relaxed);
        self.per_class.entry(class).or_insert((0, 0)).1 += 1;
    }

    fn snapshot(&self, entry_count: u64, weighted_size: u64) -> Value {
        let per_class: Vec<Value> = self
            .per_class
            .iter()
            .map(|e| {
                let (hits, misses) = *e.value();
                json!({ "class": class_name(*e.key()), "hits": hits, "misses": misses })
            })
            .collect();
        json!({
            "enabled": true,
            "hits": self.hits.load(Ordering::Relaxed),
            "misses": self.misses.load(Ordering::Relaxed),
            "evictions": self.evictions.load(Ordering::Relaxed),
            "entry_count": entry_count,
            "weighted_size_bytes": weighted_size,
            "per_class": per_class,
        })
    }
}

/// The stable wire name of a class for the stats breakdown.
fn class_name(c: Class) -> &'static str {
    match c {
        Class::Datasource => "datasource",
        Class::Series => "series",
        Class::Flows => "flows",
        Class::Ext => "ext",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_key_is_field_order_independent() {
        let a = json!({ "b": 1, "a": 2, "nested": { "y": 1, "x": 2 } });
        let b = json!({ "nested": { "x": 2, "y": 1 }, "a": 2, "b": 1 });
        assert_eq!(build_key("ws", "v", &a, 0), build_key("ws", "v", &b, 0));
    }

    #[test]
    fn generation_changes_the_key() {
        let a = json!({ "x": 1 });
        assert_ne!(build_key("ws", "v", &a, 0), build_key("ws", "v", &a, 1));
    }

    #[test]
    fn workspace_changes_the_key() {
        let a = json!({ "x": 1 });
        assert_ne!(build_key("wsA", "v", &a, 0), build_key("wsB", "v", &a, 0));
    }

    #[test]
    fn null_and_absent_key_distinctly() {
        let with_null = json!({ "x": null });
        let absent = json!({});
        assert_ne!(
            build_key("ws", "v", &with_null, 0),
            build_key("ws", "v", &absent, 0)
        );
    }
}
