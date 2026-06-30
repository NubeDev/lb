# Flows — the persistent runtime view (the Node-RED / PLC steady state)

- Area: flows
- Status: shipped (green), live-verified. Follow-up: a stateful `counter` node (non-goal here).
- Scope: [`scope/flows/flow-persistent-runtime-scope.md`](../../scope/flows/flow-persistent-runtime-scope.md).
- Extends: [`flow-plc-reliability-scope.md`](../../scope/flows/flow-plc-reliability-scope.md) (reactive
  cron firing) + Decision 5 of [`flows-scope.md`](../../scope/flows/flows-scope.md) (`flow_node_state`).
- Debug: [`debugging/flows/canvas-shows-finite-run-not-persistent-node-state.md`](../../debugging/flows/canvas-shows-finite-run-not-persistent-node-state.md).

## The ask

"When I open a flow I should SEE if it's running." "The count isn't going up." "What if I have 2×
triggers?" "We need a proper persistent flow runtime — like Node-RED with PLC reliability." The user
twice (rightly) pushed back on patches and asked for a deep design review.

## Root cause (the deep review's finding)

The engine + canvas treated a flow as "a graph re-run from scratch each firing," and the canvas's only
runtime view was **one finite `flows.runs.get` snapshot**. A cron flow's runs are finite (ms), so
between firings there's no live run → the canvas painted the last terminal one and looked dead, with a
self-contradictory banner.

But the spec already had the right model: **Decision 5 — `flow_node_state` is last-value-only, one
upserted record per node, updated in place each scan** — the Node-RED "each wire shows its current
value" / PLC "the rung holds its last result". `record_outcome` (`run_store.rs`) **already writes** it
every node Ok. The persistent runtime state existed in the store but was **unreachable**: no read verb,
never painted.

## What was built

- **`flows.node_state {id}` verb** (`crates/host/src/flows/node_state.rs`, gated, ws-walled) → every
  node's `{node, value, rev}` from `flow_node_state:{flow}:*` + the flow's armed fields.
  `GET /flows/{id}/node_state` mirrors it. (Bug found + fixed via the regression test: `lb_store::scan`
  returns `Row.id` as `{table}:{flow}:{node}`, so the id strip must drop the table segment too.)
- **Canvas paints node_state as the BASE steady-state**, run snapshot OVERLAID while watching a run
  (`nodeStateValues` in `flowGraph.ts`; `values = {...base, ...overlay}`). Fetched on open + on the
  armed poll tick so values track each cron firing — no frozen "DONE".
- **Honest armed banner** (`FlowArmedBanner.tsx` + pure `armedState.ts`): "Armed · next fire in N ·
  last fired N ago · N runs" with a live 1s clock; no contradiction; "no runs yet" only when truly so.
- **Deterministic multi-trigger** (`derive_cron_from_trigger`): identical cron specs collapse;
  conflicting specs **reject the save**; non-cron triggers ignored.
- **runs.list** now returns `ts` and is newest-first (so the banner finds the latest run + dates it).

## Tests (real store/caps/bus — no mocks)

- Host: `node_state_returns_persistent_last_value_updated_in_place` (value present after run + SAME
  record updated in place, rev bumps), `node_state_denied_and_workspace_walled` (cap-deny +
  ws-isolation), `multi_trigger_cron_derivation_is_deterministic` (one cron + inject derives the cron;
  conflicting specs reject; identical collapse). Frontend: `flowGraph::nodeStateValues`,
  `armedState` (8). Flows backend suites (38) + UI unit (195) + flows gateway e2e (24) green.

## Live verification

`GET /flows/chain4/node_state` returned every node's current value (node `a` on **rev 59**); on the
next cron firing the rev advanced **59 → 60** — the persistent value updates in place each scan and
the verb reflects it live. The canvas now shows, on open: the armed banner (next fire / last fired /
run count) + each node's current value — the "is it running and what is each node showing" answer.

## Honest note + follow-up

`count` counts an array's length (constant 4) — it is **not** a per-firing counter, so "the count
going up" was never going to come from `count`. What advances per firing is the run count, last-fired,
and each node-state record's rev. A stateful **`counter`/accumulator** builtin (value increments per
firing, reading+writing its own `flow_node_state`) is the natural "count goes up" node — a clean
follow-up, explicitly a non-goal of this slice (which surfaces the EXISTING per-node state).
