# Flows scope — N independent triggers per flow + per-trigger subgraph runs (the Node-RED model)

Status: scope (the ask). Promotes to `public/flows/` once shipped. Extends
[`flow-plc-reliability-scope.md`](./flow-plc-reliability-scope.md) (the reliability + reactive-cron
slice that shipped just before this) and [`flow-run-scope.md`](./flow-run-scope.md) (the run engine).
Sibling: [`triggers-lifecycle-scope.md`](./triggers-lifecycle-scope.md) (the cron/reconcile/source
machinery this slice de-flow-levels).

## The ask (from the user, against the live node)

> "I've never said max one trigger. The whole design is wrong if that's the case — it needs to be like
> Node-RED, unlimited nodes." … "It's not a cron thing — what if I add a webhook or an MQTT-sub node?
> It's a core design issue."

The trigger for the complaint: saving `chain4` with **two cron trigger nodes** (`*/2 * * * *` and
`1 * * * *`) was **rejected** with "a flow has one schedule — use one cron trigger." That rejection is
a symptom; the disease is that a flow's reactive identity is modelled as **one trigger**.

## Root cause (read end-to-end in the code)

A flow's reactive state is **hoisted to the flow level**, and a run executes the **whole graph**. Two
independent design bugs:

1. **One trigger's worth of reactive state per flow.** The schedule/cursor/arm live as scalar fields
   on the `Flow` record, not on the source nodes:
   - `flow.cron: Option<String>` (`crates/flows/src/model.rs`) — one schedule;
   - `flow.next_attempt_ts: u64` — one cron cursor;
   - `flow.enabled: bool` — one arm switch.
   `flows.save::derive_cron_from_trigger` collapses *all* cron trigger nodes into that single
   `flow.cron` and **rejects** the save when two cron specs disagree. There is structurally nowhere to
   hold two cron cursors — and the same wall hits two MQTT subs or two webhooks (no per-node arm/route
   state). The reactor (`react_to_flows_cron`) scans **flows**, reading the one `flow.cron`.

2. **A run fires the whole DAG from every root.** `run_store::create_run` enqueues **every**
   indegree-0 node (`claim = Enqueued` for `indegree == 0`). So even with N trigger nodes, one "run"
   fires all of them at once — there is no notion of "*this* trigger fired and pushed a message down
   *its* wires."

In Node-RED a flow (tab) is a soup of nodes with **N independent source nodes** — each inject (cron),
mqtt-in, http-in is its own entry that emits a message down **only its downstream wires**, on its own
schedule/subscription. The reactive state belongs to the **node**, not the flow.

## The model decision (confirmed with the user)

- **N triggers per flow, each independent.** A flow may carry any number of trigger/source nodes —
  multiple crons, multiple MQTT subs, multiple webhooks — each with its **own** schedule/subscription
  and its **own** durable cursor/arm. No collapse, no "one schedule" rejection.
- **A firing runs only the triggered node's reachable subgraph** (the user's explicit choice over
  "whole graph with T as entry"). Firing trigger `T` injects a message at `T` and propagates through
  the nodes **reachable from `T`** (its transitive `dependents`). Indegree/fan-in is computed over the
  **induced subgraph**, so a join only waits on upstreams actually in `T`'s reachable set. Each firing
  is one run = one message; shared downstream nodes execute once **per firing** (Node-RED semantics).
- **`flow.enabled` stays the flow-level master switch** (deploy/undeploy the whole tab). Per-node
  enable is a follow-up; for v1 every trigger in an enabled flow is armed, each on its own cursor.

## Goals

- **Per-node cron cursor.** A durable `flow_trigger_state:{ws}:{flow}:{node}` record holds each cron
  trigger node's `next_attempt_ts`. The schedule is the node's own `config.cron`. The reactor scans
  **every cron trigger node** of every enabled flow, fires each due one independently, and advances
  **that node's** cursor (fire-once-then-skip, deterministic id per `(flow,node,scheduled_ts)`).
- **No single-schedule rejection.** `flows.save` validates each cron trigger node's spec and seeds/
  refreshes its per-node cursor on a schedule change. Two different specs are **valid** and both fire.
- **Per-trigger subgraph runs.** The run engine takes an `entry: Option<&str>`: `Some(node)` seeds the
  frontier from `node` and the **induced subgraph** (`reachable_from(node)`), with indegrees computed
  within that set; `None` keeps today's whole-graph seed (manual "run all", resume, subflow). Cron,
  inject, boot, and source-event firings all pass `Some(trigger_node)`.
- **The run records its entry** (`flow_run.entry_node`) so the canvas can show which trigger fired.
- **The persistent runtime view is per-trigger.** `flows.node_state` returns, per trigger/source node,
  its armed flag + next-fire (from `flow_trigger_state`), not one flow-level `cron`/`nextAttemptTs`.

## Non-goals

- **Per-node enable/disable** (arm one trigger while disarming its sibling in the same flow) — `enabled`
  stays flow-level for v1; the per-node cursor makes the per-node enable a small follow-up.
- **Webhook/http-in route registration** — sources already arm per-node (`arm_source`/`disarm_source`
  keyed by `node_id`); this slice does NOT add a new source kind, it removes the flow-level cron wall
  and the whole-graph run so *any* number of sources coexist. A real http-in node is its own scope.
- **Replacing the run engine** — induced-subgraph seeding reuses `indegrees`/`dependents`; no rewrite.
- **Cross-trigger fan-in across two firings** (a node that joins trigger A's run with trigger B's run)
  — each firing is its own run/message; a node shared by two triggers' subgraphs runs once per firing.

## Intent / approach

1. **Graph helpers (`lb-flows`).** `Flow::reachable_from(node) -> HashSet<String>` (BFS over
   `dependents`, inclusive of `node`) and `Flow::indegrees_within(&set) -> HashMap<String,usize>`
   (count only `needs` whose source is in `set`). Add `table::FLOW_TRIGGER_STATE`.
2. **Per-node cursor record (`host record.rs` + a small `trigger_store.rs`).** `FlowTriggerState
   {flow_id, node_id, next_attempt_ts}`; read/write keyed `{flow}:{node}` via `write_locked`.
3. **`create_run` takes `entry: Option<&str>`.** With `Some`, seed only `reachable_from(entry)` and
   use `indegrees_within` for the claim state; persist `entry_node` on the run record. Thread the
   param up through `coordinator::start` → `run_flow_to_completion` / `flows_run` / `flows_run_async`.
4. **Rewrite `react_to_flows_cron` per node.** For each enabled flow, for each `mode:cron` trigger
   node with a valid `config.cron`: read its cursor, init on first sight, fire when due (entry = that
   node, deterministic id `{flow}-cron-{node}-{ts}`), advance its cursor.
5. **De-flow-level `save`.** Replace `derive_cron_from_trigger`'s collapse+reject with a per-node
   cursor seed/refresh; drop the `flow.cron`/single-schedule path as the reactor's source of truth.
6. **Entry at the other firing sites.** `flows.inject` (entry = the inject node — already has it),
   reconcile `boot` (entry = the boot trigger node), source-event (entry = the event trigger node).
7. **`node_state` per-trigger.** Surface each trigger/source node's armed + next-fire from
   `flow_trigger_state`; the canvas paints per-trigger armed chips, not one flow banner.

## Testing plan (real `mem://` + real store/bus/jobs/caps — no mocks)

- **Multi-cron independence (MANDATORY):** a flow with two cron trigger nodes on different specs
  (`*/2 * * * *`, `*/3 * * * *`) **saves green** (no rejection) and, driven over logical time, each
  fires on **its own** schedule with **its own** cursor — neither starves nor double-fires the other.
- **Per-trigger subgraph isolation (MANDATORY):** a flow with trigger A→X and trigger B→Y; firing A
  runs only {A,X} (Y stays un-run for that run), firing B runs only {B,Y}. A shared downstream node Z
  (A→Z, B→Z) executes once per firing under each run.
- **Induced fan-in:** a join node J with `needs:[A,B]` where only A is in the fired subgraph waits on
  the in-subgraph upstreams only (indegree computed within the reachable set), settles, no hang.
- **Entry recorded:** `flows.runs.get` / run record carries the firing `entry_node`.
- **Whole-graph back-compat:** a manual `flows.run` with no entry still seeds all roots (resume +
  subflow unaffected).
- **Capability deny + workspace isolation** for any changed verb surface (`scope/testing`).
- **Frontend (Vitest, real spawned gateway):** a 2-trigger flow saves and the canvas paints two armed
  chips with independent next-fire; export round-trips both triggers.

## Risks & hard problems

- **Induced indegree correctness.** A node reachable from `T` may also `need` a node NOT reachable
  from `T`; within `T`'s run that need can never satisfy. Decision: `indegrees_within` counts only
  in-subset needs, so the node fires on its in-subset upstreams — matching "the message only carries
  what came down these wires." Out-of-subset `needs` resolve to their retained/last-value or null at
  binding time (the existing `flow_input`/null path), not a hang. Document + test.
- **Cursor migration.** Existing flows carry `flow.cron`/`flow.next_attempt_ts`; the new truth is
  per-node. `save` seeds per-node cursors from each cron trigger node; the old flow-level fields are
  left dormant (no reader) — a later slice removes them. No data loss, no double-fire (the reactor
  reads only the per-node cursor now).
- **Idempotency key must include the node.** Two cron triggers in one flow firing at the same instant
  would collide on the old `{flow}-cron-{ts}` id; the new id is `{flow}-cron-{node}-{ts}`.

## Debugging entries to log (this session)

- `debugging/flows/flow-level-cron-rejects-multiple-triggers.md` — the single-schedule wall +
  whole-graph run root cause and the per-node/per-subgraph fix.

## Related

- README `§3` (state vs motion, the wall), `flow-plc-reliability-scope.md` (the reactive-cron slice
  this de-flow-levels), `triggers-lifecycle-scope.md` (cron/reconcile/source), `flow-run-scope.md`
  (the engine), `flow-persistent-runtime-scope` (the `node_state` view this makes per-trigger).
- Promotes to `public/flows/flows.md`.

## Resolution (shipped 2026-06-30 — see the session doc)

Shipped + green. Session: [`sessions/flows/flow-multi-trigger-reactive-session.md`](../../sessions/flows/flow-multi-trigger-reactive-session.md).

Decisions made:
- **Reuse vs build:** evaluated edgelinkd/reflow/phlow/dora for reuse-as-library on a Pi. Chose to
  **keep + evolve our engine** (it already has the durable/resumable/capability-gated/ws-walled run-store
  the in-memory candidates lack) and **borrow the ideas** — per-wire isolation + durable node memory.
  Rejected adopting edgelinkd/reflow (re-homing durability onto an in-memory actor lifecycle is the
  expensive part, and regresses in-flight durability) — revisit only if flows become the headline product.
- **Run scope (user's call):** firing one trigger runs **only its downstream subgraph** (true Node-RED),
  not the whole graph with the trigger as entry. Out-of-subgraph `needs` resolve to retained/last value.
- **Counter:** a NEW `counter` builtin (durable node memory + atomic `lb_store::increment`), not a mode
  on `count` (kept pure). Memory in a dedicated `flow_node_memory` seam, distinct from the last-output
  `flow_node_state`, so it's the foundation for future stateful nodes.
- **`flow.cron`/`flow.next_attempt_ts`:** left dormant (no reader) rather than removed this slice — a
  later cleanup drops the fields. node_state surfaces per-trigger schedules + a soonest-fire summary.

Remaining (noted, not bugs): UI per-trigger armed chips; per-node enable/disable; orphan-cursor sweep on
trigger removal; a native http-in/webhook source node (its own scope).
</content>
</invoke>
