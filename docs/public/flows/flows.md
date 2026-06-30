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
- **The mqtt native sidecar binary** — the manifest `[[node]]` contract + host arm/disarm + series
  bridge ship now; the OS process is the shipped native-tier pattern generalised (a mechanical
  follow-up). Tier-parity holds (host picks transport from the install, no engine branch).
- **`flows.watch` SSE** — the canvas's preferred live source; the canvas falls back to a bounded
  `flows.runs.get` poll until it lands.
- **Host-side `flows.save` journaling** — the canvas undo is client-side (a transient edit history)
  until `flows.save` moves onto the store `write_journaled` seam; then undo rides the undo journal
  for free. Traces to the undo scope.
- **Subflow "park"** — realised as an inline child drive (a reactor-driven park is a follow-up).
  Traces to Decision 11.
- **Cross-node owner failover** — a `node-roles` deferral (triggers-lifecycle-scope non-goal). Traces
  to Decision 10.
- **`chains.*` as the alias** — `chains.*` continues to ship on its own engine; the formal alias
  (delegating `chains.*` to the flow engine) lands when callers migrate, per Decision 6 (no breaking
  cut). The two share the identical frontier/CAS/run-store shape today.

## What's shipped (Wave 3 — the editor + the dashboard binding)

The **React Flow canvas** (Slice E, `flows-canvas-scope`) + the **dashboard↔flow binding** (Slice F,
`dashboard-binding-scope`) — pure clients of the shipped `flows.*` / `flows.nodes` gateway verbs
(**no new host work, no new caps, no new tables**).

- **Gateway routes** (`role/gateway/src/routes/flows.rs`, mirrors `chains.rs`): one route per
  `flows.*` verb, each re-checking `mcp:flows.<verb>:call` server-side via `lb_host::call_tool` (ws +
  principal from the token). An invalid DAG / schema-invalid node config → `400` inline (the canvas
  edge error). Dev `member_caps` gained the `mcp:flows.*` set (member-level).
- **UI client** (`ui/src/lib/flows/`): `flows.types.ts` + `flows.api.ts` (one export per verb, 1:1)
  + the `flows_*` http command mapping.
- **Canvas** (`ui/src/features/flows/`, one component/hook per file): the typed-node DAG
  (`flowGraph.ts`); the **schema-driven `SchemaForm`** (JSON-Schema 2020-12 → shadcn primitives,
  `ajv` validation — **no per-node hand-coded form**); the palette from `flows.nodes` (built-ins +
  ext `[[node]]`, grouped by category); run/suspend/resume/cancel + a bounded `runs.get` poll that
  colours nodes live; the **executed-node-lock + v-pinned banner + `flows.patch_run`** (config-only,
  unexecuted nodes — Decision 1/12); import/export; undo (restores a node + edges atomically).
- **Dashboard binding** (Slice F): `flows.inject` is one more granted action tool a control calls
  through the shipped `WidgetBridge` (`/flows/{id}/inject`); a flow-node series
  (`flow:{ws}:{flow}:{node}`) is one more `series.watch`/`series.read` source — **no new dashboard
  mechanism, no new read verb**. The Cooler-Control round-trip (slider → retained input →
  event-triggered one-shot run → output series → chart) closes on existing paths, re-checked per
  call, workspace-walled.

**Tests (real, no mocks):** `SchemaForm` 8 (ajv) · `FlowsCanvas.gateway` 13 (palette/save/dag-deny/
schema-deny/run-colours/import-export/undo/ws-isolation/cap-deny/patch_run) ·
`FlowDashboardBinding.gateway` 5 (inject-retain/run-reads-it/viewer-deny/ws-isolation/series
read-out) · Rust `flows_routes_test` 7. `cargo build/test/fmt --workspace` green; `pnpm test` 176;
`pnpm lint` 0 errors.

## Where to read
- Scope (the ask, Decisions 1–13): `scope/flows/` (`README.md` index + the seven sub-docs).
- Sessions (the working logs): `sessions/flows/` (node-descriptor · flow-run · extension-triggers ·
  flows-canvas · dashboard-binding).
- Spine primitives reused: `lb-rules/workflow/` (DAG math + binding grammar), `lb-jobs`,
  `lb-outbox`, the extension manifest + `build_call_context`, the dashboard write/watch path, undo at
  `write_tx`.
