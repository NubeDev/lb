# `capped_insert` overgrows the cap under concurrency (flaky `count == cap + k`)

- Area: observability (lb-store::capped)
- Status: resolved
- First seen: 2026-06-30
- Resolved: 2026-06-30
- Session: ../../sessions/observability/telemetry-console-session.md
- Regression test: rust/crates/store/tests/capped_test.rs::concurrent_inserts_past_cap_leave_exactly_cap

## Symptom

The headline concurrency test for the capped-retention primitive failed **intermittently**:

```
assertion `left == right` failed: final count must be EXACTLY cap under concurrency
  left: 21
 right: 20
```

100 concurrent `capped_insert`s on one key with `cap = 20` left **21** rows, not 20. It passed
when the single test ran alone, and passed most full-file runs — a classic flaky-under-load
failure. Per the scope ("treat 'it usually stays near 1000' as a bug, not a pass") this is a
real defect on the load-bearing primitive, not noise.

## Reproduce

`cargo test -p lb-store --test capped_test` repeatedly (the failure surfaced ~1 in 5 full-file
runs; the in-process scheduler interleaving is what varies). The trim is a single
`BEGIN…COMMIT` doing `CREATE` then `LET $keep = (SELECT … ORDER BY seq DESC LIMIT cap)` then
`DELETE … WHERE seq NOT IN $keep`.

## Investigation

- The implementation already did insert + trim in **one** SurrealDB transaction (the scope's
  mandated design), so "two un-transacted statements race" was ruled out.
- SurrealDB 2's `kv-mem` engine uses **optimistic, snapshot-isolated** transactions. Two
  concurrent inserts on the same key each compute `$keep` from a snapshot taken *before* the
  sibling's `CREATE` is visible. Each trim therefore deletes the complement of a `$keep` that
  is missing the other in-flight row — so each under-deletes, and the ring overgrows the cap by
  the number of overlapping in-flight inserts.
- The engine does **not** reliably raise a write-write conflict for this read-set/write-set
  shape (the trims delete *different* stale rows), so a bare retry-on-conflict loop cannot fix
  it — there is no conflict to catch.

## Root cause

Snapshot isolation + a snapshot-derived trim set means concurrent same-key transactions don't
serialize on the cap invariant. The transaction is atomic (insert+trim commit together), but
atomicity is not isolation: nothing forced the two trims to see each other.

## Fix

Serialize the at-most-millisecond insert+trim transaction **per `(ns, table, cap_key)` bucket**
with an in-process async lock (`key_lock` in `crates/store/src/capped.rs`): a
`OnceLock<Mutex<HashMap<key, Arc<tokio::Mutex<()>>>>>` whose per-key `Arc<Mutex>` is held across
the transaction. The SurrealDB transaction stays the atomic unit; the lock removes the
*interleaving* that defeated snapshot isolation. Different keys never contend (a chatty source
can't block a quiet one — the same property per-source capping gives), so this is not a global
write lock. It is legitimate for a capped ring: a single node owns its ring, and a capped table
is recent operational data, not cross-node synced state (telemetry-console-scope, "each node's
ring is independent").

**A second, distinct failure surfaced once the Layer drove the primitive under a real telemetry
test:** SurrealDB `kv-mem` *does* raise a retryable write conflict — not between two same-key
inserts (the lock serializes those), but between the insert+trim transaction and a **concurrent
reader or a cross-key writer** in the same namespace ("…failed transaction…can be retried"). The
fire-and-forget Layer swallowed that error, so the row silently never landed (an *under*-count,
the mirror of the original over-count). Fix: a bounded **retry-on-conflict** loop in
`capped_insert` (`MAX_CONFLICT_RETRIES`, escalating backoff so a burst desynchronizes rather than
livelocks). So the complete correct design is **single transaction + per-key serialization +
retry on the retryable conflict** — each retry is the same single transaction, so the cap
invariant is preserved.

Considered and rejected: retry-*only* (without the per-key lock the same-key trims still
over-evict — no conflict is raised for that shape); a SurrealDB `LIVE`/native ring (doesn't exist
— count-bounded FIFO is why we own the primitive); count-then-delete (the original racy shape
this primitive was written to avoid).

## Verification

`cargo test -p lb-store --test capped_test` run 10× consecutively — 6/6 green every run,
including `concurrent_inserts_past_cap_leave_exactly_cap`. The flake is gone.

## Prevention

The concurrency test is the standing guard: 100 concurrent inserts at 5× cap, asserting the
final count is **exactly** cap (never over-evicted, never overgrown). It fails-before /
passes-after this fix and is the proof the single-tx + per-key-serialization design holds.
