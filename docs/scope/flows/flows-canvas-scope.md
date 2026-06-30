# Flows scope — the canvas (the editor: author, configure, version, run)

Status: **shipped (Wave 3)** — see [`sessions/flows/flows-canvas-session.md`](../../sessions/flows/flows-canvas-session.md)
+ the Wave 3 section of [`public/flows/flows.md`](../../public/flows/flows.md). This is the ask kept
as the contract. The **editor** sub-doc of the
`scope/flows/` set — read the spine [`flows-scope.md`](flows-scope.md) first; it owns the canonical
**Decisions (v1)** this doc references by number. This is the **frontend surface** (cross-linked from
`../frontend/`): the React Flow canvas where a user authors and edits a flow, renders each node's
config form from its schema, manages versions/drafts, runs and watches it, and imports/exports.

We want a **node-red-style editor in the shell**: a logged-in user opens a `flow`, drags typed nodes
from a palette onto a `@xyflow/react` canvas, wires them, fills a **schema-rendered** config form per
node, saves (DAG-validated up front), runs it, and watches nodes **colour live**. It **generalises the
shipped chain canvas** (`rules-workbench-scope.md`) from the chain `Step` to the full `Node` model — it
is **not** a new canvas, and it adds **no new authority**: it is a pure **client of the shipped
`flows.*` / `flows.nodes` gateway verbs**, exactly as the rules workbench is a client of `chains.*`.

## Goals

- **The canvas.** A `@xyflow/react` graph of typed nodes + `needs` edges over a loaded `flow:{ws}:{id}`
  — reusing/extending the rules-workbench chain canvas, not a fresh build. Nodes **colour by run
  status** from `flows.watch` (SSE — preferred) or `flows.runs.get` (poll — fallback).
- **The palette** from `flows.nodes` (the merged registry, [`node-descriptor-scope.md`](node-descriptor-scope.md)):
  built-in node kinds **plus every installed extension's `[[node]]` descriptors**, grouped by
  `category`. Drag a node type onto the canvas → a node instance (a `flow` record edit).
- **Schema-driven config forms** (the "no hardcoded UI" goal, Decision 3): each node's settings panel is
  **rendered from its descriptor's inline JSON-Schema (2020-12)** by a JSON-Schema form renderer and
  **validated in-browser with `ajv`** before save. **No per-node-type hand-coded form**, ever.
- **Legible draft-vs-pinned + executed-node-lock** (Decision 1): editing produces a **new version** on
  save; during an active run the editor banners "this run is on v3; your edits become v4," renders
  **executed nodes read-only**, and offers `flows.patch_run` (config-only) for **unexecuted** nodes of
  the live run. When that config-only `flows.patch_run` is offered on a live run, the `SchemaForm`
  renders the run's **pinned** node schema (the pinned flow version), **not** the latest descriptor —
  the editor side of [Decision 12](flows-scope.md); otherwise the form would offer fields the pinned run
  rejects. A structural edit is plainly a new-version action — `ResumePointDrift` never surprises.
- **Import/export** a flow as JSON (graph + node configs + version); **validate on import** (schema +
  DAG validity) through `flows.save`, which validates up front.
- **Undo/redo** that rides the store `write_tx` journal (`../undo/`) — undo for free, with a multi-record
  edit (add-node = node record + edges) grouped so it **reverses atomically**.
- **Run controls in the canvas** — run/suspend/resume/cancel mapped to
  `flows.run` / `flows.suspend` / `flows.resume` / `flows.cancel`; a live node-status overlay; a failed
  node shows its **error**; a `Halt`-pruned subtree is **greyed**.

## Non-goals

- **No new host work, no new verbs, no new caps.** The canvas consumes the shipped `flows.*` /
  `flows.nodes` surface (spine + `flow-run-scope.md` + `node-descriptor-scope.md`). The `flows.watch`
  SSE route is the one *named* host slice it prefers — built in `flow-run-scope.md`, not here. That
  route is the **first** implementation of the still-scoped watch/SSE pattern (`chains.watch` is itself
  `status: scope` in `../rules/rule-chains-scope.md`, not shipped); flows **builds** it rather than
  inherits it. Until it lands the canvas falls back to polling `flows.runs.get` for live status.
- **No editor-side execution, scheduling, or validation authority.** DAG validity, version allocation,
  step-claim, and `ResumePointDrift` are the **engine's** (`flow-run-scope.md`); the canvas renders the
  decision, it does not make it. An invalid edge is a `flows.save` rejection shown inline, not a
  client-side veto pretending to be the rule.
- **No second canvas library or form library.** `@xyflow/react` is already a dependency (rules
  workbench); the JSON-Schema form is a shadcn-styled renderer (below), not Monaco/MUI/AntD.
- **No client-persisted graph state.** The `flow` record is the truth; transient unsaved buffer lives in
  component state only — no durable `localStorage` graph (rule 4, the rules-workbench precedent).
- **No new sharing UI.** Family caps + the grant model gate access (Decision 7); per-flow narrowing is
  the platform-wide authz follow-up, not invented in the editor.

## Intent / approach

**Generalise the shipped chain canvas; add no authority.** The rules workbench already ships a
`@xyflow/react` DAG (nodes = chain steps, edges = `needs`, coloured by polling `chains.runs.get`). The
flows canvas is that same surface lifted to the `Node` model: nodes are **typed** (Trigger / Tool /
Rhai / Subflow / Sink, spine model), the colour source is `flows.watch`/`flows.runs.get`, and the
node-config form is **rendered from a schema** instead of a fixed step card. It mirrors the dashboard
template the workbench uses — a `lib/flows/flows.api.ts` client (one export per verb, 1:1) → a
`features/flows/canvas/` React surface — **minus the host layer** (it exists).

The headline is **two pieces of legibility**:

1. **No hardcoded UI** — one generic `SchemaForm` renders *every* node's settings from its descriptor's
   JSON-Schema and validates with `ajv` before `flows.save`. A new extension node gets a config form for
   free; the editor learns nothing about it.
2. **Versioning made visible** — the editor shows, at all times, whether you are editing a draft that
   will become a new version and whether a run is pinned to an older one. The "edit-while-running"
   footgun is dissolved by Decision 1; the canvas's job is to make that *obvious* (read-only executed
   nodes, the v-pinned banner, `patch_run` only on unexecuted nodes).

```
  features/flows/canvas/FlowCanvas (@xyflow/react)   nodes=typed Node, edges=needs
      │  palette ──flows.nodes──► PaletteGroups (by category: built-in + ext [[node]])
      │  drag node type ──► add node (flow record edit, journalled → undo)
      │  select node ──► SchemaForm(descriptor.schema) ──ajv──► valid? ──► flows.save
      │  Save ──flows.save {graph}──► validated up front (DAG) → new version | inline error
      │  Run  ──flows.run──► {run_id};  watch ──flows.watch (SSE)──► colour nodes live
      │  Suspend/Resume/Cancel ──flows.suspend/resume/cancel──►
      │  active run? ──► v-pinned banner + executed nodes read-only + patch_run (unexecuted)
      ▼
  gateway routes/flows.rs (re-check mcp:flows.<verb>:call, ws from token)  ── NO new authority
      ▼
  host flows.* engine (SHIPPED via the sibling scopes) — DAG validate · version · run · watch
```

**Rejected alternatives:**

- *Hand-code a config form per node type.* Rejected — it defeats the whole "extensions contribute
  nodes" thesis (every new node would need shell code) and contradicts Decision 3 (the descriptor's
  inline JSON-Schema *is* the form contract). One `SchemaForm` over `ajv`, styled with shadcn primitives.
- *A bespoke or third-party JSON-Schema form lib (MUI/AntD/RJSF default theme).* Rejected — it drags in
  a non-shadcn component kit against `ui-standards-scope.md` (shadcn-first, Tailwind, responsive). The
  renderer is a **shadcn-styled JSON-Schema form component** (a thin renderer mapping
  string/number/enum/boolean/object/array to shadcn `Input`/`Select`/`Switch`/field-array primitives),
  with `ajv` doing the validation — RJSF's *engine* is acceptable behind a shadcn theme, but the visible
  widgets are ours.
- *Mutate the running graph in place (true edit-while-running).* Rejected by Decision 1 (rewrites
  append-addressed history). The canvas surfaces the **new-version** model and `patch_run` for
  config-only tweaks instead.
- *Poll `flows.runs.get` as the primary live source.* Rejected as the default — that is motion dressed
  as a snapshot loop (rule 3). `flows.watch` SSE is **preferred**; the snapshot poll is the **fallback**
  (and the late-open one-shot rebuild), matching the rules-workbench's named `chains.watch` follow-up,
  now realised for flows.
- *A new graph library.* Rejected — `@xyflow/react` already ships in the rules workbench.

## How it fits the core

- **Tenancy / isolation (rule 6):** the canvas only ever edits **ws-scoped** `flow:{ws}:{id}` records
  through the gateway, ws taken from the **token, not the body** (the workbench pattern). A user cannot
  load, save, run, or watch another workspace's flow — the gateway is workspace-first and the canvas has
  no cross-ws path. **Mandatory isolation test** at the UI + gateway boundary.
- **Capabilities (rule 5/7) — show but gate:** the canvas is a **caller** of shipped caps, adding none.
  Nav + run controls gate on `mcp:flows.{run,suspend,resume,cancel,save,get,list,patch_run}:call` and
  the `flows.runs.get`/`flows.watch` reads. The **palette is filtered to nodes whose underlying tool the
  user could call** — but a node whose tool the user lacks is **shown and gated**, not silently hidden:
  the menu reflects permissions, and an attempt to run/inject through it is **refused honestly** by the
  engine (`caller ∩ grant`, no widening). The UI gate is convenience; the **gateway + engine are the
  wall**. **A capability-deny test** surfaces this.
- **Placement / symmetric nodes (rule 1):** one app, two deliveries — Tauri `invoke` on the
  `workstation`, SSE/HTTP from the `hub` to a `browser`. The same `flows.*` routes are role-mounted by
  config, never `if cloud`.
- **MCP surface — consumed, not added (§6.1):** this scope **defines no MCP tools**. The shapes it drives:
  - **CRUD:** `flows.save` (create/update — version-allocating), `flows.delete`; node add/remove/rewire
    are graph edits *within* a `flows.save`, not separate verbs.
  - **Get / list:** `flows.get` (load the graph), `flows.list` (the picker), `flows.nodes` (the palette
    registry), `flows.runs.get` (the snapshot the late-open and fallback poll read).
  - **Live feed (SSE):** **`flows.watch`** — the **preferred** colour source (state vs motion, rule 3);
    the canvas subscribes for the active run, falling back to the bounded `flows.runs.get` poll only when
    SSE is unavailable. Flows **builds the first** implementation of this watch/SSE pattern — the
    workbench's `chains.watch` is still scoped (`../rules/rule-chains-scope.md`), not shipped; the canvas
    depends on `flow-run-scope.md` landing `flows.watch`, and polls `flows.runs.get` for live status until
    it does (see [Decision 3](flows-scope.md) on the state-vs-motion stance).
  - **Run controls:** `flows.run` → `{run_id}` (a flow run is a job), `flows.suspend` / `flows.resume` /
    `flows.cancel`, and **`flows.patch_run {run_id, node, config}`** (config-only, unexecuted nodes —
    Decision 1).
  - **Batch:** **N/A** — the user edits/runs one flow at a time; the run *is* the durable job. Import is a
    single `flows.save`. Stated per §6.1, not a silent omission.
- **Data (SurrealDB):** **no new tables.** The canvas reads/writes the shipped `flow:{ws}:{id}` graph
  and reads `flow_run` / `flow_step_output` / `flow_node_state` via the verbs. No client-durable graph.
- **State vs motion (rule 3):** the graph + config are **state** (`flows.get`/`flows.save`); the live run
  colour is **motion** (`flows.watch` SSE), never a `setInterval` masquerading as live beyond the bounded
  fallback.
- **Undo (rule "undo for free"):** graph edits journal through the store `write_tx` seam (`../undo/`); the
  editor surfaces `undo`/`redo` and **groups** a multi-record edit (add-node = node record + its edges)
  so it reverses **atomically** — no half-deleted node with dangling edges.
- **SDK/WIT:** **none.** The canvas touches no plugin boundary; node types arrive via the already-frozen
  `[[node]]` descriptor (the spine's only manifest addition), surfaced through `flows.nodes`.

## Example flow

1. **Open + palette.** Alice (in `kfc`, holding `mcp:flows.*` and the node tools) opens the **Flows**
   canvas (cap-gated nav). `flows.nodes` returns the merged registry; the palette groups built-ins and
   the installed `mqtt` extension's nodes by `category`. A node whose tool she lacks shows but is marked
   gated.
2. **Author.** She drags `mqtt.in` (an extension node), a `Rhai` node, and an `inbox.raise` **Tool** node
   onto the canvas and wires `mqtt.in → Rhai → inbox.raise` (each drag/edit is journalled).
3. **Configure (schema-rendered).** Selecting `mqtt.in` opens `SchemaForm` rendered from its descriptor's
   JSON-Schema (broker, topic, qos). She types an invalid `qos: 9`; `ajv` flags it inline and **Save is
   blocked** — no hand-coded form, no fake accept.
4. **Save (DAG-validated).** She fixes it and **Save** → `flows.save`, validated up front. A cyclic wire
   would return a validation error rendered as an **inline edge error**; a valid graph persists as
   **version 1**.
5. **Run + watch live.** **Run** → `flows.run` → `{run_id}`; the canvas subscribes to `flows.watch` and
   nodes colour live: `mqtt.in` green, `Rhai` Running → green, `inbox.raise` green. A failed node shows
   its **error**; a `Halt`-pruned subtree greys.
6. **Pause, patch, resume (legible versioning).** A long run is mid-flight on **v3**. Alice **Suspend**s;
   the banner reads "this run is on v3 — your edits become v4." Executed nodes are **read-only**; she
   config-patches an **unexecuted** node via `flows.patch_run {run_id, node, config}`, then **Resume** —
   the engine validates the next frontier still matches (no `ResumePointDrift`). A *structural* edit
   instead would have been a v4 save for the next run, plainly signposted. **Export** writes the flow as
   JSON (graph + configs + version); a teammate's **Import** re-validates (schema + DAG) via `flows.save`.

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md` — **Vitest against a REAL spawned gateway**
(`pnpm test:gateway`, **no `*.fake.ts`**, testing-scope §0), seeded with **real** records through the
real write path (a real installed extension node registered so `flows.nodes` returns it):

- **Workspace isolation (MANDATORY).** Two real sessions: a ws-B principal **cannot load, save, run, or
  watch** a ws-A flow; the picker and canvas are workspace-partitioned. Across gateway + store.
- **Capability deny — surfaced honestly (MANDATORY).** A node whose underlying tool the user lacks is
  **shown but gated**: a run/inject through it is **refused** by the engine (`caller ∩ grant`), rendered
  as an honest deny, never a fake success. One deny-test per gated path.
- **Offline / sync (via the engine).** Resume idempotency is the engine's (`flow-run-scope.md`); the
  canvas test asserts the resume-after-suspend path re-colours from `flows.watch`/`flows.runs.get`
  without a duplicate run, reusing that harness — not a new mechanism.
- **Hot-reload (palette).** Swapping a node-providing extension re-populates the palette from a fresh
  `flows.nodes` without an editor restart (the spine's hot-reload claim, at the UI boundary).

Plus this surface's specific cases:

- **Palette renders from a real `flows.nodes`** — built-ins + a seeded real extension node, grouped by
  `category`; a gated node is shown-but-marked.
- **Schema-driven config form** — a node's `SchemaForm` renders from its JSON-Schema and **rejects an
  invalid value via `ajv`** before save (no hand-coded form path exercised).
- **Save round-trips** through `flows.save` and **rejects an invalid DAG** (cycle/dangling) with an
  inline edge error — no save.
- **Run colours nodes live from `flows.watch`** — a seeded graph run drives the SSE; nodes go
  Pending → Running → ok/err; a failed node shows its error; a `Halt` subtree greys. A late open rebuilds
  the same colours from the `flows.runs.get` snapshot.
- **Executed-node-lock + v-pinned banner** appear during an active run; `flows.patch_run` succeeds on an
  unexecuted node and is unavailable on an executed one.
- **Import/export round-trips** a flow JSON (graph + configs + version) through `flows.save` validation.
- **Undo restores a deleted node + its edges atomically** (the grouped `write_tx` journal entry).

## Risks & hard problems

- **The schema form's coverage is the load-bearing risk.** "No hardcoded UI" only holds if `SchemaForm`
  covers the JSON-Schema subset the descriptors actually use (string/number/enum/boolean/object/array +
  the common keywords). A descriptor that exceeds it must **fail loud** ("unsupported schema"), never
  silently drop a field — Decision 3's "a huge schema is a smell, split it" is the guardrail; the form
  must surface the gap, not paper over it.
- **Version legibility under an active run.** The executed-node-lock + v-pinned banner must track the
  *engine's* notion of executed/frontier exactly (from the run snapshot), so the read-only set and the
  `patch_run`-eligible set match what `flows.resume` will accept — a mismatch is exactly the
  `ResumePointDrift` surprise this UX exists to prevent. Drive both from the same `flows.runs.get` /
  `flows.watch` truth, never a client guess.
- **Reattach to an active run on open.** The canvas holds a `flow_id`, but `flows.watch` and the
  run-controls need the active **`run_id`** — which an editor opened mid-run does not yet have. On open
  it **reattaches** by calling **`flows.runs.list {flow_id, status:"active"}`** (the active-run lookup
  added to the run surface in [`flow-run-scope.md`](flow-run-scope.md)) — or by reading the active run id
  off `flows.get`. Without this lookup the canvas cannot rejoin a run already in flight, so it is an
  explicit step, not an inference.
- **Canvas ↔ record 1:1 mapping + watch fallback bound.** The `@xyflow/react` node/edge model must
  serialise **faithfully** to the `flow` graph record (the rules-workbench chain-canvas constraint,
  generalised) — no canvas-only state the record can't hold. And when SSE is unavailable the fallback
  `flows.runs.get` poll must stay bounded (interval while non-terminal, stop on terminal, one-shot on
  late open) so it never hammers the node — the workbench settle-poll discipline, carried over.

## Related

- [`flows-scope.md`](flows-scope.md) — the spine; the canonical **Decisions (v1)** (esp. **1** version
  pinning + `patch_run`, **3** inline JSON-Schema config, **6** `flows.*` generalises `chains.*`, **7**
  sharing). Read first.
- [`node-descriptor-scope.md`](node-descriptor-scope.md) — the `[[node]]` descriptor, the merged
  `flows.nodes` registry, and the inline JSON-Schema the `SchemaForm` renders.
- [`flow-run-scope.md`](flow-run-scope.md) — the run job, `flows.run/suspend/resume/cancel`,
  `flows.patch_run`, `flows.watch`/`flows.runs.get`, `ResumePointDrift` — what the canvas drives.
- [`dashboard-binding-scope.md`](dashboard-binding-scope.md) — the sibling read/write surface
  (`flows.inject` from a control, a widget reading a node output).
- [`../frontend/rules-workbench-scope.md`](../frontend/rules-workbench-scope.md) — the **shipped** chain
  canvas (`@xyflow/react`, settle-colouring) this generalises; the gateway-route + `*.api.ts` + React
  template it reuses.
- [`../frontend/ui-standards-scope.md`](../frontend/ui-standards-scope.md) — shadcn-first primitives,
  Tailwind, responsive — the standard the `SchemaForm` and canvas chrome follow.
- [`../undo/`](../undo/) — the `write_tx` journal the graph edits ride for free (grouped per edit).
- README **§6.5** (MCP — the contract), **§6.12/§6.13** (one-app-two-deliveries + the gateway SSE/HTTP
  path), **§3** (the non-negotiables — state vs motion, workspace wall, capability-first, symmetric
  nodes).
- This is the **frontend surface** of flows and should also be linked from `../frontend/` once promoted.
