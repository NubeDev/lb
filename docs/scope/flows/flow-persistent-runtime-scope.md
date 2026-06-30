# Flows scope — the persistent runtime view (the Node-RED / PLC steady state)

Status: **shipped (green), live-verified** (2026-06-30) — see
[`sessions/flows/flow-persistent-runtime-session.md`](../../sessions/flows/flow-persistent-runtime-session.md).
The `counter`/accumulator node (non-goal below) remains the one follow-up. Promotes to `public/flows/`. Extends
[`flow-plc-reliability-scope.md`](./flow-plc-reliability-scope.md) (unique runs + conflict-safe writes
+ reactive cron firing) and builds on Decision 5 of [`flows-scope.md`](./flows-scope.md)
(`flow_node_state` is last-value-only).

## The ask (from the user, against the live canvas)

> "When I login and open a flow I should see if it's running or not." … "The count still isn't going
> up." … "What if I have 2× triggers?" … "Something seems wrong, this should be SO simple. We need a
> proper persistent flow runtime — like Node-RED with PLC reliability."

Three failures on the live canvas (`chain4`, ws `acme`):

1. **Opening an armed flow shows a frozen, contradictory state.** The banner read "Armed … 54 runs"
   *and* "no runs yet"; the nodes showed a single finished run's `DONE` snapshot, not a living value.
2. **"The count isn't going up."** The canvas paints **one finite run's** snapshot — it never reads
   the **persistent per-node value** that updates every scan. (Also: the `count` node counts an
   array's length, so its value is constant by design — the thing that advances is the run count /
   last-fired, or a stateful node, not `count`.)
3. **Two trigger nodes** → the cron derivation silently picked the first; ambiguous + wrong.

## Root cause (design, not a one-off bug)

The engine + canvas treat a flow as **"a graph re-run from scratch each firing"** and the canvas only
ever renders **one `flow_run` snapshot**. But the spec already has the right primitive:

> **Decision 5 (`flows-scope.md`): `flow_node_state` is last-value-only — one upserted record per
> node holding its latest value**, updated in place per run (the dashboard "instant read").

`record_outcome` **already writes** `flow_node_state:{flow}:{node}` on every node Ok
(`run_store.rs`). So the persistent, scan-after-scan runtime state — the Node-RED "each wire shows its
current value", the PLC "the rung holds its last result" — **already exists in the store**. It is just
**unreachable**: there is no verb to read it, and the canvas never paints it. The canvas's only view
is a finite run, which is why an armed flow looks dead between firings.

## The model decision — the steady-state view is `flow_node_state`, not a run snapshot

- **Persistent runtime (the default, always-on view).** Each node paints its **current value** from
  `flow_node_state:{flow}:{node}` — the value the last scan left there, updated in place every firing.
  This view exists whether or not any run is in flight; it is the answer to "is it running and what is
  each node showing right now." It refreshes on the canvas's armed-poll tick.
- **Run watch (the overlay).** While the user explicitly watches a run (manual Run, or reattaching to
  a live one), the SSE run stream overlays per-node *progress* (pending→running→done) on top. When the
  run settles, the steady-state `flow_node_state` view remains — never a frozen "DONE".

This is exactly the spine's existing split (state vs motion, rule 3): `flow_node_state` is **state**
(persistent, read any time); the run stream is **motion** (one run's progress). We are surfacing the
state half that was already written but never read.

## Goals

- **`flows.node_state {id}` verb** (gated `flows.node_state:call`, ws-walled): returns every node's
  current persistent value `[{ node, value, rev }]` from `flow_node_state:{flow}:*`, plus the flow's
  armed fields (`enabled`, `cron`, `nextAttemptTs`). One read; the canvas's steady-state source.
  `GET /flows/{id}/node_state` mirrors it 1:1.
- **The canvas paints the persistent view.** On open and on the armed-poll tick, each node shows its
  `flow_node_state` value; the run-snapshot overlay applies only while watching a run and never
  replaces the steady-state value with a stale terminal one.
- **Deterministic multi-trigger.** `derive_cron_from_trigger` handles 2+ trigger nodes honestly: at
  most one `mode:cron` trigger may set the schedule; **two conflicting cron specs reject the save**
  with a precise error (never silently pick one). Multiple non-cron triggers are fine.
- **Honest banner.** "Armed · next fire in N · last fired N ago · N runs" with no contradiction; a
  flow with no runs shows "no runs yet" and no run count.

## Non-goals

- **A new `counter`/accumulator node** (a node whose value increments per firing). The user's "count
  going up" is most naturally that node; it is a clean follow-up (a stateful builtin reading+writing
  its own `flow_node_state`). This slice surfaces the *existing* per-node state; it does not add a new
  stateful node type.
- **Per-node history charts on the canvas** — `flow_node_state` is last-value (Decision 5); history is
  the node's series (`series.*`), already shippable via the dashboard binding. Not duplicated here.
- **A parked/interactive long-lived run** (Decision 9 rejected it) — the runtime stays one-shot runs +
  retained state; "persistent" means the *state* persists, not a single never-ending run.

## Testing plan (real store/caps/bus — no mocks)

- **node_state verb:** drive a flow to terminal, then `flows.node_state` returns each node's last
  value (the count node's `{count:4}`); a second run **updates in place** (rev bumps, value reflects
  the new run) — proving last-value-in-place, not append.
- **Steady-state survives run completion:** after a run settles, `node_state` still returns the
  values (the canvas view is not a frozen run — it's the persistent record).
- **Multi-trigger:** a flow with two trigger nodes where only one is `mode:cron` derives that cron;
  two conflicting cron triggers **reject the save**; two non-cron triggers save fine.
- **Cap-deny + workspace-isolation** on the new verb (ws-B can't read ws-A's node state).
- **Frontend:** `armedState` unit (done) + a canvas test that paints node values from `node_state`
  (not only from a run snapshot) and shows the honest banner.

## Debugging entries to log

- `debugging/flows/canvas-shows-finite-run-not-persistent-node-state.md` — the read-the-wrong-thing
  root cause + the `flow_node_state` surfacing fix.

## Related

- `flows-scope.md` Decision 5 (`flow_node_state` last-value), rule 3 (state vs motion).
- `flow-plc-reliability-scope.md` (reactive cron firing — this view shows what that firing produces).
- Promotes to `public/flows/flows.md`.
