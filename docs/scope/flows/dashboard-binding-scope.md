# Flows scope — dashboard ↔ flow binding (a control drives a flow; a widget reads it)

Status: **shipped (Wave 3)** — see
[`sessions/flows/dashboard-binding-session.md`](../../sessions/flows/dashboard-binding-session.md) +
the Wave 3 section of [`public/flows/flows.md`](../../public/flows/flows.md). This is the ask kept
as the contract. Read the spine
[`flows-scope.md`](flows-scope.md) first — it owns the canonical **Decisions (v1) 1–13** this doc
references by number.

We want the "really nice UX" the spine promises: **a single dashboard that both drives a flow and
visualises it, live.** A dashboard *control* (slider/switch/button) **sets a flow node's retained
value** ([Decision 9](flows-scope.md)) — the held input the *next* run reads; a dashboard *widget*
(chart/stat/gauge) reads a flow node's output back out. Both ride the
**already-shipped** dashboard write/watch paths from [`widget-builder-scope.md`](../frontend/dashboard/widget-builder-scope.md)
(v2, SHIPPED). This doc adds only the **flow-side glue** — one write tool (`flows.inject`) and one
binding convention (read a node's series) — **not a new dashboard mechanism, and not a new read or
write transport.**

## Goals

- **WRITE-IN.** `flows.inject {id, node, value}` — the write tool a dashboard control calls. Per
  [Decision 9](flows-scope.md), an inject into a **retained** input node **SETS that node's held
  value** in `flow_input:{ws}:{flow}:{node}` — it does **not** advance an in-flight run (runs are
  one-shot); the *next* run reads it (a slider sets a retained `setpoint`, a switch sets a retained
  `enabled` gate). (An inject into a *firing* trigger node starts a one-shot run — also Decision 9 —
  but the control-loop dashboard below is built from retained inputs.) Gated by
  `mcp:flows.inject:call`, re-checked per call like any control write (caller ∩ grant; workspace from
  the token).
- **READ-OUT.** A widget reads a flow node's output by binding to that node's **series subject** —
  an ordinary v2 widget source, no new verb. Per **Decision 5**, `flow_node_state:{ws}:{flow}:{node}`
  holds the node's **last value** (state, instant render); the live tick is **motion** on the node's
  series (`series.watch`/`bus.watch`).
- **THE BIDIRECTIONAL UX (the headline).** One dashboard DRIVES (control → `flows.inject` → retained
  input node, read by the next run) and VISUALISES (chart ← output node series) the same flow, live —
  the full round-trip on existing paths, re-checked per call, workspace-walled.

## Non-goals

- **No new dashboard write/watch mechanism.** Controls call arbitrary granted write tools through the
  shipped `bridge.call`; reads stream over the shipped series SSE. `flows.inject` is just one more
  granted tool; a flow-node series is just one more source. Nothing here touches the v2 widget/bridge
  contract.
- **No "widget write verb."** There is none in the shipped dashboard (a control calls any granted
  write tool); we add none. `flows.inject` is a *flow* verb, not a dashboard verb.
- **No polling for live values.** Live = the series watch (motion). We do **not** poll
  `flows.runs.get` on a timer — that is a run-inspection read, not a value feed (rule 3; see "State
  vs motion" below).
- **No new persistence.** Last-value is `flow_node_state` (Decision 5); history is the node's series.
  No new table, no ring buffer here.
- **The `inject` node itself** (how a flow declares a retained input port vs a firing trigger, the
  run an event trigger fires) is owned by [`triggers-lifecycle-scope.md`](triggers-lifecycle-scope.md);
  this doc owns only the *tool that sets it from a dashboard* and the *binding that reads a node back*.

## Intent / approach

**Two existing seams meet; we add the one tool and one convention that joins them.**

- A dashboard **control** already declares `action: {tool, argsTemplate}` and, on interaction
  (slider value, switch state), fills the template and calls the write tool through the
  host-mediated `bridge.call` — the host re-checks workspace (from the session token) + capability
  (against the install grant) **per call**, and the page never holds a token. So driving a flow is
  just **`flows.inject` as that action tool**: `action: {tool:"flows.inject", argsTemplate:{id, node,
  value:"{{value}}"}}`, which **sets the named node's retained input** ([Decision 9](flows-scope.md))
  — the run reads it on its next firing. The reachable tool set is `requested ∩ admin_approved`,
  unchanged.
- A dashboard **read widget** already binds a view to `{tool, args}` and streams over SSE
  (`series.watch`/`bus.watch`) or reads history (`series.read`). So reading a flow node back is just
  **binding to that node's series subject** — `{tool:"series.watch", args:{series:"flow:{ws}:{flow}:{node}"}}`
  for the live tick, or last-value off `flow_node_state` for an instant stat. **Decision 2** already
  bridges source events onto `flow:{ws}:{flow}:{node}`; output nodes emit on the same convention.

**Why a dedicated `flows.inject` and not "let the control call the node directly."** A control could
in principle call the node's underlying tool — but a flow input is a *retained node*, not a tool: the
inject must **upsert the held value** in `flow_input:{ws}:{flow}:{node}` ([Decision 9](flows-scope.md))
the next version-pinned run ([Decision 1](flows-scope.md)) reads, coalesce a drag burst, and stay
workspace-owned. One narrow verb (`flows.inject`) with its own cap is the clean, gated, testable
door — and it composes (never widens) with the flow's own grant per [Decision 7](flows-scope.md).
*Rejected:* a new
"dashboard→flow" channel (forks the shipped control write path) and reusing a generic
`ingest.write` (loses the retained-input/coalesce/version-pin semantics the inject node owns).

**Why read-out is "just a series binding" and not a `flows.read_node` verb.** The live value is
motion; the instant value is last-value state. Both are already first-class dashboard sources
(`series.watch`, and a last-value read of `flow_node_state`). A bespoke read verb would duplicate the
shipped series path. *Rejected:* a `flows.node_value` read tool (re-implements `series.read`/the
SSE for no gain) and polling `flows.runs.get` (a run-inspection read mistaken for a value feed).

## How it fits the core

- **Tenancy / isolation (rule 6):** `flows.inject` derives the workspace from the **session token**,
  never from the cell or the args — exactly as the shipped bridge does for every control write. A
  ws-A control physically cannot inject into ws-B's flow: the verb resolves `flow_input:{ws}:{flow}:{node}`
  in the caller's namespace, and the node series `flow:{ws}:{flow}:{node}` is ws-scoped, so a ws-A
  widget cannot watch a ws-B node. **Mandatory two-session test, across both store and MCP.**
- **Capabilities (rule 5/7) — composition, never widening (Decision 7):** `flows.inject` is its own
  cap `mcp:flows.inject:call`, re-checked per call. A control can only inject into a flow the user is
  granted: injecting needs `mcp:flows.inject:call` **and** the caller must be granted the target flow
  (the same `caller ∩ grant` the spine fixes; per-resource narrowing rides the platform-wide
  `authz-grants` follow-up, Decision 7). A control surfaced in the dashboard but called by a viewer
  **without** the cap is **denied at the bridge** (the headline deny) — the retained input is never
  touched. No new cap for read-out — it
  reuses the shipped `mcp:series.watch:call` / `mcp:series.read:call`.
- **Placement (rule 1):** one path, two transports (Tauri `invoke` / gateway SSE+HTTP). `flows.inject`
  routes through the existing queryable host dispatch; no role branch.
- **MCP surface (§6.1):**
  - **Write:** `flows.inject {id, node, value}` — one tool, one file, one cap. Synchronous and
    bounded (it upserts the retained value in `flow_input:{ws}:{flow}:{node}` and returns; the *next*
    run — a separate event trigger — reads it, [Decision 9](flows-scope.md)). This is the **only** new
    verb this doc adds.
  - **Get/list:** N/A — read-out reuses shipped `series.read` (history) + a last-value read of
    `flow_node_state` (instant). No new read verb.
  - **Live feed:** the shipped series SSE (`series.watch`/`bus.watch`) carries the node's live ticks.
    No new transport, no polling.
  - **Batch:** N/A — a control fires one inject per interaction.
- **One datastore / state vs motion (rule 3):** last-value on `flow_node_state` (state, Decision 5);
  history is the node's **series** (motion); the control write is a gated tool call. Three distinct
  things — kept distinct. No new table.
- **Durability:** **none here.** A control write is a **best-effort gated state-set** — `flows.inject`
  upserts the retained input and returns. If a flow *node* needs a must-deliver effect (an actuator command
  that must reach a device), that is the **outbox**, inside the `flow-run` job
  ([`flow-run-scope.md`](flow-run-scope.md)) — the inject is not pretending to be an actuation ack.
- **Stateless / SDK-WIT:** no durable state in the dashboard or the inject path (graph in the store,
  run in the job, live value on the series). No manifest/WIT change — `flows.inject` is an ordinary
  host verb and the series binding is the shipped v2 source.

## Example flow — a "Cooler Control" dashboard

A genuine control loop built from **one-shot runs** ([Decision 9](flows-scope.md)). The flow
`cooler-ctl` (ws `kfc`) has a **retained** `inject` node `setpoint-in`, a **retained** `inject` node
`enabled-gate`, an **event-trigger** node `temp-in` (watching the temp source node's series), and an
output node `temp-out` emitting to `flow:kfc:cooler-ctl:temp-out`. The dashboard has a **slider**, a
**switch**, and a **live chart**.

1. **Set the setpoint — slider.** Alice (a `kfc` session granted `cooler-ctl`) drags the slider to
   4 °C. The control fills `argsTemplate` → calls `flows.inject {id:"cooler-ctl", node:"setpoint-in",
   value:4}` through `bridge.call`. The host re-checks `mcp:flows.inject:call` + the `kfc` workspace
   (from Alice's token, **not** the cell) + that Alice is granted `cooler-ctl`. It **upserts the
   retained value** `flow_input:kfc:cooler-ctl:setpoint-in = 4`. **No run starts** per drag — the held
   setpoint just waits for the next run to read it (a drag burst coalesces per the `coalesce` posture
   in [`flow-run-scope.md`](flow-run-scope.md)).
2. **Set the gate — switch.** Toggling the switch calls `flows.inject {id:"cooler-ctl",
   node:"enabled-gate", value:true}` → it **upserts** `flow_input:kfc:cooler-ctl:enabled-gate = true`,
   a held flag the runs read. The same per-call ws + cap recheck applies. Again **no run starts** — it
   sets state, it does not arm a live node.
3. **A reading drives a one-shot run.** A temp reading arrives on the source node's series; the
   `temp-in` event trigger fires **one** run, which **reads** the retained `setpoint-in` (4 °C) and
   `enabled-gate` (true) and acts (cool / hold). The run runs to terminal completion — it is not
   parked waiting on the controls.
4. **Run → output emits.** The run computes and the `temp-out` node emits its value: it upserts
   `flow_node_state:kfc:cooler-ctl:temp-out` (last value) and publishes the tick as motion on
   `flow:kfc:cooler-ctl:temp-out`.
5. **Visualise — chart.** The chart, bound to `{tool:"series.watch", args:{series:"flow:kfc:cooler-ctl:temp-out"}}`,
   receives the tick over the **shipped series SSE** and redraws — live, no poll. A stat widget bound
   to the same node renders `flow_node_state` instantly on open, then updates over SSE.
6. **The round-trip, closed.** control sets retained input → next event-triggered run reads it →
   output node emits to its series → widget updates — all on existing paths, re-checked per call,
   ws-walled. One dashboard, both directions, live — out of one-shot runs, never a long-lived one.
7. **DENY.** Dave is a **viewer**: his grant lacks `mcp:flows.inject:call` (and/or he is not granted
   `cooler-ctl`). He **sees** the slider (it is part of the saved layout), but dragging it is
   **refused at the bridge** — the host denies the call server-side; the retained input is never
   touched. Read-out still works if he has `series.watch` (he can watch, not drive).
8. **Isolation.** Erin (a `mcdonalds` session) builds the same slider pointing at `cooler-ctl`. Her
   inject resolves `flow_input:mcdonalds:cooler-ctl:setpoint-in` — a different (or absent) flow; she
   **cannot** touch `kfc`'s. Her chart watching `flow:kfc:...` is denied/empty. The wall holds across
   both directions.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`) — real gateway, real store (`mem://`), real
flow + series records seeded, **no `*.fake.ts`**:

- **Capability-deny (headline).** (a) A control whose caller lacks `mcp:flows.inject:call` → the
  `flows.inject` call is **refused at the bridge / denied server-side** (assert the host denies even
  if the bridge filter were bypassed; the retained input is **not** upserted). (b) A caller **not
  granted the target flow** → denied — **no widening** (having `flows.inject` does not grant an
  ungranted flow). Deny is opaque.
- **Workspace-isolation (across store AND mcp).** Two real sessions: a ws-A control calling
  `flows.inject` for `cooler-ctl` cannot fire ws-B's `cooler-ctl` (ws from the token; the verb
  resolves in the caller's namespace); a ws-A widget cannot `series.watch` a ws-B node series. Assert
  at both the store (the `flow`/`flow_node_state` record is ws-namespaced) and the MCP (the verb +
  watch are denied) layers.
- **Token never crosses the boundary.** The session token appears in no `bridge` arg or SSE payload
  for the inject call or the watch — reuses the shipped widget-builder assertion.

Plus this slice's cases (Vitest against a **real spawned gateway**, `pnpm test:gateway`):

- **Inject sets the retained input; the next run reads it.** A slider `flows.inject {flow,
  node:setpoint-in, value}` against a seeded `cooler-ctl` flow **upserts**
  `flow_input:kfc:cooler-ctl:setpoint-in` and starts **no** run (assert the retained record changed,
  no `flow_run` created); then a `temp-in` event fires a one-shot run that **reads** the new setpoint
  (assert the run consumed it / the resulting `flow_node_state` change).
- **Chart renders a flow node's live series value.** A chart bound to a flow output node's series
  renders a value pushed onto that series over the shipped SSE (seed the series, push a tick, assert
  the redraw).
- **Last-value renders instantly, then updates over SSE.** A stat/gauge bound to a node reads
  `flow_node_state` on mount (instant) and then updates from the series watch — proving the state /
  motion split (no poll).

## Risks & hard problems

- **`flows.inject` must be re-checked exactly like any control write.** The whole safety story is
  per-call ws (from the token) + cap + grant; if the host skips the recheck for this verb, a viewer's
  visible slider becomes a privileged actor. The deny test must bite a **real** ungranted inject, not
  a UI hint. **Load-bearing.**
- **Inject fan-out / coalesce.** A slider dragged continuously fires many injects → many retained-input
  upserts. Setting a retained value (not starting a run per drag, [Decision 9](flows-scope.md)) already
  defuses the worst of it, but the upserts should still coalesce per the canonical `coalesce` enum in
  [`flow-run-scope.md`](flow-run-scope.md); the control should also throttle interaction-side. Name it;
  don't let a drag hammer the store.
- **State/motion confusion.** The instant render (last-value) and the live tick (series) are different
  reads; binding a chart to `flow_node_state` (no history) or polling `flows.runs.get` for a value
  feed are the two easy mistakes. The view→source mapping must steer to the series for live.
- **A control write is not an actuation ack.** Best-effort gated state-set; must-deliver device effects
  are the outbox in `flow-run`. The UI must not imply the inject *reached the device* — only that it
  set the retained input the next run will read.

## Related

- [`flows-scope.md`](flows-scope.md) — the spine; **Decisions 1** (version-pinned runs), **2** (source
  → series bridge / the `flow:{ws}:{flow}:{node}` convention this read-out reuses), **5**
  (`flow_node_state` last-value vs the series), **7** (composition, never widening), **9** (inject sets
  a node's retained `flow_input` value; runs stay one-shot — the load-bearing decision for this doc).
- [`triggers-lifecycle-scope.md`](triggers-lifecycle-scope.md) — the `inject` node `flows.inject` sets
  (retained input vs firing trigger) and the event trigger that fires the one-shot run.
- [`flow-run-scope.md`](flow-run-scope.md) — the durable one-shot run that reads the retained input; the
  canonical `coalesce` enum; must-deliver node effects through the outbox.
- [`../frontend/dashboard/widget-builder-scope.md`](../frontend/dashboard/widget-builder-scope.md) — the
  **SHIPPED** v2 control-write (`action:{tool,argsTemplate}` → `bridge.call`, host re-check per call)
  and read-widget (`series.watch`/`series.read`) paths this doc rides; and
  [`../frontend/dashboard/`](../frontend/dashboard/) for the dashboard scope set.
- [`../../vision/0003-iot-dashboard.md`](../../vision/0003-iot-dashboard.md) — the IoT dashboard product
  this bidirectional binding lights up.
- README **§6.1** (API shape), **§6.13** (gateway SSE / the three gates), **§7** (tenancy), **§3**
  (rules 3/5/6/7).
