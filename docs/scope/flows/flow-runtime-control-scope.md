# Flows scope — runtime control (async drive, live watch, node-config CRUD)

Status: scope (the ask). Promotes to `public/flows/` once shipped.

The flow runtime already runs **headless** (the cron scan, the boot/event reconcilers, and a
manual `flows.run` all call the same `coordinator::drive`). But every entry point drives the
whole frontier **to terminal synchronously inside the call** before returning — so a run is
already finished by the time any observer (the canvas, or a bus subscriber) can watch it or
interrupt it. The result, seen end-to-end against the live node: you can **start a flow but not
stop it**, you **never see live per-node values** (only the final snapshot), and a config change
to one node forces a **full-`Flow` re-`save`**. This scope makes the existing runtime
**observable and interruptible while it runs** — the Node-RED posture the engine was built for —
and adds a **per-node config CRUD** pair so a node tweak is not a whole-graph write.

## Goals

- A manual run is a **background job**: `flows.run` returns `{run_id}` immediately; the driver
  loops the frontier as a detached task. (Cron/boot/event firings already have deterministic run
  ids — they stop blocking too.)
  > **Superseded note (see [`flow-plc-reliability-scope.md`](./flow-plc-reliability-scope.md)):**
  > this slice assumed the *manual* run id was unique-enough. It is not — `flows.run` derived it from
  > `gw.now`, which is **frozen at gateway startup**, so every manual run of a flow reused the same id
  > and re-drove the same record (→ store `Invalid revision`/transaction-conflict, flickering
  > controls). The reliability slice mints a ULID per manual run and hardens the run-store write; the
  > "background job returns `{run_id}` immediately" shape here is unchanged and correct.
- **Stop actually stops.** The driver checks the run's durable status between frontier steps and
  halts on `cancelled`/`suspended` — `flows.cancel`/`flows.suspend` become real mid-run controls,
  not post-hoc no-ops. `flows.resume` re-drives from the durable frontier (already shipped).
- **Live values stream.** Each node settle publishes onto a workspace-walled Zenoh subject; a new
  `flows.watch {run_id}` surfaces it over a gateway **SSE** route (snapshot-then-deltas, mirroring
  `agent.watch`/`run_stream`). Any subscriber sees it — the canvas, or a headless listener. If
  nobody is watching, the run still completes (fire-and-forget).
- **Per-node config CRUD on a *saved* flow:** `flows.node.get {id, node}` and
  `flows.node.update {id, node, config}` — read/replace one node's config in place, validated
  against its descriptor schema, bumping `flow.version` like any structural edit. (Distinct from
  `flows.patch_run`, which targets an *unexecuted node of a live run* against the run's pinned
  schema.)

## Non-goals

- **Cross-node owner failover** (re-driving a backgrounded run on another node when the home node
  dies) stays the `node-roles` deferral (Decision 10) — the background task is local to the node
  that started it; on restart, `flows.resume` re-drives from durable state (unchanged).
- **Per-node *step*-level streaming** (token deltas inside a single node's tool call) — a node is
  the unit of motion here; one settle event per node.
- **Renaming/retyping a node** via `flows.node.update` — config-only, exactly like `patch_run`.
  Topology edits stay in `flows.save`.
- Replacing the bounded `runs.get` poll in the canvas is optional follow-through: `flows.watch`
  is additive; the poll remains the fallback for a client that doesn't open the stream.

## Intent / approach

**One change of altitude, not a new engine.** `coordinator::drive` already is the loop; today
`run_flow_to_completion` `await`s it inline. We (1) **spawn** the drive on a task so the verb
returns at `run_id`, (2) add a **cooperative cancel check** in the drive loop (read the run status
before each frontier batch; break on terminal-by-control), and (3) **publish a settle event** from
`execute_node` right after it persists each node's durable outcome — the exact ordering
`publish_run_event` uses (record first, motion as its projection, §3 rule 3). The watch surface is
a near-verbatim copy of the shipped `run_events` trio (`subject` + `publish` + `watch`) and its
gateway SSE route — proven code, re-seamed onto a `flow_run` subject.

`flows.node.get/update` are thin verbs over the existing flow record: read it (workspace-walled),
read/replace one node's `config`, **re-validate that node against its descriptor schema** (the same
validator `flows.save` runs per node), bump `version`, persist. Rejected alternative: making the UI
keep posting the whole `Flow` on every keystroke — it works but is a lost-update hazard (two
editors clobber) and wastes the descriptor-scoped validation the engine already has per node.

Rejected alternative for the run model: **keep synchronous drive + only add SSE**. It streams
values but can't fix Stop — the run is terminal before the first cancel can land. The user's
report ("start but not stop") is precisely the synchronous drive, so the decouple is the fix.

## How it fits the core

- **Tenancy / isolation:** the `flow_run` record, the bus subject (`flow:{ws}:{run}` under
  `ws/{id}/`), and the flow record are all workspace-scoped. A ws-B caller can neither watch nor
  cancel nor node-update a ws-A run/flow (read-first, then the wall). Tested per verb.
- **Capabilities:** `flows.watch`, `flows.node.get`, `flows.node.update` are each their own MCP
  cap (`mcp:flows.watch:call`, `mcp:flows.node.get:call`, `mcp:flows.node.update:call`),
  re-checked at the bridge. Deny → opaque `403`/`Denied`. `flows.run`/`cancel`/`suspend` keep their
  existing caps; the behavior change (async, mid-run) needs no new grant. **Composition unchanged:**
  the backgrounded drive still gates every node-tool under `caller ∩ grant` exactly as today.
- **Placement:** `either` — a run is owned by the node that started it (manual = the gateway's
  node; cron/boot = the reconciler's elected owner). No `if cloud`. The background task is config-
  agnostic.
- **MCP surface** (API shape §6.1):
  - **CRUD:** `flows.node.update` (write one node's config; own cap). `flows.node.get` (read).
    No new delete — a node is removed via `flows.save` topology edit (stated non-goal).
  - **Get / list:** `flows.node.get`; run reads stay `flows.runs.get`/`runs.list` (unchanged).
  - **Live feed (SSE / watch):** `flows.watch {run_id}` — the headline add. Bus-backed motion,
    snapshot-then-deltas, gateway SSE route `GET /flows/runs/{run_id}/stream?token=`. **This is the
    "fire on the eventbus if anyone is listening" surface.** Replaces *needing* to poll; the poll
    stays as a no-stream fallback.
  - **Batch:** N/A as a new verb — *a run is already the job* (§6.1 "a long batch MUST be a job"):
    this scope finally makes `flows.run` behave like one (returns an id, watched via a feed,
    survives restart via resume) instead of a blocking loop.
- **Data (SurrealDB):** no new tables. `flows.cancel`/`suspend` already write run status into the
  run-store; the drive loop now *reads* it. `flows.node.update` rewrites the `flow` record. The
  per-node outcome records (the snapshot source) are unchanged — the settle event is their
  projection.
- **Bus (Zenoh):** new subject class `flow:{ws}:{run}` (relative `flow/{run}` under the ws prefix),
  message class **fire-and-forget** (a dropped settle event is non-fatal; the durable per-node
  record is the truth and a late watcher catches up from the snapshot — identical to run events).
- **Sync / authority:** node-local authority for the live drive; durable state is the resume
  contract. Offline: a node with no watcher still drives + completes headless (the existing
  behavior, now non-blocking). No outbox — settle events are observational motion, not must-deliver
  effects (the node-tool side effects that *are* must-deliver already go through their own outbox).
- **State vs motion:** state = the per-node run records + flow record (SurrealDB); motion = the
  settle stream (Zenoh). The SSE route is a projection of state then a fold of motion — never the
  store of record.

## Example flow

1. Canvas calls `flows.run {id}` → host creates the `flow_run` job, **spawns** the drive, returns
   `{run_id}` in ~1ms.
2. Canvas opens `GET /flows/runs/{run_id}/stream?token=` → receives the snapshot (nodes settled so
   far) then live `node-settled` events as each node finishes.
3. User clicks **Stop** → `flows.cancel {run_id}` writes status `cancelled`. The drive loop reads
   it before the next frontier batch and breaks; the remaining nodes stay un-run (audit kept).
4. The stream emits a terminal `run-finished {status: cancelled}`; the canvas paints the partial
   result and the Stop/Suspend/Resume controls retire.
5. Separately: user edits the `count` node's config in the panel and clicks **Save node** →
   `flows.node.update {id, node:"count-2", config:{…}}` validates against the `count` descriptor,
   bumps `version`, persists — **no whole-`Flow` post**. A bad config → `400` with the validation
   message inline.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`), all on real `mem://` store + real bus +
real jobs/caps — no mocks:

- **Capability deny — per new verb:** `flows.watch`, `flows.node.get`, `flows.node.update` each
  denied without the grant (a `403`/`Denied` before any effect/stream body).
- **Workspace isolation — per new verb:** a ws-B principal cannot watch a ws-A run, cannot
  `node.get`/`node.update` a ws-A flow (read-first wall).
- **Async drive:** `flows.run` returns before the run is terminal (assert a non-terminal snapshot
  is observable), then the run reaches terminal on its own.
- **Mid-run cancel actually stops:** seed a multi-node flow where node B depends on A; cancel after
  A settles; assert B is never claimed/run and status is `cancelled` (regression for "start but not
  stop").
- **Watch snapshot-then-deltas:** a late watcher attaches after node A settles → its snapshot
  contains A, then it receives B's settle as a live delta (mirror the `watch_run` test).
- **`flows.node.update` validates + versions:** a schema-valid config persists and bumps
  `version`; a schema-invalid config rejects `BadInput` and leaves the record unchanged. `node.get`
  round-trips it. Workspace-walled.
- **Resume still exactly-once** after a backgrounded run is interrupted (the CAS claim holds — a
  re-drive of an already-settled node is a no-op).
- **Frontend (Vitest, real spawned gateway):** the canvas shows the Stop button *during* a run
  (non-terminal snapshot drives `runActive`), Stop transitions to `cancelled`, live values render
  from the stream/poll, and **export round-trips `needs`** (the connections-in-export regression).

## Risks & hard problems

- **The settle-event ordering** must be record-then-publish (a watcher must never see a node done
  that the durable record doesn't yet show) — copy `publish_run_event`'s discipline exactly.
- **Cancel granularity:** the check is between frontier batches, so a long *single* node finishes
  before cancel bites. Acceptable for v1 (documented); a within-node abort is a node-tool concern.
- **Background task lifecycle:** a panicking drive task must not poison the node; wrap it so a
  failure marks the run `failed` (durable) and is observable, never a silent hang. On node restart
  an in-flight backgrounded run is resumed via the existing `flows.resume` path (no new failover).
- **Export connections** — diagnosed as a UI-side issue (the backend `flows.get` returns `needs`
  correctly, verified live); pin down whether it's a stale-edges/serialization gap and add the
  round-trip test as the regression. Log under `debugging/flows/` if non-trivial.

## Open questions — RESOLVED (this session)

1. ~~Does `flows.watch` need a `list`-of-watchers or liveliness?~~ **No** — fire-and-forget like
   `agent.watch`; a publish with no subscriber is a no-op and a late watcher catches up from the
   snapshot. Shipped that way.
2. ~~Should `flows.node.update` accept a `with`-binding edit too, or strictly `config`?~~ **Config
   only**, matching `patch_run`. Topology + bindings stay `flows.save` (the version-bumping structural
   write). An absent node is `NotFound` (never a silent create).
3. ~~Primary live source: SSE or poll?~~ **SSE primary, poll fallback.** `useFlowRun` opens
   `openFlowRunStream` first and folds deltas; it falls back to the bounded `flows.runs.get` poll only
   when no gateway stream exists (Tauri/tests). The poll was **not** removed.

## Resolved deviations from the scope (recorded for the reader)

- **NotFound is opaque-`Denied`, not 404.** The scope spoke of `NotFound`, but every flows verb
  collapses a missing flow/node to `Denied` (the MCP existence-hiding discipline — see
  `debugging/host/flows-get-absent-collapse-to-denied.md`). The node verbs follow suit for
  consistency: a ws-B caller, or a missing flow/node, is a `403`. The deny-/isolation-tests assert
  this.
- **Async only for the manual `flows.run` verb.** The cron/boot/inject reactors keep the synchronous
  `flows_run` — they own their cadence and stay deterministic under test. Only the user-facing verb
  backgrounds (the only path that needs Stop/live-values).
- **An async-recursion `Send` snag** surfaced backgrounding the drive — logged + fixed in
  `debugging/flows/async-run-not-send-recursion.md`.

## Related

- README `§6.10` (jobs), `§3` (the non-negotiables — state vs motion, capability-first, the wall).
- Sibling scopes: `flow-run-scope.md` (the run engine + lifecycle this extends),
  `flows-canvas-scope.md` (the canvas client), `triggers-lifecycle-scope.md` (the headless cron/
  reconcile firings that also stop blocking), `node-descriptor-scope.md` (the per-node schema
  `flows.node.update` validates against).
- Pattern reused verbatim: `run_events/{subject,publish,watch}.rs` + `routes/run_stream.rs`
  (agent-run watch) and the generic `routes/stream.rs` / `/bus/{subject}/stream`.
- Promotes to `public/flows/flows.md`.
