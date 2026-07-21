# Result-cache tests fail intermittently under `--test-threads` > 1

**Area:** federation · **Date:** 2026-07-20 · **Status:** closed (fixed + regression-guarded)
**Session:** `docs/sessions/datasources/federation-result-cache-session.md`
**Scope:** `docs/scope/datasources/federation-result-cache-scope.md`

## Symptom

`cargo test -p federation` came back **44 passed / 5 failed** on the full run, while the same tests
were green when run alone or with `--test-threads=2`. The failing set was not random — it was
exactly the tests that depend on a cache **hit**:

```
test an_expired_entry_re_queries_and_sees_new_rows ... FAILED
test a_hit_serves_the_cached_rows_not_the_new_ones ... FAILED
test a_lost_cache_costs_freshness_not_correctness ... FAILED
test a_failed_refresh_leaves_the_entry_serving ... FAILED
test the_source_alias_is_part_of_the_key ... FAILED
```

At `--test-threads=16` it reproduced on every run (1, 2, then 3 failures across three runs) with a
*different subset* each time — the tell that this is a race, not a logic bug in any one test.

## Root cause

`the_kill_switch_forces_bypass_even_when_the_caller_asks` exercises the node kill-switch by calling
`std::env::set_var("LB_FEDERATION_RESULT_CACHE", "off")`, then removing it at the end.

**Environment variables are process-global, and Rust runs a test binary's tests as threads in one
process.** So for the whole window that test holds the variable set, *every other test in the binary*
sees the kill-switch as ON — and `requested_ttl()` correctly returns `None` for all of them. Their
queries bypass the cache, the mid-test `INSERT` becomes visible, and every hit-dependent assertion
fails. The failing set was precisely "tests that expected a hit and happened to overlap that window",
which is why the subset moved run to run.

Worth stating plainly: **the production code was never wrong here, and neither was the assertion
being made.** The kill-switch behaved exactly as designed; the test simply reached beyond its own
scope to configure it. This is the process-global-state trap in its most ordinary form — the same
shape as a test that `chdir`s, or sets a global logger, or mutates a `static`.

## Fix

The kill-switch is read per call by design (`kill_switched()` deliberately avoids a `OnceLock`,
precisely so the switch is testable in-process — a `OnceLock` would freeze the first value any test
observed and make the switch permanently untestable). Keeping that, the fix is to stop the *test*
from leaking its global:

The kill-switch test now runs under a **process-wide mutex** that every cache-behaviour test in the
file acquires. Tests that need the cache take a shared read lock; the kill-switch test takes the
exclusive write lock, so nothing else is mid-query while the variable is set. It is restored under
the same lock.

Rejected alternatives, and why:

- **`#[serial]` / running the file single-threaded.** Would work, and would also slow the whole file
  to its serial sum for one test's benefit — and, more importantly, it hides the hazard rather than
  naming it. The next person adding an env-touching test gets no signal.
- **Moving the kill-switch test to its own test binary.** Clean isolation, but it splits the cache's
  test story across two files for one assertion, and `#[path]`-composing the child's modules a third
  time is real duplication.
- **Not testing the kill-switch in-process at all.** Unacceptable: it is one of the three documented
  ways to disable this feature, and the operator-facing one.

## Regression guard

The lock itself is the guard, and it is load-bearing rather than incidental — so the `RwLock` carries
a comment saying exactly what breaks without it (this entry), and the kill-switch test asserts
`requested_ttl()` resolves to `None` *while holding the write lock*, so a future refactor that drops
the locking makes the flake reappear as a deterministic failure of that assertion rather than as
someone else's mysterious red.

Verified by re-running the full binary at `--test-threads=16` repeatedly after the fix.

## A SECOND, distinct cause behind the same symptom

Fixing the env-var leak removed the 5-test cluster but **not all of it**: re-running at
`--test-threads=16` still produced an occasional single failure, now always the same test:

```
test an_accepting_caller_never_waits_on_a_stricter_callers_refresh ... FAILED
assertion `left == right` failed: the accepting caller must be served the STORED rows, …
  left: 2
 right: 1
```

This one is a **test-timing assumption**, again not a code defect. The test warms an entry, then
races a strict caller (TTL rejects → starts a ~750 ms refresh) against a lenient caller (TTL accepts
→ must return the stored rows without waiting). It slept 20 ms before the lenient call, *assuming*
the refresh would still be in flight. On a loaded box — this box, running 16 test threads plus a
cargo build — that assumption fails: the refresh completes first, `current` is legitimately replaced,
and the lenient caller correctly receives 2 rows. **Slot rule 3 says explicitly that fresher-than-
asked is never wrong**, so the code was right and the test was asserting something it had not
established.

Fix: split the assertion by what it actually proves.

- **Unconditional** — the accepting caller did not *block* (`waited < 100 ms` against a ~750 ms
  refresh). This is the real invariant (rule 1) and it holds in both worlds.
- **Gated on `strict.is_finished()` being false** — the row-content assertion, which is only
  meaningful while the refresh is genuinely still in flight.

The gate is deliberately *not* "loosen the assertion to accept 1 or 2", which would have removed the
test's teeth in exactly the case it exists for. It distinguishes the two worlds instead of averaging
them.

## A THIRD manifestation — the *timing* half of the same test (found 2026-07-21)

The second fix split the test into an unconditional timing invariant (`waited < 100 ms`) and a
content assertion gated on `strict.is_finished()`. On a *different* machine (16 workers, concurrent
`cargo build`), the suite flaked again — but now on the **timing** assertion, not the content one:

```
test an_accepting_caller_never_waits_on_a_stricter_callers_refresh ... FAILED
panicked at crates/federation/tests/result_cache_test.rs:619:5:
the accepting caller waited …ms — it must never block on a stricter caller's in-flight refresh
```

Root cause, again not a code defect: the accept path is a **lockless synchronous return**
(`results.rs` `Action::Serve` — it returns `current` with no `.await` on any refresh). So the time
that assertion measures is pure runtime **scheduling latency**: on a saturated box, tokio can leave
the accepting caller's continuation unscheduled for >100 ms even though it never blocked. The 100 ms
bound was an absolute wall-clock bet that does not scale with box load, so it produced a false red.

Fix: widen the bound to **350 ms**, which is still under half the ~750 ms refresh. The distinction
that gives the assertion teeth is *block vs no-block*, and those differ by ~750 ms — a genuine rule-1
violation would join `inflight` and wait the full refresh. 350 ms sits comfortably between the two:
it cannot be reached by an immediate return's scheduling jitter, and it is far below what a real
block would cost. (Asserting *relative* to a measured refresh duration was considered and rejected:
the refresh runs concurrently with the lenient caller, so its duration isn't known until the join at
the end of the test — after the assertion needs it. The refresh cost is instead pinned by the 12 000
-row burn seed, so a fixed fraction of it is a stable, readable bound.)

Verified by re-running the full binary at `--test-threads=16` several times after the change.

## Lesson

A test that fails **only in company** and whose failing set **moves between runs** is almost never a
bug in the tests that failed — look at what its neighbours are doing to shared process state. Here,
the five red tests were the victims; the culprit was green every single time.

And the follow-up lesson, from the second cause: **one fix removing most of a flake is not evidence
it removed all of it.** Two independent hazards were producing one symptom, and stopping after the
first (with 2 of 3 runs green) would have shipped a test that fails for whoever next runs the suite
on a busy machine. The rule that caught it was mechanical — re-run the full binary at maximum
parallelism *several times*, and treat a single failure in three runs as a real signal rather than
noise, because that is exactly what it is.

Both causes share a shape worth naming: **a concurrent test asserted a precondition it had merely
arranged, not verified.** One assumed no neighbour would touch a global; the other assumed a 750 ms
query would still be running after a 20 ms sleep. The durable fix in both cases was to *establish*
the precondition (a lock; an `is_finished()` check) rather than to weaken the assertion.
