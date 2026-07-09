# Flows — scope index

The visual **node-graph flow engine** (node-red over the shipped plane). This is a **keystone
feature**, so it is scoped as several focused docs rather than one monolith — one per concern,
each a self-contained ask. Read top-to-bottom; `flows-scope.md` is the spine and owns the
canonical **Decisions (v1)** that every other doc references by number.

> The thesis (and the line every sub-doc holds): **flows are not a new engine.** A flow is the
> generalisation of the shipped `chains` rule-DAG — a typed `Node` model + a backend node
> **registry** + a React Flow editor, run on `lb-jobs` (durable suspend/resume), with state in
> SurrealDB and motion on Zenoh. No second datastore, no second runtime (CLAUDE rules 1–4).

## The docs

| Doc | The ask |
|---|---|
| [flows-scope.md](flows-scope.md) | **Spine.** The node model, the versioning principle, "generalises `chains`", the canonical **Decisions (v1)**, the rejected `crossflow` runtime, and the global testing/risk posture. Start here. |
| [node-descriptor-scope.md](node-descriptor-scope.md) | **The keystone contract.** The `[[node]]` manifest block, the five node kinds, ports, the inline **JSON-Schema** config, the merged `flows.nodes` registry, and the built-in descriptors. The data-driven editor and extension nodes both key off this. |
| [data-nodes-scope.md](data-nodes-scope.md) | **The data & JSON node pack — 20 new built-ins.** `change`/`select`/`merge`/`map`/`flatten`/`sort`/`range`/`aggregate`/`template` + `csv`/`xml`/`yaml`/`base64` (Tier A, stateless) · `filter`(RBE)/`unique`/`batch` (Tier B, durable state) · `switch`/`split`/`join`/`delay` (Tier C, engine-extending). Descriptors in the frozen node-descriptor shape — Node-RED's function/sequence/parse nodes over the shipped plane. Consumes the descriptor contract; no new mechanism. |
| [flip-flop-node-scope.md](flip-flop-node-scope.md) | **A self-driving boolean oscillator — one new built-in.** `flipflop`: **no input, one output**; emits `true, false, true, …` on a user-set `period_ms`. A *stateful trigger*, not a data-pack node — the durable cron clock says *when*, the Decision-5 `flow_node_state` record says *which side*. Reuses `react_cron`'s durable cursor (interval instead of 5-field cron); no new table, no new cap. The smallest "make something happen on a clock" source. |
| [extension-nodes-scope.md](extension-nodes-scope.md) | **Extensions add backend nodes — WASM *and* native.** The three interaction shapes (transform / sink / source), execution through the gated `caller ∩ install-grant` callback, source **arm/disarm** + the `ingest.write`→series bridge, and the worked `mqtt/extension.toml`. |
| [flow-run-scope.md](flow-run-scope.md) | **Durable execution.** The run as an `lb-jobs` `flow-run` job, the frontier driver + CAS step-claim ported from `chains`, the run-store records, suspend/resume, **version-pinning**, `flows.patch_run`, `ResumePointDrift`, failure policy, and the high-frequency fan-out posture. |
| [triggers-lifecycle-scope.md](triggers-lifecycle-scope.md) | **What starts a flow, and where.** The trigger kinds (`manual\|cron\|event\|inject\|boot`), enable/disable, `start_on_boot` via a `react_to_flows` reconciler, and `placement` across node roles. |
| [dashboard-binding-scope.md](dashboard-binding-scope.md) | **Dashboard ↔ flow, both ways.** `flows.inject` driving an input node from a control; a widget reading a node's output over its series subject — on the shipped dashboard write/watch paths. The "really nice UX" goal. |
| [flow-message-envelope-scope.md](flow-message-envelope-scope.md) | **Node-RED-style messages.** A `{payload, topic}` envelope on every wire, **auto-wire on connect** (drag a wire and data flows — no hand-typed binding), metadata carry-through, the binding grammar widened to field paths (`${steps.x.payload}`), and the `counter` throughput trap removed. A **breaking** data-model change (no back-compat; in dev). Prereq for the binding-UX doc. |
| [flow-dashboard-binding-ux-scope.md](flow-dashboard-binding-ux-scope.md) | **The binding made authorable.** A flow-aware **source picker** (pick flow → node → port/slot), switch/slider wired for you, **structured JSON in *and* out**, port-aware `flows.inject`, and controls that reflect a node's real current value. Builds on `dashboard-binding-scope.md` (the shipped mechanism) — adds the UX + read-back, no new transport. |
| [flows-canvas-scope.md](flows-canvas-scope.md) | **The editor.** The React Flow canvas, the palette from `flows.nodes`, schema-driven config forms, the draft/version + executed-node-lock UX, import/export, and undo. |
| [flow-runtime-control-scope.md](flow-runtime-control-scope.md) | **Observable + interruptible runtime.** Decouples the driver from the request (a run is a background job), makes `cancel`/`suspend` bite **mid-run**, streams per-node settles over a Zenoh subject + a gateway **SSE** `flows.watch` (the "fire on the eventbus if anyone's listening" feed), and adds **per-node config CRUD** (`flows.node.get`/`update`) so a node tweak isn't a whole-`Flow` post. |
| [chains-retirement-scope.md](chains-retirement-scope.md) | **Retire `chains` — flows are the one DAG engine.** Executes Decision 6 to its end: **delete** the `chains.*` verbs, the host `chains` module, the `lb_rules::workflow` model, the gateway routes, and the React chain canvas (flows are a proven superset). No alias — a clean pre-1.0 cut. `rule-chains-scope.md` retires to lineage. |
| [flow-context-scope.md](flow-context-scope.md) | **Node-RED context — node / flow / global state.** One `flow_context` table (three id shapes: `node:{flow}:{node}:{key}` / `flow:{flow}:{key}` / `global:{key}`), rhai `context`/`flow`/`global` handles via a `ContextSeam` (the shipped `ai`/`inbox` handle pattern) with atomic `incr`, `${context.<scope>.<key>}` in bindings so `change`/`switch` read it, `flows.context.*` verbs + a canvas context panel, governors + teardown/orphan GC. Generalises the `counter` node's `flow_node_memory`. Flags the `catch`/`status` observability-node pack as the next parity gap (owned by `data-nodes`' defer-list). |
| [flow-plc-reliability-scope.md](flow-plc-reliability-scope.md) | **PLC-grade reliability + the reactive run model.** Fixes the frozen-`gw.now` constant-run-id bug (one finished run looked perpetually re-runnable → store `Invalid revision`/transaction-conflict, flickering controls), hardens the run-store write against concurrent rev RMW (per-key lock + retry, the capped-ring precedent), and wires **Run = deploy** for triggered/source flows (Node-RED reactive posture: arm via `flows.enable`+`start_on_boot`, reconciler-owned, survives restart; Stop disarms) while a manual chain stays one-shot. |
| [flow-ui-polish-scope.md](flow-ui-polish-scope.md) | **Editor UI polish — "less is more".** UI-only: the header consolidated to ≤4 primary controls (morphing Run⇄Stop, one Pause⇄Resume toggle, `⋯` overflow for Enable/Undo/Export/Import/Delete), **one right dock with Config \| Debug tabs** (the two panels stop co-rendering), a designed schema config form with one context-aware primary action, an export/import **dialog** (preview, copy, pretty/compact, selected-nodes scope), and bounded canvas interaction polish. No verb, descriptor, or runtime changes — every feature stays reachable. |
| [flow-input-ports-scope.md](flow-input-ports-scope.md) | **Multi-input, done right — port-labelled edges + a per-input-port join policy.** Executes spine Decision 14's deferred port-labelled edge model: an edge targets a named input port, and each input port declares a `join` policy — **`all`** (the barrier/AND join, default for transforms) or **`any`** (Node-RED's fire-per-message OR funnel, default for sinks). `any` fires once per settled upstream, each firing scoped by a propagated **firing context (`fctx`)** — an additive envelope field that scopes every downstream claim key, `${steps.*}` resolution, per-node job key and outbox dedup key (empty in the all-`all` case, so today's paths are byte-identical; multiplicity survives past the funnel, exactly-once per firing, still one run, no fan-out storm). Adds a `link-out`/`link-in` pair (virtual OR edges) and makes `debug`'s input an `any` funnel. Replaces the envelope scope's ≥2-input lint with a real, authored policy. A **breaking** edge-model change; no new verb/cap/table. |
| [debug-node-scope.md](debug-node-scope.md) | **Node-RED's debug node + sidebar, over the shipped plane.** One new host-resolved built-in `debug` (kind `sink`, one `payload` in, no out — runs under `flows.run`, no new exec cap) that publishes each wire message as **motion** onto a ws-walled `flow_debug:{ws}:{flow}` subject (fire-and-forget, no SurrealDB record — rule 3 made literal), one new live-feed verb `flows.debug.watch` + a gateway SSE route (a near-verbatim copy of `flows.watch`), and a dockable **debug panel** rendering `json`/`text`/`markdown` type-aware (JSON tree, `react-markdown`+`remark-gfm` already deps) with **auto-collapse** for long values. v1 is motion-only (browser tail, no replay); persistence-to-disc is the named follow-up. Ships the `debug` node from the observability pack `data-nodes`/`flow-context` defer-list; `catch`/`status`/`complete`/`link` stay sibling scopes. |

## Build order (suggested)

`node-descriptor` → `flow-run` → `extension-nodes` → `triggers-lifecycle` → `flows-canvas`
→ `dashboard-binding`. The descriptor is the contract everything else consumes; the run engine
is the spine the triggers and editor drive; the canvas and dashboard binding are the surfaces.

**In flight (post-Wave-3 ergonomics):** `flow-message-envelope` (the `{payload, topic}` + auto-wire
model — a breaking engine change, build first) → `flow-dashboard-binding-ux` (the flow-aware picker +
switch/slider/JSON, which consumes the envelope's `payload`/`topic` ports).

**Node pack (`data-nodes`):** consumes the descriptor contract, so it lands *after* `node-descriptor`
+ `flow-message-envelope` and builds in its own risk tiers — **Tier A** (stateless transforms/parse)
any time, **Tier B** (durable-state nodes) once the accumulator record is settled, **Tier C**
(`switch`/`split`/`join`/`delay`) only after the `flow-run` engine seam for gating/sequences is
decided. Not a prerequisite for anything else; purely additive palette content.

**Observability (`debug-node`):** the Node-RED debug node + sidebar — a host-resolved `sink` that
publishes wire messages as **motion** onto a `flow_debug` subject, plus a `flows.debug.watch` SSE verb
+ a canvas debug panel (json/text/markdown, auto-collapse). Builds *after* `flow-runtime-control`
(it copies the `flows.watch` SSE trio verbatim) and `flow-message-envelope` (it reads `payload`); ships
the `debug` node only, with `catch`/`status`/`complete`/`link` left as the recommended next pack.

## Related (platform primitives reused)

- `../rules/rules-engine-scope.md` — the single-rule `rhai` cage a `Rhai`/`Tool` node runs (stays).
- `../rules/rule-chains-scope.md` — **retired to lineage** by `chains-retirement-scope.md`: the
  `rubix-cube` workflow-DAG port that flows generalised and now *replaces* (read as history).
- `../jobs/jobs-scope.md` — the durable resumable run + suspend/resume.
- `../extensions/extensions-scope.md` (`extension.toml`), `host-callback-scope.md`,
  `reference-extensions-scope.md` (the native MQTT bridge), `ui-federation-scope.md`.
- `../frontend/rules-workbench-scope.md` (the React Flow canvas) and
  `../frontend/dashboard/widget-builder-scope.md` (the read/write binding).
- `../node-roles/node-roles-scope.md` (placement), `../reminders/` (the boot reconciler + cron),
  `../undo/` (graph-edit journaling), `../inbox-outbox/outbox-scope.md` (must-deliver sinks).
- `../../vision/0003-iot-dashboard.md` — the product these flows light up.
</content>
