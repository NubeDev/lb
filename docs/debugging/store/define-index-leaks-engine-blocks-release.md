# A `DEFINE INDEX` leaks the embedded engine — the online pass's release-wait never succeeds

- Area: store (online compaction handle-swap, `crates/store/src/compact.rs`)
- Found: 2026-07-15, when the `store.compact` job-flow test failed under a booted node
  (issue #67 session). Sibling of `compaction-merge-eats-next-sessions-writes.md`.
- Severity: feature-blocking (no data loss) — the pass skipped itself on every real node.
- Status: fixed (quiesce-by-stability fallback) + regression test.

## Symptom

`store.compact` jobs completed `Failed` with `engine did not release <path> within 30s; pass
skipped` — but only under a real node. The store-crate tests (plain `Store`) passed: there,
dropping the last `Surreal<Db>` released the engine's files in 74–240 ms.

## Root cause (upstream: surrealdb-core 2.6.5, `kvs/index.rs`)

Any `DEFINE INDEX` spawns a detached index-builder task (`IndexBuilder::build` →
`spawn(async move { … })`) whose `Building` holds the **transaction factory — the engine**.
That reference is never dropped: measured, the store's `clog`/`manifest` fds are still open
**120 s after the last `Surreal<Db>` clone dropped**. Every real workspace defines the jobs
`(kind, status)` index on first `job` write, so "wait for full fd release" can never succeed
in production — the swap-based pass was structurally dead on arrival, and only the spike-style
plain-store tests could pass.

Minimal repro (store crate): open → write → `DEFINE INDEX` → swap/drop handle → fds under the
store dir never reach zero.

## Why the fallback is safe (the load-bearing argument)

The leaked holder is **inert by construction** once the drop completes:
- the local engine's router task exits when the last handle drops — no query can ever reach
  the old engine again (nothing holds a `Surreal` pointing at it);
- the router's exit path cancels the background tickers (node membership, changefeed GC,
  index compaction) and runs `kvs.shutdown()` **before** finishing;
- the leaked index-builder only writes when queries queue documents — impossible without a
  handle.

So after shutdown's own writes stop, *nothing* can write through the leaked engine — and
"its writes stopped" is directly observable as file stability.

## Fix

`wait_for_release` became `wait_for_quiesce` (`compact.rs`): fast path = full fd release
(covers the no-index case, 74–240 ms); fallback = every file under the store dir keeps an
unchanged `(size, mtime)` across a 2 s window; hard timeout still skips the pass (never
compact under an engine that might write). Cost: a pass on an index-bearing store takes ~7 s
of quiesce-wait instead of ~200 ms — irrelevant for a rare, threshold-driven job.

Note: each online pass strands one leaked (inert) engine instance's memory/fds for the
process lifetime — bounded by the number of passes per process, i.e. a handful; recorded in
the scope. Upstream issue text drafted with the minimal repro.

## Regression test

`crates/store/tests/index_leak_quiesce_test.rs::online_pass_succeeds_after_a_define_index` —
defines the exact jobs index, runs the online pass on the live handle, requires success +
intact data. Fails against the fd-zero-only gate (times out, pass skipped), passes with the
quiesce fallback. The end-to-end proof is
`crates/host/tests/store_admin_test.rs::compact_job_enqueues_drains_and_records_outcome`
(the original failing test, now green).

## Cross-links

- Session: `sessions/store/online-compaction-session.md` · Scope:
  `scope/store/online-compaction-scope.md` (issue #67)
- Sibling engine bug found the same day: `compaction-merge-eats-next-sessions-writes.md`
