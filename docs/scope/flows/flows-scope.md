# Flows scope — the spine (the node-graph engine, generalised from chains)

Status: scope (the ask). Promotes to `public/flows/` once shipped. **Read this first** — it is
the overview for the `scope/flows/` set (see `README.md`) and owns the canonical
**Decisions (v1)** the sibling docs reference by number.

We want a **visual, node-graph flow engine** — a node-red-style canvas where a user wires
**typed nodes** (triggers, transforms, sinks, sub-flows) into a graph, runs it, and watches it
work; where **extensions contribute backend node types** (an `mqtt` extension ships an "MQTT
publish" node dropped into a flow that does the real work when the flow runs); where a running
flow can be **paused, edited, and resumed**; and where a **dashboard reads from and writes to**
a flow. The headline, and the line every sub-doc holds: **this is not a new engine.** It is a
**node model + editor + backend node-registry** over machinery already shipped (`chains.*`
durable rule-DAG, `lb-rules` rhai cage, `lb-jobs` suspend/resume, the dashboard read/write
bridge, the extension host-callback, undo, and the grant model). The detail lives in the
focused sub-docs; this spine fixes the **model**, the **decisions**, and the **shape**.

## Goals (the whole feature)

- A `flow:{ws}:{id}` record holding a **typed, versioned node graph**, authored on a React Flow
  canvas, **import/export** as JSON. *(model here; record + versioning below;
  editor → `flows-canvas-scope.md`.)*
- A **node model** generalising the chain `Step` (below), every node carrying a **descriptor**
  (ports + a config **JSON-Schema** the editor renders a form from — no hardcoded per-node UI).
  *(→ `node-descriptor-scope.md`.)*
- **Extensions contribute backend node types** via `extension.toml`, **identically for WASM and
  native**; the extension does the processing and returns the value through the existing
  `caller ∩ install-grant` callback. *(→ `extension-nodes-scope.md`.)*
- A flow **run** is a durable `flow_run` coordinator + **one `lb-jobs` job per node** (the
  `chains` topology, Decision 8): survives restart, idempotent on resume, concurrent branches,
  supports **pause → edit → resume**. *(→ `flow-run-scope.md`.)*
- **Triggers** `manual | cron | event | inject | boot`; **enable/disable**; **start on boot**;
  a **placement** so "run on the hub" vs "on the appliance" is config, not a branch.
  *(→ `triggers-lifecycle-scope.md`.)*
- **Dashboard ↔ flow binding** — a control writes an input node; a widget reads an output node —
  on the shipped dashboard write/watch paths. *(→ `dashboard-binding-scope.md`.)*
- **Sharing** via the existing grant model; **undo/redo** of graph edits for free at the store
  journal. *(both ride existing primitives — see "How it fits the core".)*

## Non-goals (the whole feature)

- A **new execution runtime, scheduler, or persistence layer.** Flows run on `lb-jobs` + the
  `chains` frontier driver; state is SurrealDB; motion is Zenoh (CLAUDE rules 1–4).
- **Modelling domain entities** (users, teams, records) as graph nodes. "Flow node" (a step) and
  "domain entity" are two different meanings of *node*; we keep them apart.
- A **role-aware placement scheduler** that auto-chooses a node — we honour an explicit
  `placement`; cross-node auto-placement is deferred to `node-roles`.
- Re-cutting the extension manifest beyond the additive `[[node]]` block, or any new SDK/WIT
  world beyond what `host-callback` already froze.

## The node model

A flow is a validated DAG whose every node is a tagged variant — the chain `Step` ("invoke
`rule:{ws}:{id}`") promoted to:

```
Node =
  | Trigger(manual | cron(<spec>) | event(<series>) | inject | boot)
  | Tool(<mcp-verb>, args)        // everything-is-a-node: any granted MCP tool
  | Rhai(<source>)                // the lb-rules cage — the "function node"
  | Subflow(<flow-id@version>)    // parent/child: a node containing a child graph
  | Sink(inbox | outbox | channel | series | <ext-node>)
```

Edges carry **whole-value bindings** in the chain grammar verbatim (`${steps.<id>.output}`,
`${steps.<id>.findings}`, `${params.<name>}`, or a literal — no templating). Because every
capability is already an MCP tool (rule 7), a generic **Tool node** makes the whole system
uniformly reachable — "everything is a node" for *actions*, earned for free. DAG validation
(Kahn cycle-detect, frontier execution, CAS step-claim, `Halt`/`Continue` policy), the run-store,
and the React Flow canvas all carry over from `chains` unchanged.

**Versioning is the load-bearing principle.** A `flow` carries a monotonic `version`; a run
**pins** the version it executes. Editing a flow writes a **new version**, so a live run is
never mutated underneath itself — this is what makes pause-edit-resume safe rather than a
footgun (Decision 1). Everything else hangs off this.

## Decisions (v1)

The canonical, settled decisions for the whole feature. Sibling docs reference these by number
rather than re-deciding. Each names the alternative rejected and why.

1. **Flows are versioned; a run pins a version.** A `flow` carries a monotonic `version`;
   `flows.run` pins `flow_version` into the `flow_run`. **This dissolves "edit-while-running":**
   editing writes a **new version** (the suspended run keeps its pinned graph). "Pause → tweak a
   node → resume" is an **in-place config-only `flows.patch_run {run_id, node, config}`** to
   unexecuted nodes; a **structural** change (add/remove/retype a node, re-wire an edge) becomes
   a new version for the *next* run. On resume the engine validates the next frontier nodes still
   exist with the same type + ports; a mismatch fails cleanly with `ResumePointDrift` in
   `flows.runs.get`. *Rejected:* mutating the running graph in place (rewrites append-addressed
   history) and forbidding all edits during a run (kills the headline use case).
2. **Source nodes bridge via `ingest.write` → a host-allocated series, host-armed.** A `source`
   `[[node]]`'s tool writes incoming external events through `ingest.write` onto the series
   `flow:{ws}:{flow}:{node}` (the shipped MQTT-bridge path); the host **arms** (start, passing
   the series id + validated config) and **disarms** (stop) the source as the flow
   enables/disables. The event-trigger node watches that series. *Rejected:* a per-node raw
   Zenoh subject (forks the ingest convention) and letting the extension choose the series name
   (ws-scoping + uniqueness must be host-owned).
3. **Node config schema is an inline JSON-Schema (2020-12) TOML table in `[[node]]`.** Matches
   the manifest's "metadata inline, no external file refs" convention; validated host-side with
   the `jsonschema` crate and in the editor with `ajv`; the React Flow settings form renders
   from it. A node needing a huge schema is a smell — split it. *Rejected:* a `schema/<node>.json`
   bundle ref (the manifest declares no external paths) and a bespoke dialect (JSON-Schema is
   what the form renderer speaks).
4. **Subflow params reuse the chain binding grammar verbatim.** A `Subflow` references a child
   `flow:{ws}:{id}` (pinned by version at save) and maps parent inputs → child `${params.<name>}`
   and child output nodes → parent ports **by name**, **one whole-value `${…}` reference or a
   literal per binding** — no templating. *Rejected:* a partial-interpolation syntax (resist
   until a real caller needs it).
5. **`flow_node_state` is last-value-only.** One upserted record per node holding its latest
   output, for the dashboard's instant read. Time-history is a **series** the node emits and a
   chart queries — not this record (state vs motion, rule 3). *Rejected:* a ring buffer here.
6. **`flows.*` is the general surface; `chains.*` becomes its rule-only special case.** A chain
   is a flow whose nodes are all `Rhai`/`Tool` rule steps; the flow engine *is* the generalised
   chain frontier driver. `chains.*` ships on as a **thin compatibility alias** delegating to the
   flow engine; new work targets `flows.*`, and `chains.*` deprecates once callers migrate.
   *Rejected:* two parallel engines (duplicate machinery) and an immediate breaking cut.
7. **Sharing is family-level caps + the grant model in v1; per-flow resource caps ride the
   cross-cutting authz follow-up.** `mcp:flows.<verb>:call` gate access; a flow is private to its
   owner and shared to a user/team through the existing grant/role/team model. **Per-resource
   narrowing** (`store:flow/<id>:read`) is the same follow-up `authz-grants` already carries for
   chains/dashboards/queries — it lands for all resource types at once, not flows-first.
   *Rejected:* a per-flow ACL ahead of (and divergent from) that platform-wide mechanism.
8. **Execution topology: one `lb-jobs` job per node, coordinated by the `flow_run` record — the
   `chains` model verbatim.** A run is a `flow_run` coordinator + one job per node (the
   `Job::WorkflowStep` shape generalised to a `flow-step`); the frontier driver enqueues a node's
   job when its in-degree hits 0, and **concurrency is independent branch jobs competing for the
   `lb-jobs` semaphore** — not async tasks inside one worker. This makes "ported verbatim" honest
   and makes Decision 6's alias true at the **execution** layer, not just the MCP layer. Exactly-once
   has **two owners, two layers:** the **CAS claim** (`Enqueued→Running`) on
   `flow_step_output` owns **cross-node** exactly-once under redelivery; **`append_event`**
   idempotency owns **within-a-single-step** replay (a node that is itself a multi-turn job).
   *Rejected:* one job walking a single linear transcript+cursor (it cannot run concurrent
   branches — the framing an earlier draft used, now corrected).
9. **`flows.inject` sets a node's *retained* value; it fires a run only for a firing-trigger
   node. Runs stay one-shot — no long-lived "parked" runs.** The frontier runs to terminal
   completion. A **control loop** is built from **retained input nodes** (a value held in
   `flow_input:{ws}:{flow}:{node}`, read by every run) + **event-triggered one-shot runs** that
   read them: a slider sets a retained `setpoint`, a switch sets a retained `enabled`, and temp
   readings drive runs that consult both. An inject into a *retained* node updates state and does
   **not** start a run; an inject into a *firing* trigger node starts one. *Rejected:* an
   interactive run that **parks** at a gate awaiting inject (a real new primitive that fights
   "frontier runs to completion" — deferred; one-shot + retained inputs covers the IoT loop).
10. **A flow has exactly one owner node; `placement` is the *eligible set*, not replication.** The
    reconciler **elects a single owner** and arms the source / fires runs **once**: `local-only` →
    the install node, `cloud-only` → a hub, `either` → the home node recorded on the flow.
    Cross-node failover is a `node-roles` deferral. *Rejected:* arming on every placement-matching
    node (N broker sockets and N runs per event — the spatial dual of a fan-out storm).
11. **Subflow nodes park on a child run.** A `subflow` node enqueues a **pinned** child `flow_run`
    and its step **parks** (suspends) until the child reaches terminal, then maps child outputs →
    parent ports (Decision 4 grammar). Child failure → the parent node's `Outcome::Err` under the
    parent's `FailurePolicy`; a **parent suspend cascades** to the child; parent and child pin
    versions **independently**. "A step waits on a child run" is the one genuinely new coordination
    pattern in the engine — named here, detailed in `flow-run-scope.md`. *Rejected:* inlining the
    child graph into the parent run (loses independent versioning + the clean parent/child wall).
12. **`patch_run` validates against the run's *pinned* node schema, and the editor renders that
    pinned schema.** A live run pinned to an old `config_version` (Decision 1) accepts a
    config-only patch **only** against the old schema, and the canvas, when patching a live run,
    renders the **pinned** schema — never the latest descriptor. *Rejected:* validating the patch
    against the current descriptor (the run would reject fields the form just offered).
13. **Teardown is guarded and ordered.** `flows.delete`/`flows.enable{enabled:false}`:
    **disarm sources first**, then **cancel in-flight runs** (or refuse when active runs exist,
    caller's choice), then drop the cron registration + its deterministic firing ids — idempotent.
    *Rejected:* removing the `flow` record out from under an armed socket / live run / pending
    firing.
14. **`switch` is edge-gating, not a new wire `Outcome`.** A `switch` settles `Ok` (a pass-through
    envelope) like any node; the executor then reads its `config.rules` (each rule carries `to:
    [node_ids]` — the rule→port→wire mapping made explicit, since this DAG's edges are node-id
    `needs` with no port label), evaluates them against the routed value, **releases only the matched
    dependents**, and marks each unmatched immediate dependent's exclusive subtree `skipped` (a
    "gated skip"). A stateful node that **suppresses** a firing (RBE `filter`, a buffering `batch`, a
    `unique` duplicate) uses the same seam: it settles `Skipped` and its subtree is gated. This
    resolves data-nodes Open Q1 **without** a new frontier `Outcome` variant on the wire (the gate is
    computed at release time from config + the settled value) and without changing the edge model.
    *Rejected:* a port-labelled edge model (a bigger change than this pack owns) and a null/skip
    sentinel payload (a dependent can't tell "gated" from a legitimately-null value). Branches are
    expected disjoint (a Node-RED switch fans to distinct wires); a dependent shared with a live
    branch is left to fire on its other path. **The deferred port-labelled edge model this names is now
    owned by [`flow-input-ports-scope.md`](flow-input-ports-scope.md)** — the Node-RED multi-input slice
    (an edge targets a named input port; each port declares an `all` join vs `any` funnel policy).
15. **`split`/`join` are array-carry, not per-message fan-out.** Node-RED fans a `split` into N
    independent messages; our one-shot-run model (Decision 9 — no parked runs, no per-event fan-out
    storm) resolves data-nodes Open Q2 to **array-carry**: `split` emits the sequence as **one**
    settle whose `payload` is the array plus a top-level `parts` descriptor
    (`{id, count, kind, keys?}`), and `join` reads the carried `parts` to recombine (rebuilding an
    object from `keys`, or returning the array). The `parts` field carries forward down the wire like
    `topic` (D4), so `split → map → join` round-trips **without** a new frontier behaviour —
    `split`/`join` collapse to pure array transforms (exactly the collapse the pack predicted).
    Per-element work between them is the array-native `map`/`sort`/`aggregate`, **not** a scalar node
    per element. `parts` is an additive envelope field (the versioned sequence contract, Risk 2),
    designed once and shared by `split`/`join`/`batch`. *Rejected:* per-message fan-out (N downstream
    runs — the fan-out storm Decision 9 exists to avoid).
16. **`delay` parks on the resume seam, never an in-memory sleep.** A `delay` records its release
    instant (`delay` mode) or last-release (`rate` mode) in the bounded-accumulator record and, while
    the timer has not elapsed, returns a **park**: the executor resets the node to `Enqueued` and
    marks the run `suspended` (the same suspend/resume seam the `subflow` park rides, Decision 11). A
    `flows.resume` with an advanced clock re-drives the node, which now releases. This survives a
    restart (the release instant is durable, not a live task) — the honest realisation of a timer
    under "frontier runs to completion" (Decision 9 forbids a long-lived parked async task). v1
    resume is operator/reactor-driven (like the `subflow` inline-drive); a timer-reactor that
    auto-resumes an elapsed park is a follow-up. *Rejected:* a `tokio::sleep` inside the run task (in
    memory, lost on restart, and a parked task fighting the one-shot model).

## How it fits the core (overview — detail in the sub-docs)

- **Tenancy / isolation:** every record is `flow:{ws}:{id}` / `flow_run:{ws}:{run}` /
  `flow_step_output:{ws}:{run}:{node}` / `flow_node_state:{ws}:{flow}:{node}` /
  `flow_input:{ws}:{flow}:{node}` (retained inject values, Decision 9) in the workspace
  namespace; the run + step jobs, their outbox effects, and their series are ws-scoped. A flow physically
  cannot read another workspace's nodes. **Isolation tests are mandatory in every sub-doc that
  adds a verb.**
- **Capabilities:** the `flows.*` family, one cap per verb. **Composition, never widening** (the
  `query.run` precedent): running a flow needs `mcp:flows.run:call` **and** every tool a node
  calls passes its own gate under `caller ∩ grant`. Detail + deny matrix → `flow-run-scope.md`
  and `extension-nodes-scope.md`.
- **One datastore / state vs motion:** SurrealDB holds the graph + run-store + last-value;
  Zenoh/series carry live ticks; must-deliver sinks go through the **outbox**. No second store.
- **Stateless extensions:** a flow instance and its nodes hold **no durable state** — graph in
  the store, run state in the job, live values on the series. Hot-reloading a node-providing
  extension is safe; long-lived sources keep their socket in the (supervised, native) extension.
- **Undo:** graph edits journal through the store `write_tx` seam → **undo for free**, grouped
  per edit so add-node (record + edges) reverses atomically (`scope/undo/`).
- **Observability / debugging:** a keystone with fan-out, debounce, nested subflows, and
  placement needs a "why didn't this fire / why was this node denied / why did resume drift"
  story. The run-store **is** that record — `flow_run`/`flow_step_output` carry per-node
  outcome + the deny/drift reason — surfaced by `flows.runs.get`/`flows.watch`, and emitted into
  the cross-cutting `observability/` traces + `audit/` deny-ledger (the host dispatch chokepoint,
  README §6.5). Each sub-doc names the fields it records; debug entries follow `scope/debugging/`.
- **SDK/WIT:** the `[[node]]` block is the **only** manifest addition (additive, forever-ish —
  flag the §11.2 gate); **no new WIT world** (node execution reuses the frozen `tool.call` /
  `host.call-tool`). Detail → `node-descriptor-scope.md`.

## Testing posture (global)

Every sub-doc that adds surface must, per `scope/testing/testing-scope.md`, prove itself against
the **real** store (`mem://`) / bus / jobs / outbox / gateway — no mocks; seed real `flow`
records; the only permitted fake is a true external (the MQTT broker) behind one extension trait.
The four mandatory categories apply across the feature and are claimed per-doc:

- **Capability-deny** — `flow-run` (no-widening) + `extension-nodes` (install-grant narrowing).
- **Workspace-isolation** — every doc adding a record/verb.
- **Offline/sync** — `flow-run` (resume idempotent across a disconnect).
- **Hot-reload** — `extension-nodes` (swap a node-providing extension mid-flow).

## Risks & hard problems (global)

Structural questions are settled in **Decisions (v1)**. The genuine build-time risks (carried in
the relevant sub-docs) are: high-frequency **fan-out** (one job per source event) → the
event-trigger debounce posture (`flow-run-scope.md`); **config-schema evolution** across node
versions → `config_version` + version-pinned runs (`node-descriptor-scope.md`); the
**everything-is-a-node altitude** guardrail (actions yes, entities no — review-time); and the
**draft-vs-pinned editor UX** so `ResumePointDrift` never surprises (`flows-canvas-scope.md`).

## Rejected alternative — `open-rmf/crossflow` as the runtime

Considered and rejected as the engine: it holds workflow state in an in-process **Bevy ECS**
world (breaks stateless-extensions rule 4), runs a second game-loop runtime (breaks
symmetric-one-binary rule 1, and is heavy on a Raspberry Pi appliance), and its Zenoh plane
bypasses our capability/workspace wall (rules 5/6). We borrow its *idea* — a JSON node-graph +
a diagram editor — and run it on our durable, gated plane. The reusable pieces (its graph-JSON
shape, its TypeScript diagram editor) are referenced where relevant, not adopted wholesale.

## Related

- The sibling scopes (see `README.md`): `node-descriptor` · `extension-nodes` · `flow-run` ·
  `triggers-lifecycle` · `dashboard-binding` · `flows-canvas`.
- README §3 (the rules), §6.5/§6.8 (host dispatch + write_tx seam), §6.10 (jobs), §6.13 (the
  three gates / gateway SSE), §13 (manifest is the contract).
- `scope/rules/rule-chains-scope.md`, `scope/jobs/jobs-scope.md`,
  `scope/extensions/{extensions,host-callback,reference-extensions,ui-federation}-scope.md`,
  `scope/frontend/{rules-workbench-scope.md,dashboard/widget-builder-scope.md}`,
  `scope/node-roles/node-roles-scope.md`, `scope/reminders/`, `scope/undo/`,
  `scope/inbox-outbox/outbox-scope.md`.
- `vision/0003-iot-dashboard.md` (the product these flows light up).
</content>
