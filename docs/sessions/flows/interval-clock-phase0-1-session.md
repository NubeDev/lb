# Session — interval-source clock, Phases 0 + 1 (honest sweep + decoupled firing)

- Scope: [`../../scope/flows/interval-source-clock-scope.md`](../../scope/flows/interval-source-clock-scope.md)
- Debugging entry: [`../../debugging/flows/interval-cursor-drifts-and-fires-serially.md`](../../debugging/flows/interval-cursor-drifts-and-fires-serially.md)
- Follow-on scope written this session: [`../../scope/flows/flow-loopback-scope.md`](../../scope/flows/flow-loopback-scope.md)
- Date: 2026-07-29

## The ask

Flows had "felt off/buggy" for months. Peer review traced it to **three defects behind one cause**
and the owner then widened the target: *"it needs to be like Node-RED — a node can take many inputs,
fire its output many times, many nodes all firing at once and when they need, with no restrictions —
and loopbacks must be handled in a smart way without race conditions."*

That answer selected **Option A** (per-node interval timers) as the destination and made the
decoupled-firing work **mandatory rather than optional** — concurrency is the requirement, the clock
is only half of it. The scope's staged plan follows: Phase 0 (honest sweep), Phase 1 (decouple firing
from running), Phase 2 (the timer reconciler), Phase 3 (loopbacks).

**This session shipped Phases 0 and 1.**

## What changed

### Phase 0 — make the sweep honest

- **`crates/host/src/flows/react_interval.rs`** — added the pure, clock-injected
  `next_slot_after(scheduled_ts, period_secs, now)`: the next slot on the period grid **strictly
  after `now`**, reached in one step (`scheduled + (elapsed/period + 1) * period`). It replaces the
  blind `scheduled_ts + period_secs` in **both** the fire path and the idempotent-skip path — the
  latter mattering as much as the former, since a re-scan that advanced by one period re-introduced
  the drift the fire path had just fixed. A corrupt zero period is guarded (`period.max(1)`) so it can
  neither divide by zero nor pin the cursor in place. Four unit tests land **in the same change**
  (see "Why the test had to land with the fix").
- **`crates/flows/src/builtins/core.rs`** — the `flipflop` descriptor's `period_secs` `minimum` moved
  `1 → 5`, enforced at save via `save.rs` `validate_config`. This is **deliberately interim**: it
  stops the UI accepting a period the engine cannot serve, and drops back to fractional seconds when
  Phase 2's timers land. Writing the deployment's sweep constant into a schema is a smell we are
  accepting *temporarily and in the open* (the scope argues this as Option B's "schema honesty").

### Phase 1 — decouple firing from running

- **`react_interval.rs`, `react_cron.rs`, `react_source.rs`** — every reactor firing now takes the
  `flows_run_async` **seed-durably-then-drive-detached** seam instead of `run_flow_to_completion(…)`
  inline. The job + run record still exist when the call returns, so the idempotency check that reads
  `lb_jobs::load` before firing keeps working unchanged; what changes is that N due nodes fire
  independently and no subgraph blocks the sweep. This is the shape the **manual** run path already
  had — Phase 1 is largely "make the reactor path look like the path that was already right".
- **`react_interval.rs`, `react_cron.rs`** — per-node **log-and-continue** replaces `?`. A failing
  node is logged with ws/flow/node and the pass **continues**; the next tick retries it. Previously
  one broken flow aborted the rest of the workspace's triggers for that tick.

### Tests

- **`crates/host/tests/flows_flipflop_test.rs`** — new
  `flipflop_late_scan_fires_once_and_lands_the_cursor_in_the_future`: prime at t=100 with period 10,
  scan at t=157 → **exactly one** run fires, the cursor lands at **160** (strictly future — no
  backfill burst), and an immediate re-scan is a no-op.
- **`crates/host/tests/flows_multi_trigger_test.rs`** —
  `counter_node_increments_across_firings` now awaits each spawned run via `run_snapshot` before
  reading counter memory. This is the general consequence of Phase 1 and worth stating loudly:
  **reactor tests may no longer assert immediately after a reactor pass** — firings are asynchronous,
  so the assertion races the run. Poll `flows.runs.get` until terminal. (The counter itself is an
  atomic `lb_store::increment`, so it is correct under overlap; only the *observation* needed fixing.)

## Why the test had to land with the fix

An earlier AI session **claimed the drift fix was already shipped** — same symbol name
(`next_slot_after`), same rationale. Peer review found the symbol **nowhere in the tree**, no fix
commit in `git log` on `react_interval.rs`, and both paths still advancing `scheduled_ts + period`.
The bug was "fixed" in a session log and live in production for weeks.

The structural answer is not "be more careful": it is that **the schedule arithmetic is now a pure
function with its own unit tests**. A claimed fix that carries a failing-before test cannot silently
un-land. The scope was amended to record the false claim rather than quietly re-fixing it, and the
same discipline applies to this document: the verification below is the evidence, not the claim.

This session also had to re-verify its own handover the same way — the handover described Phase 0+1 as
uncommitted working-tree changes, and they were in fact already committed to `master` by a parallel
session (`5bb89ae7`). Verified against the tree before continuing; nothing was re-applied.

## Verification

```
cargo test -p lb-host --lib react_interval        4/4   (next_slot_after arithmetic)
cargo test -p lb-host --test flows_flipflop_test  9/9
cargo test -p lb-host --test flows_multi_trigger_test          5/5
cargo test -p lb-host --test flows_triggers_test              19/19
cargo test -p lb-host --test scheduled_rules_test             14/14
cargo test -p lb-host --test rules_workflow_convergence_test  12/12
cargo test -p lb-host   (full)      — see below
cargo test -p lb-flows  (full)      — see below
cargo fmt
```

The two **mandatory categories** (`scope/testing/testing-scope.md`) are satisfied by tests that had to
pass **verbatim, unmodified** — which is the actual assertion being made, since the firing mechanism
must grant nothing:

- **Capability deny** — `flipflop_capability_deny_no_run_no_state`: a reactor-fired run with the
  flows-run cap removed writes no run and no state. A *spawned* firing must hit the identical caps
  wall as an inline one.
- **Workspace isolation** — `flipflop_workspace_isolation`: a ws-B reactor never fires a ws-A
  flip-flop.

No mocks: real `mem://` store, real jobs, real caps (rule 9). The timing-sensitive logic is pure and
clock-injected, so **no test asserts on real elapsed time** — the flakiness magnet the scope's Risks
section names.

## What is NOT fixed

**Defect 1 — the 5s sweep floor — is still there.** Phase 0 makes it *honest* (a sub-floor period is
now rejected at save with a reason) but does not remove it. `period_secs: 1` remains unexpressible
until Phase 2. This is stated plainly because the temptation after a green suite is to read "drift
fixed" as "clock fixed"; it is not.

## Next — Phase 2 (Option A, the real fix)

Per-node interval timers owned by **one reconciler module** (`flows/interval_timers.rs`), converging
live timer tasks against the durable enabled graph on enable / disable / edit / delete / restart. The
timer tick calls the **same idempotent fire path** (deterministic run id, durable `flop` in
`FlowTriggerState`) — the *value* stays durable in the store, only the *cadence* moves to the timer.
That is the state-vs-motion framing the scope uses to argue the rule-4 exception **in the open**: do
not smuggle a quiet `tokio::interval` in anywhere outside that module. Generalise as an `interval`
trigger kind with `flipflop` as its first tenant; cron stays on the 5s sweep (its own resolution is
minute-granular, so the sweep is invisible there).

Mandatory tests for Phase 2, per the scope: lifecycle orphan/leak (enable+disable N times → **zero**
live tasks — a leaked timer firing a disabled flow is worse than the bug being fixed), no double-fire
on double-enable, ws-isolation, cap-deny unchanged, `flows.node_state` parity with the live deadline.
Only then lower the schema floor (fractional `period_secs`, floor ~100ms–1s), gated on a run-retention
policy for high-frequency sources — a 100ms oscillator is 10 runs/sec of real store writes.

Then **Phase 3 — loopbacks** ([`flow-loopback-scope.md`](../../scope/flows/flow-loopback-scope.md)):
`feedback: true` edges whose crossing enqueues a **new durable run** rather than re-entering the live
one (race-safety by construction), a hop budget (default 100, `loopLimit` outcome), optional per-edge
pace. Its prerequisite — spawned firings — is met as of this session. Flagged interaction to watch: a
feedback firing must **bypass `Concurrency::Skip`**, or a looping flow deadlocks its own loop.
