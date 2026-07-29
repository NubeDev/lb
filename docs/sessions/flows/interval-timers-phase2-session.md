# Session — interval source timers, Phase 2 (Option A: `period_secs: 1` finally means one second)

- Scope: [`../../scope/flows/interval-source-clock-scope.md`](../../scope/flows/interval-source-clock-scope.md)
- Debugging entry: [`../../debugging/flows/interval-cursor-drifts-and-fires-serially.md`](../../debugging/flows/interval-cursor-drifts-and-fires-serially.md)
- Predecessor session: [`interval-clock-phase0-1-session.md`](./interval-clock-phase0-1-session.md)
- Date: 2026-07-29

## The ask

Phase 0 made the sweep *honest*; Phase 1 made firings *concurrent*. Neither removed the defect the
scope was opened for: **every interval source was a durable row noticed by one 5-second workspace
sweep**, so no period below 5s was expressible and `period_secs: 1` was rejected at save rather than
served. Phase 2 is Option A — the real fix.

The owner selected Option A explicitly (*"sub-second intervals real, many nodes all firing at once
and when they need"*), and the scope requires the rule-4 exception be **argued in the open** rather
than smuggled in as a stray `tokio::interval`.

## The design, and why it is not a rule-4 violation

`flows/interval_timers.rs` is the **one owner** of live interval cadence on a node. The argument the
scope makes, and this implementation honours literally:

> An interval source's **value** is state; its **cadence** is motion.

The value (`flop`) and the next slot stay exactly where they were — durable in `FlowTriggerState`, in
SurrealDB, surviving restart. What moved into memory is only *when to wake up and run the existing
fire path*. A timer here owns **no state at all**: kill the process and every timer dies with it; the
reconciler rebuilds the set from the durable graph on the next tick and each rebuilt timer reads its
cursor and value back out of the store. Today's sweep, by contrast, *persisted the cadence and polled
it* — which is the category error: modelling a **period** as a durable row a coarse sweeper notices
costs a full workspace store scan to approximate what `sleep(period)` does for free, **and is still
wrong at any period below the sweep**.

Three choices carry most of the correctness:

1. **The timer does not re-implement firing.** It sleeps to the durable `next_attempt_ts`, then calls
   `react_interval::fire_flipflop_node` — the same function the sweep called, with the same
   deterministic run id, the same `lb_jobs::load` idempotency read, the same caps wall, and the same
   `next_slot_after` advance. The timer is *a precise scheduler for an existing idempotent scan*. That
   is what makes double-fire impossible **by construction** rather than by coordination, and it is why
   the capability-deny and workspace-isolation tests could stay byte-identical.
2. **Exclusive ownership.** Precisely *because* a timer fires the run id the sweep would, running both
   would race the idempotency read. So `tick_once`'s interval leg is **replaced** by
   `reconcile_interval_timers` — timers own every flip-flop, or nothing does. Cron is untouched and
   keeps the 5s sweep: cron has absolute wall-clock instants at minute granularity, so a durable
   cursor is exactly right and the sweep is invisible there.
3. **Teardown is structural.** The registry is the sole owner of each `JoinHandle`, and `LiveTimer`
   aborts its task on `Drop` — so removing a key from the map *is* the teardown. The scope calls a
   leaked timer worse than the bug being fixed (a flow the operator disabled but that keeps firing is
   silent and compounding); that must not depend on remembering to call `abort()`.

The 5s tick is now the **convergence** cadence — how fast an enable/disable/period-edit takes effect —
never a floor on how often a node can fire.

## What changed

- **`crates/host/src/flows/interval_timers.rs`** (new) — `IntervalTimers` registry, `TimerKey`,
  `reconcile_interval_timers`, and the per-node `timer_loop`. Workspace-walled: a pass converges
  exactly one ws and never touches another ws's keys.
- **`crates/host/src/flows/react_interval.rs`** — added `fire_flipflop_node`, the single-node fire
  seam the timer drives. The fire *logic* stays in the module that owns it; the timer owns only
  cadence and lifecycle (one responsibility per file). A disabled/deleted flow, or a node that is no
  longer a flip-flop, is a silent no-op tally — racing the reconciler's teardown must not error.
- **`crates/host/src/flows/reactor_loop.rs`** — owns the `IntervalTimers` registry; `tick_once`
  reconciles instead of scanning-and-firing.
- **`crates/flows/src/builtins/core.rs`** — `period_secs` `minimum` dropped **5 → 1**. The floor is
  now the durable cursor's unit (whole seconds), not a deployment constant.

### Forward-progress guards in `timer_loop`

Two shapes that would otherwise be a hot spin loop against the store, handled explicitly rather than
by a blanket sleep floor (which would add latency to every period):

- An **uninitialised cursor** makes the fire path take its init branch and return *without* firing —
  legitimate, and possible at most once per timer (init writes a non-zero cursor). So that iteration
  goes straight round instead of backing off; otherwise every enable would cost a dead period.
- Any **other** non-advance (an error, or the flow disabled mid-flight) sleeps a full period before
  retrying, so a persistently failing node can never spin — and cannot affect any other node's timer.

## Verification

```
cargo fmt --check                                          clean
cargo test -p lb-host --lib                               379/379
cargo test -p lb-host --test flows_interval_timers_test     7/7    (new)
cargo test -p lb-host --test flows_flipflop_test            9/9
cargo test -p lb-host --test flows_multi_trigger_test       5/5
cargo test -p lb-host --test flows_triggers_test           19/19
cargo test -p lb-host --test flows_run_test                49/49
cargo test -p lb-flows                                    100/100
```

**Pre-existing, not caused by this work** (established against a full `-p lb-host` baseline taken on
the merged tree *before* these edits): `flows_plc_reliability_test::concurrent_same_run_id_never_
conflicts_and_settles_once` fails with a SurrealDB read/write conflict. It fails identically with
these changes stashed, and in isolation, so it is a real pre-existing defect on this tree rather than
a load flake — worth its own entry, out of scope here. The same baseline also showed `agent_suite`
(9), `ext_boot_spawn`/`fleet_monitor` (the known wasm/sidecar build prerequisite), `federation_test`
(7) and `document_store_test` (1) red; none touch flows.

The two **mandatory categories** are satisfied by tests that pass **unmodified**, which is the actual
claim being made — the new firing mechanism must grant nothing:

- **Capability deny** — `flows_flipflop_test::flipflop_capability_deny_no_run_no_state`.
- **Workspace isolation** — `flows_flipflop_test::flipflop_workspace_isolation`, plus a new
  reconciler-level `a_reconcile_pass_never_touches_another_workspaces_timers` covering **both**
  directions (a ws-B pass may neither start a ws-A timer nor retire one — the teardown half is what a
  naive "remove everything not desired" reconciler gets wrong, and it would silently stop a healthy
  tenant's flows).

New suite `flows_interval_timers_test.rs`:

| Test | What it pins |
|---|---|
| `enable_spawns_one_timer_and_disable_tears_it_down` | the basic lifecycle |
| `reconciling_an_unchanged_graph_is_a_no_op_no_double_spawn` | the pass runs every tick, so "converged" must mean "does nothing" — a second spawn would silently double the firing rate, and only in production where the tick repeats |
| `a_period_edit_replaces_the_timer_rather_than_duplicating_it` | edit = teardown + restart, so the new cadence applies at once |
| `repeated_enable_disable_leaves_no_orphan_timer_still_firing` | **the orphan/leak test** |
| `a_reconcile_pass_never_touches_another_workspaces_timers` | rule 6, both directions |
| `a_one_second_period_really_fires_about_once_a_second` | **the regression this scope exists for** |
| `the_oscillator_value_keeps_flipping_across_timer_firings` | the value is still durable state |

Two of these deserve their reasoning stated, because a weaker version of each would pass against
broken code:

**The leak test: two wrong versions before a right one.** This is the most useful thing the session
produced, so it is worth the space.

*Version 1* asserted the obvious thing — after five enable/disable cycles, open a quiet window and
assert no new runs fire. It **failed against correct code**: each enable legitimately gets one firing
(the oscillator emits as soon as it is armed), slots are whole seconds, and a legitimate pre-disable
firing shares the boundary second with any `T` sampled just after the disable. Fixed by comparing the
**set** of fired slots across the window instead of "nothing after `T`" — set-equality has no boundary
to straddle.

*Version 2* was that fixed behavioural assertion. Then the mandatory revert-check — gut
`LiveTimer::drop` so it leaks, confirm the test goes red — showed it **passing 7/7 against
deliberately-leaking code**. Two independent maskers:

- the fire path re-reads the flow and finds it **disabled**, so an orphaned timer fires nothing. The
  `enabled` re-check is an **inner gate** that makes "did anything fire?" blind to whether the task
  still exists;
- run-id idempotency hides it a second way — even two live timers on one node cannot produce two runs
  for one slot.

*Version 3* asserts the property **only teardown has**: the task is actually gone
(`num_alive_tasks`). A leak's real cost here is a task polling the store forever, once per period, per
orphan, compounding with every cycle — so the measure is the task census, not an effect. Measured
over the same five cycles:

| `LiveTimer::drop` | alive tasks | result |
|---|---|---|
| gutted (leaking) | baseline **+6** | **FAILS** |
| real | baseline **+1** | passes |

The `+1` is an in-flight detached run-drive task from `flows_run_async`; it is O(1) while an orphan is
O(cycles), which is what gives the assertion its teeth. The test drains transients and allows a fixed
tolerance of 2 — chosen from those measurements, and re-revert-checked *with* the tolerance in place
to confirm it had not been blunted (still failed at +6).

The lesson generalises past this module, and it is a **recurrence**: this repo has been bitten before
by a reader-task leak test that passed against a deliberately-broken restart. When the code under test
has an inner gate that independently produces the "correct" outcome, an effect-based assertion cannot
distinguish teardown from a leak. Assert the resource, and always revert-check — the first two
versions of this test both looked completely reasonable, and neither worked.

**The 1s-cadence test cannot pass without a real timer.** Under the old sweep it could reach at most
*one* firing in its window, so it fails against the pre-Phase-2 mechanism by construction rather than
by a tuned threshold. Timing assertions are a flakiness magnet (scope, Risks), so it asserts a
generous bound — several firings on distinct **consecutive one-second slots** inside a ~4s window,
plus "the newest firing tracks the wall clock" for no-drift — never an exact count or an exact
instant. Run ids are deterministic per scheduled instant, so *existence of the job* is the firing
record: no scanning and no ordering assumptions.

## What is still not possible

**Sub-second periods.** `FlowTriggerState.next_attempt_ts` is whole seconds, so fractional
`period_secs` needs a **millisecond field on that record** — and it is a closed struct, so a new axis
is silently dropped until the Rust struct carries it (additive + nullable, per the store's upgrade
rules: absent-key and present-null are two different upgrade bugs and `#[serde(default)]` only covers
one). It also needs a **run-retention policy for high-frequency sources** first: a 100ms oscillator is
10 real runs/sec of store writes, which makes it trivial to author a flow that hammers the store.
Both are prerequisites, not polish — the schema floor stays at 1s until they exist. Tracked as Phase
2b in the scope.

## Next

**Phase 3 — loopbacks** ([`flow-loopback-scope.md`](../../scope/flows/flow-loopback-scope.md)):
`feedback: true` edges whose crossing enqueues a **new durable run** rather than re-entering the live
one (race-safety by construction), a hop budget (default 100, `loopLimit` outcome), and an optional
per-edge pace. Its prerequisite — spawned firings — landed in Phase 1. The flagged interaction to
watch: a feedback firing must **bypass `Concurrency::Skip`**, or a looping flow deadlocks its own loop.
