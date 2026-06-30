# Flows — dashboard ↔ flow binding (slice F, Wave 3)

- Area: flows
- Status: shipped (green)
- Scope: [`scope/flows/dashboard-binding-scope.md`](../../scope/flows/dashboard-binding-scope.md).
- Spine: [`scope/flows/flows-scope.md`](../../scope/flows/flows-scope.md) (Decisions **2, 5, 7, 9**).
- Session: this file. Sibling: [flows-canvas](flows-canvas-session.md) (slice E).

## What this slice is

The "really nice UX" the spine promises: **one dashboard that both drives and visualises a flow.** A
control (slider/switch) writes a retained input via `flows.inject`; a widget reads a flow node's
output back out over its series. Both ride the **shipped** v2 dashboard write/watch paths —
`flows.inject` is just one more granted action tool a control calls through the host-mediated
`WidgetBridge`; a flow-node series is just one more `series.watch`/`series.read` source. **No new
dashboard mechanism, no new read verb, no new write transport.**

This slice proved the round-trip end to end. It added **no production code** beyond the slice-E
gateway route for `flows.inject` (the `/flows/{id}/inject` POST) — the binding is wiring over shipped
seams, so the deliverable is the proof (the real-gateway test) + the doc.

## How the round-trip closes (the Cooler-Control flow)

1. **Control → retained input.** A slider/switch's `action:{tool:"flows.inject", argsTemplate:{id,
   node, value:"{{value}}"}}` is interpolated and called through `bridge.call`. The host re-checks
   `mcp:flows.inject:call` + the workspace (from the token, **not** the cell) per call and upserts
   `flow_input:{ws}:{flow}:{node}`. **No run starts** (Decision 9 retain-vs-fire — `fired_run:false`).
2. **Event-triggered one-shot run reads it.** A reading on the source node's series fires one run,
   which reads the retained setpoint and acts. Runs stay one-shot (no parked run).
3. **Output → series → chart.** The output node emits onto `flow:{ws}:{flow}:{node}` (Decision 2);
   the chart, bound to `{tool:"series.watch", args:{series}}`, redraws over the shipped series SSE.

## What shipped

- **`/flows/{id}/inject` POST** (`routes/flows.rs`, slice E) — `flows.inject` over the gateway,
  re-checked per call. Returns `{fired_run}` (Decision 9). The `WidgetBridge` (`features/dashboard/
  builder/widgetBridge.ts`) already carries any granted write tool through `mcp_call`, so a control
  declaring `flows.inject` in its cell tool set drives it with **no bridge change**. The dashboard's
  `SliderControl`/`SwitchControl`/`ButtonControl` already interpolate `argsTemplate` and call
  `bridge.call` — `flows.inject` is one more action tool.

### Tests (real, no mocks)
- **`FlowDashboardBinding.gateway.test.tsx`** (real gateway, 5): a slider control drives
  `flows.inject` → retained input set, **`fired_run:false`** (no run); the next run reads it (the run
  executes the inject node); **capability-deny** (a viewer without `mcp:flows.inject:call` is refused
  at the bridge leash AND the host — retained input never touched); **workspace isolation** (ws-A's
  control resolves in ws-A's namespace, never ws-B's); **read-out** (a flow-node series binds as a
  widget source over the shipped `series.read` path — the `flow:{ws}:{flow}:{node}` convention).

Green output:

```
ui: FlowDashboardBinding.gateway.test.tsx (5 passed)
```

## Decisions made this slice

- **The live chart redraw is proven at the transport, not in jsdom.** jsdom has no `EventSource`, so
  the `series.watch` SSE round-trip is asserted at the wiring level here (the bridge binds a
  flow-node series source) — the SSE transport itself is proven in `role/gateway/tests/` (the series
  stream + the flows routes). This mirrors the bus-bridge gateway-test precedent.
- **Token never crosses the boundary** — reused the shipped widget-builder assertion. The
  `WidgetBridge.call`/`watch` carry only `{tool, args}`; the shell holds the token and the host
  re-checks cap + workspace per call. `flows.inject` is no exception.
- **No "widget write verb" and no `flows.read_node`.** A control calls any granted write tool
  (`flows.inject` is a *flow* verb, not a dashboard verb); a read-out reuses `series.read` (history)
  + a last-value read of `flow_node_state` (instant). No polling `flows.runs.get` for a value feed
  (that is a run-inspection read, not motion — rule 3).

## Open questions / follow-ups
- A first-class "flow-node series" picker in the dashboard builder (today a cell binds the
  `flow:{ws}:{flow}:{node}` series by name; a picker is a UX nicety, not a contract change).
- Last-value stat over `flow_node_state` is read via the shipped series-latest path today; a direct
  `flow_node_state` read helper is a future convenience.
