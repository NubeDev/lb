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
| [extension-nodes-scope.md](extension-nodes-scope.md) | **Extensions add backend nodes — WASM *and* native.** The three interaction shapes (transform / sink / source), execution through the gated `caller ∩ install-grant` callback, source **arm/disarm** + the `ingest.write`→series bridge, and the worked `mqtt/extension.toml`. |
| [flow-run-scope.md](flow-run-scope.md) | **Durable execution.** The run as an `lb-jobs` `flow-run` job, the frontier driver + CAS step-claim ported from `chains`, the run-store records, suspend/resume, **version-pinning**, `flows.patch_run`, `ResumePointDrift`, failure policy, and the high-frequency fan-out posture. |
| [triggers-lifecycle-scope.md](triggers-lifecycle-scope.md) | **What starts a flow, and where.** The trigger kinds (`manual\|cron\|event\|inject\|boot`), enable/disable, `start_on_boot` via a `react_to_flows` reconciler, and `placement` across node roles. |
| [dashboard-binding-scope.md](dashboard-binding-scope.md) | **Dashboard ↔ flow, both ways.** `flows.inject` driving an input node from a control; a widget reading a node's output over its series subject — on the shipped dashboard write/watch paths. The "really nice UX" goal. |
| [flow-message-envelope-scope.md](flow-message-envelope-scope.md) | **Node-RED-style messages.** A `{payload, topic}` envelope on every wire, **auto-wire on connect** (drag a wire and data flows — no hand-typed binding), metadata carry-through, the binding grammar widened to field paths (`${steps.x.payload}`), and the `counter` throughput trap removed. A **breaking** data-model change (no back-compat; in dev). Prereq for the binding-UX doc. |
| [flow-dashboard-binding-ux-scope.md](flow-dashboard-binding-ux-scope.md) | **The binding made authorable.** A flow-aware **source picker** (pick flow → node → port/slot), switch/slider wired for you, **structured JSON in *and* out**, port-aware `flows.inject`, and controls that reflect a node's real current value. Builds on `dashboard-binding-scope.md` (the shipped mechanism) — adds the UX + read-back, no new transport. |
| [flows-canvas-scope.md](flows-canvas-scope.md) | **The editor.** The React Flow canvas, the palette from `flows.nodes`, schema-driven config forms, the draft/version + executed-node-lock UX, import/export, and undo. |
| [flow-runtime-control-scope.md](flow-runtime-control-scope.md) | **Observable + interruptible runtime.** Decouples the driver from the request (a run is a background job), makes `cancel`/`suspend` bite **mid-run**, streams per-node settles over a Zenoh subject + a gateway **SSE** `flows.watch` (the "fire on the eventbus if anyone's listening" feed), and adds **per-node config CRUD** (`flows.node.get`/`update`) so a node tweak isn't a whole-`Flow` post. |
| [flow-plc-reliability-scope.md](flow-plc-reliability-scope.md) | **PLC-grade reliability + the reactive run model.** Fixes the frozen-`gw.now` constant-run-id bug (one finished run looked perpetually re-runnable → store `Invalid revision`/transaction-conflict, flickering controls), hardens the run-store write against concurrent rev RMW (per-key lock + retry, the capped-ring precedent), and wires **Run = deploy** for triggered/source flows (Node-RED reactive posture: arm via `flows.enable`+`start_on_boot`, reconciler-owned, survives restart; Stop disarms) while a manual chain stays one-shot. |

## Build order (suggested)

`node-descriptor` → `flow-run` → `extension-nodes` → `triggers-lifecycle` → `flows-canvas`
→ `dashboard-binding`. The descriptor is the contract everything else consumes; the run engine
is the spine the triggers and editor drive; the canvas and dashboard binding are the surfaces.

**In flight (post-Wave-3 ergonomics):** `flow-message-envelope` (the `{payload, topic}` + auto-wire
model — a breaking engine change, build first) → `flow-dashboard-binding-ux` (the flow-aware picker +
switch/slider/JSON, which consumes the envelope's `payload`/`topic` ports).

## Related (platform primitives reused)

- `../rules/rule-chains-scope.md` + `rules-engine-scope.md` — the DAG engine, run-store, and the
  rhai cage this generalises.
- `../jobs/jobs-scope.md` — the durable resumable run + suspend/resume.
- `../extensions/extensions-scope.md` (`extension.toml`), `host-callback-scope.md`,
  `reference-extensions-scope.md` (the native MQTT bridge), `ui-federation-scope.md`.
- `../frontend/rules-workbench-scope.md` (the React Flow canvas) and
  `../frontend/dashboard/widget-builder-scope.md` (the read/write binding).
- `../node-roles/node-roles-scope.md` (placement), `../reminders/` (the boot reconciler + cron),
  `../undo/` (graph-edit journaling), `../inbox-outbox/outbox-scope.md` (must-deliver sinks).
- `../../vision/0003-iot-dashboard.md` — the product these flows light up.
</content>
