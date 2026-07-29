# Flows scope — the interval-source clock (why `period_secs` doesn't mean seconds)

Status: **scope (the ask)**. Promotes to `doc-site/content/public/flows/flows.md` once shipped.

> Read the spine first: [`flows-scope.md`](./flows-scope.md) owns the canonical **Decisions (v1)**;
> [`flip-flop-node-scope.md`](./flip-flop-node-scope.md) is the node this breaks;
> [`triggers-lifecycle-scope.md`](./triggers-lifecycle-scope.md) owns the durable cron clock the
> flip-flop was made to ride; [`flow-multi-trigger-reactive-scope.md`](./flow-multi-trigger-reactive-scope.md)
> owns the per-node trigger cursors. This is a **mechanism** ask, not a content ask — no new node.

A `flipflop` node configured `period_secs: 1` fires **once every 5 seconds**, not once a second, and
its schedule cursor falls permanently behind the wall clock. This is not a bug in the flip-flop: the
node has **no clock of its own**. Every self-driving trigger in flows is a durable row that a single
5-second workspace sweep occasionally notices, so the sweep interval is a hard floor on every
interval node, and any period shorter than it silently runs slow and drifts without bound. The
config field accepts a number the engine cannot honour. This scope names that mismatch, decides
which trigger kinds should keep the sweep, and proposes giving interval sources a real timer.

## The evidence

Measured on a live node (`asdasd`, `period_secs: 1`, 283 runs) — the scheduled instants advance
correctly by 1s, but the wall-clock arrivals are exactly 5s apart:

```
run id (scheduled instant)              wall ts      Δ scheduled   Δ wall
asdasd-flip-flipflop-1-1785292628       1785292705         1          5
asdasd-flip-flipflop-1-1785292627       1785292700         1          5
asdasd-flip-flipflop-1-1785292626       1785292695         1          5
asdasd-flip-flipflop-1-1785292625       1785292690         1          5
```

Two distinct defects fall out of one cause:

1. **The 5s floor.** [`spawn_flow_reactors`](../../../rust/crates/host/src/flows/reactor_loop.rs) is
   spawned with `Duration::from_secs(5)` ([`node/src/reactors.rs:24`](../../../rust/node/src/reactors.rs)).
   A trigger fires only when that sweep runs, so **no interval below 5s is expressible**, whatever the
   config says.
2. **Unbounded drift.** [`fire_one_flipflop`](../../../rust/crates/host/src/flows/react_interval.rs)
   advanced its cursor by exactly `scheduled_ts + period_secs` — one period per sweep, regardless of
   how far behind it was. At `period_secs: 1` the cursor gains 1s per 5s elapsed, so it slides 4s
   further behind every tick (observed ~60s adrift). The module doc already promised
   *"fire-once-then-skip-to-next-future-slot"*; the code never skipped.

Defect 2 is a straight bug against the documented policy and is **already fixed** in this session
(`next_slot_after`, matching `react_cron`'s `next_after(schedule, now)`). Defect 1 is the design
question this scope exists for — the fix makes the clock *honest* (no drift) but still cannot make
`period_secs: 1` mean one second.

The original scope saw this coming and wrote it down: *"the reactor scans every few seconds —
sub-second resolution is impossible, so seconds is the honest unit."* That reasoning is sound for the
mechanism it assumed; the gap is that `minimum: 1` in the descriptor schema then advertises a
resolution the mechanism cannot deliver.

## Goals

- An interval source fires at its configured period, in real time, with no accumulating drift.
- Sub-5s periods are either **genuinely supported** or **structurally impossible to configure** —
  never accepted-then-approximated.
- Cron triggers keep their durable-cursor semantics unchanged (they are correct today).
- A restart, a disable, or a period edit leaves the runtime in a defined state — no orphan timers,
  no double-fire, no lost oscillator.

## Non-goals

- **Not** a rewrite of the flows execution engine. Within a run, node-to-node propagation is already
  reactive and is not in question here — this is only about what *starts* a run.
- **Not** changing cron. Wall-clock schedules stay on the durable cursor + sweep.
- **Not** millisecond control-loop guarantees. Even with a timer, this is a general-purpose flow
  runtime on a shared executor, not a real-time controller. We should say what we *do* guarantee.
- **Not** backfilling missed firings. The flip-flop contract is "hold each value for `period_secs`";
  replaying skipped slots after a pause would burst runs.

## Intent / approach

**Split the two trigger kinds by mechanism, because they are different problems.**

| Trigger kind | Nature | Right mechanism |
|---|---|---|
| `trigger mode:"cron"` | absolute wall-clock instants, coarse (minute+) | durable cursor + sweep — **keep** |
| `flipflop` (and future interval nodes) | relative "every N", potentially fine-grained | **a real timer owned by the enabled flow** |
| `webhook` | event-driven, no clock | series cursor — unaffected |

A cron trigger *must* survive restart and fire at an absolute time; a durable cursor is exactly right,
and a 5s sweep is far below cron's own resolution, so the floor is invisible there. An interval
oscillator has no absolute instants at all — it has a period. Modelling a period as a row that a
coarse sweeper polls is the category error: it costs a full workspace store scan to approximate what
`sleep(period)` does for free, and it is still wrong at any period below the sweep.

**Prior art — [logic-mesh](https://github.com/rracariu/logic-mesh)** (a reactive Rust dataflow engine
in the same problem space, HVAC/BAS control): every block *is* a `Future` driven on Tokio, and it
awaits either an input change or a real `sleep_millis`. Its base cadence is a 200ms default that any
block may ignore — `OnDelay` reads its delay in **milliseconds** against `current_time_millis()`.
Notably it ships *both* a `Schedule` block and timer blocks, and they do **not** share a mechanism —
the same split proposed here. Node-RED does the same thing: `inject` holds its own interval.

### Options

**Option A — interval sources own a timer (recommended).** One task per enabled interval node,
sleeping its period, emitting, tearing down on disable. `period_ms` becomes meaningful; sub-second is
real. This is what logic-mesh and Node-RED do.

*Cost, stated plainly:* it argues an explicit exception to the "no long-lived in-process timer"
posture that [`react_interval.rs`](../../../rust/crates/host/src/flows/react_interval.rs) opens with,
and that `flows-scope.md` §Decision 9 reflects ("no long-lived parked runs"). That exception must be
**argued in the open**, not smuggled in as a `tokio::interval` — see Risks. It also adds
reconciliation surface: enable/disable/edit/delete/restart must each converge the live task set
against the durable graph.

**Option B — keep the sweep, enforce the floor.** Raise the descriptor's `minimum` on `period_secs`
to the sweep interval and reject anything below. Cheap, no architecture change, and it makes the
engine honest — but permanently caps interval nodes at 5s, and hard-codes a *deployment* constant
(the tick) into a *schema*, which is its own smell.

**Option C — tighten the sweep to 1s.** Rejected. It is still a sweep — a full ws-scoped store scan
per second per workspace to emulate a timer — and it merely moves the floor from 5s to 1s while
multiplying steady-state load on every node, including those with no interval triggers at all.

**Recommendation: A, with B's schema honesty as the interim.** Ship the `minimum` clamp now so the UI
stops accepting periods the engine cannot serve (a one-line, shippable truth), and scope A as the
real fix. If the answer to Open Q1 is "5s is fine forever", B alone is a legitimate end state — but say
so deliberately rather than by accident.

## How it fits the core

- **Tenancy / isolation:** a timer task is minted per (workspace, flow, node) and carries the
  workspace in its principal exactly as the sweep does today. A ws-B timer can never fire a ws-A
  flow. The reconciler is ws-scoped; the existing `flipflop_workspace_isolation` test generalises.
- **Capabilities:** unchanged. A timer-fired run dispatches through the same `reactor_caps()` system
  principal and hits the identical caps wall — the firing mechanism grants nothing. The existing
  `flipflop_capability_deny_no_run_no_state` test must stay green verbatim.
- **Placement:** `either`. The timer runs wherever the flow's placement already puts its reactors;
  role stays config, never a code branch.
- **MCP surface:** no new verbs. `flows.node_state` already reports the interval schedule as of this
  session's fix (`periodSecs` / `nextAttemptTs` / `armed` per node) — that stays the read surface, and
  with Option A it reports the live timer's next deadline. Per §6.1: **CRUD** N/A (no new records),
  **get/list** already covered by `flows.node_state`, **live feed** already covered by
  `flows.watch`, **batch** N/A.
- **Data (SurrealDB):** `FlowTriggerState` remains the durable record — under Option A it holds the
  oscillator's **value** (`flop`) and last-fire instant so the value survives restart, while the
  *motion* (when to next fire) moves into the timer. This is a cleaner state/motion split than today,
  where one row conflates both.
- **Bus (Zenoh):** unchanged; firings already surface as run events.
- **Sync / authority:** node-local. An interval source is meaningless to backfill from another node.
- **Secrets:** N/A.

## Example flow

1. Author sets `flipflop-1` to `period_secs: 1` on flow `asdasd` and deploys.
2. `flows.save` validates against the descriptor. **Today:** accepted. **After B:** rejected unless
   ≥ the floor, with the reason naming the sweep interval.
3. Author enables the flow.
4. **Today:** the 5s sweep notices the cursor is due and fires one instant per tick — 1 run per 5s,
   cursor drifting 4s further behind each tick.
   **After A:** the reconciler spawns one timer for `flipflop-1`; it sleeps 1s, emits `true`, sleeps
   1s, emits `false`, … Each firing persists `flop` durably.
5. Canvas polls `flows.node_state`; the banner counts down against the live deadline.
6. Author disables the flow → the reconciler tears the timer down; no orphan task, no further runs.
7. Author edits the period to 250ms → reconciler replaces the timer; the oscillator keeps its value,
   restarts its cadence.
8. Node restarts → reconciler rebuilds timers from the durable graph; `flop` resumes from the store,
   so the oscillator continues on the correct side rather than re-seeding to `start`.

## Testing plan

Mandatory categories from [`scope/testing/testing-scope.md`](../testing/testing-scope.md):

- **Capability deny** — a timer-fired run with the flows-run cap removed writes no run and no state
  (existing `flipflop_capability_deny_no_run_no_state`, must pass unchanged).
- **Workspace isolation** — a ws-B reactor/timer never fires a ws-A interval node.
- **No mocks** — real store (`mem://`), real jobs, real caps. Timing tests inject the clock where the
  logic is pure; where a real `sleep` is unavoidable, assert on *ordering and count within a
  generous window*, never on exact wall-clock equality (flaky-test risk, see Risks).

Key cases:

- **The regression this scope exists for:** an interval node with `period_secs` **below** the sweep
  interval fires at its configured rate (A), or is rejected at save with a clear reason (B). This is
  the test that fails today.
- **No drift:** over N periods the k-th firing's scheduled instant is within one period of
  `start + k*period` — pins defect 2 permanently (the `next_slot_after` fix should be covered here).
- **Fire-once-then-skip:** after a pause longer than several periods, exactly one firing occurs and
  the cursor lands strictly in the future — no backfill burst.
- **Value continuity:** `flop` survives a store round-trip and a simulated restart (existing
  `flipflop_value_survives_a_store_round_trip`).
- **Lifecycle (new surface, Option A):** enable spawns exactly one timer; disable tears it down;
  a period edit replaces rather than duplicates it; delete removes it; double-enable does not
  double-fire. **Orphan/leak test:** enabling and disabling N times leaves zero live tasks.
- **`flows.node_state` parity:** the reported next-fire instant tracks the live timer.
- **E2E (rubix-ai):** the canvas banner counts down and the run counter advances at the configured
  rate — the user-visible symptom that opened this.

## Risks & hard problems

- **The rule-4 exception is the crux, and it must be argued, not assumed.** "State in the store,
  motion on the bus, no long-lived in-process timers" is load-bearing across lb. The honest framing
  is: an interval source's *value* is state (durable, in the store) and its *cadence* is motion —
  today we wrongly persist the cadence and poll it. Option A does not weaken the rule so much as
  apply it correctly. **If reviewers don't accept that framing, the answer is Option B, not a quiet
  timer.**
- **Timer/graph reconciliation is where this will actually break.** Every mutation path
  (enable/disable/edit/delete/restart/placement change) must converge the live task set. Leaked
  tasks are silent and compounding — a flow disabled but still firing is far worse than the bug we
  are fixing. This deserves a single owner module and the orphan test above.
- **Timing tests are a flakiness magnet.** Any assertion on real elapsed time will eventually fail
  in CI. Keep the schedule *arithmetic* pure and clock-injected (as `next_slot_after` now is); let
  only a thin lifecycle layer touch real time.
- **Load at small periods.** A 100ms oscillator is 10 runs/sec, each a real run with store writes and
  retention pressure. Sub-second periods make it trivial to author a flow that hammers the store. A
  floor (even a low one) and/or run-retention policy for high-frequency sources needs deciding.
- **Scope creep into "every node is a task".** This is deliberately *only* about what starts a run.
  Turning flows into logic-mesh's per-block-Future model is a much larger redesign and is explicitly
  out of scope here.
- **Downstream contract.** If `period_secs` becomes `period_ms`, existing saved flows must migrate or
  both must be accepted. Prefer additive: keep `period_secs`, add finer granularity as a separate
  field or accept fractional seconds.

## Open questions

1. **Do interval sources need to be genuinely sub-second, or just fast-and-honest?** BAS point
   control implies yes and selects Option A; heartbeat/demo pulses imply Option B is sufficient
   forever. **This single answer picks the option** — it is the question to settle first.
2. If Option A: what is the **lowest permitted period**, and is it enforced by schema, by config, or
   by a per-workspace policy? (A hard schema minimum is simplest; a deployment constant in a schema
   is a smell.)
3. Does the unit change to milliseconds (`period_ms`), or stay seconds with fractional values? Names
   the migration story for already-saved flows.
4. Should the timer own only `flipflop`, or a general **`interval` trigger kind** that future
   self-driving nodes (pulse, sampler, poller) inherit? Scoping the seam once is cheaper than
   retrofitting it per node.
5. Does the 5s flow-reactor sweep stay at 5s for cron once intervals leave it? (Probably yes — cron
   is minute-granular — but it should be a decision, not an inherited constant.)

## Skill doc

**N/A.** This changes the *mechanism* behind an existing surface; it adds no new MCP verb, gateway
route, or automatable task. `flows.node_state` / `flows.save` / `flows.enable` keep their shapes, so
the existing flows skill coverage stays accurate. If Option A introduces an operator-facing verb for
inspecting live timers, that flips to a skill update — flag it at implementation time.

## Related

- [`flip-flop-node-scope.md`](./flip-flop-node-scope.md) — the node this breaks; its "sub-second
  resolution is impossible" note is the earliest statement of this constraint.
- [`triggers-lifecycle-scope.md`](./triggers-lifecycle-scope.md) — the durable cron clock (stays).
- [`flow-multi-trigger-reactive-scope.md`](./flow-multi-trigger-reactive-scope.md) — the per-node
  trigger cursors this reads and writes.
- [`flow-persistent-runtime-scope.md`](./flow-persistent-runtime-scope.md) — `flows.node_state`, the
  read surface for the schedule.
- [`flows-scope.md`](./flows-scope.md) — Decisions (v1), incl. Decision 9 on long-lived runs.
- [logic-mesh](https://github.com/rracariu/logic-mesh) — prior art: per-block `Future`s, ms-resolution
  timers, and a `Schedule` block kept separate from timer blocks.
- Implementation touchpoints:
  [`react_interval.rs`](../../../rust/crates/host/src/flows/react_interval.rs),
  [`reactor_loop.rs`](../../../rust/crates/host/src/flows/reactor_loop.rs),
  [`node/src/reactors.rs`](../../../rust/node/src/reactors.rs),
  [`builtins/core.rs`](../../../rust/crates/flows/src/builtins/core.rs).
