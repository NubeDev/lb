# A reader-task leak test passed against deliberately leaking code

**Area:** extensions (native Tier-2 transport) · **Status:** closed (2026-07-20)
**Session:** [`sessions/extensions/native-call-concurrency-session.md`](../../sessions/extensions/native-call-concurrency-session.md)
**Scope:** [`scope/extensions/native-call-concurrency-scope.md`](../../scope/extensions/native-call-concurrency-scope.md)

## Symptom

While building the multiplexed native control line, `repeated_restarts_do_not_leak_reader_tasks`
(scope testing plan §7 — "no task is leaked per restart") went **green against a `restart` that was
deliberately broken to leak the outgoing generation**.

The revert-check was:

```rust
// deliberately DO NOT close the outgoing generation
let _leaked = self.conn.take();
```

Every *other* revert-check in the session went red exactly as designed. This one did not, which is
what made it worth an entry: the test was reporting a property it could not actually observe.

## Root cause

The test asserted on a **task count**:

```rust
let baseline = tokio::runtime::Handle::current().metrics().num_alive_tasks();
for _ in 0..10 { sc.restart(&l).await.unwrap(); }
let after  = tokio::runtime::Handle::current().metrics().num_alive_tasks();
assert!(after <= baseline + 4);
```

`Conn` has a `Drop` impl that aborts its reader task and drains its pending map — a safety net for
error paths where `close()` never runs. In the test, nothing held a reference to the superseded
generation, so `self.conn.take()` dropped the last `Arc` immediately and `Drop` cleaned up the reader
**even though `restart` had forgotten to close it**. The task count never grew, so the assertion held.

The count was measuring `Drop`, not `restart`. The leak it was written to catch only becomes
observable **while something still references the old generation** — which is precisely the real
case the test exists for: an in-flight call holding an `Arc<Conn>` across a restart.

## Fix

Assert the **observable behavioural property** instead of a proxy metric: a superseded generation
must be *dead while still referenced*.

```rust
let old = sc.conn().unwrap();          // hold the generation across its own restart
sc.restart(&l).await.expect("restart");
assert!(
    old.call_with_caller("q", "0", None).await.is_err(),
    "a superseded generation still served a call — restart did not close it, \
     so its reader task and pending map leak for as long as anything holds it"
);
retired.push(old);                     // keep it alive; drop them all at the end
```

A call on the old generation is refused **only** if `restart` actually closed it. The task-count
assertion is retained afterwards (after dropping the held generations) as a secondary check, but it
is no longer the load-bearing one.

## Regression test

`crates/supervisor/tests/concurrent_call_test.rs::repeated_restarts_do_not_leak_reader_tasks`.

Verified fails-before / passes-after against the leaking `restart`:

```
thread 'repeated_restarts_do_not_leak_reader_tasks' panicked:
a superseded generation still served a call — restart did not close it, so its reader
task and pending map leak for as long as anything holds it
```

The reasoning is commented in the test so the next reader does not "simplify" it back to a task
count.

## Lesson

**A `Drop` safety net can mask the bug the test is aimed at.** When a type cleans itself up on drop,
any test that lets the object drop is testing `Drop`, not the verb under test — the assertion passes
for the wrong reason.

More generally: **assert the behaviour, not a proxy metric.** `num_alive_tasks()` is a proxy for "the
reader stopped"; "a call on the old generation is refused" *is* the property. Proxies are exactly
where a test goes vacuous while still looking rigorous.

This is the third vacuous test found in this area — the
[federation pool-cache session](../../sessions/datasources/federation-pool-cache-session.md) found
two (a timeout test that never reached the timeout arm; an eviction assertion that was vacuous on a
cold start). The pattern across all three: **the test never ran the code path it claimed to cover**,
and only breaking the implementation on purpose revealed it. Revert-check everything.
