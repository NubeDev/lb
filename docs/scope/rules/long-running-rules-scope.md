# Rules scope — long-running rule runs: jobs, checkpoints, pause/resume

Status: scope (the ask). Promotes to `doc-site/content/public/rules/` once shipped.

A `rules.run` is deliberately synchronous and bounded (10 s wall-clock, 5 M ops — the governors are
the contract). That is right for the Playground and for flow nodes, but a real workspace also has
**batch-shaped rule work**: sweep a month of series data, classify ten thousand rows through `ai.*`,
build a report across twenty sources. Today that work either trips the governors or gets smuggled
into an oversized flow. This scope makes a rule run a **first-class durable job**: started in the
background, observable, **pausable and resumable across restarts**, with **author checkpoints** so a
resume never re-spends completed work.

> Read with: `rules-engine-scope.md` (the cage + seams this extends), `../jobs/jobs-scope.md` +
> `../jobs/job-control-scope.md` (the `lb-jobs` record + the cooperative-control doctrine),
> `rules-messaging-scope.md` (the deterministic-id/upsert contract that makes replay safe),
> `../flows/flow-runtime-control-scope.md` (the suspend/resume precedent), README §6.9 (jobs), §6.1
> (batch-as-job).

---

## Goals

- **`rules.run_async {body|rule_id, params}` → `{run_id}`** — enqueue a rule evaluation as a durable
  `lb-jobs` job (kind `rule-run`), seeded synchronously (an immediate `runs.get`/`cancel` finds it),
  driven on a detached task — the flows `flows_run_async` pattern, reused.
- **A long-run governor profile** — the job path gets its own limits (`LB_RULES_JOB_TIMEOUT_MS`
  default 10 min, `LB_RULES_JOB_MAX_OPERATIONS` default 500 M, `LB_RULES_JOB_AI_MAX_CALLS` /
  `LB_RULES_JOB_AI_MAX_TOKENS` / `LB_RULES_JOB_MAX_WRITES` scaled up) — still *bounded*, just sized
  for batch. The cage is unchanged; only the knob values differ.
- **Cooperative pause/cancel that needs no author cooperation** — a shared `RunControl` observed by
  the cage's per-operation governor: `rules.runs.suspend` parks the run at the next bytecode op
  (job → `Suspended`), `rules.runs.cancel` aborts it (job → `Cancelled`). Mirrors `flows.suspend`/
  `flows.cancel` and job-control D2 (cancel is terminal from any non-final state; both are clean
  no-ops when already settled).
- **Author checkpoints — the `job` scope handle** (in the cage, every run): `job.step(key, || …)`
  memoizes a unit of work into the job transcript; `job.set/get/has` are raw checkpoint state;
  `job.progress(pct, msg)` records observable progress; `job.should_stop()` exposes the control
  intent for tidy early returns. In a synchronous `rules.run` the same handle is ephemeral
  in-memory — one body, both modes.
- **Resume = replay over checkpoints.** `rules.runs.resume` re-runs the body from the top with the
  persisted checkpoint map folded back in: `job.step` blocks replay as lookups (no re-spend), and
  every messaging write replays onto the **same deterministic id** (the pinned `now` + write
  ordinal), upserting — the rules-messaging determinism contract is exactly what makes resume safe.
  Works after a pause, after a node restart (an orphaned `Running` job is simply resumable), and
  under redelivery.
- **Observe:** `rules.runs.get {run_id}` (status, live flag, latest progress, checkpoint keys,
  bounded transcript tail, result when terminal) and `rules.runs.list {status?, limit?}` —
  ws-scoped reads.

## Non-goals

- **No generic `job.*` MCP family.** That is `../jobs/job-control-scope.md` (owner-routed controls
  for every kind). These are **owner verbs** (`rules.runs.*`) — the rules service is the chokepoint
  for its own kind, exactly the architecture that scope preserves. When the generic family ships,
  it routes `rule-run` to the same hooks.
- **No `rules.runs.watch` SSE in v1.** Progress is polled via `runs.get` (bounded tail). The live
  feed rides the run-events vocabulary later — additive, and the flows watch route is the template.
  Deliberate deferral, not a silent gap.
- **No auto-resume of orphans at boot.** A rule run is caller-initiated; auto-resume would require
  persisting a runnable principal (a security surface we refuse). After a restart an orphaned run
  shows `live:false` in `runs.get`/`list`; any holder of `rules.runs.resume` re-attaches it — and
  the resumed run executes under the **resumer's** `caller ∩ grant`, re-checked at every seam.
- **No mid-op preemption.** A native seam call (a big collect, an `ai.complete`) finishes before
  the control flag is observed — the same honest boundary as job-control's step-boundary contract.
- **No new queue.** One job record, one detached task; the multi-worker claim/lease machinery stays
  deferred where `jobs-scope.md` left it.

## Intent / approach

**Pause is safe at any tick because replay is idempotent by construction.** We do NOT try to
serialize a live rhai VM (impossible to do honestly). Pause aborts the eval with a typed token; all
durable effects so far are (a) checkpoints — persisted eagerly at each `job.set`/`job.step`, and
(b) messaging writes — deterministic-id upserts. Resume re-evaluates the body from the top: `step`
blocks short-circuit to persisted values, replayed writes land on their original ids. The only
re-run cost is un-checkpointed pure compute. This is the same replay-not-snapshot doctrine as the
agent loop's append-addressed transcript — reused, not reinvented.

**Checkpoints ride the `lb-jobs` transcript.** Two additive `TranscriptEvent` variants (the enum is
`#[non_exhaustive]` + versioned for exactly this): `Checkpoint {key, value}` (JSON-as-string, the
`ToolCallProposed.args` precedent) and `Progress {pct?, msg}`. Append-addressed slots make replayed
persistence a no-op. Bounded: 256 checkpoints/run (author error past it), 1000 durable progress
beats (advisory, dropped past it) — a loop cannot flood the store.

**Control is a shared flag + a durable record, owner-enacted.** `suspend`/`cancel` set the live
run's `RunControl` (in-process registry, run_id → control) *and* act on the job record through the
rules service. The cage's `on_progress` governor returns a typed abort token; the engine maps it to
`RuleError::Paused`/`Cancelled`; the worker then writes the honest terminal/parked status. A
control verb on a non-live run acts on the record alone (suspend a crashed-orphan → parked; cancel
→ terminal). Never a thread kill — the `flows.cancel` cooperative model, applied one level deeper.

**Rejected — "a long rule is just a one-node flow."** A flow suspends only *between* frontier
nodes; a single long rhai node is exactly the un-pausable unit this scope fixes. Flows keep the DAG
role; this gives the leaf its own durability.

**Rejected — storing a principal in the job payload for auto-resume.** A serialized principal is a
forgeable ambient authority; every existing background path (flows drive task) holds the principal
in memory only. We accept "orphans wait for a caller" as the honest v1.

## How it fits the core

- **Tenancy / isolation:** the job is `job:{id}` in the workspace namespace (the hard wall);
  `run_async` pins `ws` from the token; every `runs.*` verb resolves the job inside the caller's
  ws only. ws-B cannot see/suspend/resume/cancel a ws-A run. Mandatory isolation test.
- **Capabilities:** one cap per verb — `mcp:rules.run_async:call`, `mcp:rules.runs.get:call`,
  `mcp:rules.runs.list:call`, `mcp:rules.runs.suspend:call`, `mcp:rules.runs.resume:call`,
  `mcp:rules.runs.cancel:call`. Read ≠ control (an observer role holds get/list only). Inside the
  run, every data/ai/messaging verb re-checks `caller ∩ grant` at the seam exactly as today — a
  job-backed run widens nothing. Deny is opaque; a deny test per verb.
- **Placement:** either — host code over the store; the worker runs where the verb ran. No
  `if cloud`.
- **MCP surface (§6.1):** run (`rules.run_async` — the batch-as-job form; `rules.run` stays the
  bounded sync form), get/list (`rules.runs.get`/`list`, keyset-limited), control
  (`rules.runs.suspend`/`resume`/`cancel` — intent verbs, not raw table writes), live feed
  (deferred, see Non-goals), batch (N/A — the run *is* the batch).
- **Data (SurrealDB):** the existing `job` table only — payload carries `{body|rule_id, params,
  now, route}`; checkpoints/progress/result are transcript events. No new table.
- **Bus (Zenoh):** none in v1 (no watch). `alert`/messaging effects route exactly as today.
- **Sync / authority:** the job record is node-authoritative; kill the node mid-run → the job is
  `Running`+`live:false` on restart, `resume` replays exactly-once-per-effect (deterministic ids).
- **Secrets:** none — the payload carries no credential; the model/DSN stay behind their seams.
- **One responsibility per file:** `host/src/rules/runs/` — `start.rs`, `worker.rs`, `get.rs`,
  `list.rs`, `suspend.rs`, `resume.rs`, `cancel.rs`, `registry.rs` (live-run control map),
  `seam.rs` (the `JobSeam` over `lb-jobs`); engine-side `control.rs` + `verbs/job.rs` in `lb-rules`.
- **SDK/WIT impact:** none — host verbs + an in-cage handle; no ABI change.

## Example flow

1. An analyst starts a month-long sweep: `rules.run_async {rule_id:"monthly-anomalies"}` →
   `{run_id:"rr_9f2"}`. The body:
   ```rhai
   let days = job.step("plan", || make_day_list(param("month")));
   let mut done = 0;
   for d in days {
       if job.should_stop() { break; }
       job.step(`day:${d}`, || scan_one_day(d));   // each day memoized
       done += 1;
       job.progress(done * 100 / days.len(), `day ${d} done`);
   }
   ```
2. `rules.runs.get {run_id}` shows `running`, `live:true`, `pct:40`.
3. The operator pauses (`rules.runs.suspend`): the governor observes the flag within one bytecode
   op, the eval aborts `Paused`, the job parks `Suspended`. Checkpointed days are durable.
4. Overnight the node restarts. `runs.get` → `suspended`, `live:false`.
5. `rules.runs.resume {run_id}` — the body replays: `plan` and every finished `day:*` return as
   lookups (instant, zero AI spend), the loop continues at day 13. Alerts re-emitted during replay
   land on their original deterministic ids — upserts, no duplicates.
6. A rogue variant loops forever without checkpoints: `rules.runs.cancel` bites at the next op —
   `Cancelled`, terminal, transcript kept. A second cancel is a no-op.
7. **Deny paths:** ws-B `runs.get {run_id:"rr_9f2"}` → opaque NotFound-equivalent; a get/list-only
   principal is refused suspend/resume/cancel.

## Testing plan

Per `scope/testing/testing-scope.md` — real store, real caps, real host; no mocks.

- **Capability-deny (mandatory):** each of the six verbs denied opaquely without its cap; read ≠
  control asserted.
- **Workspace-isolation (mandatory):** ws-B cannot get/list/suspend/resume/cancel a ws-A run;
  a ws-A run never lists under ws-B.
- **Pause/resume (the headline):** start a checkpointing run, suspend mid-loop, assert `Suspended`
  + persisted checkpoints; resume; assert completed steps did NOT re-run (a counter seam proves
  no re-spend), the run finishes, and messaging effects are not duplicated (same ids, upserted).
- **Restart resume (offline/sync §2.3):** run with checkpoints, drop the worker (simulated crash),
  reload on a fresh node handle, resume — finishes exactly-once-per-effect.
- **Cancel:** bites mid-loop without author cooperation; terminal; idempotent re-cancel; a cancel
  of a `Suspended` run works (job-control D2).
- **Governor profile:** the job path runs past the sync 10 s/5 M-op profile under its own knobs; an
  infinite loop still dies on the job profile's ceiling.
- **Cage (unit, lb-rules):** `RunControl` pause/cancel abort tokens map to `Paused`/`Cancelled`;
  `job.step` memoizes + round-trips through JSON; checkpoint budget errs at 257; progress cap
  drops durably but keeps running; ephemeral mode never persists and `should_stop()` is false.
- **Determinism:** a resumed run's replayed write ids are byte-identical to the first attempt's.

## Resolved decisions

- **Verb family: `rules.runs.*` owner verbs, not raw `jobs.*`** — honors the jobs chokepoint
  doctrine; the future generic `job.*` family routes to these hooks by kind.
- **Suspend vocabulary matches flows** (`suspend`/`resume`, UI may label Pause) — one vocabulary
  across the platform's pausable things.
- **Pause at any governor tick, not only at checkpoint boundaries** — replay-idempotency (not VM
  snapshotting) is the resume model, so an arbitrary abort point is safe; prompt pause beats
  checkpoint-gated pause.
- **`now` is pinned at enqueue and reused on every resume** — determinism across attempts; the
  transcript records real progress, the ids never drift.
- **Checkpoint values are JSON** (`dynamic_to_json` round-trip) — a Grid/handle cannot checkpoint;
  `job.step` normalizes its result through JSON so what you resume with is what you persisted.
- **The `job` handle exists in sync runs too (ephemeral)** — one body runs in both modes; authors
  develop in the Playground and promote to `run_async` unchanged.

## Related

- `rules-engine-scope.md` — the cage/governors/seams; `rules-messaging-scope.md` — the
  deterministic-id contract replay relies on.
- `../jobs/jobs-scope.md`, `../jobs/job-control-scope.md` — the record, the cooperative-control
  doctrine, the future generic surface.
- `../flows/flow-runtime-control-scope.md` — the suspend/resume/cancel precedent this mirrors.
- `data-stdlib-scope.md` — the sibling scope shipping in the same build (the `time`/`json`/`stats`/
  `mathx`/`Frame` families a long run computes with).
- README §6.9 (jobs), §6.1 (batch-as-job), §3 rules 4/5/6.
