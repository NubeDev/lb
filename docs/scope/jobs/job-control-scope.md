# Jobs scope — the observe/control surface (list · get · cancel · retry · watch)

Status: scope (the ask). Sub-scope of `jobs-scope.md`. Promotes to `public/jobs/` once shipped.

Jobs are the platform's durable, resumable work (`lb-jobs`), and today they are a **host-internal
primitive with no caller-facing read surface** — you cannot list what is running, inspect a stuck job,
or cancel/retry one from the UI or the `lb` CLI. That was fine while the only jobs were the agent
session and the coding workflow (each observed via its owning service's run stream). But the moment
reminders fire jobs, flows run as jobs, and the resource-verb convention makes `delete`/`import`
enqueue jobs (`core/resource-verbs-scope.md`), a user is enqueuing durable work they **cannot see or
stop**. This scope adds the minimal **observe + control** surface — `job.list|get|cancel|retry|watch`
— as the runnable-trait member of the resource-verb convention, **without** breaking the deliberate
"no raw `jobs.*`" architecture: every verb goes through a **host-owned, capability-checked** chokepoint
that reads/acts on the jobs table on the caller's behalf, never a raw table verb an extension could call.

## Goals

- **See running work:** `job.list` (ws-scoped, filter by `status`/`kind`, keyset-paged) and `job.get`
  (one job: status, kind, cursor, attempts, a bounded transcript tail) — the "what's happening" view the
  operator and the UI need.
- **Stop running work:** `job.cancel` — cooperative cancellation (set intent; the worker observes it at
  the next step boundary), because a SurrealDB-native worker can't be killed mid-step.
- **Recover failed work:** `job.retry` — re-queue a `failed`/dead-lettered job, resuming from its cursor
  (idempotent — a persisted step is a lookup, not a re-spend) or restarting from scratch on request.
- **Watch live:** `job.watch` — the same SSE the agent/flow runs already emit, exposed uniformly so a job
  is observed identically whoever owns it (the runnable-trait `.watch`).
- **Surface dead-letters:** a `job.list {status:"failed", exhausted:true}` filter so retries-exhausted
  jobs are findable, not silently stuck — the operator can see and `retry` them.
- **Honor the architecture:** these are **host verbs over the owning-service chokepoint**, not new raw
  `lb-jobs` verbs. An extension still cannot raw-read or raw-cancel the jobs table.

## Non-goals

- **No raw `jobs.*` table API.** The jobs skill is explicit: `lb-jobs` exposes *unauthorized* store
  verbs (`create`/`load`/`append_event`/`complete`/`cancel`/`suspend`), and the **caps chokepoint is the
  host service that owns the job** (workflow, agent). This scope does **not** expose those raw verbs to
  callers; it adds authorized host verbs that call them internally. An extension raw-writing the jobs
  table stays impossible.
- **No new job kinds or scheduler.** This is a read/control surface over the existing record; enqueuing
  new kinds is each owning feature's job (reminders, flows). The multi-worker claim/lease machinery stays
  deferred where `jobs-scope.md` left it.
- **Not a redesign of resume.** `retry` reuses the existing append-addressed transcript + cursor; it does
  not invent a new resume model. Foreign-loop resume (external-agent) stays #5's concern.
- **No cross-workspace admin console.** `job.list` is ws-scoped like everything else; a fleet-wide job
  view is a separate observability concern.

## Intent / approach

**A thin authorized read/control layer, owner-routed.** The problem the jobs skill guards against is a
caller reaching *into* the jobs table and bypassing the service that owns a job's semantics (a coding
job half-applied, an agent run mid-step). So the observe/control verbs do **not** act on the table
directly — they **dispatch through the owning service**:

- `job.list` / `job.get` are **reads**: they query the ws-scoped jobs table for status/metadata/transcript
  (never mutating), gated by a read cap. Reads can be direct because they change nothing and reveal only
  what the run SSE already reveals — just as a snapshot instead of a stream.
- `job.cancel` / `job.retry` are **controls**: they set an **intent** (`cancel_requested`, or re-queue),
  and the **owning worker** enacts it at its next step boundary — cooperative, idempotent, and safe
  because the worker (not the caller) decides how to unwind. This mirrors `flows.cancel`, which already
  bites mid-run cooperatively (`flow-runtime-control-scope.md`).

**Why not kill the worker?** A SurrealDB-native worker holds no OS handle a caller could signal; it
drives a record. The honest cancel is a flag the worker checks — the same cooperative model
`flows.cancel` ships. A hard kill belongs only to the external-agent subprocess supervisor (#5), which
owns a real PID; a native job worker has none.

**Why a job read surface at all, when the jobs skill says "observe via the owning service's stream"?**
Because the number of owning services is growing (agent, workflow, reminders, flows, batch-deletes), and
making each one re-expose "list my jobs / cancel my job" duplicates the surface N times with N different
verb names — exactly the drift `core/resource-verbs-scope.md` fixes. One `job.*` family, owner-routed
under the hood, gives the uniform grammar while keeping the chokepoint. Rejected: per-owner job verbs
(`workflow.jobs`, `agent.jobs`, `reminder.jobs`, …) — N surfaces, no uniformity, and the palette/CLI
can't render "jobs" as one thing.

**Cancel/retry route to the owner by `kind`.** A job's `kind` (`agent-session`, `coding-session`,
`flow-run`, `reminder-fire`, `external-agent-run`, `batch-delete`, …) identifies its owning service. The
control verb looks up the owner from a **static kind→owner registry** (one file, FILE-LAYOUT) and calls
that owner's cancel/retry hook, which knows how to unwind that kind safely. Unknown kind → refused, not
guessed.

## How it fits the core

- **Tenancy / isolation:** every job carries `ws` (jobs scope, the hard wall). `job.list`/`get`/`cancel`/
  `retry`/`watch` are ws-scoped; a ws-B caller can never see, cancel, or retry a ws-A job. **Mandatory
  isolation test** (mirror the existing `lb-jobs` ws-B-can't-claim-ws-A test, extended to the read/control
  verbs).
- **Capabilities:** four new caps, one per verb (FILE-LAYOUT): read `mcp:job.list:call` /
  `mcp:job.get:call`; control `mcp:job.cancel:call` / `mcp:job.retry:call`; `job.watch` reuses the run
  SSE gate. **Read ≠ control**: a principal can be granted list/get without cancel/retry (an observer
  role). Deny is opaque per verb. **Mandatory deny test** each.
- **Placement:** either — host code over the store; the control verb acts on whatever node runs the
  owning worker (the worker enacts intent locally). No `if cloud`.
- **MCP surface (API shape §6.1):**
  - **Get/list:** `job.get` (by id) + `job.list` (ws-scoped, `{status?, kind?, exhausted?, limit, cursor}`
    → `{items, next_cursor}` — the shipped keyset shape, `page-cursor-scope.md`). The read verbs.
  - **Control (not classic CRUD):** `job.cancel` / `job.retry` — mutate *intent*, not the transcript. No
    `job.create` (jobs are enqueued by their owning feature, never by a raw caller) and no `job.delete`
    (a job is history — it completes/fails/cancels; a retention sweep is a separate batch job, not a
    caller `delete`). Say so explicitly.
  - **Live feed:** `job.watch` — reuses the existing run SSE over the job's motion subject; the snapshot
    (`get`) and the stream (`watch`) are distinct (state vs motion, rule 3).
  - **Batch:** N/A for the control verbs themselves. (A "cancel all failed" is a bounded ws-scoped sweep;
    if it ever fans out large it becomes a job — but v1 has no bulk control caller.)
- **Data (SurrealDB):** no new table — reads/controls act on the existing `job:{id}` record. `cancel` sets
  a `cancel_requested` field (owner-observed); `retry` re-queues by resetting `status→queued` and bumping
  `attempts` under the owner's retry policy. State only; no new persistence.
- **Bus (Zenoh):** `job.watch` reuses the existing run-event subject; no new subjects. `cancel`/`retry`
  are record writes the worker observes on its next scan/claim, not bus commands (must-deliver control is
  a durable field, not a fire-and-forget message — the same reason `flows.cancel` writes state).
- **Sync / authority:** node-local record writes, workspace-authoritative; a cancel set during an outage
  is observed by the worker on recovery (at-least-once, idempotent — a second cancel of an already-cancelled
  job is a no-op).
- **Secrets:** none — a job's `payload` may reference a tool + args but not secret material (secrets are
  mediated by the tool the job calls, §6.7); `job.get` redacts any secret-bearing payload field exactly as
  the owning service's own read would.
- **Stateless:** the kind→owner registry is static config, not per-instance durable state — hot-reload safe.
- **One responsibility per file (FILE-LAYOUT):** `host/src/jobs/list.rs`, `get.rs`, `cancel.rs`,
  `retry.rs`; the kind→owner map in `host/src/jobs/owners.rs`; the SSE reuse in the existing run-stream
  route. No `jobs.rs` grab-bag; no touching the raw `lb-jobs` crate's verbs.
- **SDK/WIT impact:** none on the guest ABI. Extensions gain no new job access; if a future extension job
  kind wants owner-routed cancel/retry, it registers a kind→owner hook (host-side), not a new guest call.

## Example flow

A reminder fires a long MCP-tool job; the operator watches, then cancels a stuck one:

1. A reminder's firing enqueues `job:{id}` kind `reminder-fire`. The operator runs `lb job ls` →
   `job.list` → the paged table shows it `running`, same shape as `lb flow ls`/`lb reminder ls`.
2. `lb job watch job_9a2` → `job.watch` streams its steps over SSE — identical to watching an agent run.
3. The tool hangs. `lb job cancel job_9a2` → `job.cancel` (cap-checked) looks up `kind=reminder-fire` →
   the reminder owner's cancel hook sets `cancel_requested`; the worker observes it at the next step
   boundary and ends the job `cancelled`, no half-applied effect (idempotent).
4. A different job dead-lettered overnight. `lb job ls --status failed --exhausted` → `job.list` surfaces
   it. `lb job retry job_7c1` → `job.retry` re-queues from its cursor; the persisted steps replay as
   lookups (no re-spend), only the failed tail re-runs.
5. Bob (granted `job.list`/`job.get` but not `job.cancel`) sees the jobs but `lb job cancel` denies
   opaquely — the observer role holds.

## Testing plan

Per `scope/testing/testing-scope.md`; real store/bus/gateway, seeded real jobs, no mocks (§0).

- **Capability-deny (mandatory):** each verb denies opaquely without its cap; a list/get-only principal is
  refused `cancel`/`retry` (read ≠ control).
- **Workspace-isolation (mandatory):** ws-B `job.list` never returns ws-A jobs; ws-B `cancel`/`retry`/
  `get`/`watch` on a ws-A job id is refused as if it doesn't exist (opaque). Mirror the `lb-jobs` isolation
  test, extended.
- **Cooperative cancel:** a `job.cancel` sets intent; the worker ends the job at the next step boundary;
  already-applied steps are **not** rolled back (idempotent effects, per jobs scope). Assert no
  double-effect and no mid-step corruption.
- **Idempotent retry:** `job.retry` on a failed job resumes from the cursor — persisted steps are lookups,
  only the failed tail re-runs (no re-spend). A retry of an already-`done` job is a no-op/clean error, not
  a re-run.
- **Owner routing:** each `kind` routes cancel/retry to the right owner hook; an unknown kind is refused,
  not silently table-mutated. Table-driven across the shipped kinds.
- **Envelope conformance:** `job.list` returns `{items, next_cursor}` and `job.get` returns `{item}` — the
  uniform resource-verb envelope (`core/resource-verbs-scope.md`).
- **Read isolation from raw table:** assert the new verbs never expose the raw `lb-jobs` store verbs to a
  caller (a caller cannot `append_event`/`complete` — only observe/cancel/retry through the chokepoint).
- **Watch parity:** `job.watch` streams the same events the owning service's run stream does (assert an
  agent job watched via `job.watch` matches `agent.watch`).
- **CLI integration (real gateway, rule 9):** `lb job ls|show|cancel|retry|watch` map to the verbs; `-o
  json` yields the uniform envelope.

## Risks & hard problems

- **Breaking the chokepoint by accident.** The whole risk is that `job.cancel`/`retry` become a backdoor
  into the jobs table that bypasses the owning service's unwind logic. **Mitigation:** control verbs
  *only* set intent / re-queue and delegate the actual unwind to the owner hook — they never call the raw
  `lb-jobs` mutators to force a state. Reviewed per verb.
- **Cancel latency / non-cooperative steps.** A step that never checks the flag can't be cancelled
  cooperatively. **Mitigation:** document the step-boundary contract; for external-agent jobs the #5
  supervisor's hard kill backs it up (a real PID); for pure-record workers, bound step length so the flag
  is checked promptly.
- **Retry storms / poison jobs.** A job that always fails and is auto-/hand-retried burns work. **Mitigation:**
  `retry` respects the owner's attempt ceiling; a dead-lettered job requires an explicit `job.retry` (no
  auto-loop), and `job.list --exhausted` makes poison jobs visible rather than silently retried.
- **Transcript size in `get`.** A long run's transcript is large. **Mitigation:** `job.get` returns a
  **bounded tail** + counts; the full stream is `job.watch`/the run SSE, not a giant snapshot.
- **Owner registry drift.** A new job kind without a registered owner hook can't be cancelled/retried.
  **Mitigation:** the kind→owner map is one file with a test asserting every shipped kind has an owner;
  an unregistered kind fails loudly at the control verb, not silently.

## Open questions

- **`retry` semantics default:** resume-from-cursor (default) vs restart-from-scratch — one flag
  (`{from:"cursor"|"start"}`), default `cursor`? (Recommend yes, default cursor — the cheap, safe path.)
- **`cancel` of a `suspended` job:** does cancel apply to a suspended (paused) job the same as a running
  one? (Recommend yes — cancel is terminal from any non-final state.)
- **Bulk control:** ship `job.cancel {all, status:"running", kind:"…"}` in v1, or single-id only?
  (Recommend single-id v1; bulk becomes a bounded sweep or a job later — don't build an unbounded loop.)
- **Retention/`delete`:** confirmed out of scope here — but where does the completed-job retention sweep
  live (a scheduled reminder/flow, or a host reactor)? (Flag for a separate retention scope.)
- **Does `job.list` need a `since`/time filter** for the "jobs in the last hour" view, or is
  status+kind+keyset enough for v1? (Recommend status+kind+keyset v1; add `since` when a caller needs it.)

## Related

- `scope/jobs/jobs-scope.md` — the `lb-jobs` record + the "no raw `jobs.*`, owner is the chokepoint"
  decision this scope honors.
- `skills/jobs/SKILL.md` — the reference that documents "there is NO `jobs.*` MCP surface"; **this scope
  updates that skill** on ship (the observe/control surface is now the sanctioned, owner-routed way — the
  skill must say `job.list|get|cancel|retry|watch` exist and route through owners, not that no surface
  exists). Skill maintenance is a ship deliverable (§6 checklist).
- `scope/core/resource-verbs-scope.md` — the runnable-trait (`list|get|cancel(=stop)|retry(=restart)|
  watch`) this implements for the jobs family; the uniform envelope.
- `scope/flows/flow-runtime-control-scope.md` — the cooperative mid-run `cancel` precedent this mirrors.
- `scope/reminders/reminders-scope.md`, `scope/flows/flow-run-scope.md` — the job-enqueuing callers whose
  work this makes observable.
- `scope/external-agent/run-lifecycle-scope.md` — the external-agent run is a job; its subprocess
  supervisor provides the hard-kill backstop this scope's cooperative cancel lacks.
- `scope/datasources/page-cursor-scope.md` — the keyset `{items, next_cursor}` `job.list` reuses.
- `README.md` §6.9 (jobs), §6.1 (API shape), §3 (rules 5/7), §6.13 (gateway SSE).
- `public/jobs/jobs.md` — promotion target on ship.
</content>
