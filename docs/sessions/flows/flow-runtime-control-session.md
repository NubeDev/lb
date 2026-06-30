# Flows — runtime control (async drive, live watch, node-config CRUD)

- Area: flows
- Status: shipped (green)
- Scope: [`scope/flows/flow-runtime-control-scope.md`](../../scope/flows/flow-runtime-control-scope.md).
- Spine: [`scope/flows/flows-scope.md`](../../scope/flows/flows-scope.md); extends
  [`flow-run-scope.md`](../../scope/flows/flow-run-scope.md) (the run engine) +
  [`flows-canvas-scope.md`](../../scope/flows/flows-canvas-scope.md) (the canvas client).
- Debug: [`debugging/flows/async-run-not-send-recursion.md`](../../debugging/flows/async-run-not-send-recursion.md).

## The ask (from the user, against the live node)

Four reports on the running flows canvas:
1. **"I can start but not stop."** The Stop button never appeared / did nothing.
2. **"Can't see any live values."** Nodes only ever showed their final value, never progression.
3. **"Export — I can't see the node connections."**
4. **"Add an API for updating/getting a node's settings — saving posts the whole flow."**

Per `HOW-TO-CODE`, the session **reproduced everything end-to-end first**, then scoped, then built.

## Root cause (reproduced end-to-end on :8080)

The flow runtime already runs **headless** (the cron scan, the boot/event reconcilers, and a manual
`flows.run` all call the same `coordinator::drive` — Node-RED posture). But every entry point drove
the whole frontier **to terminal synchronously inside the call**:

```
POST /flows/{id}/run  → blocks ~35ms, returns AFTER status:"success"
immediate runs.get    → already terminal, all nodes done
cancel after the fact → {"ok":true} but a NO-OP (run already finished)
```

So a run was over before any observer (browser *or* bus subscriber) could watch it progress or
interrupt it. That single fact explains reports **1 and 2**: `runActive` (which gates Stop/Suspend/
Resume) was never true, and there was only ever one terminal snapshot to paint. Report **3** was a
red herring on the backend — `flows.get` returns `needs` correctly (verified live + unit-proven);
report **4** was a real gap (`flows.patch_run` only targets a *live run's* unexecuted node, not the
saved flow).

The user's instinct — *"don't we have a flow runtime already? it should run headless and fire on the
eventbus if anyone's there"* — reframed the fix: **the runtime already runs headless; make it
observable and interruptible while it runs.**

## What shipped

### Host `flows` service (`crates/host/src/flows/`)
- **`watch.rs`** (new) — the live settle surface, a near-verbatim copy of the shipped
  `run_events/{subject,publish,watch}` trio re-seamed onto a `flow:{ws}:{run}` Zenoh subject:
  `flow_run_subject`, `publish_flow_event` (fire-and-forget), `node_settled_event` /
  `run_finished_event`, and `watch_flow_run` (gated `mcp:flows.watch:call`, subscribe-then-snapshot
  so a late watcher gets the catch-up then live deltas). `FlowWatch { snapshot, stream }`.
- **`node_config.rs`** (new) — `flows_node_get` / `flows_node_update`: read/replace ONE node's config
  on the saved flow, validated against the node's descriptor schema (same validator as `flows.save`),
  bumping `flow.version` (Decision 1). Config-only — topology stays `flows.save`.
- **`run.rs`** — `flows_run_async`: the manual run is now a **background job**. It seeds the run-store
  synchronously (so an immediate `runs.get`/`watch`/`cancel` finds it), then `tokio::spawn`s the drive
  via the named `drive_run_task` and returns `run_id` at once. The cron/boot/inject reactors keep the
  synchronous `flows_run` (they own their own loop cadence — and tests of them stay deterministic).
- **`coordinator.rs`** — `drive` now (a) **checks the durable run status between frontier batches** and
  halts on `cancelled`/`suspended` (Stop actually stops), and (b) publishes a terminal `run-finished`
  settle event on every terminal exit. `execute_node` publishes a `node-settled` event right AFTER it
  persists each node's outcome (record-then-publish — the watcher never leads the record).
- **`mod.rs`** — dispatches `flows.node.get`/`flows.node.update`; `flows.run` uses the async path;
  `call_flows_tool_boxed` (a concrete `Pin<Box<dyn Future + Send>>`) cuts the async-recursion cycle so
  the background drive is `Send` (see debug entry).
- **`lib.rs`** — exports `watch_flow_run` / `FlowWatch` / `FlowsError` + a `flow_engine` test seam
  (`start`/`drive`/`set_run_status`) for the deterministic mid-run-cancel test.

### Gateway (`role/gateway/src/routes/flows.rs` + `server.rs`)
- `GET /flows/runs/{run_id}/stream?token=` — the **live SSE feed** (mirrors `run_stream`): one
  `snapshot` frame then `flow` deltas (`node-settled` / `run-finished`). `mcp:flows.watch:call` gated
  inside `watch_flow_run` (403 before any body); the bus subject is workspace-walled.
- `GET|POST /flows/node/{id}/{node}` — per-node config read/replace (`flows.node.get`/`update`).
- `credentials.rs` — the dev session now carries `mcp:flows.watch:call` + `mcp:flows.node.get:call`
  + `mcp:flows.node.update:call`.

### UI (`ui/src/features/flows/`, `ui/src/lib/flows/`)
- **`flow.stream.ts`** (new) — `openFlowRunStream`: `EventSource` for the run, `snapshot` then `flow`
  events. Returns `null` with no gateway (Tauri/tests) so the poll fallback kicks in.
- **`useFlowRun.ts`** — **SSE-first, poll-fallback**: opens the stream, seeds from the `snapshot`,
  folds each `node-settled`/`run-finished` delta. A run is now observably non-terminal while it runs,
  so the Stop button appears and values animate. `cancelled` added to the terminal set.
- **`flows.api.ts`** / **`http.ts`** — `getFlowNode` / `updateFlowNode` (`flows_node_get`/`_update`).
- **`NodeConfigPanel.tsx`** + **`FlowCanvas.tsx`** — a **"Save node"** button calls `updateFlowNode`
  (just that node), alongside "Save flow" (the whole-graph save). The export now also emits a derived
  human-readable `edges: [{from,to}]` so connections are visible at a glance (informational; import
  re-derives from `needs`).

## Tests (all green — real `mem://` store + real bus/jobs/caps, no mocks)

### Backend — `crates/host/tests/flows_runtime_control_test.rs` (9 new)
```
test node_update_validates_and_bumps_version_then_node_get_round_trips ... ok
test node_update_rejects_a_schema_invalid_config_unchanged ... ok
test node_verbs_deny_without_their_caps ... ok
test node_verbs_are_workspace_walled ... ok
test watch_denies_without_the_cap_and_across_workspaces ... ok
test watch_delivers_snapshot_then_a_live_settle_delta ... ok
test run_is_a_background_job_returns_before_terminal_then_settles ... ok
test cancel_before_run_stops_the_drive_leaving_downstream_unrun ... ok
test cancel_status_written_before_drive_is_honored_deterministically ... ok
test result: ok. 9 passed; 0 failed
```
Plus the existing flows suites stay green (40 total) — `flows_run_test`'s `run_flow` helper now awaits
terminal (the run is a background job; the test observes it as one).

### Frontend — real spawned gateway + a unit test
```
FlowsRuntimeControl.gateway.test.ts ... 6 passed   (node.update round-trip + version, schema reject,
                                                    deny per verb, ws-isolation, async run settles,
                                                    export `needs` round-trip)
flowGraph.test.ts                   ... 4 passed   (export round-trip preserves every `needs`)
FlowsCanvas.gateway.test.ts         ... 13 passed  (no regression)
```

### Live smoke (the headline proof)
Opening the SSE stream **mid-run** caught the run non-terminal (`a`,`b` done, **`c` running**, status
`pending`), then a live `node-settled` delta for `c`, then `run-finished{success}` — exactly the
"fire on the eventbus if anyone's listening" the user described.

## Decisions made this session

- **Async only for the MANUAL verb.** The cron/boot/inject reactors keep the synchronous `flows_run`
  — they own their cadence, and it keeps those tests deterministic. Only the user-facing `flows.run`
  backgrounds, which is the only path that needs Stop/live-values.
- **Cancel granularity = between frontier batches.** A long *single* node finishes before cancel
  bites (documented). A within-node abort is a node-tool concern, deferred.
- **NotFound is opaque-Denied** (the established flows convention), so a missing flow/node on the node
  verbs is a 403, not a 404 — consistent with every other flows verb (the scope's NotFound note was
  corrected to match).
- **`flows.watch` is SSE-only**, like `agent.watch`: no JSON dispatch arm; the gateway route calls
  `watch_flow_run` directly. The poll stays as the no-stream fallback (not removed).

## Follow-ups (unchanged deferrals)

- Cross-node owner failover for a backgrounded run (re-drive on another node when the home node dies)
  stays the `node-roles` deferral; on restart `flows.resume` re-drives from durable state.
- Per-node *step*-level token streaming (deltas inside one node's tool call) — a node is the unit of
  motion here.
