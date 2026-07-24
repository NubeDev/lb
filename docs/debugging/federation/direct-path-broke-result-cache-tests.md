# Direct fast path (PR #100) regressed two result-cache tests — schema staleness + faster warm

**Area:** federation (result cache tests) — a downstream effect of PR #100's direct query path
**Date:** 2026-07-24
**Symptom:** After PR #100 merged, `cargo test -p federation` (default sqlite build) went red in
`result_cache_test`: `a_failed_refresh_leaves_the_entry_serving` and
`an_accepting_caller_never_waits_on_a_stricter_callers_refresh` both failed. Bisected to the PR: both
**passed at the base commit** (`81298a0a`, DataFusion path) and **fail at the merged head** (direct
path). Two DISTINCT root causes, both consequences of the direct path — neither a product defect in
the result cache itself.

## Cause 1 — `a_failed_refresh`: a warm SQLite connection serves a stale SCHEMA

The test forces a failing refresh by `ALTER TABLE marker RENAME TO marker_gone` under a warm entry,
then expects `SELECT id FROM marker` to error (`no such table`). At base the DataFusion path built a
FRESH per-table provider (new connection) each query, so it saw the rename and errored. The direct
path reuses the **warm pooled SQLite connection** (`pool::cached_connect`), and that connection caches
the schema: after an out-of-band `RENAME`/`DROP` it keeps serving the query as an empty **`Ok(0 rows)`,
never an error** — so `failed.is_err()` was false.

Confirmed by probe: a fresh source sees the rename (`Ok(0)`/error), but the *same* warm source keeps
serving stale. Note this is schema-specific — a plain **INSERT is seen fine** by the same warm
connection (verified); only `RENAME`/`DROP`/file-delete hit the per-connection schema cache.

**Fix (test):** `pool::evict("sqlite", &dsn)` before the failing refresh (and before the recovery
query). This mirrors production exactly: a schema change is invisible to a warm pool until it is
dropped, which is precisely why `probe`/`datasource.test` evicts. The fix makes the refresh open a
fresh connection that sees `no such table` and genuinely fails — the behaviour the test asserts.

## Cause 2 — `an_accepting_caller`: the direct path made `warm` too FAST for the TTL margin

The test warms an entry, then a "strict" caller with a short TTL is meant to REJECT the warm entry and
run a real refresh (the ~750 ms burn self-join), while a "lenient" 60 s caller accepts the stale rows.
At base the `warm` query itself took ~750 ms (DataFusion planning the 12k-row burn join), so by the
time control reached the strict caller the entry was already well-aged and the strict TTL rejected it.

The direct path returns `warm` in **microseconds**. So the entry is now only ~tens of ms old when the
strict caller checks — younger than its TTL — and the strict caller **ACCEPTS** it (slot rule 1),
returning the stale 1 row in ~276 µs instead of refreshing. The "refresher must see the fresh 2 rows"
assertion then fails (`left: 1, right: 2`). Instrumentation showed `strict took 276µs -> Ok(1)` (a
hit), not a ~750 ms refresh.

**Fix (test):** widen the age-vs-TTL margin so the reject is deterministic regardless of how fast
`warm` runs — sleep 50 ms against a 10 ms strict TTL (5× margin). After the fix the strict caller
genuinely refreshes (~2.7 s wall, the real burn query runs) and returns 2 rows. Green 8/8 isolated.

## Lesson

**A fast path doesn't just change latency — it changes the timing assumptions every test built on the
slow path.** A test that relied on a query being slow enough to age a cache entry breaks silently when
the query gets fast; the failure looks like a cache bug but is a test-timing bug. When adding a fast
path, grep the suite for tests that sleep/age relative to a query's duration and re-anchor them on an
explicit margin, not on the incidental cost of the operation under test.

And: **a warm connection pool caches schema.** The direct path's reuse of a pooled connection means an
out-of-band DDL is invisible until the pool is evicted — the same reason `probe`/`datasource.test`
exists. Tests (and callers) that mutate schema under a warm source must evict.

## Not fixed here (pre-existing, out of scope)

`an_accepting_caller`'s **wall-clock timing assertion** (`waited < 350 ms`, "the accepting caller must
never block") flakes under heavy box load — it fires at the BASE commit too (measured 1.7–2.5 s waits
on a saturated box), so it is the pre-existing parallelism flake already recorded in
[result-cache-tests-flake-under-parallelism.md](result-cache-tests-flake-under-parallelism.md), not a
PR-100 regression. Left untouched; the content fixes above are independent of it and pass in isolation.

## Regression coverage

The two tests themselves are the regression tests: each fails at the merged head before these fixes
and passes after (verified fail-before/pass-after per cause). Both are green 8×/8× in isolation and in
the full binary when the box is not saturated.
