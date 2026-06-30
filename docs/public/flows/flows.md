# Flows — public

The visual **node-graph flow engine** (node-red over the shipped plane). A `flow:{ws}:{id}` typed
node graph authored on a React Flow canvas, run as a durable resumable `lb-jobs` session, with
**extension-contributed backend node types** (`[[node]]` in `extension.toml`, identical for WASM and
native — only the execution transport differs). The headline holds: **flows are not a new engine.** A
flow is the generalisation of the shipped `chains` rule-DAG — a typed `Node` model + a backend node
registry + an editor, run on `lb-jobs`, state in SurrealDB, motion on Zenoh (Decisions 1–13).

## What's shipped (the backend spine — Waves 1–2)

| Slice | What | Tests |
|---|---|---|
| **node-descriptor** | `lb-flows` crate: the `NodeDescriptor` keystone contract, the additive `[[node]]` manifest block (parse + validate), the five built-in descriptors, the merged `flows.nodes` registry (built-ins ∪ installed-ext nodes — a read-time union over `install` records), the JSON-Schema 2020-12 config gate, the typed `Flow` graph model + DAG math (Kahn), the chain binding grammar. `flows.nodes` verb. | lb-flows 26 · ext-loader 16 · host 5 |
| **flow-run** | the durable run engine over `lb-jobs`: `flow_run` coordinator (pins `flow_version`) + one `flow-step` job per node, the `chains` frontier driver ported verbatim, CAS exactly-once (`Enqueued→Running`), `FailurePolicy`, suspend/resume/cancel, `flows.patch_run` (config-only to an unexecuted node, validated against the pinned schema), `ResumePointDrift`, subflow-parks-on-child, the full `flows.*` run MCP surface incl. `flows.runs.list` (reattach), the canonical `coalesce` enum. | host 12 |
| **extension-nodes** | descriptor-aware ext-node dispatch under `caller ∩ install-grant` (the shipped `build_call_context` chokepoint — two-direction deny, no widening); the source shape (host-owned `flow:{ws}:{flow}:{node}` series, arm/disarm); the worked `mqtt` reference manifest. | host 5 |
| **triggers-lifecycle** | the five trigger kinds; `flows.enable`; the two lifecycle passes — `react_to_flows_cron` (durable clock-scan, deterministic firing id, fire-once-then-skip) + `reconcile_flows` (single-owner election, arm/disarm, guarded teardown); `flows.inject` (Decision 9 retain-vs-fire); placement matched as data. | host 8 |

**~30 Rust integration tests + 26 lb-flows unit + 16 ext-loader**, all on real `mem://` store + real
`lb-jobs` + real caps + real outbox + real install records — no mocks. Mandatory categories across the
feature: capability-deny (per verb + the no-widening run gate), workspace-isolation (every record
ws-scoped), offline/sync (resume/cron-replay exactly-once), and the deny matrix.

## The `flows.*` MCP surface (one cap per verb)
- **CRUD:** `flows.save` (DAG + every node config validated; version bumped on edit) · `get` · `list` ·
  `delete` · `nodes` (the merged registry).
- **Run:** `flows.run {id,params}→{run_id}` (returns immediately — the run is the durable job) ·
  `resume` · `suspend` · `cancel` · `patch_run {run_id,node,config}` (config-only, unexecuted).
- **Inspection:** `flows.runs.get` (per-node snapshot + pinned version, the `ResumePointDrift`
  surface) · `flows.runs.list` (reattach).
- **Triggers:** `flows.enable {id,enabled,start_on_boot}` · `flows.inject {id,node,value}`.
- **Reactors (host-internal, mounted by config):** `react_to_flows_cron` · `reconcile_flows`.

Composition, never widening: `flows.run` plus every node-tool's own gate under `caller ∩ grant` — a
node that calls a tool the caller lacks is **denied at that node**, recorded `Err`, the run continues
under `FailurePolicy`.

## Records (SurrealDB, workspace-walled)
`flow:{ws}:{id}` (graph + version + lifecycle) · `flow_run:{ws}:{run_id}` (coordinator + pinned
`flow_version`) · `flow_step_output:{ws}:{run_id}:{node}` (CAS claim + outcome) ·
`flow_node_state:{ws}:{flow}:{node}` (last-value, Decision 5) · `flow_input:{ws}:{flow}:{node}`
(retained inject values, Decision 9). The run-store mirrors the chain run-store (Decision 6: one
engine, `chains.*` the alias).

## What's deferred (with the decision it traces to)
- **Wave 3 surfaces** — the React Flow canvas (`flows-canvas-scope`) + the dashboard↔flow binding
  (`dashboard-binding-scope`, the Cooler-Control round-trip). These are the frontend slices; the full
  `flows.*` / `flows.nodes` contract they consume is shipped.
- **The mqtt native sidecar binary** — the manifest `[[node]]` contract + host arm/disarm + series
  bridge ship now; the OS process is the shipped native-tier pattern generalised (a mechanical
  follow-up). Tier-parity holds (host picks transport from the install, no engine branch).
- **`flows.watch` SSE** — the canvas's preferred live source; the canvas falls back to a bounded
  `flows.runs.get` poll until it lands.
- **Subflow "park"** — realised as an inline child drive (a reactor-driven park is a follow-up).
  Traces to Decision 11.
- **Cross-node owner failover** — a `node-roles` deferral (triggers-lifecycle-scope non-goal). Traces
  to Decision 10.
- **`chains.*` as the alias** — `chains.*` continues to ship on its own engine; the formal alias
  (delegating `chains.*` to the flow engine) lands when callers migrate, per Decision 6 (no breaking
  cut). The two share the identical frontier/CAS/run-store shape today.

## Where to read
- Scope (the ask, Decisions 1–13): `scope/flows/` (`README.md` index + the seven sub-docs).
- Sessions (the working logs): `sessions/flows/` (node-descriptor · flow-run · extension-triggers).
- Spine primitives reused: `lb-rules/workflow/` (DAG math + binding grammar), `lb-jobs`,
  `lb-outbox`, the extension manifest + `build_call_context`, the dashboard write/watch path, undo at
  `write_tx`.
