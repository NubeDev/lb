# The single session mutex serialized every store op — foreground reads stalled ~400ms behind background scans

**Date:** 2026-07-26
**Symptom (reported):** the rubix-ai dashboard took **~12s** to become usable; `/nav/resolve` and
`/nav/pref` (among many calls) were slow. Filed as a nav problem.
**Actual root cause:** `lb_store`'s single embedded SurrealDB connection was guarded by ONE session
`Mutex` **held across the whole query**, so every store operation node-wide serialized. A continuous
background scan (a reactor's unbounded `SELECT`, ~400ms) held the mutex almost continuously, and every
foreground point-read waited one scan-duration to acquire it.

## The evidence trail (all measured on the live node)

- `/health` (store-free) = **3ms**; `/nav/pref` (one store point-read) = **~400ms**, spaced/idle → the
  cost is in the store read, not HTTP/auth/nav logic.
- `/nav/pref` (1 read) and `/dashboards` (a full scan) both ≈ **450–550ms** → the latency is a **fixed
  per-request wait to acquire the lock**, NOT proportional to the work. A background lock-holder.
- Reads were **fast (~14ms) under a concurrent flood** but **slow (~500ms) sequential** — the FIFO
  mutex signature: flooded reads batch-drain once acquired; a lone read waits behind whatever grabbed
  the lock between.
- Not reproducible with a bare `Store` (fresh, tiny) — reads are <1ms even idle. It needed the live
  node's *continuous background store activity* (sims ingest + drain/insight/outbox/federation
  reactors) to expose the serialization.
- Isolated repro of the mechanism: one shared handle behind a `tokio::Mutex` held across the query,
  with a continuous background scan → foreground reads **~487ms median**. The SAME shape with a
  per-query `USE` and no mutex → **0ms**. (SurrealDB executes concurrent queries in parallel; only the
  lb mutex serialized us.)

Ruled out along the way: nav.resolve algorithm (it's 3 point-reads for the admin fallback), HTTP/auth,
the 558MB commit log (compaction didn't change it), namespace/catalog count, and simple write
contention (fast writes hold the lock <1ms).

## Why the mutex existed (and how the fix keeps both guarantees)

Per `open.rs`, the mutex did two jobs:
1. **The workspace wall.** `use_ns(ws)` mutates the shared session's namespace — a distinct `await`
   from the query, so two ops for different workspaces could interleave and a query could run against
   the wrong namespace (the flaky-login `concurrent-use-ns-namespace-race`). The mutex made
   `use_ns`+query a critical section.
2. **The compaction handle-swap.** The `Surreal<Db>` lived inside the mutex so the online pass could
   drop → compact → reopen → swap it with no query in flight.

The fix removes the serializing mutex while keeping both:
- **Wall → per-query `USE`.** Every op prepends `` USE NS `<ws>` DB main; `` to its own query
  (`scope_sql`), so the namespace is scoped to that ONE query call. SurrealDB isolates it per call even
  under concurrency (verified: 400 interleaved cross-ns reads, 0 contaminated). No shared session
  mutation ⇒ no race ⇒ no mutex needed. `<ws>` is validated (`[A-Za-z0-9_.-]`) and backtick-quoted, so
  the wall is uninjectable (a bare hyphenated slug like `ws-a` would otherwise parse as `ws - a`).
- **Swap → `RwLock`.** The handle lives in an `Arc<RwLock<Surreal<Db>>>`. Data ops take the READ guard
  and hold it across their query (shared — all concurrent); compaction takes the WRITE guard (waits for
  in-flight ops, swaps, releases). Same exclusion the mutex gave the swap, without serializing reads.

Multiple handles to one SurrealKV path were tried and **rejected** — two `Surreal::new` on the same
path give inconsistent MVCC views (immediate cross-handle read-after-write returned `None`; concurrent
writes corrupted/mismatched). One engine handle only.

## Atomicity was never the mutex's job

The RMW verbs handle their own atomicity and are unaffected: `write`/`increment` use a single
server-side atomic UPSERT; `write_locked`/`increment` add a per-`(ws,table,id)` async lock + retry;
`write_tx`/`write_journaled`/`capped`/`write_batch` use real `BEGIN…COMMIT` transactions. Removing the
global mutex changed none of that.

## The change

`crates/store/src/open.rs`: `session: Arc<Mutex<Surreal>>` → `handle: Arc<RwLock<Surreal>>`;
`query_ws` prepends the scoped `USE` under the READ guard and returns a `ScopedResponse` whose `take`
selectors are shifted by one (the injected `USE` is real statement 0) — so all ~140 `query_ws` callers
keep their 0-based `take`/`check` unchanged. `use_ws`/`WsGuard` removed; the 11 in-crate verbs
(`read`/`write`/`list`/`create`/`delete`/`read_versioned`/`increment`/`write_tx`/`write_journaled`/
`write_batch`, plus `scan`/`graph`/`tables` already on `query_ws`) route through it. `compact.rs` takes
the RwLock WRITE guard to swap.

## Verification

- `cargo test -p lb-store` — all green, incl. `concurrent_ns_test` (the wall race), namespace isolation
  on disk, `capped`/`write_locked` (RMW), and both compaction tests.
- New `read_concurrency_test.rs` — foreground reads stay <100ms under a continuous background scan
  (was ~487ms with the mutex); fails loudly if the serialization is reintroduced.
- Whole workspace compiles; `lb-tags`/`lb-ingest`/`lb-prefs` suites green.

## Follow-ups (separate)

The continuous ~1-core background store load (dev sim ingest, debug build) is what *exposed* the
serialization; it is not itself a bug, but a debug build makes every store op slower and a release
build should be the perf baseline. Reactor scans that do unbounded `SELECT`s are still worth indexing
(the series.list PR #39 pattern) so they hold the engine briefly — now they no longer block reads, but
they still cost CPU.
