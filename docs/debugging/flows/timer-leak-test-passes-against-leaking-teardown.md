# An interval-timer orphan test passed 7/7 against deliberately-leaking teardown

- Area: flows (interval timer reconciler) / testing method
- Status: resolved
- First seen: 2026-07-29 (caught by the mandatory revert-check, not by a failure)
- Resolved: 2026-07-29
- Session: ../../sessions/flows/interval-timers-phase2-session.md
- Scope: ../../scope/flows/interval-source-clock-scope.md (Phase 2, Testing plan "orphan/leak test")
- Regression test: `rust/crates/host/tests/flows_interval_timers_test.rs::repeated_enable_disable_leaves_no_orphan_timer_still_firing`

## Symptom

No failure — that is the point. The new orphan/leak test for the interval-timer reconciler passed.
It also passed with `LiveTimer::drop` gutted so that **every timer task leaked**: the whole suite
stayed green (7/7) while five enable/disable cycles left five orphaned tasks running.

The scope had explicitly called a leaked timer *worse than the bug being fixed* ("a flow disabled but
still firing is far worse"), and the test written to prevent exactly that could not detect it.

## Reproduce

In `crates/host/src/flows/interval_timers.rs`, empty the `Drop` body:

```rust
impl Drop for LiveTimer {
    fn drop(&mut self) { /* leak */ }
}
```

`cargo test -p lb-host --test flows_interval_timers_test` → **7 passed; 0 failed**.

## Root cause — two independent maskers

The test asserted the *effect*: after N enable/disable cycles, open a quiet window and assert no new
runs appear. Nothing fires — but not because the timer stopped:

1. **An inner gate produces the same outcome.** A leaked timer wakes on schedule and calls the shared
   fire path, which re-reads the flow and finds it **disabled**, so it fires nothing and returns an
   empty tally. The `enabled` re-check independently guarantees the asserted outcome, so the
   assertion cannot distinguish "task torn down" from "task alive and being turned away".
2. **Run-id idempotency hides it a second way.** Firings derive a deterministic run id per scheduled
   instant, so even two live timers on one node cannot produce two runs for one slot. Any assertion
   phrased as "how many runs appeared" is doubly blind.

The registry count (`timers.count() == 0`) is no better: it measures what the map *believes*, and the
leak is precisely a task that outlived its map entry.

## A second, unrelated defect in the same test

The first version of the assertion failed against **correct** code: each enable legitimately produces
one firing (the oscillator emits as soon as it is armed), slots are whole seconds, and a legitimate
pre-disable firing shares the boundary second with any `T` sampled just after the disable. Fixed by
comparing the **set** of fired slots across the window rather than "nothing after `T`" — set-equality
has no boundary to straddle. Worth recording because it is the opposite error (a false red) in the
same few lines, and "fixing" it by widening the window would have made the false green worse.

## Fix

Assert the property **only teardown has** — the task is actually gone — via
`tokio::runtime::Handle::current().metrics().num_alive_tasks()`. A leak's real cost is a task polling
the store forever, once per period, per orphan, compounding with every cycle; the task census measures
exactly that, where no observable *effect* can. Transients are drained first, then a fixed tolerance
of 2 absorbs the in-flight detached run-drive task that a firing spawns.

Measured over five enable/disable cycles:

| `LiveTimer::drop` | alive tasks | result |
|---|---|---|
| gutted (leaking) | baseline **+6** | **FAILS** |
| real | baseline **+1** | passes |

The separation is structural, not tuned: an orphan is O(cycles), the run-drive transient is O(1). The
revert-check was then re-run **with the tolerance in place** to confirm it had not been blunted (still
red at +6). The behavioural set-equality assertion is kept alongside — it is a real property, just not
a sufficient one.

## Prevention

`IntervalTimers::count`'s doc comment now warns the next author directly: neither the registry count
nor "did anything fire?" is evidence a task stopped, with the measurement that proves it.

## Lesson

**This is a recurrence** — the same class as `repeated_restarts_do_not_leak_reader_tasks`, which
passed green against a `restart` deliberately broken to leak the outgoing channel generation. The
generalisation: *when the code under test contains an inner gate that independently produces the
outcome you are asserting, an effect-based assertion cannot detect the outer defect.* Ask what
property the fix has that the broken version does not, and assert that — here, a resource count rather
than a behaviour.

And the meta-lesson, which cost nothing to apply and caught this: **revert-check every regression
test.** The first two versions of this test both looked entirely reasonable; one was a false red and
one a false green, and only deliberately breaking the code distinguished them.
