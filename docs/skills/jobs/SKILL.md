---
name: jobs
description: >-
  Understand and work with the Lazybones durable job queue (`lb-jobs`) — the SurrealDB-native
  resumable-session record that backs agent runs and the coding workflow. Read this when a task
  involves "durable jobs", "resumable sessions", "the job queue", "why is there no jobs.* MCP verb",
  "how a coding/agent job is started, resumed, or observed", "job transcript/cursor/steps", or
  "idempotent resume". IMPORTANT: there is NO `jobs.*` MCP surface — a job is a host-internal
  primitive driven by the `workflow`/agent services (the caps chokepoint) and observed via the run
  SSE stream. This skill explains the record, its lifecycle, and how to drive/observe it.
---

# The durable job queue (`lb-jobs`) — a reference

Jobs are the platform's **durable, resumable work**: a job is a SurrealDB record (no separate
datastore — rule 2), a worker drives it, and a restart resumes it from where it left off. The
decision (README §13, jobs scope) is a **thin SurrealDB-native queue**, NOT an apalis backend — the
atomic-claim primitive is a one-line conditional `UPDATE` and LIVE queries give push pickup, so an
external framework adds more than it saves.

**There is no `jobs.*` MCP verb.** `lb-jobs` exposes **raw store verbs with no authorization**
(`create` / `load` / `append_event` / `complete` / `cancel` / `suspend` / `unsuspend`), exactly like
`lb-inbox` / `lb-assets`. The **caps chokepoint is the host service that owns the job** — the
`workflow` service (coding jobs) and the agent service (agent sessions). You don't call a job; you
call a workflow verb that creates/advances one, and you observe it over the run stream. This keeps
each verb single-responsibility and means an extension can't raw-write the jobs table.

## The record (`job:{id}`)

```jsonc
{
  "status": "queued",       // queued | running | done | failed  (+ suspended)
  "kind": "coding-session", // static set today: "agent-session" | "coding-session"
  "payload": {…},           // opaque JSON — the driver stores its goal + caller here
  "cursor": 3,              // the resume point — the next step index
  "steps": [ /* steps[i] = the durable result of step i */ ],
  "attempts": 1,
  "ws": "acme",             // the hard wall — every job is workspace-scoped
  "ts": 1719800000000       // injected logical clock — no wall-clock (determinism, §3)
}
```

The **transcript + cursor IS the durable session state** (README §6.9): a remote agent/coding session
lives here and drives progress through it. `steps` is **append-addressed** — `steps[i]` is the durable
result of step `i`, so re-running a persisted step is a *lookup, not a re-spend*.

## Lifecycle & the load-bearing property: idempotent resume

`append_event(i, result)` **upserts** `steps[i]`; the cursor only advances past steps that durably
landed. So re-applying a persisted step changes nothing — a session that survives an edge disconnect
resumes **without double-applying**. This is the same idempotency the inbox/outbox/ingest paths rely
on, and it's proven by the agent slice's offline/sync test.

- **External effects do NOT ride the job** — an effect (open a PR, notify) goes through the **outbox**
  (`docs/skills/channels-inbox-outbox/SKILL.md`), a *dedicated* `outbox` table. A job is a *resumable
  session*; an effect is a *fire-once intent* — separate lifecycles, separate tables (decided at S6).

## How a job is driven (the `workflow` service)

Coding jobs are created and advanced through the host `workflow.*` MCP verbs — those are the
capability-gated surface; the job record is the durable state behind them. The end-to-end path is
webhook → triage → **approval** → JOB → outbox → GitHub:

- `workflow.ingest_issue`, `workflow.request_approval`, `workflow.resolve_approval` — the triage/
  approval front half (see the workflow tool + the inbox `Decision`).
- **`start_coding_job` is THE GATE** — it starts the durable job **only when an item's resolution is
  `Approved`** (gated `mcp:workflow.start_job:call`, workspace-first). It `create`s the job with a
  deterministic id (re-resolve/re-scan → ONE job).
- **`react_to_approvals`** — the resolution reactor: a durable scan over `lb_inbox::approved` that
  **auto-starts** the coding job the moment its approval lands, closing the loop with no manual
  start step. Idempotent on the deterministic job id.

Agent sessions (`kind: "agent-session"`) are driven by the agent service the same way — the job holds
the transcript + cursor; the service is the caps chokepoint.

## How to observe a job (the run stream)

A running job's progress is a `RunEvent` projection streamed over SSE, the agent-run analog of the
channel stream (snapshot-then-deltas):

```
GET /runs/{job}/stream?token=<jwt>
```

Auth is `?token=` (browser `EventSource` can't set a header); the workspace comes from the token. Use
this to watch a coding/agent job advance, rather than polling the record.

## What's shipped vs deferred

- **Shipped (S5+):** the durable **resumable-session subset** — `create` / `load` / `append_event` /
  `complete` (+ `cancel`/`suspend`/`unsuspend`), all workspace-namespaced, with **idempotent resume**.
  Two static kinds (`agent-session`, `coding-session`).
- **Deferred (S6+, when multi-worker contention exists):** the atomic-claim primitive
  (`UPDATE … WHERE status='queued' RETURN BEFORE`), `run_at` scheduling, backoff, lease/heartbeat +
  dead-worker reclamation, and cron. Recorded, not built — no contention yet with the single
  hub-hosted session. **Per-workspace capability-gated `kind` dispatch** opens when extensions provide
  job kinds (a registry, S7).

## Gotchas

- **Don't look for a `jobs.*` tool** — drive jobs through `workflow.*` (or the agent service); the raw
  `lb-jobs` verbs are unauthenticated internals and must not be reached directly by an extension.
- **Order/resume on the `cursor` + `steps` index**, not wall-clock; `ts` is an injected logical clock.
- **Effects are not job steps** — they belong in the outbox (must-deliver, dedup on idempotency_key).
- **Workspace wall** — every job carries `ws`; a ws-B worker never claims or sees a ws-A job.
- **Re-running a persisted step is free** — it's a lookup; the cursor guards against re-spend.

## Related

- Scope: `docs/scope/jobs/jobs-scope.md` (the forever decision + the S5 record).
- The must-deliver sibling (effects, NOT jobs): `docs/scope/inbox-outbox/outbox-scope.md`,
  `docs/skills/channels-inbox-outbox/SKILL.md`.
- Rules chained into a DAG run on jobs: `docs/scope/rules/rule-chains-scope.md`,
  `docs/skills/rules/SKILL.md`.
- Agent/coding-workflow sessions that own their state in the job record:
  `docs/scope/agent/agent-scope.md`, `docs/scope/coding-workflow/coding-workflow-scope.md`.
- README §6.9 (jobs), §6.10 (outbox), §13 (the queue decision). Source: `rust/crates/jobs/`; the
  driver: `rust/crates/host/src/workflow/`; the run stream: `rust/role/gateway/src/server.rs`
  (`/runs/{job}/stream`).
