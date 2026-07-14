# `rules_test` hangs (not slows) under heavy box load — green when quiet

- **Date:** 2026-07-14 (behaviour known since ~2026-07-12; this is its first entry)
- **Area:** rules (test harness — `crates/host/tests/rules_test.rs`)
- **Status:** open — understood, not fixed. Low priority: it never fails a quiet run.
- **Found by:** full-suite triage; reproduced by running six `cargo test` invocations alongside a
  `--workspace` sweep on this 28-core/31G box.

## Symptom

Under load the binary stops dead rather than running slowly. Several independent tests report the
60s warning at once — including ones that share nothing but the runtime:

```
test run_rollup_alert_rule_raises_inbox_item has been running for over 60 seconds
test ws_b_cannot_get_a_ws_a_saved_rule has been running for over 60 seconds
test ws_b_rule_cannot_post_into_a_ws_a_channel has been running for over 60 seconds
test ws_b_rule_cannot_raise_or_close_into_ws_a has been running for over 60 seconds
```

It never recovers. Killed after ~11 minutes at **0.9% CPU** — the tell that separates a hang from
slow work: a starved-but-progressing test burns CPU, this one does not.

Process state at the hang: **164 threads**, parent sleeping in `futex_do_wait`.

## Why it is a hang, not slowness

`rules_test` holds **27 `#[tokio::test(flavor = "multi_thread", worker_threads = 1)]` tests**. The
libtest harness runs them **concurrently**, so the binary stands up ~27 single-worker runtimes at
once (hence ~164 threads once each runtime's blocking pool and the store's threads are counted).

A `worker_threads = 1` runtime has exactly one thread to drive every future it owns. That is fine
when the box can schedule it promptly. When ~27 of them contend with a saturated machine, a runtime
whose single worker is descheduled (or blocked inside a `block_on`-style call) cannot make progress
at all — nothing else can pick up its work. The result is a genuine deadlock under starvation, not a
long wait: no CPU is being consumed because no worker is runnable.

That is why it presents as "many unrelated tests stall simultaneously": they aren't related, they are
all downstream of the same scheduling starvation.

## Verification

| box state | result |
|---|---|
| 6 parallel `cargo test` + a `--workspace` sweep | hangs indefinitely (killed at ~11 min, 0.9% CPU) |
| quiet | **green, exit 0** (`cargo test -p lb-host --test rules_test -j 4`) |

So a `rules_test` FAILED line in a sweep that shared the box with other cargo runs is an artifact of
the harness, **not a regression** — confirm on a quiet box before chasing it.

## Fix options (for whoever owns rules)

1. **Give the tests more than one worker** — `worker_threads = 2` (or drop to the default
   `#[tokio::test]` current-thread flavor where the test does not need real parallelism). A single
   worker is the precondition for the starvation deadlock.
2. **Cap harness concurrency** for this binary (`--test-threads`) so 27 runtimes never coexist.

(1) is the real fix: it removes the "one blocked worker = one dead runtime" property. Worth checking
whether the whole `worker_threads = 1` convention across the host test suites carries the same
latent fragility — `rules_test` may just be the biggest file, and therefore the first to show it.

## Lesson

**0% CPU is the difference between "slow" and "stuck".** A starved test still burns CPU; a deadlocked
one does not. Check `%cpu` and thread state (`futex_do_wait`) before deciding a suite is merely slow —
and before blaming the change under test.

**Don't run parallel cargo invocations against a sweep you intend to trust.** Beyond CPU contention,
it makes any load-sensitive failure in that sweep unattributable — the run stops being evidence.
