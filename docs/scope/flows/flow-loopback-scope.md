# Flows scope — loopbacks (feedback edges without races)

Status: **scope (the ask)**. Promotes to `public/flows/flows.md` once shipped.

> Read the spine [`flows-scope.md`](./flows-scope.md) first;
> [`flow-plain-wiring-scope.md`](./flow-plain-wiring-scope.md) owns the per-message engine
> (`fctx` firing contexts, universal `any` joins) this builds on;
> [`interval-source-clock-scope.md`](./interval-source-clock-scope.md) records the owner decision
> that opened this scope.

## The ask (owner, 2026-07-29)

> "It needs to be like Node-RED — a node can take many inputs, fire its output many times, many
> nodes all firing at once and when they need, with no restrictions — and we must make sure we
> don't get race conditions from loopbacks, and can handle loopbacks in a smart way."

Most of that sentence is already the shipped model: N independent triggers each firing their own
subgraph ([`flow-multi-trigger-reactive-scope.md`](./flow-multi-trigger-reactive-scope.md)), many
wires in/out of every port with per-message firing (`flow-plain-wiring`), envelopes flowing on
connect (`flow-message-envelope`). What is **not** possible today is drawing a wire *backwards*:
the graph is a validated DAG — Kahn cycle-detect **rejects the save** (`crates/flows/src/model.rs`,
`FlowError::Cycle`) — and `flows-scope.md` deferred loops to "retained inputs + the next firing".
The owner has overridden that deferral: a loop drawn on the canvas must work.

## Goals

- An author can wire a node's output back to an upstream node's input — the canvas accepts it, the
  save accepts it, and messages flow around the loop.
- **No race conditions by construction**, not by discipline: loop iterations must never mutate
  shared in-memory state; everything shared is durable and CAS-guarded (the existing run-store
  posture).
- **No hot loops.** Node-RED famously lets an unguarded wire loop peg the runtime; we do better —
  every loop has a bounded hop budget and an optional per-crossing pace.
- Resume, audit, cancel, and the persistent-runtime view keep working — a loop is observable, not a
  black hole.

## Non-goals

- **Not** a while-loop language feature. A loopback is a wire, not a control structure; `split`/
  `join`/rhai stay the tools for bounded iteration *within* a message.
- **Not** unbounded live runs. Decision 8/9 stand: a run is a durable job that terminates.

## Intent / approach — the feedback edge

**A cycle is never executed; it is authored.** The graph gains one edge attribute:

```jsonc
{ "from": "pid-out", "to": "setpoint-calc", "to_port": "payload", "feedback": true }
```

- **Validation:** the graph **minus feedback edges** must still be a DAG (Kahn unchanged). Any
  cycle that remains after removing feedback edges is still rejected, with the fix named in the
  error ("mark one edge in the cycle as feedback"). The canvas draws feedback edges distinctly
  (dashed, back-arc) and can offer to auto-mark when a user draws a cycle-closing wire.
- **Execution:** when a message settles across a feedback edge, it does **not** re-enter the live
  run. It **enqueues a new firing**: a fresh durable run entered at the target node (the same
  `entry`/spawned-run seam every trigger uses), params = the crossing envelope. The current run
  still terminates normally. This is the whole race-safety argument: *a loop iteration is a run*,
  so every iteration gets the pinned graph, the CAS step-claims, the `fctx`-scoped keys, and the
  run-store's conflict-safe writes that already exist. There is no new shared mutable state to
  race on — state shared between iterations lives where cross-run state already lives
  (`flow_context`, retained inputs, `flow_node_state`), all durable and store-arbitrated.
- **Hop budget:** the envelope carries `__loop: {hops, budget}`. Each feedback crossing increments
  `hops`; at `budget` (default **100**, settable per feedback edge) the crossing is dropped and the
  run records a `loopLimit` outcome on that edge — visible in the run view and the debug panel,
  never a silent stall or a silent runaway.
- **Pace (the smart part):** a feedback edge may set `min_interval_secs` (fractional). A crossing
  arriving sooner than the interval after the previous one is **coalesced** (latest envelope wins,
  delivered when the interval elapses) — the existing `coalesce.rs` debounce posture applied to the
  loop seam. A paced loop is how "PID recalculates at most every 2s" is authored without a timer
  node.

**Alternatives rejected:**

- *Raw cycles inside one run* (frontier revisits nodes, TTL in the message): breaks the pinned-DAG
  frontier math, makes "run to terminal" undefined, and turns resume into replaying an arbitrary
  interleaving. The entire durability story is built on the acyclic frontier — keep it.
- *Retained-inputs only* (the status quo): works, but the loop is invisible on the canvas and its
  cadence is welded to the next trigger firing instead of to the feedback event. The owner asked
  for the drawn wire.

## How it fits the core

- **Tenancy / isolation:** a feedback firing is spawned under the same ws-scoped principal as the
  crossing run; ws-B can never enqueue into ws-A. Existing isolation tests generalise.
- **Capabilities:** unchanged — a feedback firing dispatches through `flows.run` under the same
  principal and hits the same caps wall. Deny ⇒ no run, no state.
- **MCP surface:** no new verbs. `flows.save` validates the new attribute; runs list normally;
  the run record names its `parent_run`/edge so a loop's lineage is traceable. Per §6.1: CRUD via
  the existing flow save; get/list via existing run verbs; live feed via `flows.watch`/debug.
- **Data:** one additive edge field + `__loop` in the envelope + a small durable pace record per
  feedback edge (the coalesce buffer). No new table if the existing node-buffer table serves.
- **State vs motion:** the crossing is motion (a firing); anything held is durable (pace buffer,
  context) — rule 3 intact.

## Testing plan

- **Capability deny:** a feedback crossing with the run cap removed produces no child run, no state.
- **Workspace isolation:** ws-B cannot receive/trigger a ws-A feedback firing.
- **Budget:** a tight loop stops at exactly `budget` iterations with a durable `loopLimit` outcome.
- **No double-fire:** a re-driven (resumed) run does not re-enqueue an already-enqueued feedback
  firing — the crossing derives a deterministic child-run id from `(run, edge, fctx)`.
- **Pace:** N rapid crossings under `min_interval_secs` deliver once with the latest envelope.
- **Race regression:** two concurrent iterations writing the same `flow_context` key both settle;
  the store's CAS arbitration decides; no lost update.
- **DAG guard:** a cycle not covered by a feedback edge is still rejected at save.

## Risks & hard problems

- **Loop lineage explosion.** A 100-hop loop is 100 runs; run retention must treat loop children as
  one lineage (retain the head, trim the tail) or the run list drowns. Extend `retain_runs`.
- **Default budget is a policy, not a truth.** 100 is a guess; make it a flow-level override and
  surface `loopLimit` loudly so authors tune it instead of discovering silent drops.
- **Interaction with per-flow concurrency.** A feedback firing must bypass `skip` (it is the
  continuation of work, not a new trigger) or a looping flow with `skip` deadlocks its own loop —
  decide and test explicitly.
- **Prereq ordering.** This assumes reactor firings are already spawned-not-inline
  (`interval-source-clock` Phase 1); feedback firings enqueue through the same seam.

## Open questions

1. Does a feedback crossing inherit the *originating* trigger's `fctx` lineage (so `${steps.*}`
   from before the loop resolve) or start clean? Recommend: carry the envelope only — lineage
   refs across a loop boundary become a save lint, same posture as cross-branch refs.
2. Is the pace buffer per-edge or per-`(edge, topic)`? Per-edge is simpler; per-topic matches
   Node-RED's per-stream intuition. Start per-edge.

## Related

- [`flow-plain-wiring-scope.md`](./flow-plain-wiring-scope.md) — the per-message engine this rides.
- [`flow-context-scope.md`](./flow-context-scope.md) — the durable cross-iteration state.
- [`interval-source-clock-scope.md`](./interval-source-clock-scope.md) — the owner decision + the
  spawned-firing prerequisite.
- [`flow-run-scope.md`](./flow-run-scope.md) — the run/frontier machinery kept intact.
