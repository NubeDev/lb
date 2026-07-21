//! `cached_query` — a TTL-bounded, process-local **query-result** cache in front of the federated
//! read (federation-result-cache scope). Sibling to `pool.rs`, and the third and last layer of the
//! same campaign: the pool cache killed the per-call connect (2,530 ms → ~150 ms warm), the
//! concurrency scope killed the transport queue (13 queries: 12.68 s → 1.85 s), and what remained
//! was the query itself — ~0.9 s per warm remote read, paid again for an answer this child computed
//! seconds ago, once per tile per viewer per refresh tick.
//!
//! **Opt-in, never assumed.** A call caches only when the host threads a caller-declared freshness
//! window (`cache: {ttl_s}`). No field, `ttl_s: 0`, or `LB_FEDERATION_RESULT_CACHE=off` in the
//! child's environment → [`cached_query`] runs the inner future and stores nothing, so the default
//! path is today's behaviour bit for bit. Only the surface that knows its own refresh contract
//! (the dashboard page) may declare staleness; nothing in lb core invents a TTL.
//!
//! **Why this is not durable state (`main.rs` §3.4).** Same resolution the pool cache already
//! wrote into that header: every entry is reconstructible from the next call's own input, it is
//! invisible in results (a hit returns exactly the rows the query would have returned), and a kill
//! + respawn costs one slow query, never correctness. SurrealDB holds state; a cache is motion.
//!
//! **The key.** `(kind, sha256(dsn), sha256(canonical(args)))`, where `args` is the child-received
//! input minus `cache` and `dsn` — so every field the child actually receives participates in
//! identity automatically. Canonicalization is deterministic (recursively sorted object keys, nulls
//! dropped). The DSN is hashed with the same discipline `pool.rs` uses and **never stored raw**
//! (`datasources-scope.md` §155). Note the mechanism's real limit, which is a review rule and not a
//! guarantee this file can make: the child's input is HOST-ENUMERATED
//! (`host/src/federation/query.rs`), so a future result-shaping field reaches neither the query nor
//! this key unless the host verb threads it through.
//!
//! **The slot (scope Risk 4 — dogpile and racing refreshers).** `pool.rs`'s `OnceCell` shape does
//! NOT transfer: `OnceCell` is set-once, which fits a connection that never refreshes, but a result
//! slot must REFILL on expiry — and because the TTL is caller-relative, so is "expired". Hence
//! `{ current, inflight }` with four rules:
//!
//!   1. A caller whose TTL **accepts** `current` returns it immediately. It NEVER waits on an
//!      in-flight refresh, even if a stricter caller started one.
//!   2. A caller whose TTL **rejects** `current` (or finds none) **joins** the in-flight refresh if
//!      one exists, else starts one and installs the shared handle under the map lock. Exactly one
//!      query per key runs at a time — which is cold-start single-flight (13 identical tiles → 1
//!      query) and the no-racing-refreshers rule, the same mechanism.
//!   3. A joiner may receive data FRESHER than its TTL required. Always acceptable — fresher than
//!      asked is never wrong; staler than asked is the bug class rule 2 prevents.
//!   4. Completion replaces `current` and clears `inflight` atomically under the map lock. A FAILED
//!      refresh clears `inflight` and leaves `current` untouched (the next rejecting caller retries;
//!      accepting callers were never blocked). That last rule is also what makes adding
//!      serve-stale-on-error later a small change rather than a redesign.
//!
//! The map `Mutex` is never held across an `.await` — same discipline as `pool.rs`.
//!
//! **Bounds (scope Risk 3).** Frames are the biggest objects this child handles, so the three caps
//! are load-bearing rather than tuning: [`MAX_RESULT_ENTRIES`], [`MAX_RESULT_BYTES`], and
//! [`MAX_ENTRY_BYTES`] (an over-cap result is served but not stored). Size is MEASURED on the
//! serialized envelope, never guessed. The byte cap is **per child**, and children are per
//! `(ws, ext_id)` — a node's worst case is [`MAX_RESULT_BYTES`] × active workspaces.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::sync::broadcast;

/// Max distinct cached results held per child.
pub const MAX_RESULT_ENTRIES: usize = 128;
/// Max total bytes of cached envelopes per child (see the per-workspace multiplication above).
pub const MAX_RESULT_BYTES: usize = 64 * 1024 * 1024;
/// A single result bigger than this is served but NOT cached — one tile must not consume the whole
/// budget. (The paging scopes exist so tiles don't fetch 4 MB in the first place.)
pub const MAX_ENTRY_BYTES: usize = 4 * 1024 * 1024;

/// The env kill-switch. Set to `off` (or `0`/`false`) on the NODE process — the child inherits the
/// environment at spawn (`supervisor/src/os.rs` uses `.envs(env)` with no `env_clear()`), so no
/// `init`-handshake threading is needed. When set, every call bypasses regardless of caller input.
pub const KILL_SWITCH_ENV: &str = "LB_FEDERATION_RESULT_CACHE";

/// How a query was resolved against the result cache — the `result_cache` field on the query event.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ResultCache {
    /// Served from memory. Carries `age_ms` on the event.
    Hit,
    /// Caching was requested but no acceptable entry existed; the query ran (possibly joined to an
    /// in-flight refresh) and the result was stored.
    Miss,
    /// Caching was not in play at all: no `ttl_s`, `ttl_s: 0`, or the kill-switch.
    Bypass,
}

impl ResultCache {
    pub fn as_str(self) -> &'static str {
        match self {
            ResultCache::Hit => "hit",
            ResultCache::Miss => "miss",
            ResultCache::Bypass => "bypass",
        }
    }
}

/// A cached result: the rows the caller would have received, plus what it costs and when it landed.
/// `Arc`-shared so a hit is a refcount bump, never a deep copy of a frame.
#[derive(Debug)]
pub struct Envelope {
    pub columns: Vec<String>,
    pub rows: Vec<Value>,
    /// Serialized size, MEASURED (scope Risk 3: "must measure the serialized envelope, not guess").
    pub size_bytes: usize,
}

impl Envelope {
    pub fn new(columns: Vec<String>, rows: Vec<Value>) -> Self {
        // Measure the JSON the caller would receive. `to_string().len()` on the pair is the honest
        // number for a rows-of-arrays payload; a struct-field guess undercounts badly.
        let size_bytes = serde_json::json!({ "columns": &columns, "rows": &rows })
            .to_string()
            .len();
        Envelope {
            columns,
            rows,
            size_bytes,
        }
    }
}

/// `(kind, dsn_hash, args_hash)`. Never the DSN, never the raw SQL.
type CacheKey = (String, String, String);

/// The shared handle a joiner waits on. `broadcast` rather than `Shared<Future>` because the
/// envelope is `Arc`-cheap to clone and the error is a `String`: every joiner gets the same outcome,
/// and a receiver that is dropped (caller cancelled) never blocks the sender.
type Inflight = broadcast::Sender<Result<Arc<Envelope>, String>>;

struct Slot {
    /// The last completed result and when it landed. `None` until the first refresh completes.
    current: Option<(Arc<Envelope>, Instant)>,
    /// A refresh running right now. Exactly one per key (rule 2).
    inflight: Option<Inflight>,
}

struct Cache {
    slots: HashMap<CacheKey, Slot>,
    /// Insertion order for the cap eviction. Front is oldest.
    order: Vec<CacheKey>,
    /// Running total of `current` envelope sizes — kept in step with `slots` on every mutation.
    bytes: usize,
}

fn cache() -> &'static Mutex<Cache> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    CACHE.get_or_init(|| {
        Mutex::new(Cache {
            slots: HashMap::new(),
            order: Vec::new(),
            bytes: 0,
        })
    })
}

/// SHA-256 hex of a DSN — the same discipline as `pool.rs::dsn_hash` (a connection string must
/// never sit in a long-lived map in readable form, §155).
fn dsn_hash(dsn: &str) -> String {
    let digest = Sha256::digest(dsn.as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// Canonicalize a JSON value deterministically: object keys recursively sorted, null-valued keys
/// dropped. Two inputs that differ only in key order or in an explicit-null field must hash the
/// same, or the cache silently double-stores; two that differ in any VALUE must not.
fn canonical(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut out = serde_json::Map::new();
            for k in keys {
                let v = &map[k];
                if v.is_null() {
                    continue;
                }
                out.insert(k.clone(), canonical(v));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(canonical).collect()),
        other => other.clone(),
    }
}

/// Hash the child-received input MINUS `cache` (not part of identity — it is the freshness contract,
/// compared at read time) and MINUS `dsn` (keyed separately via its own hash, so the raw string
/// never reaches this digest's input alongside anything reversible).
///
/// The `source` alias IS hashed, deliberately: two aliases over one DSN double-cache (wasteful,
/// harmless) and a rename invalidates naturally.
fn args_hash(input: &Value) -> String {
    let mut stripped = input.clone();
    if let Some(obj) = stripped.as_object_mut() {
        obj.remove("cache");
        obj.remove("dsn");
    }
    let digest = Sha256::digest(canonical(&stripped).to_string().as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// Read the caller's freshness window out of the child input, honouring the kill-switch.
///
/// Returns `None` (bypass) when: the switch is off, there is no `cache` object, or `ttl_s` is
/// absent/zero/negative. `ttl_s` may be fractional (a 0.5 s window is legitimate for a fast page).
pub fn requested_ttl(input: &Value) -> Option<std::time::Duration> {
    if kill_switched() {
        return None;
    }
    let ttl = input.get("cache")?.get("ttl_s")?.as_f64()?;
    if !(ttl > 0.0) || !ttl.is_finite() {
        return None;
    }
    Some(std::time::Duration::from_secs_f64(ttl))
}

/// Whether the node operator has forced bypass. Read per call rather than cached in a `OnceLock`:
/// the read is a cheap env lookup, and a `OnceLock` would make the switch untestable in-process
/// (every test in one binary would share the first value observed).
fn kill_switched() -> bool {
    match std::env::var(KILL_SWITCH_ENV) {
        Ok(v) => {
            let v = v.trim().to_ascii_lowercase();
            v == "off" || v == "0" || v == "false"
        }
        Err(_) => false,
    }
}

/// Run `query` for `(kind, dsn, input)` through the result cache, returning the envelope and how it
/// was resolved (plus `age_ms` on a hit, for the event).
///
/// `ttl` is the CALLER's window, compared at read time — so two pages with different refresh
/// intervals share one entry and each gets its own freshness contract: a 5 s page never accepts a
/// 30 s-old row just because a slower page stored it. `None` bypasses entirely: the future runs and
/// nothing is stored or read.
pub async fn cached_query<F, Fut>(
    kind: &str,
    dsn: &str,
    input: &Value,
    ttl: Option<std::time::Duration>,
    query: F,
) -> (Result<Arc<Envelope>, String>, ResultCache, Option<u128>)
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<Envelope, String>>,
{
    let Some(ttl) = ttl else {
        let out = query().await.map(Arc::new);
        return (out, ResultCache::Bypass, None);
    };

    let key: CacheKey = (kind.to_string(), dsn_hash(dsn), args_hash(input));

    // Phase 1 — decide under the lock, with NO `.await` inside this scope. Three outcomes: serve
    // `current` (rule 1), join an in-flight refresh (rule 2), or become the refresher (rule 2).
    enum Action {
        Serve(Arc<Envelope>, u128),
        Join(broadcast::Receiver<Result<Arc<Envelope>, String>>),
        Refresh(Inflight),
    }
    let action = {
        let mut guard = cache().lock().expect("result cache mutex");
        let slot = guard.slots.entry(key.clone()).or_insert_with(|| Slot {
            current: None,
            inflight: None,
        });
        let fresh_enough = slot
            .current
            .as_ref()
            .map(|(env, at)| (env.clone(), at.elapsed()))
            .filter(|(_, age)| *age <= ttl);
        match fresh_enough {
            // Rule 1: accepted — return immediately, never waiting on a stricter caller's refresh.
            Some((env, age)) => Action::Serve(env, age.as_millis()),
            None => match &slot.inflight {
                // Rule 2: rejected, someone is already refreshing — join it (rule 3: the answer may
                // be fresher than we asked; that is always fine).
                Some(tx) => Action::Join(tx.subscribe()),
                // Rule 2: rejected, nobody refreshing — become the refresher. Installing the sender
                // under this lock is what makes "exactly one query per key" hold.
                None => {
                    let (tx, _rx) = broadcast::channel(1);
                    slot.inflight = Some(tx.clone());
                    if !guard.order.contains(&key) {
                        guard.order.push(key.clone());
                    }
                    Action::Refresh(tx)
                }
            },
        }
    };

    match action {
        Action::Serve(env, age_ms) => (Ok(env), ResultCache::Hit, Some(age_ms)),

        Action::Join(mut rx) => {
            // Awaited OUTSIDE the map lock. A sender dropped without a send (the refresher panicked)
            // surfaces as a recv error — reported as a plain failure, not a hang.
            match rx.recv().await {
                Ok(result) => (result, ResultCache::Miss, None),
                Err(_) => (
                    Err("the in-flight refresh for this query ended without a result".to_string()),
                    ResultCache::Miss,
                    None,
                ),
            }
        }

        Action::Refresh(tx) => {
            // The one query. Also outside the map lock.
            let outcome = query().await;
            let shared: Result<Arc<Envelope>, String> = match outcome {
                Ok(env) => Ok(Arc::new(env)),
                Err(e) => Err(e),
            };

            // Rule 4 — install and clear atomically under the lock.
            {
                let mut guard = cache().lock().expect("result cache mutex");
                match &shared {
                    Ok(env) => {
                        if env.size_bytes <= MAX_ENTRY_BYTES {
                            let previous = guard
                                .slots
                                .get(&key)
                                .and_then(|s| s.current.as_ref())
                                .map(|(e, _)| e.size_bytes)
                                .unwrap_or(0);
                            guard.bytes = guard.bytes + env.size_bytes - previous;
                            if let Some(slot) = guard.slots.get_mut(&key) {
                                slot.current = Some((env.clone(), Instant::now()));
                                slot.inflight = None;
                            }
                            enforce_bounds(&mut guard, &key);
                        } else {
                            // Over the per-entry cap: served, not stored. The slot is dropped rather
                            // than left holding a stale `current` under a key that will never refill.
                            eprintln!(
                                "{}",
                                serde_json::json!({
                                    "evt": "federation.result_cache.too_large",
                                    "kind": kind,
                                    "size_bytes": env.size_bytes,
                                    "max_entry_bytes": MAX_ENTRY_BYTES,
                                })
                            );
                            remove(&mut guard, &key);
                        }
                    }
                    // A failed refresh clears `inflight` and leaves `current` untouched.
                    Err(_) => {
                        if let Some(slot) = guard.slots.get_mut(&key) {
                            slot.inflight = None;
                            if slot.current.is_none() {
                                remove(&mut guard, &key);
                            }
                        }
                    }
                }
            }

            // Wake the joiners. `send` errors only when nobody is subscribed — expected and fine.
            let _ = tx.send(shared.clone());
            (shared, ResultCache::Miss, None)
        }
    }
}

/// Drop a key entirely, keeping `order` and `bytes` in step. Every removal goes through here so the
/// byte accounting cannot drift from the map (a drifted counter silently disables the cap).
fn remove(guard: &mut Cache, key: &CacheKey) {
    if let Some(slot) = guard.slots.remove(key) {
        if let Some((env, _)) = slot.current {
            guard.bytes = guard.bytes.saturating_sub(env.size_bytes);
        }
    }
    guard.order.retain(|k| k != key);
}

/// Evict oldest-first until both caps hold. `keep` (the key just stored) is never evicted — evicting
/// it would make a just-completed refresh a guaranteed miss next call.
fn enforce_bounds(guard: &mut Cache, keep: &CacheKey) {
    while guard.order.len() > MAX_RESULT_ENTRIES || guard.bytes > MAX_RESULT_BYTES {
        let Some(oldest) = guard.order.iter().find(|k| *k != keep).cloned() else {
            break;
        };
        remove(guard, &oldest);
    }
}

/// Drop every cached result for `(kind, dsn)` — the write-through invalidation lever.
///
/// Called by `federation.write`, `federation.migrate`, and a failed `probe`. Without it the child
/// would serve rows it KNOWS it invalidated, which is the difference between a cache and a bug.
pub fn evict_source(kind: &str, dsn: &str) {
    let (k, h) = (kind.to_string(), dsn_hash(dsn));
    let mut guard = cache().lock().expect("result cache mutex");
    let doomed: Vec<CacheKey> = guard
        .slots
        .keys()
        .filter(|(sk, sh, _)| sk == &k && sh == &h)
        .cloned()
        .collect();
    for key in doomed {
        remove(&mut guard, &key);
    }
}

/// Test-only: how many results are currently cached. Not a public observable — the cache's contract
/// is the rows it returns, and every behavioural test asserts row CONTENT.
#[cfg(test)]
pub fn len() -> usize {
    cache().lock().expect("result cache mutex").slots.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_of(marker: i64) -> Envelope {
        Envelope::new(vec!["id".to_string()], vec![serde_json::json!([marker])])
    }

    /// Canonicalization is order-insensitive and null-insensitive, but value-sensitive — the three
    /// properties the key rests on. A value-insensitive canon would serve one query's rows for
    /// another; that is the worst failure a datasource cache can have.
    #[test]
    fn canonical_is_deterministic_but_value_sensitive() {
        let a = serde_json::json!({ "sql": "SELECT 1", "source": "wh", "extra": null });
        let b = serde_json::json!({ "source": "wh", "sql": "SELECT 1" });
        assert_eq!(
            canonical(&a),
            canonical(&b),
            "key order + nulls must not matter"
        );
        let c = serde_json::json!({ "source": "wh", "sql": "SELECT 2" });
        assert_ne!(canonical(&a), canonical(&c), "a changed VALUE must matter");
    }

    /// The args hash ignores `cache` and `dsn` and leaks neither the DSN nor the SQL.
    #[test]
    fn args_hash_excludes_cache_and_dsn_and_leaks_nothing() {
        let base = serde_json::json!({
            "kind": "sqlite", "source": "wh", "sql": "SELECT secret FROM billing",
            "dsn": "postgres://user:hunter2@host/db",
        });
        let mut with_cache = base.clone();
        with_cache["cache"] = serde_json::json!({ "ttl_s": 30 });
        assert_eq!(
            args_hash(&base),
            args_hash(&with_cache),
            "the freshness contract is not part of query identity"
        );

        let mut other_dsn = base.clone();
        other_dsn["dsn"] = serde_json::json!("postgres://elsewhere");
        assert_eq!(
            args_hash(&base),
            args_hash(&other_dsn),
            "the dsn keys separately via its own hash"
        );

        let h = args_hash(&base);
        for forbidden in ["hunter2", "billing", "secret", "SELECT"] {
            assert!(!h.contains(forbidden), "args_hash leaked {forbidden}: {h}");
        }
        assert_eq!(h.len(), 64, "sha-256 hex");
    }

    /// A changed SQL — including one differing only in a paging cursor — is a different key.
    #[test]
    fn args_hash_separates_paging_cursors() {
        let one = serde_json::json!({ "sql": "SELECT a FROM t LIMIT 10 OFFSET 0" });
        let two = serde_json::json!({ "sql": "SELECT a FROM t LIMIT 10 OFFSET 10" });
        assert_ne!(args_hash(&one), args_hash(&two));
    }

    /// The kill-switch and the absent/zero field all resolve to bypass; a real window resolves to a
    /// duration. Fractional TTLs are honoured.
    #[test]
    fn requested_ttl_reads_the_contract() {
        assert!(requested_ttl(&serde_json::json!({ "sql": "x" })).is_none());
        assert!(requested_ttl(&serde_json::json!({ "cache": { "ttl_s": 0 } })).is_none());
        assert!(requested_ttl(&serde_json::json!({ "cache": { "ttl_s": -5 } })).is_none());
        assert_eq!(
            requested_ttl(&serde_json::json!({ "cache": { "ttl_s": 30 } })),
            Some(std::time::Duration::from_secs(30))
        );
        assert_eq!(
            requested_ttl(&serde_json::json!({ "cache": { "ttl_s": 0.5 } })),
            Some(std::time::Duration::from_millis(500))
        );
    }

    /// The envelope's size is measured from the serialized payload, and it grows with the rows.
    #[test]
    fn envelope_measures_its_serialized_size() {
        let small = env_of(1);
        let big = Envelope::new(
            vec!["id".to_string()],
            (0..1000).map(|i| serde_json::json!([i])).collect(),
        );
        assert!(small.size_bytes > 0);
        assert!(
            big.size_bytes > small.size_bytes * 100,
            "size must track the real payload: {} vs {}",
            big.size_bytes,
            small.size_bytes
        );
    }
}
