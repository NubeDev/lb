# Federation pool cache + query observability

**Topic:** `datasources` · **Name:** `federation-pool-cache` · **Status:** scoped

`run_query` opens a **new connection pool on every call** and drops it when the call ends. For
SQLite that costs milliseconds. For a remote Postgres/Timescale it costs **seconds** — and it is now
the dominant term in every federated read.

This scope is the one `federation-pushdown-scope.md` §Non-goals pre-authorised:

> *"Connection/pool reuse across calls. `run_query` reconnects per call today (~ms for SQLite, more
> for Postgres). Real, but a separate, stateful concern — the sidecar is supervised and currently
> stateless per call; **a pool cache is its own small scope if the per-call connect ever dominates.**"*

It dominates. Measured against a live Timescale (`timescale-pdnsw`, ~137 ms RTT):

| Path | Measured | Notes |
|---|---|---|
| `psql` direct, warm session | **130 ms** | the network floor — pure RTT |
| `psql` cold connect + query | ~1,200 ms | TLS handshake + auth, several round-trips |
| `federation.query` `SELECT 1` | **~2,530 ms** | 10 runs, range 2,425–2,606 ms, no drift |
| same, local SQLite control | 20–22 ms warm | first call 865 ms — same effect, smaller |

**~2,500 ms of a ~2,530 ms query is connect overhead — 98% of wall time**, and it is paid *again*
for every tile on a dashboard. The flat distribution across 10 runs is the proof: a reused pool
would have amortised it after the first call.

A second finding rides along. The crate has **no timeout on any query path** (one 5 s SQLite *pool*
timeout at `source/sqlite.rs:51`; nothing on the Postgres path, nothing in `query.rs`) and **no
logging at all** — zero `tracing`/`log`/`eprintln!` calls across 3,199 lines. During diagnosis an
unbounded query against this same remote hung for >2 minutes and wedged the child so hard that
**local SQLite queries also timed out**, until a restart. One slow remote source can currently
starve every source. That was invisible from outside, which is why observability is in this scope
rather than a later one.

## Goals

- **Reuse a connected `Source` across calls**, keyed by `(kind, dsn)`, so the connect cost is paid
  once per source per child lifetime instead of once per query.
- **Bound every query** with a configurable timeout, so a hung remote returns an error instead of
  occupying the child forever.
- **Emit structured, secret-safe events** on the connect/query paths — cache hit/miss, elapsed ms,
  row count, outcome — over the child's stderr, which the supervisor already inherits.
- Keep the existing `Source` trait, the pushdown wiring, and every verb's shape **unchanged**.

## Non-goals

- **Cross-source federation** — still single-source by design; unchanged here.
- **A durable/shared pool.** The cache is process-local to the child. A kill + respawn loses it and
  that is correct: the child stays restart-transparent (`main.rs` §3.4 "holds nothing durable" is
  about *durable* state, not a warm connection — see Risks).
- **Per-source pool tuning** (min/max connections, idle reaping). Take the connector's defaults;
  revisit only with evidence.
- **A metrics backend.** Events go to stderr in the child's existing channel. Wiring federation into
  `lb-telemetry`'s SurrealDB sink is a separate scope — the child is a *supervised OS process* and
  cannot reach the host's subscriber (see §How it fits).

## Intent / approach

**The cache.** `connect()` returns `Box<dyn Source>` today. Change it to `Arc<dyn Source>` and put a
process-local map behind it:

```
static SOURCES: OnceLock<Mutex<HashMap<CacheKey, Arc<dyn Source>>>>
```

`CacheKey` is `(kind, dsn_hash)` — **the DSN is hashed, never stored as a key**. A raw DSN in a
long-lived map is exactly the leak `datasources-scope.md` §155 forbids ("never to a rule, the page, a
record, or a log"). Hashing keeps the cache correct (a changed DSN misses naturally and connects
fresh) while keeping the password out of process memory in a readable form.

All 7 `connect()` call sites (`query.rs` ×4, `sample.rs`, `write.rs`, `migrate.rs`) keep working:
`Arc<dyn Source>` derefs the same way, so `source.as_ref()` is untouched.

**Invalidation** is by natural miss: edit a DSN → different hash → new entry. The stale pool is
dropped on eviction. No explicit `datasource.save` hook — that would couple the child to a host verb
it does not serve, and the miss path already gives the correct result.

**The timeout** wraps the query future (`tokio::time::timeout`), default 30 s, overridable per call.
On elapse: drop the future, evict the cache entry (a timed-out pool is suspect), return a typed
error. This is what stops one bad source starving the others.

**The events** are `eprintln!`-level structured lines (JSON) on stderr — deliberately *not* a
`tracing` subscriber in the child, because the supervisor's `stderr(Stdio::inherit())`
(`crates/supervisor/src/os.rs:37`) already carries them to the host console with no new plumbing.
Fields: `source` name, `kind`, `cache` hit/miss, `elapsed_ms`, `rows`, `outcome`. **Never** the DSN,
**never** raw SQL — SQL goes through `lb_telemetry::params_digest` (a SHA-256 + shape summary), the
helper the audit path already uses for exactly this reason.

## How it fits the core

- **No core crate changes.** Everything lands inside `crates/federation/`, as the pushdown scope did.
- **Rule 10 holds.** The cache is keyed generically on `(kind, dsn_hash)`; no branch on a named
  source, no special case for Postgres over SQLite. Adding a MySQL kind gets caching for free.
- **Secret mediation holds.** The DSN still arrives per call from the host, still lives only inside
  the pool, and is now additionally hashed before it is ever used as a key.
- **The child stays symmetric and restart-transparent.** The cache is a warm-start optimisation, not
  a source of truth; every entry is reconstructible from the next call's input.

## Example flow

```
federation.query {source: "pdnsw", kind: "timescale", sql: "SELECT 1"}
  → cache miss  → PostgresConnectionPool::new  (~2.4 s)  → run → 2,530 ms   {"cache":"miss",…}
federation.query {same source}
  → cache hit   → reuse pool                              → run →  ~150 ms  {"cache":"hit",…}
```

Expected steady-state: **~2,530 ms → ~150–250 ms**, i.e. down to the 137 ms network floor plus
engine time. Roughly a **15×** improvement, and a 4-tile dashboard stops paying it four times.

## Testing plan

Per `scope/testing/testing-scope.md` — real engines, no mocks:

1. **Cache hit is observable** — two `run_query` calls against one SQLite source; assert the second
   is materially faster and reports `cache: "hit"`. SQLite keeps this runnable with no network.
2. **A changed DSN misses** — query source A, query source B, re-query A; assert three distinct
   pools were built and results never cross.
3. **Timeout fires and evicts** — point a source at an unreachable endpoint with a short timeout;
   assert a typed error inside the bound (not a hang), and that a *subsequent* query to a healthy
   source still succeeds. **This is the regression test for the wedge** — it is the behaviour that
   actually broke, so it is the one that must go red when the fix is removed.
4. **No secret leaks** — capture emitted events across every path; assert no line contains the DSN,
   the password substring, or raw SQL.
5. **Existing suites stay green** — pushdown, discovery, migrate, sample, write are unchanged.

Per the repo's rule: after writing each test, **break the code it covers and watch it fail**. Test 3
especially — it is pinning a behaviour we have already seen happen once.

## Risks & hard problems

1. **"The child is stateless" (`main.rs` header, §3.4).** A pool cache is process-local state, so
   this is a real tension and must be resolved in the header comment, not silently. The resolution:
   §3.4 forbids *durable* state — anything a kill + respawn would lose that callers depend on. A
   cached pool is pure warm-start: reconstructible from the next call's input, invisible in results,
   lost harmlessly on restart. **Update the header to say so explicitly**, or the next reader will
   correctly file this as a violation.
2. **Unbounded cache growth.** Many distinct DSNs → many retained pools, each holding sockets. Needs
   a cap with LRU eviction. Start small (e.g. 16) — a node with 16 live datasources is already
   unusual, and eviction is cheap.
3. **A poisoned pool survives.** If a pool half-breaks (server restarts, network partition), a cached
   entry could serve errors indefinitely where per-call connect self-healed. Timeout-triggered
   eviction covers the hang case; a failed *probe* should evict too. This is the main way caching can
   be *worse* than today, so it needs the explicit eviction path, not just a TTL.
4. **Concurrency.** Two calls racing on a cold key must not build two pools. A `Mutex` held across an
   `.await` would serialise all queries — needs the connect to happen outside the lock, or a
   per-key once-cell, so a slow connect to source A never blocks a query to source B.
5. **The 9.2 s outlier.** One query during diagnosis took 9,213 ms where its neighbours took ~2,500 ms;
   it did not reproduce across 10 subsequent runs. Unexplained. Possibly a TLS retry or a far-end
   blip. Caching removes most of the surface it could hide in, and the new timing events are what
   would catch it next time — but it is *not* explained by this scope and should not be assumed fixed.

## Open questions

- **Timeout default.** 30 s is a guess sized to "slow remote, still working". A dashboard tile would
  rather fail at 5 s. Possibly two bounds: connect vs. query.
- **Should `probe` share the cache?** Sharing makes `datasource.test` fast but means a probe no
  longer proves a *fresh* connection works — which is arguably the point of a probe. Leaning: probe
  bypasses the cache and evicts on failure.
- **Is `eprintln!`-JSON the right channel long-term,** or should the child eventually own a real
  `tracing` subscriber whose output the host parses? The former ships today; the latter is tidier if
  federation grows more surface.

## Related

- `extensions/native-call-concurrency-scope.md` — the **other half of the dashboard latency story**.
  This scope removes the per-call connect; that one removes the queue in front of it (every call to a
  native sidecar is serialized behind one mutex, measured at 11.65 s for a 13-query dashboard). They
  compose, and the exit measurement for either is only meaningful with both in hand.
- `federation-pushdown-scope.md` — §Non-goals pre-authorised this scope; read it first.
- `datasources-scope.md` — the DSN/secret-mediation rules this must not break (§155, §171).
- `docs/FILE-LAYOUT.md` — one responsibility per file; the cache is its own file, not a `utils.rs`.
- `crates/supervisor/src/os.rs:37` — `stderr(Stdio::inherit())`, the channel the events ride.
