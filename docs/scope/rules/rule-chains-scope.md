# Rules scope — rule chains: a rule DAG driven by `lb-jobs`

> **⚠ Retired to lineage — not a shipping surface.** The standalone `chains.*` engine this doc
> scoped is **superseded by `flows/`** (a flow is a strict superset of a rule-chain — same binding
> grammar, same `lb-jobs`-per-node topology, same frontier driver, plus subflows/sinks/sources and a
> data-driven canvas). The `chains.*` verbs, the host `chains` module, and the `lb_rules::workflow`
> model are **removed** by [`../flows/chains-retirement-scope.md`](../flows/chains-retirement-scope.md).
> This document is kept for its **`rubix-cube` workflow-DAG port history + attribution** (referenced
> by [`rules-engine-scope.md`](./rules-engine-scope.md)) and as the design record the flow engine
> generalised — **read it as lineage, build against `flows/`.**

Status: **retired to lineage** (superseded by `flows/`). Was: scope (the ask).

We want to **chain rules into a DAG** — a workflow where each step runs one saved rule, after its
upstream steps, with a step's output bound into the next step's inputs, triggered manually / on cron /
on a bus event. This is the **workflow half of `rubix-cube`** (`rules/workflow/`), ported so it
**reuses `lb-jobs`** instead of standing up its own queue, run-store, and cron claimer. The pure DAG
math and the binding/resolution logic transfer almost verbatim; the durable machinery becomes our
existing job system, with the workspace wall and capability gate it already enforces.

> Read with: `rules-engine-scope.md` (the single-rule engine each step runs), `../jobs/jobs-scope.md`
> (the durable queue a step becomes), `../inbox-outbox/outbox-scope.md` (the reactor that wakes a
> cron/event trigger; the outbox an alert routes to), `../bus/bus-scope.md` (the event trigger source),
> `../coding-workflow/coding-workflow-scope.md` (the S6 reactor pattern reused), README §6.9 (jobs).

---

## Source: ported from `rubix-cube/rules/workflow/`, re-based on `lb-jobs`

The reference is `rust/rubix-cube/rbx-server/src/rules/workflow/` (read 2026-06-28). Crucially,
`rubix-cube` **already made a workflow step a job on a shared queue** and isolated the swappable seams —
the port is mostly a seam-swap, not a rewrite. What transfers, what swaps:

| `rubix-cube` piece | Disposition in `lb-rules` chains |
|---|---|
| `workflow/model.rs` — `Workflow`/`Step`/`needs`/`with`/`RetrySpec`/`FailurePolicy`/`Trigger`; `validate` (Kahn cycle/dangling/dup/self-edge), `indegrees`, `dependents` | **Lift verbatim.** Pure DAG math, no dependency to change. Re-key `project_id` → `workspace`. |
| `workflow/context.rs` — `RunContext`, `${steps.<id>.output}`/`${steps.<id>.findings}`/`${params.<name>}` resolution **by value**, `Outcome`/`StepRecord`/`WorkflowResult`/`WorkflowStatus`/`StepResult` | **Lift verbatim.** Binding substitution + result aggregation is engine-agnostic. |
| `workflow/coordinator.rs` — `WorkflowCoordinator`: `start` enqueues the in-degree-0 frontier; `on_step_done` records, releases ready dependents (fan-in at in-degree 0), applies `FailurePolicy`, finalizes | **Lift the logic, swap the queue.** `Job::WorkflowStep` becomes an **`lb-jobs` job**; `enqueue` calls `lb-jobs`, not `rubix-cube`'s `JobQueue`. |
| `workflow/scheduler.rs` — `StepExecutor` trait + `WorkflowLimits` (`max_concurrency`, `step_timeout`) | **Lift.** `StepExecutor::execute` calls `lb-rules` (`rules-engine-scope.md`). Concurrency backstop becomes `lb-jobs`' semaphore. |
| `workflow/run_store.rs` — `WorkflowRunStore` (claim CAS `Pending→Enqueued→Running→Done`, `ready_dependents`, `skip_subtree`, `finalize_if_complete`); `InMemoryRunStore` now, `PgRunStore` "later, same trait" | **Implement the trait over SurrealDB.** `rubix-cube` left a `PgRunStore` TODO behind the trait; we land the durable store as **SurrealDB records** instead. The CAS claim = our idempotent step execution under redelivery. |
| `RunSink` (`workflow_runs` Pending→terminal) + `DbRunSink` (Postgres) | **Swap to SurrealDB.** The run lifecycle row is a workspace-walled record. |
| `Trigger::Cron` boot thread (DB window-claim, "multi-replica doesn't double-fire") | **Swap to the S6 reactor / job scheduler.** Our durable-scan reactor (`react_to_approvals` pattern) claims due chains per workspace — no bespoke scheduler thread. |
| `Trigger::Event{topic}` on an internal EventBus | **Swap to `bus.watch`.** A subscription on a Zenoh subject wakes the chain (motion → a durable job start). |
| `routes.rs` `POST /workflows/{id}/run` + `GET …/runs/{run_id}` | **Replace with MCP verbs** (`chains.run`/`chains.get`/…) + the gateway SSE feed for live status. |

`rubix-cube`'s own comment makes the port's safety explicit: *"the in-memory and durable backends run
the SAME code path; durability is purely a `Backend` + `WorkflowRunStore` swap."* We are doing exactly
that swap — to `lb-jobs` + SurrealDB. License/attribution as in `rules-engine-scope.md`.

## Goals

- A **`Chain`** (DAG of `Step`s) persisted as a SurrealDB record per workspace: steps, `needs` edges,
  per-step `with` bindings (literal | `${steps.x.output}` | `${params.y}`), per-step `retry`, a
  workflow `failure_policy` (Halt | Continue), and a `trigger` (Manual | Cron | Event).
- **Up-front DAG validation** (cycle / dangling dep / duplicate / self-edge / size cap) — rejected
  before any step runs (port `validate` verbatim).
- **Each step is an `lb-jobs` job.** The frontier driver (`start` → enqueue in-degree-0; `on_step_done`
  → release dependents, fan-in at 0, apply failure policy, finalize) runs over `lb-jobs`, inheriting its
  **durability, retry/backoff, resume-after-restart, and idempotency** — none of which we re-implement.
- **Durable run state in SurrealDB** behind the `WorkflowRunStore` trait: the run lifecycle, per-step
  claim/outcome, and the recorded outputs that downstream `${steps.x.output}` bindings read.
- **Triggers wired to existing seams:** Cron via the S6 reactor; Event via `bus.watch`; Manual via the
  `chains.run` verb.
- **MCP surface** (`chains.*`) + a **live status feed** (the DAG canvas colours as steps settle) over
  the gateway SSE route — and an `alert` from any step routes through the **inbox/outbox**.

## Non-goals

- **The single-rule engine** — that's `rules-engine-scope.md`. A step *runs* a saved rule via that
  engine; this scope is the DAG around it.
- **A new job system.** We **reuse `lb-jobs`** — its queue, worker pool, retry, and resume. We do not
  port `rubix-cube`'s `JobQueue`/cron-thread/`InMemoryRunStore` as the durable backend.
- **Arbitrary expression interpolation in bindings.** Port `rubix-cube`'s deliberately narrow
  **whole-value `${...}` reference** rule (a binding is exactly one reference or a literal) — no
  templating mini-language. Resist regex/partial interpolation until a real caller needs it.
- **Cross-workspace chains.** A chain, its steps, its run state, and its trigger are one workspace's.
- **Streaming token-level run events.** Per-step status is the live feed (a step is the unit); a
  finer-grained `RunEvent` projection can reuse `agent-run`'s vocabulary later (deferred, additive).

## Intent / approach

**The port is a seam-swap because `rubix-cube` already designed for it.** The coordinator is pure
frontier math that *enqueues steps onto a queue*; the run-store is behind a trait with a documented
"durable backend drops in here." We keep the math and the trait, and provide the durable backend as
**`lb-jobs` + SurrealDB**. Concretely:

- **A step → an `lb-jobs` job.** `Job::WorkflowStep{run_id, chain_id, step_id, workspace}` is enqueued
  through `lb-jobs`. A worker claims it (the `WorkflowRunStore` CAS `Pending|Enqueued → Running` is the
  idempotency guard under redelivery — a lost claim no-ops, so no double rule-run), resolves the step's
  `with` bindings against recorded upstream outputs, runs the rule via `lb-rules`, records the outcome,
  and calls `on_step_done`. Retry/backoff and resume-after-restart come from `lb-jobs`, not new code.
- **Run state → SurrealDB behind the trait.** `chain_run:{ws}:{run_id}` holds the lifecycle (Pending →
  terminal `Success|PartialFailure|Failed`); a `chain_step_output:{ws}:{run_id}:{step_id}` record per
  step holds claim state + outcome + output (a row per step so concurrent step writes don't contend —
  exactly the shape `rubix-cube`'s `run_store.rs` notes for its `PgRunStore`). `finalize_if_complete`
  collapses the run when every step is terminal.
- **Triggers reuse what we have, never a bespoke scheduler.** *Cron* is claimed by the **S6 durable-scan
  reactor** (the `react_to_approvals` pattern, already running per workspace) — it reads due chains each
  tick and starts a run, the window-claim making multi-node safe without a dedicated thread. *Event* is
  a **`bus.watch`** subscription on a Zenoh subject that starts a run on a matching message (motion →
  durable job, §3 rule 3). *Manual* is the `chains.run` verb.

**Why reuse `lb-jobs` is strictly better than porting the queue.** `rubix-cube` had to *build* a job
queue, an in-memory run store, and a cron claimer to drive chains, with a `PgRunStore` left as a TODO.
We already shipped the durable, resumable, idempotent equivalent (S5/S6: jobs survive disconnect/restart
and resume idempotently; the reactor auto-starts work; the outbox delivers effects at-least-once). So
chaining costs us **the DAG model + binding resolver (lift) + a thin step-job handler + the SurrealDB
run-store (new)** — the heavy durable machinery is reuse. And the chain inherits the **workspace wall +
`caps::check`** for free, which `rubix-cube`'s Postgres version never had.

**Rejected — porting `rubix-cube`'s `InMemoryRunStore`/cron-thread as the backend.** It would duplicate
`lb-jobs` (a second queue + a second scheduler), violating rule 1 (one mechanism) and re-introducing the
durability gap `lb-jobs` already closed. We keep `rubix-cube`'s **trait shape** (it's the clean seam) and
implement it over our store; we drop its in-memory/Postgres backends.

## How it fits the core

- **Tenancy / isolation:** a chain, every step job, the run state, and the trigger carry `workspace`,
  host-set from the token. A step job's `workspace` is un-spoofable (set at enqueue, not by the worker).
  ws-B cannot run, watch, or trigger a ws-A chain; a ws-B event/cron cannot start a ws-A run. Proven
  across store + MCP + the job queue.
- **Capabilities:** `chains.save`/`chains.run`/`chains.get`/`chains.list`/`chains.delete` each gated
  `mcp:chains.<verb>:call`. A step runs its rule under `caller ∩ grant` — the chain cannot let a rule
  read a source the chain's principal lacks. A cron/event trigger runs under the chain's stored
  principal, never an ambient elevation. Every grant has a deny test.
- **Placement:** `either` (symmetric). The coordinator + run-store are placement-free; a chain runs on
  the node that owns its workspace authority, like any job. No `if cloud`.
- **MCP surface (§6.1):**
  - **CRUD:** `chains.save` (create/update a chain: steps, edges, bindings, trigger, failure policy —
    validated up front, a bad DAG is a 400-equivalent deny), `chains.delete`.
  - **Run (manual trigger):** `chains.run {chain_id, params}` → `{run_id}` immediately (a chain is a
    **job**, never a blocking call — §6.1 batch-as-job). Returns the run id to watch.
  - **Get / list:** `chains.get {id}`, `chains.list {filter?}`; `chains.runs.get {run_id}` reads one
    run's live status + per-step results (the DAG-canvas read).
  - **Live feed (the core add):** `chains.watch {run_id}` → step status transitions (Pending → Running →
    Done/Failed/Skipped, fan-out/fan-in) over the gateway SSE route (mirrors `channel_stream`). Motion,
    not a polled `list` — though `chains.runs.get` remains for a snapshot/late join (rebuilt from the
    run records).
  - **Batch:** the chain **is** the batch-as-job. No second bulk surface.
- **Data (SurrealDB):** `chain:{ws}:{id}` (the DAG), `chain_run:{ws}:{run_id}` (lifecycle), and
  `chain_step_output:{ws}:{run_id}:{step_id}` (per-step claim + outcome + output). The job itself is an
  `lb-jobs` record. All workspace-walled, the one datastore — no new persistence layer.
- **Bus (Zenoh):** step-status motion on a per-run subject (`ws/{ws}/chain/{run}/**`), fire-and-forget
  (a dropped watcher re-reads the run records to catch up — the stream is never the record, §3 rule 3).
  An Event trigger *consumes* a Zenoh subject via `bus.watch`. A step's must-deliver `alert` goes
  through the **outbox**, not pub/sub.
- **Sync / authority:** the chain run is authoritative on its hosting node; a step job survives a node
  restart and **resumes idempotently** (the CAS claim + recorded outputs + the `lb-jobs` resume path).
  A cron/event trigger that fires while offline is claimed on reconnect by the reactor — the S6
  offline/idempotent pattern, reused.
- **Secrets:** none new. A step that reaches an external source does so through `federation.query` (the
  `datasources` extension holds the DSN); the chain never sees it.
- **SDK/WIT impact:** none — host crate + MCP verbs + `lb-jobs` reuse; no wasm/native ABI change.

## Example flow

A nightly food-safety report across the EMEA fleet, with an alert fan-in.

1. An admin saves a chain via `chains.save`: steps `pull` (rule reading `source("timescale")` for last
   night's cooler readings), `roll` (needs `pull`; rollup + threshold), `summarize` (needs `roll`;
   `ai.complete` a report), `notify` (needs `summarize`; `alert`). Trigger: `Cron "0 6 * * *"`. The DAG
   validates (acyclic, deps resolve) — a cycle would be rejected here, before any run.
2. At 06:00 the **S6 reactor** claims the due chain for `acme` (window-claim → multi-node safe) and calls
   the coordinator's `start`: insert `chain_run` Pending, seed per-step state, enqueue the in-degree-0
   frontier (`pull`) as an `lb-jobs` `Job::WorkflowStep`.
3. A worker claims `pull` (CAS `Enqueued→Running`), resolves its bindings, runs the rule via `lb-rules`
   (the `federation.query` to Timescale happens inside the rule, gated), records the output, calls
   `on_step_done`. In-degree of `roll` hits 0 → enqueued. The DAG canvas (over `chains.watch` SSE) shows
   `pull` green, `roll` running.
4. `roll` → `summarize` (an `ai.complete`, metered) → `notify`. `notify`'s `alert` raises an **inbox**
   item and routes the email through the **outbox** (at-least-once, dedup — never lost, never double-sent).
5. The frontier exhausts; `finalize_if_complete` collapses the run to `Success`. `chains.runs.get`
   returns the per-step results + timings.
6. **Failure path:** `summarize` errors after its retries. Under `failure_policy: Halt`, `skip_subtree`
   marks `notify` Skipped (run = `PartialFailure`); under `Continue`, `notify` runs with
   `${steps.summarize.output}` resolved to `null` and copes. **Restart path:** the node restarts mid-run
   → `lb-jobs` + the CAS claim resume the un-run steps exactly once (a duplicate redelivery no-ops).
7. **Deny path:** a ws-B principal calls `chains.run` on the ws-A chain → denied, opaque. A ws-B event
   on the trigger subject cannot start the ws-A run (the subject is workspace-namespaced).

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`) — the gate. **No mocks**: the **real `lb-jobs`
queue**, real store, real caps, real reactor, a real bus subject for the Event trigger; rules seeded as
real records and run through the real `lb-rules` engine. The only sanctioned fake is the model provider
behind the AI-gateway seam (for a step that calls `ai.*`).

- **DAG validation (port `rubix-cube`'s tests):** cycle / dangling dep / duplicate id / self-edge / size
  cap each rejected by `validate` **before any step runs**; a valid diamond (fan-out + fan-in) schedules
  in the right order.
- **Capability-deny (§2.1):** each `chains.<verb>` denied without its cap; a step whose rule reads a
  source the **chain's principal** lacks is denied mid-run (no widening via chaining); a cron/event
  trigger cannot run under more authority than the stored principal.
- **Workspace-isolation (§2.2):** ws-B cannot run/watch/get a ws-A chain or run; a ws-B event/cron
  cannot start a ws-A run; the step job's `workspace` is un-spoofable — across store + MCP + the queue.
- **Offline / sync (§2.3) — the headline (reuses S5/S6):** kill the node mid-run → on restart the chain
  **resumes and finishes exactly once** (the CAS claim + `lb-jobs` resume; a duplicate `WorkflowStep`
  redelivery and a reactor re-scan are no-ops, no double rule-run); a cron trigger that comes due while
  offline is claimed once on reconnect; a step's outbox `alert` delivers at-least-once, idempotently.
- **Frontier behavior (port + re-run on `lb-jobs`):** parallel fan-out runs independent branches
  concurrently; fan-in fires only when in-degree hits 0; `Halt` skips the transitive subtree of a
  failure while independent branches finish; `Continue` releases dependents with the failed output as
  `null`. Run status = `Success | PartialFailure | Failed` per the outcome mix.
- **Binding resolution (port verbatim):** `${steps.x.output}` / `${steps.x.findings}` / `${params.y}`
  substitute by value; a non-reference value is a literal; a whole-string-only reference rule (embedded
  `${` rejected); a failed/skipped upstream resolves to `null` under `Continue`.
- **Live feed (real gateway):** a `chains.watch` `*.gateway.test.tsx` shows a late watcher get a
  snapshot rebuilt from the run records, then live step transitions — against a real spawned node.

## Resolved decisions

No open questions — these are the long-term answers the build follows.

- **The durable backend → `lb-jobs` + SurrealDB, not `rubix-cube`'s queue.** Keep the
  `WorkflowRunStore`/`RunSink`/`StepExecutor` **traits** (the clean seams); implement them over our job
  system + store. Drop `JobQueue`, `InMemoryRunStore`-as-backend, and the cron-thread. Rationale: one
  job mechanism (rule 1), and `lb-jobs` already has the durability/resume/idempotency the port needs.
- **A step is an `lb-jobs` job (`Job::WorkflowStep`).** The CAS claim in the run-store is the idempotency
  guard under redelivery; retry/backoff/resume come from `lb-jobs`. No bespoke worker pool.
- **Run state shape → a record per step (`chain_step_output:{ws}:{run}:{step}`) + one lifecycle record.**
  Per-step rows so concurrent step writes don't contend (the shape `rubix-cube` documented for its
  deferred `PgRunStore`). Inline on the run only if a step count cap makes a child table needless —
  start with per-step rows, they're the contended path.
- **Triggers → reactor (cron) + `bus.watch` (event) + verb (manual); never a new scheduler thread.**
  Cron is claimed by the S6 durable-scan reactor (window-claim = multi-node safe); event is a Zenoh
  subscription; manual is `chains.run`. This reuses shipped seams and inherits their offline behavior.
- **Bindings → whole-value `${...}` references only (port the narrow rule).** A binding is exactly one
  reference or a literal — no interpolation language. `params` + `steps.<id>.output|findings` are the
  resolvable namespaces. Additive expansion later, but not v1.
- **Failure policy → `Halt` (default) | `Continue` (port verbatim).** Halt prunes the failed subtree
  (run = PartialFailure); Continue releases dependents with a `null` upstream output.
- **MCP surface, not HTTP routes.** Replace `rubix-cube`'s actix workflow routes with `chains.*` MCP
  verbs + the gateway SSE feed for `chains.watch` (rule 7).
- **Per-run/per-step limits → config (port `WorkflowLimits`).** `step_timeout` + a per-run concurrency
  knob over the `lb-jobs` semaphore backstop; a per-workspace override is additive later.

## Related

- `rules-engine-scope.md` — the single-rule engine each `Step` runs (`StepExecutor::execute` → `lb-rules`).
- `../jobs/jobs-scope.md` — the durable queue a step becomes; the resume/idempotency this inherits.
- `../inbox-outbox/outbox-scope.md` — the S6 reactor reused for cron/event wake; the outbox an `alert`
  routes to.
- `../coding-workflow/coding-workflow-scope.md` — the `react_to_approvals` durable-scan pattern the cron
  trigger reuses.
- `../bus/bus-scope.md` — `bus.watch`, the Event trigger source; the per-run status subject.
- `../datasources/datasources-scope.md` — `federation.query`, reached inside a step's rule for external
  sources.
- README `§6.9` (jobs — the chain is a job), `§6.10` (inbox/outbox), `§6.5` (MCP), `§3` (rules 1/2/4/5/6/7).
- Source: `rust/rubix-cube/rbx-server/src/rules/workflow/` (ported here; MIT/Apache-2.0).
