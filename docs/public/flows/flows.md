# Flows — public

The visual **node-graph flow engine** (node-red over the shipped plane). A `flow:{ws}:{id}` typed
node graph authored on a React Flow canvas, run as a durable resumable `lb-jobs` session, with
**extension-contributed backend node types** (`[[node]]` in `extension.toml`, identical for WASM and
native — only the execution transport differs). The headline holds: **flows are not a new engine.** A
flow is the generalisation of the `rubix-cube` rule-DAG — a typed `Node` model + a backend node
registry + an editor, run on `lb-jobs`, state in SurrealDB, motion on Zenoh (Decisions 1–13).

> **Chains removed — flows is the one DAG engine.** The earlier `chains` rule-DAG (the `chains.*`
> MCP verbs, the host `chains` module, the gateway `/chains` routes, the `lb_rules::workflow` model,
> the React chain canvas, the `mcp:chains.*` grants) has been **deleted outright** — no alias, no
> stub (`flows-scope.md` Decision 6, executed to its clean-cut end state). `flows` is a strict
> superset: same binding grammar, same `manual|cron|event` triggers, same one-job-per-node topology,
> the same frontier driver + CAS run-store (ported from `rubix-cube` via the retired chain engine),
> plus `Subflow`/`Sink`/`Source` nodes, retained inputs, and the live-SSE canvas. To "chain rules
> into a DAG", author a flow of `Rhai`/`Tool` nodes. A client still calling a retired `chains.*` verb
> gets the host's **unknown-verb** refusal, and a `/chains…` HTTP call 404s — asserted by
> `chains_retired_test.rs` + `chains_retired_routes_test.rs` (the guard against a stray re-add). See
> [`chains-retirement-scope`](../../scope/flows/chains-retirement-scope.md) and its session doc.

## What's shipped (the backend spine — Waves 1–2)

| Slice | What | Tests |
|---|---|---|
| **node-descriptor** | `lb-flows` crate: the `NodeDescriptor` keystone contract, the additive `[[node]]` manifest block (parse + validate), the five built-in descriptors, the merged `flows.nodes` registry (built-ins ∪ installed-ext nodes — a read-time union over `install` records), the JSON-Schema 2020-12 config gate, the typed `Flow` graph model + DAG math (Kahn), the rubix-cube binding grammar. `flows.nodes` verb. | lb-flows 26 · ext-loader 16 · host 5 |
| **flow-run** | the durable run engine over `lb-jobs`: `flow_run` coordinator (pins `flow_version`) + one `flow-step` job per node, the rubix-cube frontier driver ported verbatim, CAS exactly-once (`Enqueued→Running`), `FailurePolicy`, suspend/resume/cancel, `flows.patch_run` (config-only to an unexecuted node, validated against the pinned schema), `ResumePointDrift`, subflow-parks-on-child, the full `flows.*` run MCP surface incl. `flows.runs.list` (reattach), the canonical `coalesce` enum. | host 12 |
| **extension-nodes** | descriptor-aware ext-node dispatch under `caller ∩ install-grant` (the shipped `build_call_context` chokepoint — two-direction deny, no widening); the source shape (host-owned `flow:{ws}:{flow}:{node}` series, arm/disarm); the worked `mqtt` reference manifest. | host 5 |
| **triggers-lifecycle** | the five trigger kinds; `flows.enable`; the two lifecycle passes — `react_to_flows_cron` (durable clock-scan, deterministic firing id, fire-once-then-skip) + `reconcile_flows` (single-owner election, arm/disarm, guarded teardown); `flows.inject` (Decision 9 retain-vs-fire); placement matched as data. | host 8 |
| **runtime-control** (Wave 3) | the manual run is now a **background job** (`flows.run` spawns the drive + returns `run_id` at once); `cancel`/`suspend` **bite mid-run** (the driver checks the durable status between frontier batches — Stop actually stops); a **live SSE watch** (`flows.watch` → `GET /flows/runs/{run}/stream`, snapshot-then-`node-settled`/`run-finished` deltas over a workspace-walled `flow:{ws}:{run}` Zenoh subject — "fire on the eventbus if anyone's listening"); **per-node config CRUD** on the saved flow (`flows.node.get`/`flows.node.update`, schema-validated, version-bumped) so a node tweak isn't a whole-`Flow` post. | host 9 |

**~30 Rust integration tests + 26 lb-flows unit + 16 ext-loader**, all on real `mem://` store + real
`lb-jobs` + real caps + real outbox + real install records — no mocks. Mandatory categories across the
feature: capability-deny (per verb + the no-widening run gate), workspace-isolation (every record
ws-scoped), offline/sync (resume/cron-replay exactly-once), and the deny matrix.

## The `flows.*` MCP surface (one cap per verb)
- **CRUD:** `flows.save` (DAG + every node config validated; version bumped on edit) · `get` · `list` ·
  `delete` · `nodes` (the merged registry).
- **Run:** `flows.run {id,params}→{run_id}` (returns immediately — the run is a **background** durable
  job) · `resume` · `suspend` · `cancel` (the last two bite **mid-run**, between frontier batches) ·
  `patch_run {run_id,node,config}` (config-only, unexecuted node of a live run).
- **Node config (on the saved flow):** `flows.node.get {id,node}` · `flows.node.update {id,node,config}`
  (schema-validated against the node's descriptor, bumps the flow version) — a per-node edit without
  re-posting the whole `Flow`. Distinct from `patch_run` (which targets a live run's pinned schema).
- **Inspection / watch:** `flows.runs.get` (per-node snapshot + pinned version, the `ResumePointDrift`
  surface) · `flows.runs.list` (reattach) · **`flows.watch {run_id}`** — the live SSE settle feed
  (snapshot then `node-settled`/`run-finished` deltas; the canvas folds it, falling back to the
  bounded `runs.get` poll when no stream).
- **Triggers:** `flows.enable {id,enabled,start_on_boot}` · `flows.inject {id,node,value,port?}` (the
  optional `port` drives the per-port retained `flow_input`; precedence per-port > node-level > `with`).
- **Reactors (host-internal, mounted by config):** `react_to_flows_cron` · `reconcile_flows`.

Composition, never widening: `flows.run` plus every node-tool's own gate under `caller ∩ grant` — a
node that calls a tool the caller lacks is **denied at that node**, recorded `Err`, the run continues
under `FailurePolicy`.

## Records (SurrealDB, workspace-walled)
`flow:{ws}:{id}` (graph + version + lifecycle) · `flow_run:{ws}:{run_id}` (coordinator + pinned
`flow_version`) · `flow_step_output:{ws}:{run_id}:{node}` (CAS claim + outcome) ·
`flow_node_state:{ws}:{flow}:{node}` (last-value, Decision 5) · `flow_input:{ws}:{flow}:{node}`
(node-level retained inject value, Decision 9) · `flow_input:{ws}:{flow}:{node}:{port}` (per-port
retained value; precedence per-port > node-level > static `with`). The run-store mirrors the chain run-store (Decision 6: one
engine, `chains.*` the alias).

## What's deferred (with the decision it traces to)
- **The mqtt native sidecar binary** — the manifest `[[node]]` contract + host arm/disarm + series
  bridge ship now; the OS process is the shipped native-tier pattern generalised (a mechanical
  follow-up). Tier-parity holds (host picks transport from the install, no engine branch).
- ~~**`flows.watch` SSE**~~ — **shipped** (Wave 3, runtime-control). The canvas's preferred live
  source; it falls back to the bounded `flows.runs.get` poll only when no gateway stream is available
  (Tauri/tests).
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

## What's shipped (PLC reliability — unique run ids + conflict-safe writes)

`flow-plc-reliability-scope` — the run engine is now reliable under concurrency, the property the
"run like a PLC" ask demanded.

- **Every manual run is its own run.** `flows.run` mints a fresh **ULID** when no `run_id` is given,
  so two runs of one flow are two distinct `flow_run` records — never a re-drive of a terminal run.
  A caller-supplied `run_id` is still honored (resume / subflow / idempotent retry).
- **The gateway clock advances.** `Gateway::now` reads live wall time per request (it used to freeze
  at boot, which froze every derived run id); tests still inject a fixed clock.
- **Same-record writes can't corrupt `rev`.** A store-level `lb_store::write_locked` (per-
  `(ws,table,id)` async lock + bounded retry-on-conflict — the `capped_insert` design) backs the
  run-store and `lb-jobs`, so concurrent seeds/drives of one run never surface `Invalid revision` /
  `read or write conflict`. `create_run` seeds create-if-absent (idempotent under a racing `start`).

**Before/after (live, `chain4`):** 8 concurrent `POST /flows/chain4/run` went from *one shared id +
6 conflict errors* to *8 distinct ULID ids, zero errors, each settling `success`*.

**Tests:** `store::write_locked_test` (concurrent same-record writes, coherent rev) ·
`host::flows_plc_reliability_test` (concurrent-same-id-settles-once [the mandatory regression],
unique-id, cap-deny, ws-isolation). Debug history:
`debugging/flows/frozen-gw-now-collides-run-ids.md`,
`debugging/flows/run-store-rev-conflict-under-concurrency.md`.

- **Cron triggers fire headless.** A reactor tick (`spawn_flow_reactors`, wired into node boot) scans
  every `mode:"cron"` **trigger node** of every enabled flow on a live clock and fires each due one from
  its own durable cursor (`flow_trigger_state:{flow}:{node}`) — so a flow armed in the canvas runs on its
  own, survives browser close, and resumes from its per-node cursor on restart (fire-once-then-skip, no
  backfill). Previously the scan existed but **nothing drove it**, and the schedule was a single
  flow-level field (see "N independent triggers" below — that wall is now gone).

## What's shipped (the persistent runtime view — Node-RED / PLC steady state)

`flow-persistent-runtime-scope` — opening a flow now SHOWS whether it's running and each node's live
current value, not a frozen last-run snapshot.

- **`flows.node_state {id}`** (gated, ws-walled; `GET /flows/{id}/node_state`) returns every node's
  **current persistent value** `[{node, value, rev}]` from `flow_node_state` (Decision 5: last-value,
  updated in place each scan) plus the flow's armed fields. This is the steady state — readable any
  time, independent of any single run (state, not motion — rule 3). The value was always written by
  `record_outcome`; it just had no reader.
- **The canvas paints node_state as the base steady-state**, overlaying a live run snapshot only while
  watching a run — so an armed cron flow shows each node's current value (refreshing each firing), and
  a run-in-flight shows live deltas, never a frozen "DONE". An **armed banner** shows "Armed · next
  fire in N · last fired N ago · N runs".
**Live (`chain4`):** `node_state` returned each node's value with node `a` on rev 59; the rev advanced
59→60 on the next cron firing — the persistent value updates in place each scan.

## What's shipped (N independent triggers + per-wire subgraph runs + a real counter)

`flow-multi-trigger-reactive-scope` — a flow is now a Node-RED-style soup of nodes with **any number of
independent triggers**, and firing one trigger flows only down **its own wires**.

- **N triggers per flow, each independent.** A flow may carry any number of `mode:"cron"` (and source)
  triggers — multiple crons, MQTT subs, webhooks — each with its **own** schedule and its **own** durable
  cursor `flow_trigger_state:{ws}:{flow}:{node}`. The reactor scans **trigger nodes**, not flows. The old
  "a flow has one schedule" rejection is gone (only a *malformed* cron is rejected at save). The
  flow-level `flow.cron`/`next_attempt_ts` are retired as the source of truth.
- **A firing runs only the triggered node's reachable subgraph** (`Flow::reachable_from` +
  `indegrees_within`). `create_run` takes `entry: Option<&str>`: `Some` seeds only that subgraph (a join
  waits only on its in-subgraph upstreams), `None` keeps the whole-graph run (manual "run all", resume,
  subflow). The run records `entry_node` (`flows.runs.get` → `entryNode`); cron / inject / boot / `flows.run
  {entry}` all fire from their node.
- **A real `counter` node** — the stateful accumulator (Node-RED / PLC "the rung holds its last result").
  It reads its durable running total and **increments atomically** per firing (delta is set by the
  explicit `mode` — see the envelope slice below; `reset` zeroes it), so the count GOES UP across runs and
  survives a restart. Backed by
  durable **node memory** `flow_node_memory:{ws}:{flow}:{node}` + the new atomic `lb_store::increment`
  (server-side accumulate, per-key serialized — a retry can't double-add). The pure `count` transform is
  unchanged. This is the foundation for future stateful nodes (rate, debounce, moving-average).
- **node_state is per-trigger:** each trigger node's entry carries `{cron, nextAttemptTs, armed}`; the
  flow-level summary is the **soonest** upcoming fire (the armed banner).

Proven: `host::flows_multi_trigger_test` (multi-cron independence, per-trigger subgraph isolation, the
counter going 1→2→3, cap-deny, ws-isolation), `store::increment_test` (64 concurrent firings → unique
totals 1..=64, ws-walled), `lb-flows` model helpers. Debug:
`debugging/flows/flow-level-cron-rejects-multiple-triggers.md`.

*Open (follow-up, not bugs):* UI per-trigger armed chips; per-node enable/disable; orphan-cursor sweep on
trigger removal; a native http-in/webhook source node (its own scope).

## What's shipped (the Node-RED message envelope — `payload`/`topic`, auto-wire on connect)

`flow-message-envelope-scope` — a flow message is now a JSON **envelope**: a primary `payload` slot
(always on a node's output) + an optional `topic` (routing/name) + free metadata (`findings`, …). It
flows down a wire **automatically** on connect, and `topic` carries through a chain — the Node-RED
ergonomics, over our own durable per-node engine (no shared mutable `msg`). **Breaking change** (flows is
in dev): old `${steps.x.output}` bindings and bare node outputs are gone.

- **Auto-wire (D3).** A node with **exactly one** upstream and no `with.payload` receives that upstream's
  **full recorded envelope** as its input message — no binding typed. With an explicit `with`, inputs come
  from the bindings only. A **join** (≥2 upstreams) with no `with.payload` is rejected at save by the
  `UnboundJoin` lint — auto-wire never silently picks one of several upstreams.
- **Carry-forward (D4).** A node's recorded envelope = `{ ...carry, ...emitted }`, where `carry` is the
  incoming inputs minus `payload` (so `topic` propagates) and `emitted` is what the node produced (always
  a fresh `payload`). A node clears a carried field by emitting it as `null`; a join carries nothing.
- **Binding grammar (D5).** `${steps.x}` → the whole envelope; `${steps.x.<dot.path>}` → a field path into
  it (`payload`, `topic`, `findings`, `payload.items.1`, … ; missing → `null`); `${params.y}` unchanged;
  literal otherwise. Whole-reference only, no interpolation. `.output`/`.findings` are no longer special —
  they are ordinary field paths.
- **Per-builtin envelopes (D6).** Every builtin reads `inputs["payload"]` and emits an envelope:
  `trigger` → `{payload: <firing value>, topic: config.topic?}`; `count` → `{payload: <size>}`; `rhai` →
  the script return as `payload`, or if it returns an object with a `payload` key that object **is** the
  envelope (`return msg`), with rules `findings` on the `findings` field; `tool` → the verb result as
  `payload` (args merged with an object payload); `sink` → pass-through `{payload}`, destination =
  `msg.topic ?? config.name`, writing `msg.payload`; `subflow` → the child's folded outputs as `payload`.
- **`counter` mode is explicit (D7) — the implicit-throughput trap removed.** `mode: "tick" | "throughput"`
  (default `tick` = +`step` every firing **regardless of payload**; `throughput` = +the payload's size).
  An auto-wired counter no longer surprise-jumps by input size.
- **Canvas shows `payload` (D10).** `flow_node_state` stores the whole envelope; `flowGraph` maps it to its
  `payload` for the value badge (falling back to the whole envelope when there's no `payload` key).

Proven: `lb-flows::binding` (field-paths, whole-envelope, missing→null), `model` (the join lint),
`builtins` (no stray non-envelope ports); `host::flows_run_test` (auto-wire 3-node chain with no `with`,
the join-lint save rejection, topic carry-forward, `rhai return msg`, counter tick-vs-throughput),
`flows_sink_test` (sink reads `payload`, `msg.topic` routes the write); `ui flowGraph.test` (envelope →
payload badge + fallback). Session: `sessions/flows/flow-message-envelope-session.md`.

*Open (follow-up, not this slice):* `flow-dashboard-binding-ux-scope` — the picker offering
`payload`/`topic` ports and read views defaulting to `payload`.

## What's shipped (the flow⇄dashboard binding UX — pick a node + port; switch / slider / JSON, both ways)

`flow-dashboard-binding-ux-scope` — the bidirectional binding is now **authorable in clicks** and
carries **structured JSON both ways**, on top of the shipped `flows.inject` write + read-out (no new
transport). The backend gained the per-port reach + read-back the UX needs:

- **Port-aware `flows.inject {id, node, value, port?}`.** With `port`, the value lands in the per-port
  record `flow_input:{ws}:{flow}:{node}:{port}`; without it, the node-level `flow_input:{ws}:{flow}:{node}`
  (unchanged). The only verb change — an additive optional arg, the **same** `mcp:flows.inject:call` cap +
  per-call ws + grant recheck (no widening).
- **Binding precedence (run input): per-port retained > node-level retained > static `with` / auto-wire.**
  `resolve_node_bindings` overlays the current retained `flow_input` onto a node's resolved inputs in both
  the auto-wire and explicit-`with` branches: the node-level value sets `payload`, a per-port record sets
  that named port (and wins). A run always reads the CURRENT retained value, so a control's inject takes
  for the next run.
- **`flows.node_state {id}` reads retained inputs back.** Each node entry now also carries `input` (the
  node-level retained `payload`) and `inputs` (the per-port map). One read drives both the canvas and the
  dashboard; a control seeds its current state from its OWN input, not its output. No new verb.

Frontend (see `public/frontend/dashboard.md`): a **Flows source-picker group** (flow → node → port) that
emits the right Action/Source with no tool name shown; switch/slider/**JSON** controls that drive a port
and seed their current value on mount; and a **JSON/object read view**.

Proven: `host::flows_triggers_test` (port upsert, precedence by a real run, object round-trip, node_state
read-back, inject deny node- AND port-keyed, ws-isolation); `ui` unit `flowsPicker.test.ts`; `ui` gateway
`FlowDashboardBinding.gateway.test.ts` (picker offer, slider port-inject + precedence, current-value
mount read, switch boolean, JSON validate/reject, JSON read view advance, ws read-isolation).

## Where to read
- Scope (the ask, Decisions 1–13): `scope/flows/` (`README.md` index + the seven sub-docs).
- Sessions (the working logs): `sessions/flows/` (node-descriptor · flow-run · extension-triggers ·
  flows-canvas · dashboard-binding).
- Spine primitives reused: `lb-rules/workflow/` (DAG math + binding grammar), `lb-jobs`,
  `lb-outbox`, the extension manifest + `build_call_context`, the dashboard write/watch path, undo at
  `write_tx`.
