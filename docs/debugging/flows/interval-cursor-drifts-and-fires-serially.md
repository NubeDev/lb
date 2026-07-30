# A `flipflop` at `period_secs: 1` fires every 5s and drifts ~60s behind — three defects, one cause

- Area: flows (interval reactor clock + reactor firing concurrency)
- Status: resolved (all three defects; sub-second remains out of reach — see "What is still not fixed")
- First seen: 2026-07-29 (peer review of a months-old "flows feel off/buggy" report)
- Resolved: 2026-07-29 (defects 2 + 3 in Phase 0+1; defect 1 in Phase 2)
- Session: ../../sessions/flows/interval-clock-phase0-1-session.md,
  ../../sessions/flows/interval-timers-phase2-session.md
- Scope: ../../scope/flows/interval-source-clock-scope.md
- Regression tests:
  - `rust/crates/host/src/flows/react_interval.rs::tests` (4 — `next_slot_after` arithmetic)
  - `rust/crates/host/tests/flows_flipflop_test.rs::flipflop_late_scan_fires_once_and_lands_the_cursor_in_the_future`
  - `rust/crates/host/tests/flows_interval_timers_test.rs::a_one_second_period_really_fires_about_once_a_second`
    (defect 1 — cannot pass under the sweep, which could manage at most one firing in the window)

## Symptom

A `flipflop` node configured `period_secs: 1` fired **once every 5 seconds**, and its schedule cursor
fell **permanently further behind** the wall clock the longer the node ran (~60s adrift observed).
More generally: flows "felt off" — with several trigger nodes on a canvas, they did not fire
independently, and one broken node stopped the others firing at all.

## Reproduce

Live node `asdasd`, `period_secs: 1`, 283 runs. Scheduled instants advance by 1s each; wall-clock
arrival times are exactly 5s apart:

```
run id (scheduled instant)              wall ts      Δ scheduled   Δ wall
asdasd-flip-flipflop-1-1785292628       1785292705         1          5
asdasd-flip-flipflop-1-1785292627       1785292700         1          5
asdasd-flip-flipflop-1-1785292626       1785292695         1          5
```

The `Δ scheduled 1` vs `Δ wall 5` split is the whole bug in two columns: the cursor believes it is
keeping 1s time, the world sees 5s, and the gap between them accumulates forever.

## Root cause — three defects behind one cause

The cause is that **an interval source has no clock of its own**. Every self-driving trigger in flows
is a durable row that one 5-second workspace sweep occasionally notices
(`spawn_flow_reactors`, `Duration::from_secs(5)` at `node/src/reactors.rs`).

1. **The 5s sweep floor.** A trigger fires only when the sweep runs, so **no period below 5s is
   expressible**, whatever the config says. The descriptor advertised `minimum: 1` — a resolution the
   mechanism could not deliver. *(Design defect — Phase 2 fixes it; Phase 0 makes the schema honest.)*
2. **Unbounded drift.** `fire_one_flipflop` advanced its cursor by exactly `scheduled_ts + period_secs`
   — **one period per sweep**, regardless of how far behind it already was. At `period_secs: 1` the
   cursor gains 1s per 5s elapsed, sliding 4s further behind every tick, without bound. The module doc
   already promised *"fire-once-then-skip-to-next-future-slot"*; the code never skipped.
3. **Serial inline firing** (the likely main "feels off"). A reactor firing ran
   `run_flow_to_completion(...).await` **inline inside the sweep** — node after node, flow after flow,
   sequentially per workspace per tick. So ten trigger nodes never fired independently even at 5s
   granularity: one slow subgraph (an `ext-call`, a store-heavy branch) delayed every other trigger,
   and if a pass exceeded the sweep period `MissedTickBehavior::Skip` dropped ticks outright —
   everyone's period stretched. Compounding it, both reactors propagated a single node's error with
   `?`, **aborting the rest of the workspace pass** — one broken flow starved every other trigger
   until the next sweep.

## The meta-defect: a fix that was claimed but never landed

An earlier AI session **claimed defect 2 was already fixed** (`next_slot_after`, matching
`react_cron`'s `next_after(schedule, now)`). Peer review on 2026-07-29 found the fix **nowhere in the
tree**: no `next_slot_after` symbol, no fix commit in `git log` on `react_interval.rs`, and both the
fire path and the idempotent-skip path still advancing `scheduled_ts + period_secs`. The bug had been
"fixed" in a session log and left live in production for weeks.

This is why the no-drift regression test had to land **in the same change** as the fix — a claimed
fix with no test can silently un-land, and the next reader trusts the claim.

## Fix (Phase 0 + Phase 1)

1. **`next_slot_after(scheduled_ts, period_secs, now)`** (`flows/react_interval.rs`) — the next slot on
   the period grid **strictly after `now`**, computed in one step (`scheduled + (elapsed/period + 1) *
   period`), used in **both** the fire path and the idempotent-skip path. A late scan advances past
   `now` at once instead of one period per scan. Pure + clock-injected, so it is unit-testable with no
   wall clock; guards a corrupt zero period (`period.max(1)`) so it can neither divide by zero nor pin
   the cursor in place.
2. **Per-node error isolation** in `react_interval` and `react_cron` — a failing node is logged
   (`tracing::warn!` with ws/flow/node) and the pass **continues**, instead of `?`-aborting the
   workspace's remaining triggers. The next tick retries the failed one.
3. **Spawned firings** — reactor firings (interval, cron, and webhook sources) now take the
   `flows_run_async` **seed-durably-then-drive-detached** seam instead of `run_flow_to_completion`
   inline. The job + run record exist on return, so the idempotency check above still holds, but N due
   nodes fire independently and no subgraph blocks the sweep. This is the shape the *manual* run path
   already had.
4. **Honest schema floor** — the `flipflop` descriptor's `period_secs` `minimum` moved `1 → 5`
   (`crates/flows/src/builtins/core.rs`), enforced at save by `validate_config`. This is deliberately
   an **interim**: it stops the UI accepting a period the engine cannot serve, and drops back to
   fractional-seconds once Phase 2's per-node timers land.

## Verification

Unit: `next_slot_after` — on-time scan advances one period (100,10,100→110); **the drift regression**,
a late scan skipping to the next future slot (100,1,157→**158**, not 101; 100,10,157→160); a
future slot kept unchanged; a zero period still advancing.

Integration: `flipflop_late_scan_fires_once_and_lands_the_cursor_in_the_future` — prime the cursor at
t=100 with period 10, scan at t=157: **exactly one** run fires, the cursor lands at **160** (strictly
future, no backfill burst), and an immediate re-scan is a no-op.

The two mandatory categories stayed green **verbatim**, which is the point — the firing mechanism
grants nothing: `flipflop_capability_deny_no_run_no_state` and `flipflop_workspace_isolation`.

## Fix (Phase 2 — defect 1, the 5s floor itself)

`flows/interval_timers.rs`: a **per-node interval timer reconciler**, the one owner of live interval
cadence. It converges timer tasks against the durable enabled graph, and each timer sleeps to the
durable `next_attempt_ts` and then calls the **same** idempotent fire path the sweep called. The
sweep's interval leg is removed — a timer fires the same deterministic run id the sweep would, so
running both would race the idempotency read; timers own every flip-flop exclusively. Cron is
untouched and keeps the 5s sweep (minute granularity — the sweep is invisible there).

The 5s tick is now the **convergence** cadence (how fast an enable/disable/period-edit takes effect),
never a floor on firing. Descriptor `minimum` dropped `5 → 1`. Teardown is structural: the registry
solely owns each `JoinHandle` and `LiveTimer` aborts on `Drop`.

The rule-4 framing, argued in the scope rather than smuggled in: an interval source's **value** is
state (durable, in the store, unchanged by this) and its **cadence** is motion. The old design
persisted the cadence and polled it; a timer here owns no state at all — kill the process and the
reconciler rebuilds the set from the durable graph.

## What is still not fixed

**Sub-second periods.** `FlowTriggerState.next_attempt_ts` is whole seconds, so fractional
`period_secs` needs a millisecond field on that record (additive + nullable — it is a closed struct)
**and** a run-retention policy for high-frequency sources: a 100ms oscillator is 10 real runs/sec of
store writes. The schema floor stays at 1s until both exist (scope, Phase 2b).

## Prevention

- The drift arithmetic is now a **pure function with its own unit tests**, so it cannot un-land
  invisibly the way the first claimed fix did.
- Reactor tests must now **poll `flows.runs.get` until terminal** — firings are asynchronous, so
  asserting immediately after a reactor pass is a race. `counter_node_increments_across_firings` was
  updated to await each spawned run via `run_snapshot` before reading counter memory (the counter
  itself is an atomic `lb_store::increment`, so it is safe under overlap).
- The remaining defect (the 5s floor) is **named in the schema**, not hidden: a period below the floor
  is now rejected at save with the sweep interval as the reason, rather than accepted and approximated.

## Lesson

Three symptoms, one category error: **modelling a period as a durable row that a coarse sweeper
polls**. A cron trigger has absolute instants and a durable cursor is exactly right; an interval
oscillator has no instants at all — only a period — so the sweep costs a full workspace store scan to
approximate what `sleep(period)` does for free, *and is still wrong at any period below the sweep*.
Separately: **verify a claimed fix against the tree, never against the session log that claims it** —
and make the regression test land in the same change so the claim is self-checking.
