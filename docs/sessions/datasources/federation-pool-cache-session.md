# Session: federation pool cache + query observability

**Scope:** `docs/scope/datasources/federation-pool-cache-scope.md` · **Status:** implemented, unit-tested; **live verification pending**

Implements the three changes the scope called for, all inside `crates/federation/`. No core crate
touched, no verb shape changed.

## What shipped

**1. Warm-pool cache — `src/pool.rs` (new).**
`source::connect` now returns `Arc<dyn Source>` instead of `Box<dyn Source>`, so one connected
source can be shared across calls; all 7 call sites keep working through `.as_ref()`. The cache is a
process-local map keyed on `(kind, sha256(dsn))` — **the DSN is hashed, never stored raw** (§155).
Capped at 16 entries with oldest-first eviction (Risk 2).

Concurrency (Risk 4) is handled by installing a per-key `tokio::sync::OnceCell` under the map
`Mutex`, releasing the lock, then awaiting the connect on that cell. The `Mutex` is never held
across an `.await`, so racers on one key build a single pool and a slow connect to source A cannot
block source B. A connect that *fails* is evicted rather than retained, so a transient outage does
not pin an error for the child's lifetime.

**2. Query timeout — `src/query.rs`.**
`run_query_with(kind, dsn, sql, source_name, timeout)` wraps connect + execute in
`tokio::time::timeout`; `run_query` delegates with `DEFAULT_QUERY_TIMEOUT` (30 s). On elapse the
cache entry is **evicted** (Risk 3 — without this, caching is strictly worse than the per-call
connect it replaces) and a typed error is returned.

`probe` deliberately **bypasses** the cache and uses a fresh `connect` — one of the scope's open
questions. Resolved in favour of the scope's own leaning: a probe that reuses a cached pool no
longer proves a new connection can be established, which is the entire question `datasource.test`
is asked. It also evicts on failure, so a probe doubles as a manual "drop what you're holding" lever.

**3. Structured events — `src/event.rs` (new).**
One JSON line per query on **stderr** (`{evt, source, kind, cache, sql_digest, elapsed_ms, rows,
outcome}`), which the supervisor's existing `stderr(Stdio::inherit())` already carries to the host
console. The crate previously had zero logging across 3,199 lines. The DSN never appears; SQL
appears only as `sql_digest` (SHA-256 prefix + length).

`main.rs` now reads the optional `source` field from `federation.query` input and passes it through
as an opaque label for the event.

## Decisions worth recording

**`sha2` directly rather than `lb_telemetry::params_digest`.** The scope named `params_digest` as
the helper to route SQL through. Rejected on inspection: `lb-telemetry` depends on `lb-store`
(SurrealDB) and `lb-bus` (Zenoh), and linking the datastore + message bus into a supervised sidecar
to hash a string is a much worse trade than the 20 lines of `sha2` it saves. The redaction
discipline is identical and documented in both new files.

**Risk 1 — the "stateless child" header.** `main.rs`'s §3.4 header now distinguishes *durable* state
(forbidden) from a warm pool (a cache, reconstructible from the next call's own input, invisible in
results, harmlessly lost on restart). Written out explicitly, as the scope required, so the next
reader does not correctly file this as a violation.

## Testing

`crates/federation/tests/pool_cache_test.rs` (new) covers the scope's §Testing plan. Full crate
suite: **31 passed, 0 failed** (twice — bin + integration target). Postgres feature build is green.

Per repo rule, every test was verified by breaking the code it covers:

| Break | Test | Result |
|---|---|---|
| Remove `tokio::time::timeout` | `timeout_fires_and_evicts…` | ✅ red — returned rows past its bound |
| Remove `evict` on timeout arm | `timeout_fires_and_evicts…` | ✅ red — retained a poisoned entry |
| `cached_connect` → `connect` | `second_query_hits_the_cache` | ✅ red — pool not warm after first query |

**Two false greens were caught and fixed during this session** — both worth recording, because both
would have shipped a test that could never fail:

1. The obvious timeout test used an unroutable Postgres address (`postgres://…@192.0.2.1`). It
   passed — but `postgres` is a cargo *feature*, so in the default build `connect` returned "not
   built in" **instantly**: `elapsed_ms: 0`, timeout arm never reached. It would have stayed green
   with the timeout deleted entirely. Rewritten against SQLite (always compiled in) so the bound is
   raced against real engine work.
2. The eviction assertion was vacuous on a cold start: a bound that elapses *during* the initial
   connect leaves nothing cached, so `is_warm` read false whether or not the timeout arm evicted.
   Confirmed by breaking it — the test stayed green. Fixed by warming the pool first, which is also
   the real Risk 3 shape: a pool that worked, then broke, and must not be retained.

Both are commented in the test file so the reasoning survives.

## Not verified yet — live

The measurement this scope exists for (**~2,530 ms → target ~150–250 ms** against `pdnsw`) has
**not** been confirmed against a running node. Unit tests prove the cache is *populated and reused*;
they cannot prove the latency win, because local SQLite's connect cost is milliseconds. That
verification is the next step and is being handed to the user.

## Open questions still open

- **Timeout default (30 s).** Unchanged from the scope's guess. A dashboard tile would rather fail
  at ~5 s; the per-call override exists, but nothing sets it yet. Wants a real decision once the
  live numbers land.
- **Split connect vs. query bounds.** Still one bound covering both.
- **`eprintln!`-JSON vs. a real subscriber in the child.** Shipped the former, as scoped.
