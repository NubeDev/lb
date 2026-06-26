# Jobs scope

Status: scope. **S0 decision doc** — fixes the README §13 "job queue" forever decision.
Implementation lands at S5 (remote workflow jobs); S0 only records the choice.

> Read with: `../../README.md` §6.9 (jobs), §11.7 (no native apalis backend — validate),
> §6.10 (outbox, the durability backstop jobs lean on).

---

## Goal

A **SurrealDB-native durable job queue** — no separate datastore (§3.2). Jobs are records;
workers claim atomically; LIVE queries give instant pickup; `run_at` + record-range scans
handle scheduling. Record the *choice* now so nothing builds a second datastore by accident.

## DECISION (forever): native SurrealDB queue, not an apalis backend (for v1)

README §6.9 offered two paths: a custom `apalis-surrealdb` backend (to keep apalis's
worker/middleware/cron ergonomics) **or** a thin from-scratch SurrealDB-native queue.

**Decision: build the thin native queue.** Reasons:

- **No native apalis-surrealdb backend exists** (§11.7) — we'd be writing+maintaining the
  hard part (the storage backend) anyway, then also carrying apalis's abstraction on top.
- **The atomic-claim primitive is small in SurrealDB**: a conditional `UPDATE … WHERE
  status = 'queued' RETURN BEFORE` in a transaction is the whole claim. LIVE queries on the
  jobs table give push pickup; an indexed `run_at` + record-range scan gives scheduling and
  delays. That is the entire mechanism — an external framework adds more than it saves.
- **Symmetric nodes:** the queue must run identically on a Pi edge and a cloud hub. A native
  queue over the one datastore we already embed has no extra moving parts; apalis's worker
  model would still need the same SurrealDB backend underneath.

**Rejected:** `apalis-surrealdb`. Revisit only if we find we're re-implementing cron/retry
middleware ergonomics at a cost that exceeds maintaining a custom backend — measure at S5,
don't pre-build.

### The record shape (S5 — as built)

`job:{id}` with fields: `status` (queued|running|done|failed), `kind`, `payload` (opaque
JSON — the agent stores its goal + caller here), `cursor` (the resume point — the next step
index), `steps` (the append-addressed transcript: `steps[i]` is the durable result of step
`i`, so re-running a persisted step is a lookup, not a re-spend), `attempts`, `ws` (the hard
wall — every job is workspace-scoped), `ts` (injected logical clock — no wall-clock, testing
§3). Remote workflow sessions (§6.9, agent scope) own durable session state **in this same
record** (the transcript + cursor) and drive progress through it; external effects go through
the **outbox** (§6.10) — deferred past S5, queued as job-owned state for now.

**S5 scope vs the full sketch:** S5 builds the *durable resumable session* subset the agent
needs — create / load / append-step / advance-cursor / complete, all workspace-scoped, with
**idempotent resume** (re-applying a persisted step is a no-op). The atomic-claim primitive
(`UPDATE … WHERE status='queued'`), `run_at` scheduling, backoff, lease/heartbeat, and cron
are NOT needed for the single hub-hosted agent session and are deferred — they land when a
multi-worker queue has contention to resolve (S6+). Recording the choice so nothing builds a
second datastore or a claim race by accident.

## How it fits the core

- **One datastore** (§3.2): jobs persist in SurrealDB on every node.
- **Workspace wall** (§3.6): `ws` field on every job; claim/scan queries are ws-scoped; a
  worker for ws A never claims ws B's jobs (a mandatory isolation test at S5).
- **Capability-first**: enqueuing/claiming is a host-mediated, caps-checked operation, not a
  raw table write by an extension.

## Testing plan (at S5, when built)

- Atomic claim under contention: two workers, one job, exactly one wins (property/concurrency).
- `run_at` scheduling and backoff math (unit, injected clock — testing §3).
- **Mandatory isolation:** ws-B worker cannot claim/see ws-A jobs.
- Durability: a claimed job survives a node restart and is re-leased after lease expiry.

## Open questions

- Lease/heartbeat interval and dead-worker reclamation policy → **deferred past S5** (the single
  hub-hosted agent session has no contending workers; lands with the multi-worker queue, S6+).
- Atomic claim under contention (two workers, one job) → **deferred past S5** with the queue; the
  primitive (`UPDATE … WHERE status='queued' RETURN BEFORE`) is recorded but unbuilt.
- Cron representation (a row vs a separate scheduler) → deferred past S5.
- Whether `kind` dispatch is a static registry or capability-gated per workspace → S5 uses a single
  `agent-session` kind; **S6 added a second kind** (`coding-session`, driven by the host `workflow`
  service) — still a static set, no registry yet; the per-workspace capability-gated dispatch opens
  when extensions provide job kinds (S7).
- Outbox vs job queue (§6.10): **DECIDED at S6** — the must-deliver outbox is a *dedicated* `outbox`
  table (the new `lb-outbox` crate), not a reuse of the job queue. A job is a resumable session; an
  effect is a fire-once intent — separate lifecycles, separate tables. See
  `../inbox-outbox/outbox-scope.md`.

## What shipped in S5 (durable resumable session)

The `lb-jobs` crate: the `Job` record (above) + the raw store verbs `create` / `load` /
`append_step` / `complete`, all workspace-namespaced (the hard wall, §7), **no authorization**
(raw verbs, like `lb-inbox`/`lb-assets` — the host's agent service is the caps chokepoint). Resume
is idempotent: `append_step(i, result)` upserts `steps[i]`, so re-applying a persisted step changes
nothing; the cursor only advances past steps that durably landed. Proven by the agent slice's
offline/sync test (a session survives the edge disconnecting and resumes without double-applying).
See `../agent/agent-scope.md` and `../../sessions/agent/ai-core-session.md`.
