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

### The record shape (sketch, finalized at S5)

`job:{id}` with fields: `status` (queued|claimed|running|done|failed|dead), `kind`,
`payload`, `run_at` (indexed), `attempts`, `max_attempts`, `backoff`, `claimed_by`,
`lease_until`, `ws` (the hard wall — every job is workspace-scoped), timestamps. Retries =
re-queue with `run_at = now + backoff(attempts)`; cron = a periodic enqueuer row. Remote
workflow sessions (§6.9) own durable session state in their own records and *use* a job to
drive progress; external effects go through the **outbox** (§6.10), never raw pub/sub.

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

- Lease/heartbeat interval and dead-worker reclamation policy → S5.
- Cron representation (a row vs a separate scheduler) → S5.
- Whether `kind` dispatch is a static registry or capability-gated per workspace → align with
  caps grammar at S5.
