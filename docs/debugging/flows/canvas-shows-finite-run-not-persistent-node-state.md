# The canvas shows a finite run's frozen snapshot, never the persistent per-node runtime state

- Area: flows (canvas runtime view + the missing `flow_node_state` read verb)
- Status: resolved
- First seen: 2026-06-30
- Resolved: 2026-06-30
- Session: ../../sessions/flows/flow-persistent-runtime-session.md
- Scope: ../../scope/flows/flow-persistent-runtime-scope.md
- Regression tests:
  - rust/crates/host/tests/flows_runtime_control_test.rs::node_state_returns_persistent_last_value_updated_in_place
  - rust/crates/host/tests/flows_runtime_control_test.rs::node_state_denied_and_workspace_walled
  - rust/crates/host/tests/flows_runtime_control_test.rs::multi_trigger_cron_derivation_is_deterministic
  - ui/src/features/flows/flowGraph.test.ts::nodeStateValues
  - ui/src/features/flows/armedState.test.ts

## Symptom

Opening an armed cron flow showed a **frozen "DONE" snapshot** of the last finite run, with a banner
that read "Armed … 54 runs" *and* "no runs yet" (contradictory). "The count isn't going up." Two
trigger nodes silently picked one cron. The user: "we need a proper persistent flow runtime — like
Node-RED with PLC reliability."

## Root cause (design, not a one-off)

The canvas's ONLY runtime view was a `flows.runs.get` **run snapshot** — one finite run. But a
cron/source flow's runs are each finite and terminal in milliseconds; between firings there is no live
run, so the canvas painted the last terminal one and looked dead.

The spec already had the right primitive — **Decision 5 (`flows-scope.md`): `flow_node_state` is
last-value-only, one upserted record per node holding its latest value, updated in place each scan** —
and `record_outcome` (`run_store.rs`) **already writes** `flow_node_state:{flow}:{node}` on every node
Ok. So the persistent, scan-after-scan runtime state (the Node-RED "each wire shows its current value")
**existed in the store but was unreachable**: no verb read it, and the canvas never painted it.

Two compounding bugs surfaced in the same screenshot: the armed banner contradicted itself (it read
runs but not the latest-run date), and `derive_cron_from_trigger` silently picked the first of two
cron triggers.

## Fix

1. **`flows.node_state {id}` verb** (`crates/host/src/flows/node_state.rs`, gated
   `flows.node_state:call`, ws-walled) — returns every node's current value `[{node, value, rev}]`
   from `flow_node_state:{flow}:*` plus the flow's armed fields. `GET /flows/{id}/node_state` mirrors
   it. **Bug found while building:** `lb_store::scan` returns `Row.id` as the full
   `{table}:{flow}:{node}` string, so the node-id strip must drop `flow_node_state:{flow}:` (not just
   `{flow}:`) — caught by the regression test (value came back `null`).
2. **Canvas paints node_state as the BASE steady-state**, with the run snapshot OVERLAID while
   watching a run (`nodeStateValues` in `flowGraph.ts`; `values = {...base, ...overlay}` in
   `FlowCanvas.tsx`). Fetched on open + on the armed poll tick, so a cron flow's values track each
   firing. The honest **armed banner** (`FlowArmedBanner.tsx` + pure `armedState.ts`) shows
   "Armed · next fire in N · last fired N ago · N runs".
3. **Deterministic multi-trigger** (`derive_cron_from_trigger`): collect every cron trigger's spec;
   identical specs collapse; **different specs reject the save** with a precise error; non-cron
   triggers are ignored.

## Verification

Unit: `node_state_returns_persistent_last_value_updated_in_place` proves the value is present after a
run AND that a second run updates the SAME record in place (rev bumps). **Live (`chain4`):**
`GET /flows/chain4/node_state` returned every node's current value with node `a` on **rev 59**, and on
the next cron firing the rev advanced **59 → 60** — proving the persistent value updates in place each
scan and the verb reflects it live. The `*/2` cron fires every 2 min headless; the run count climbs.

## Note — what actually "goes up"

The `count` node counts an **array's length** (always 4 here) — it is not a per-firing counter. The
things that advance per firing are the **run count**, the **last-fired** time, and each node-state
record's **rev**. A stateful `counter`/accumulator node (value increments per firing) is a clean
follow-up (a builtin reading+writing its own `flow_node_state`); this slice surfaces the EXISTING
per-node state, the load-bearing "is it running and what does each node show" view.
