# Flows — N independent triggers per flow + per-trigger subgraph runs + a real counter

- Area: flows
- Status: shipped (green) — engine + tests + docs; UI per-trigger chips are a noted follow-up
- Scope: [`scope/flows/flow-multi-trigger-reactive-scope.md`](../../scope/flows/flow-multi-trigger-reactive-scope.md)
- Builds on: [`flow-plc-reliability-session.md`](./flow-plc-reliability-session.md) (the reactive-cron
  slice this de-flow-levels).
- Debug: [`debugging/flows/flow-level-cron-rejects-multiple-triggers.md`](../../debugging/flows/flow-level-cron-rejects-multiple-triggers.md)

## The ask

> "I've never said max one trigger. The whole design is wrong if that's the case — it needs to be like
> Node-RED, unlimited nodes." … "It's not a cron thing — what if I add a webhook or an MQTT-sub node?"

Plus the original ask it unblocks: "get the flow count working as a counter should — the count going
up." (Last slice shipped a `count` that is a *pure transform* — `len([1,2,3,4]) == 4` forever — so the
number never climbed. The real "goes up" needs a *stateful* node.)

## Reuse evaluation first (the user cloned four candidates)

Before building, evaluated **edgelinkd** (Node-RED-compatible Rust runtime), **reflow** (FBP actors),
**phlow** (sequential pipeline), **dora** (robotics dataflow) for reuse-as-library on a Raspberry Pi.
Verdicts: dora = wrong shape (continuous streaming, process-per-node); phlow = single-entry pipeline,
heavy OTel/tonic/git2; edgelinkd + reflow = genuine N-trigger per-wire models, but **both in-memory** —
adopting either means re-homing our durable/resumable/capability-gated/workspace-walled run-store onto
their actor lifecycle (the expensive part we already have, and which is *stronger* on in-flight
durability than they are). Decision: **keep our engine, evolve it to the per-node reactive model**, and
*borrow the ideas* — per-wire isolation (a firing flows only down its own subgraph) and durable
long-lived **node memory** (the counter). Full reasoning in the scope doc.

## Root cause (read end-to-end)

A flow's reactive identity was **hoisted to the flow level**, and a run executed the **whole graph**:
1. `flow.cron: Option<String>` + `flow.next_attempt_ts: u64` — one schedule, one cursor per flow.
   `save::derive_cron_from_trigger` collapsed all cron trigger nodes into that one field and **rejected**
   the save when two specs disagreed ("a flow has one schedule"). No room for two crons, two MQTT subs,
   two webhooks.
2. `run_store::create_run` enqueued **every** indegree-0 node — so one run fired *all* triggers at once;
   there was no "this trigger fired down its own wires."

## What shipped

**Per-node reactive triggers**
- New durable per-node cursor `flow_trigger_state:{flow}:{node}` (`FlowTriggerState{next_attempt_ts,
  cron}`) + `trigger_store.rs` (read/write + `cron_triggers(flow)` enumerator).
- `react_to_flows_cron` rewritten to scan **every cron trigger node** of every enabled flow, fire each
  due one independently (entry = that node), advance **that node's** cursor. Run id is now
  `{flow}-cron-{node}-{ts}` (the node segment lets two crons fire the same instant without colliding).
- `save` drops the collapse + single-schedule rejection (`validate_cron_triggers`): N cron triggers are
  valid; only a *malformed* spec is rejected (a typo surfaces at save, not a dead trigger).

**Per-trigger (per-wire) subgraph runs**
- `lb_flows::Flow::reachable_from(entry)` (downstream subgraph) + `indegrees_within(set)` (induced
  indegree — a join waits only on its in-subgraph upstreams; an out-of-subgraph `need` resolves to its
  retained/last value, never an unsatisfiable wait).
- `create_run` takes `entry: Option<&str>`: `Some` seeds only `reachable_from(entry)`; `None` keeps the
  whole-graph seed (manual "run all", resume, subflow). The run records `entry_node`; `finalize_if_
  complete` scopes to the run's node set (`run_node_set`) so a per-trigger run settles on **its** subgraph.
- `entry` threaded through `coordinator::start` → `run_flow_to_completion`/`flows_run`/`flows_run_async`.
  Wired at every firing site: cron (the trigger node), `flows.inject` (the inject node), reconcile boot
  (each `mode:boot` node), and `flows.run {entry|node}` (the canvas can fire one trigger). `runs.get`
  exposes `entryNode`.

**A real counter (durable node memory — the borrowed-from-Node-RED idea)**
- New `counter` builtin: reads its durable running total and **increments atomically** per firing, so the
  count GOES UP across runs and survives a restart. Delta = the input `items` size (throughput counter)
  or `config.step` (default 1); `reset` zeroes it.
- New store primitive `lb_store::increment` — a **server-side atomic accumulate** on `data.count` (the
  same in-statement trick `write` uses for `rev`), serialized per-key like `write_locked` so a retry can
  never double-add. Stored in a dedicated `flow_node_memory:{flow}:{node}` seam (distinct from
  `flow_node_state`, the last-output snapshot) — the foundation for future stateful nodes (rate, debounce,
  moving-average, state machines). `count` (the pure transform) is unchanged.

**node_state** now surfaces each trigger node's `{cron, nextAttemptTs, armed}` and a flow-level summary =
the **soonest** upcoming fire (back-compat for the existing armed banner).

## Tests (real store/jobs/caps — no mocks)

- `lb-store` `increment_test`: accumulate+reset, **64 concurrent firings each get a unique total
  1..=64** (atomic, no lost update), workspace-walled.
- `lb-host` `flows_multi_trigger_test`: multi-cron independence (two specs in one flow, distinct
  cursors), per-trigger subgraph isolation (fire A → only {A,x} run, `entryNode` recorded), counter goes
  1→2→3 across firings, cap-deny on `flows.run`, ws-isolation of the reactor.
- `lb-flows` model: `reachable_from` (downstream-only) + `indegrees_within` (out-of-subset needs not
  counted).
- Updated the two old flow-level-cron tests + the multi-trigger-derivation test (now: multiple distinct
  crons are **accepted**; malformed rejected) + the builtin-registry list (+`counter`).
- Full `lb-host` (64), `lb-store` (incl. increment), `lb-jobs`, `lb-flows` (29) green; `cargo fmt`
  clean; `cargo build --workspace` clean; UI `pnpm test` 195/195.

## Follow-ups (noted, not silently skipped)

- **UI per-trigger chips**: paint each trigger node's own armed/next-fire (the data is in `node_state`);
  the flow-level banner already shows the soonest. The canvas can already fire one trigger via
  `flows.run {entry}`.
- **Per-node enable/disable** (arm one trigger, disarm its sibling) — `enabled` is still flow-level.
- **Orphan cursor cleanup**: removing a cron trigger leaves its `flow_trigger_state` row dormant
  (harmless — never scanned); a delete-time sweep is a tidy-up.
- A native **http-in / webhook** source node is its own scope (sources already arm per-node).
</content>
