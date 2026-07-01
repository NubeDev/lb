# Flows — the flow⇄dashboard binding UX (pick a node + port; switch / slider / JSON, both ways)

- Area: flows + frontend/dashboard
- Status: shipped (green) — port-aware inject + precedence + read-back + picker + 2 views + tests + docs
- Scope: [`scope/flows/flow-dashboard-binding-ux-scope.md`](../../scope/flows/flow-dashboard-binding-ux-scope.md)
- Depends on (landed): [`flow-message-envelope-scope.md`](../../scope/flows/flow-message-envelope-scope.md)
  — verified before starting: builtin ports are `payload`/`topic`, a 3-node linear flow auto-wires with
  no `with`. Builds on the **shipped** mechanism [`dashboard-binding-scope.md`](../../scope/flows/dashboard-binding-scope.md)
  (`flows.inject` write-in + read-out) — this slice adds **authoring UX + structured values + read-back**,
  no new transport.
- Spine: [`flows-scope.md`](../../scope/flows/flows-scope.md) Decisions 9 (inject = retained `flow_input`,
  one-shot), 5 (`flow_node_state` last-value), 7 (composition, never widening).

## The ask

> Make the bidirectional binding **authorable in clicks**: a flow-aware source picker (flow → node →
> port) that wires a switch / slider / JSON control or a JSON read view in the right direction,
> reflecting the node's real current value; plus first-class structured JSON in AND out. No backwards
> compat (flows is in dev).

The scope's **Decisions (resolved)** section is the contract — no open questions. Followed verbatim.

## What shipped (the slice, end to end)

### Backend (rust)

1. **Port-aware inject** ([`host/src/flows/triggers.rs`](../../../rust/crates/host/src/flows/triggers.rs)
   `flows_inject`) — an optional `port` arg. With `port`, upsert `flow_input:{flow}:{node}:{port}`
   (per-port retained, the body carries `port`); without it the node-level `flow_input:{flow}:{node}`
   (unchanged). Same cap `mcp:flows.inject:call`, same per-call ws + grant recheck — the added arg
   widens nothing. Threaded through the host dispatch ([`mod.rs`](../../../rust/crates/host/src/flows/mod.rs))
   and the gateway route ([`role/gateway/src/routes/flows.rs`](../../../rust/role/gateway/src/routes/flows.rs)
   `InjectFlow.port`).
2. **Binding precedence** ([`host/src/flows/run_store.rs`](../../../rust/crates/host/src/flows/run_store.rs)
   `resolve_node_bindings` → new `overlay_retained_inputs`) — a run's input per port resolves
   **per-port retained > node-level retained > static `with` / auto-wire**, made explicit and applied in
   BOTH the auto-wire branch and the explicit-`with` branch. The injected value is the node's `payload`
   (node-level) or the named port (per-port). A run always reads the CURRENT retained value, so a
   control's inject takes for the next run — the "value didn't take" trap closed.
3. **Read-back of retained inputs** ([`host/src/flows/node_state.rs`](../../../rust/crates/host/src/flows/node_state.rs))
   — `flows.node_state {id}` now folds each node's retained `flow_input` into its entry: `input`
   (node-level retained `payload`) + `inputs` (the per-port map). One read drives both the canvas and
   the dashboard; a control seeds its current state from its OWN input, not its output. No new verb.

### Frontend (ui)

4. **Flow-aware source picker** ([`features/dashboard/builder/sourcePicker.ts`](../../../ui/src/features/dashboard/builder/sourcePicker.ts)
   `flowsEntries` + [`useSourcePicker.ts`](../../../ui/src/features/dashboard/builder/useSourcePicker.ts)) —
   a `flows` group built from shipped reads (`flows.list` → `flows.get` → `flows.nodes` descriptors).
   An INPUT port → an `Action {tool:"flows.inject", argsTemplate:{id,node,port,value:"{{value}}"}}`; an
   OUTPUT port → a `Source {tool:"flows.node_state", args:{id,__flowNode,__flowPort}}`. The author sees
   `Cooler Control › setpoint-in › payload (input)`, never a tool name. A flow the caller can't
   `flows.get` is silently skipped (the cap-scoped offer).
5. **Controls wired + reading back** ([`SwitchControl`](../../../ui/src/features/dashboard/views/SwitchControl.tsx),
   [`SliderControl`](../../../ui/src/features/dashboard/views/SliderControl.tsx) +
   [`useFlowNodeValue.ts`](../../../ui/src/features/dashboard/views/useFlowNodeValue.ts),
   [`flowBinding.ts`](../../../ui/src/features/dashboard/views/flowBinding.ts)) — a flow-bound switch /
   slider seeds its CURRENT value on mount from its own retained input (extracted from
   `flows.node_state`, per-port wins over node-level), advancing on the canvas-cadence refresh tick. A
   switch sets a boolean `payload`; a slider a number (min/max/step). A new **JSON control**
   ([`JsonControl.tsx`](../../../ui/src/features/dashboard/views/JsonControl.tsx)) parses + validates
   (ajv against the port schema when `options.schema` is set, else free JSON) BEFORE injecting — invalid
   JSON never calls (no fake accept).
6. **JSON / object read view** ([`JsonView.tsx`](../../../ui/src/features/dashboard/views/JsonView.tsx))
   — pretty-prints a flow node's structured `payload` (collapsible) via `flows.node_state`; the default
   is the `payload` field, `options.envelope` shows the whole `{payload, topic, …}`. Both new views
   registered in [`WidgetView.tsx`](../../../ui/src/features/dashboard/views/WidgetView.tsx)'s
   `switch (view)` (`json` / `jsonview`) and the `View` union in
   [`dashboard.types.ts`](../../../ui/src/lib/dashboard/dashboard.types.ts).

## Decisions recorded (anything the scope left to "best long-term option")

- **The OUTPUT source tool is `flows.node_state`, not `series.*`.** A read view binds a node-state read
  (instant + canvas-cadence refresh) — never `series.watch` on an arbitrary node (which silently shows
  nothing for a counter/transform that updates in place) and never `runs.get` (a run-inspection read,
  rule 3). The node/port travel as `__flowNode`/`__flowPort` on the source args; the whole-flow read is
  keyed by `id`. A `flows.node.watch` SSE remains the later live-upgrade slice.
- **Read-back reads node_state directly, not through the bridge.** The control/read views call
  `getFlowNodeState` (the same path the canvas uses) rather than routing the read-back through
  `makeWidgetBridge`; the WRITE (inject) goes through the bridge (leashed). This keeps one read serving
  both surfaces and avoids shaping a whole-flow node_state response through `useSource`'s scalar
  normaliser.
- **Per-port value wins over node-level for the same slot.** `extractFlowValue("input", port)` and the
  resolver both prefer `inputs[port]` over the node-level `input`/`payload` — one rule, tested on both
  sides.

## Follow-up: reachable in the LIVE editor + a visual JSON-path builder

Two gaps surfaced from live use (screenshots) and were closed:

1. **Reachable from the editor the user sees.** The Flows group was only in the `WidgetBuilder` "Add
   widget" path, which isn't mounted in the dashboard. The live screen is `PanelEditor` ("Edit panel").
   Wired Flows in: a **Datasource → Flows** option ([useDatasourceList.ts](../../../ui/src/features/dashboard/editor/tabs/useDatasourceList.ts)),
   a [`FlowsQuerySection`](../../../ui/src/features/dashboard/editor/tabs/FlowsQuerySection.tsx) flow→node→port
   picker, and a [`VizPicker`](../../../ui/src/features/dashboard/editor/VizPicker.tsx) that swaps to the
   control views (input port) / JSON read view (output port). `PanelEditor` computes `flowKind` from the
   carried action vs the target tool. Fixed a false **"binding broken — re-pick"** flash (it fired while
   the async picker entries were still loading; now guarded on `!loading && entries>0 && !selectedId`).

2. **Agnostic to the node type + port NAMES.** `flowsEntries` iterates `descriptor.inputs/outputs` with
   no type branch; inject + node_state read-back key on `{node}:{port}`; the output read extracts the
   **selected** port name (was hardcoded `payload` — fixed). Backend test
   `port_aware_inject_is_agnostic_to_the_port_name`. A node type a developer ships tomorrow just works.

3. **Visual JSON-path builder ("parse out the JSON").** A generic view bound to `flows.node_state` was
   dumping the whole-flow response (useless). Added [`jsonPaths.ts`](../../../ui/src/features/dashboard/views/jsonPaths.ts)
   (`valueAtPath`/`childrenOf`/`pathLabel`/`previewOf`) + [`JsonPathPicker`](../../../ui/src/features/dashboard/editor/tabs/JsonPathPicker.tsx):
   an interactive tree over the node's REAL current value (objects/arrays/nested/scalars); clicking a row
   binds exactly that path (stored as `__flowPath` on the source args), with a live preview. The picked
   path feeds **any** view — [`usePanelData`](../../../ui/src/features/dashboard/builder/usePanelData.ts)
   resolves a flow source CLIENT-SIDE through the extraction + shapes it to rows (scalar→stat/gauge/text,
   array→table/timeseries, object→one row / JSON view) — never the raw dump, no backend change.

## Tests (real spawned gateway, real seeded flows — NO `*.fake.ts`)

- **Backend (cargo, `lb-host`/`flows_triggers_test.rs`)** — 8 new, all green:
  `inject_with_port_upserts_the_per_port_record`, `inject_without_port_unchanged_node_level_record`,
  `binding_precedence_per_port_over_node_level_over_with` (a real run: `with`=1 → node-level=5 →
  per-port=9), `object_payload_round_trips_inject_to_run`, `node_state_reads_back_retained_inputs`,
  `capability_deny_inject_does_not_upsert_node_or_port_record` (node- AND port-keyed not upserted),
  `workspace_isolation_ws_b_cannot_inject_into_ws_a_flow`.
- **Frontend unit** ([`flowsPicker.test.ts`](../../../ui/src/features/dashboard/builder/flowsPicker.test.ts))
  — 6 tests: the picker resolves input→Action / output→Source with friendly labels (no tool name),
  skips unknown descriptors, folds into `buildSourceEntries`; `flowBindingOf*` recovery + the per-port-
  wins / output-payload `extractFlowValue` rules.
- **Frontend gateway** ([`FlowDashboardBinding.gateway.test.ts`](../../../ui/src/features/flows/FlowDashboardBinding.gateway.test.ts))
  — 7 new tests over the REAL gateway + REAL seeded flow: the Flows picker lists seeded flows and emits
  the correct Action/Source; a slider fires a real port-aware inject and the precedence (per-port >
  node-level > `with`) is honoured by a real run; a control reflects the seeded current value on mount;
  a switch sets a boolean; a JSON control injects a validated object (and invalid JSON never calls); a
  JSON read view renders a structured `flow_node_state` payload and advances on change; ws-A cannot read
  ws-B's `flow_node_state` and its picker lists none of ws-B's flows.

## Green output

- `cargo test -p lb-host -p lb-flows` — all flows suites green (8 new trigger tests pass; the lone
  `agent_routed_test` flake is the known bus-timing class, passes on retry, unrelated).
- `cd ui && pnpm test` — 213 passed.
- `pnpm test:gateway` — `FlowDashboardBinding.gateway.test.ts` 12/12 passed. The 4 unrelated failures
  (DashboardView isolation / SystemView / App routing / PeopleAdmin) are pre-existing on master
  (verified by `git stash` baseline run); this slice introduces no new failures.

## Notes / no debug entry

No bug was hit during the slice (the prerequisite envelope work was already merged and verified). No
`docs/debugging/` entry required.
