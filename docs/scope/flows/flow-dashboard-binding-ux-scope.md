# Flows scope — the flow⇄dashboard binding UX (pick a node + port; switch / slider / JSON, both ways)

Status: scope (the ask). Promotes to [`public/flows/flows.md`](../../public/flows/flows.md) once shipped.
Read the spine [`flows-scope.md`](flows-scope.md) (canonical **Decisions 1–13**) and the **shipped
mechanism** this builds on — [`dashboard-binding-scope.md`](dashboard-binding-scope.md) (the `flows.inject`
write-in + series read-out) — first. This doc adds **no new transport**: it makes the shipped binding
**authorable and bidirectional for structured data** with a UX that hides the magic strings.

> **Depends on [`flow-message-envelope-scope.md`](flow-message-envelope-scope.md)** (the Node-RED-style
> `{payload, topic}` envelope + auto-wire). Build that first. Once it lands, a flow node's primary
> in/out port is **`payload`** and its routing slot is **`topic`** — this picker offers those by name,
> controls write `payload`, and read views default to `payload`. Where this doc says "port/slot", read
> "`payload` / `topic` / any descriptor port".

The bidirectional binding mechanism shipped (Wave 3), but **nobody can author it without knowing internal
conventions.** Today a user who wants a dashboard switch to drive a flow node must hand-type a control
action `{tool:"flows.inject", argsTemplate:{id, node, value:"{{value}}"}}`, and to read a node back must
know the series subject string `flow:{ws}:{flow}:{node}`. The dashboard source picker
([`sourcePicker.ts`](../../../ui/src/features/dashboard/builder/sourcePicker.ts)) has groups for series,
live, extensions, SQL, and packaged tiles — **but zero flow awareness** (no `flows.*` anywhere in the
dashboard feature). We want the "really nice UX" the spine promised: **pick a flow, pick a node, pick a
port/slot — and a switch / slider / JSON control or a value/JSON widget is wired for you, in the right
direction, reflecting the node's real current value.** Plus first-class **structured JSON** in and out,
not just scalars.

## Goals

- **A flow-aware source picker.** Add a **Flows** group: choose a **flow** (`flows.list`) → a **node**
  (`flows.get`) → a **port/slot** (`flows.nodes` descriptors carry each node type's `inputs[]`/
  `outputs[]`). The picker resolves the selection to the correct binding with **no hand-typed tool names
  or subjects**:
  - an **input port** → a write **control** action (`flows.inject`, port-aware — see below);
  - an **output port** → a read **source** (the node's current value + its live updates).
- **Switch / slider that actually drive a flow.** The shipped control views
  ([`SwitchControl`](../../../ui/src/features/dashboard/views/SwitchControl.tsx),
  [`SliderControl`](../../../ui/src/features/dashboard/views/SliderControl.tsx)) wired through the picker
  with correct value mapping — a switch sets a **boolean** retained input, a slider a **number** (with
  min/max/step), each landing in `flow_input` for the next run (Decision 9).
- **Structured JSON in AND out (the explicit ask).** `flows.inject` already accepts **any JSON value** —
  surface a **JSON control** (a validated object/array editor, built on the shipped
  [`JsonPayloadField`](../../../ui/src/features/dashboard/builder/JsonPayloadField.tsx)) that sends a JSON
  document to a node, and a **JSON / object view** that renders a node's structured value back out (no
  such read view exists today — the built views are chart/stat/gauge/table/scripted/control).
- **Controls reflect the node's real current value.** A switch/slider/JSON control bound to a node reads
  the node's **current persisted value on open** (not a hard-coded default), so the dashboard shows true
  state after a reload or restart — the same "show real state, not a guess" principle the runtime banner
  now follows.
- **Round-trip, authored in clicks.** One dashboard that drives a flow (control → port → `flows.inject`)
  and visualises it (widget ← node value), built entirely by picking from lists — re-checked per call,
  workspace-walled.

## Non-goals

- **No new write/read transport.** Controls still call `flows.inject` through the shipped `bridge.call`;
  reads still come off the shipped value paths. This doc is **authoring UX + structured-value handling +
  read-back**, not a new mechanism. (Re-stating [`dashboard-binding-scope.md`](dashboard-binding-scope.md)
  Non-goals.)
- **No new dashboard cell contract.** A binding is still a `Source {tool,args}` or `Action
  {tool,argsTemplate}` on the existing cell; the Flows group just *produces* those. No new persistence.
- **No flow editing from the dashboard.** The picker **reads** the graph to offer choices; it never adds
  nodes or rewires the flow (that's the canvas). Selecting a port that doesn't exist is a save-time error,
  not a flow mutation.
- **No bespoke flow read verb beyond what's needed for live.** Read-out reuses `flows.node_state` (last
  value) for the instant render; the *live* upgrade is a small node-state watch (a later slice — see Decisions), not a
  re-implementation of `series.*`.
- **Not a generic form builder.** The JSON control is a single structured value per node port (validated
  against the port's schema when one exists), not a multi-field form designer.

## Intent / approach

**Three existing seams already meet; we add the picker that joins them and the two value-shaped views
(JSON in / JSON out) that were missing.**

1. **The picker becomes flow-aware (the headline UX).** `sourcePicker.ts` is explicitly designed so "a
   new source kind is just a tool" — we add a `flows` group built from **already-shipped reads**:
   `flows.list` (flows) → `flows.get` (the chosen flow's nodes + their `type`) → `flows.nodes` (the
   descriptor registry, which carries each type's `inputs[]`/`outputs[]` **ports**). A selected **input
   port** emits an `Action { tool:"flows.inject", argsTemplate:{ id, node, port, value:"{{value}}" } }`;
   a selected **output port** emits a `Source` reading that node's value (below). The author sees
   `cooler-ctl › setpoint-in › value (input)` — never a tool name. Because the picker reads only existing
   verbs, **no builder rewrite** and no new read surface for authoring.

2. **Port-aware inject (so "slots" are real).** Shipped `flows.inject` is **node-level** (it upserts one
   `flow_input:{flow}:{node}` value). A node with several input ports can't be driven port-by-port. We
   extend `flows.inject` with an **optional `port`**: `flow_input:{flow}:{node}` stays the whole-node
   value (back-compat, what cron/event runs read), and `flow_input:{flow}:{node}:{port}` holds a
   per-port value that the run's binding resolver prefers for that port. *Rejected:* leaving it node-only
   (the user explicitly asked to pick *ports/slots*; a multi-input node would be undrivable) and a
   separate `flows.set_port` verb (forks the one gated inject door for no gain — an optional arg composes,
   a second verb duplicates the cap + recheck).

3. **JSON is already a value, not a special case.** `flows.inject`'s `value` is `serde_json::Value` and
   `flow_node_state` stores any JSON — so the wire already carries objects/arrays. The missing pieces are
   purely **frontend views**: a **JSON control** (reuse `JsonPayloadField` — validate, then inject the
   parsed value) and a **JSON/object read view** (pretty-print the node's structured value, with
   collapse). A switch is the boolean special case of a value control; a slider the number case; the JSON
   control the general case — all three are one binding (`flows.inject` to a port) with different editors.

4. **Read-back closes the loop honestly.** A control bound to a node reads that node's **current value**
   on mount via `flows.node_state {id}` (instant, state) so the switch shows on/off as it really is. For
   an **output** read view, the same `flows.node_state` gives the instant value and a slow node-state
   refresh (the canvas's existing armed tick cadence) advances it; the live-push upgrade is an Open
   question. *Rejected:* binding a control's current state to a hard-coded default (lies after restart)
   and polling `flows.runs.get` for a value (a run-inspection read mistaken for a value feed — rule 3).

## How it fits the core

- **Tenancy / isolation (rule 6):** every picker read (`flows.list`/`get`/`nodes`/`node_state`) and the
  `flows.inject` write derive the workspace from the **session token**, never the cell. A ws-A dashboard
  cannot list, read, or inject into a ws-B flow; `flow_input:{flow}:{node}[:port]` and
  `flow_node_state:{flow}:{node}` are ws-namespaced. **Mandatory two-session test across store + MCP.**
- **Capabilities (rule 5/7) — composition, never widening (Decision 7):** the picker only **offers**
  flows/nodes the caller can read (`mcp:flows.list:call`, `mcp:flows.get:call`, `mcp:flows.nodes:call`);
  a control only drives a node the caller may inject (`mcp:flows.inject:call` **and** granted the target
  flow). A viewer **sees** a control in the saved layout but the drive is **denied at the bridge** — the
  retained input is never touched (the headline deny). The added `port` arg does not widen anything — the
  cap is still `mcp:flows.inject:call`. No new cap for read-back (reuses `mcp:flows.node_state:call`).
- **Placement (rule 1):** one path, two transports (Tauri `invoke` / gateway SSE+HTTP); no role branch.
  All picker reads + the inject route through the existing host dispatch.
- **MCP surface (§6.1):**
  - **CRUD/write:** `flows.inject {id, node, value, port?}` — the **only** verb change: an additive
    optional `port`. Synchronous, bounded (upsert + return); no new write verb.
  - **Get/list:** **no new verbs** — the picker composes shipped `flows.list` (flows), `flows.get`
    (nodes), `flows.nodes` (ports), and `flows.node_state` (current values). State the read caps above.
  - **Live feed:** instant + slow-refresh via `flows.node_state` for v1 (matches the canvas); a
    `flows.node.watch` SSE is the live upgrade (a later slice — see Decisions) — **not** polling `runs.get`.
  - **Batch:** N/A — a control fires one inject per interaction (coalesced on drag, below).
- **One datastore / state vs motion (rule 3):** retained inputs = `flow_input` (state, Decision 9);
  current node value = `flow_node_state` (state, Decision 5); the control write is a gated tool call.
  Distinct things kept distinct; no new table beyond the additive `:{port}` key on `flow_input`.
- **Durability:** none here — a control write is a **best-effort gated state-set** (`flows.inject`
  upserts and returns). A must-deliver *device* effect is the **outbox inside the run**
  ([`flow-run-scope.md`](flow-run-scope.md)), not the inject; the JSON control must not imply the value
  reached a device, only that it set the input the next run reads.
- **No mocks / no fake backend (CLAUDE §9):** the picker is tested against a **real spawned gateway**
  with **real seeded flows** (`pnpm test:gateway`); no `*.fake.ts`. The Flows group reads real
  `flows.list`/`get`/`nodes`; a control fires a real `flows.inject` and the test asserts the real
  `flow_input` record changed.
- **Stateless / SDK-WIT:** no durable state in the dashboard or picker (graph in the store, retained
  value in `flow_input`, current value in `flow_node_state`). **No manifest/WIT change** — `flows.inject`
  stays an ordinary host verb (one additive arg); the views are first-party. Flag if `port` ever needs a
  descriptor schema change — it does not (ports already live on the descriptor).

## Example flow — authoring a "Cooler Control" dashboard in clicks

The same `cooler-ctl` flow as [`dashboard-binding-scope.md`](dashboard-binding-scope.md), but **authored
through the picker** with a structured payload added.

1. **Add a control, pick the flow.** Alice clicks *Add widget → Control*. The source picker's **Flows**
   group lists her workspace's flows (`flows.list`); she picks `cooler-ctl`.
2. **Pick the node + port.** The picker calls `flows.get cooler-ctl`, lists its nodes, and for the chosen
   `setpoint-in` node resolves its type's **input ports** from `flows.nodes`. She picks the `value` input
   port. The picker emits `Action {tool:"flows.inject", argsTemplate:{id:"cooler-ctl", node:"setpoint-in",
   port:"value", value:"{{value}}"}}` — she typed nothing.
3. **Choose the control shape.** She picks **Slider**, sets min 0 / max 10 / step 0.5. On mount the
   control reads `flows.node_state cooler-ctl` and shows the **current** retained setpoint (e.g. 4), not
   0 — true state after any reload.
4. **Drag → inject.** Dragging to 4 fills `{{value}}` and calls `flows.inject {…, port:"value",
   value:4}` through `bridge.call`; the host re-checks `mcp:flows.inject:call` + the `kfc` workspace
   (from her token) + that she's granted `cooler-ctl`, then upserts `flow_input:kfc:cooler-ctl:setpoint-in:value
   = 4`. A drag burst **coalesces** (below). **No run per drag** (Decision 9).
5. **Add a JSON control for a structured input.** For a `profile-in` node she picks **JSON**, and the
   `JsonPayloadField` validates `{ "mode": "eco", "band": [3.5, 4.5] }` before injecting it as one value —
   structured data in, no per-field widgets.
6. **Add a read view.** *Add widget → for `temp-out` output port → JSON/object view*. It renders
   `flow_node_state:kfc:cooler-ctl:temp-out` instantly on open and advances on the node-state refresh.
7. **DENY.** Dave (viewer, lacks `mcp:flows.inject:call`) sees the slider but dragging is **refused at
   the bridge** — `flow_input` untouched. He may still read if he has `flows.node_state`.
8. **Isolation.** Erin (ws `mcdonalds`) builds the same control; her inject resolves
   `flow_input:mcdonalds:cooler-ctl:…` — a different/absent flow; she cannot touch `kfc`.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`) — real gateway, real store (`mem://`), real flow
records seeded, **no `*.fake.ts`**:

- **Capability-deny (headline).** (a) A control whose caller lacks `mcp:flows.inject:call` → the inject
  is **denied server-side** even if the bridge filter were bypassed; assert `flow_input` is **not**
  upserted (node- and port-keyed). (b) A caller not granted the target flow → denied (no widening). (c)
  The picker does **not** list flows the caller can't `flows.get`. Deny is opaque.
- **Workspace-isolation (store AND mcp).** Two real sessions: a ws-A control cannot inject into ws-B's
  `cooler-ctl`; a ws-A read view cannot read ws-B's `flow_node_state`; the picker in ws-A never lists
  ws-B flows. Assert at both the record layer (`flow_input`/`flow_node_state` ws-namespaced) and the MCP
  layer (verbs denied).
- **Token never crosses the boundary.** The session token appears in no `bridge` arg or payload for the
  inject or the node-state read (reuse the shipped widget-builder assertion).

Plus this slice's cases:

- **Backend (`cargo test`, real store):** `flows.inject` with `port` upserts `flow_input:{flow}:{node}:{port}`
  **and** leaves/derives the node-level value correctly; the run's binding resolver prefers a per-port
  retained value over the node-level one; an inject **without** `port` is unchanged (back-compat
  regression). A JSON-object `value` round-trips through inject → `flow_input` → run read.
- **Frontend against a real spawned gateway (`pnpm test:gateway`):**
  - The Flows picker group lists seeded flows, drills flow → node → port from `flows.get`/`flows.nodes`,
    and emits the correct `Action`/`Source` (assert the produced binding, not a mock).
  - A **slider** control fires a real `flows.inject` that changes the seeded `flow_input` (assert the
    record), and on mount reflects the seeded current value (not the default).
  - A **switch** sets a boolean retained input; a **JSON control** injects a validated object (and
    **rejects** invalid JSON before any call — no fake accept).
  - A **JSON/object read view** renders a node's seeded structured `flow_node_state` value, then advances
    when the value changes.

## Risks & hard problems

- **`flows.inject` must stay re-checked exactly like any control write** — the whole safety story is
  per-call ws + cap + grant. The added `port` arg must not open a side-door; the deny test must bite a
  **real** ungranted port-inject. **Load-bearing.**
- **Port semantics vs node-level retained value.** Introducing `flow_input:{…}:{port}` alongside the
  node-level value risks two sources of truth for a run's input. The binding resolver's precedence
  (per-port over node-level over the static `with`) must be **explicit and tested**, or a control will
  appear to set a value the run ignores — exactly the "count didn't go up / value didn't take" class of
  confusion. Define the precedence in the run scope, not ad hoc.
- **Live read-out is state, not a series.** Arbitrary nodes (counter/transform) update `flow_node_state`
  in place (rev bump) — they do **not** all emit a `series`. Binding a live widget to `series.watch
  flow:…` (the old binding-scope assumption) silently shows nothing for such a node. v1 must read
  `flows.node_state` (instant + refresh); the live upgrade needs a real node-state watch (a later slice — see Decisions),
  not a series assumption.
- **Control current-value drift.** Reading the *output* `flow_node_state` to seed a control that drives an
  *input* conflates two values. A control's "current" must read its **own input's** retained value
  (`flow_input`), which `flows.node_state` does **not** currently return — extended per Decisions; getting
  this wrong makes a switch show the wrong state.
- **Drag fan-out / coalesce.** A dragged slider fires many injects; setting a retained value (not a run
  per drag, Decision 9) defuses the worst, but the upserts must coalesce (the canonical `coalesce` enum,
  [`flow-run-scope.md`](flow-run-scope.md)) and the control should throttle interaction-side.
- **Picker staleness.** A flow re-saved (node renamed/removed) can orphan a cell's `node`/`port`. A
  missing target must render an honest "binding broken — re-pick" state, never a silent no-op.

## Decisions (resolved — no open questions)

- **Read-back of retained inputs → extend `flows.node_state`.** Fold each node's retained `flow_input`
  value (and per-port values) into the existing `flows.node_state {id}` response so the dashboard and the
  canvas share one read. A control seeds its current state from the node's retained `payload` (its
  **input**), not its output. No new verb.
- **Live read-out → refresh for v1.** Read views render `flows.node_state` instantly and advance on the
  canvas-cadence refresh tick. A `flows.node.watch` SSE (a `flow_node_state` rev stream) is a **later**
  slice, added only when a widget needs sub-second liveness — not in this one. Never poll `runs.get`.
- **Port precedence (run input):** **per-port retained > node-level retained > static `with`**. Ratify
  and test this in [`flow-run-scope.md`](flow-run-scope.md)'s resolver (it composes with the envelope
  scope's auto-wire: an explicit retained value always wins over auto-wire).
- **Output read-out → node-level `payload` for v1.** An output-port selection reads the node's
  `payload` from `flow_node_state`. Sub-field selection (a JSON path into the envelope) is a later
  refinement of the JSON read view, not v1.
- **JSON control validation → validate against the port schema when one exists.** If the node type's
  descriptor declares an input schema, the JSON control validates against it (reuse the canvas config
  panel's ajv path); otherwise it accepts free JSON. Never a fake accept.

## Related

- [`dashboard-binding-scope.md`](dashboard-binding-scope.md) — the **shipped** mechanism (the `flows.inject`
  write-in + read-out) this UX makes authorable; its Decisions 9/5/2/7 carry over verbatim.
- [`flows-scope.md`](flows-scope.md) — **Decision 9** (inject sets retained `flow_input`; runs one-shot),
  **5** (`flow_node_state` last-value), **7** (composition, never widening).
- [`node-descriptor-scope.md`](node-descriptor-scope.md) — the descriptor `inputs[]`/`outputs[]` **ports**
  the picker reads to offer slots; [`flow-run-scope.md`](flow-run-scope.md) — the binding resolver that
  must honor per-port retained values + the `coalesce` enum.
- [`../frontend/dashboard/widget-builder-scope.md`](../frontend/dashboard/widget-builder-scope.md) — the
  **shipped** control views (`switch`/`slider`/`button`), the `{tool,args}`/`{tool,argsTemplate}` cell
  contract, and the source-picker model this extends; [`../frontend/dashboard/widgets-scope.md`](../frontend/dashboard/widgets-scope.md)
  for the cell/view set the JSON read view joins.
- README **§6.1** (API shape), **§6.13** (gateway SSE / the three gates), **§7** (tenancy), **§3**
  (rules 3/5/6/7).
