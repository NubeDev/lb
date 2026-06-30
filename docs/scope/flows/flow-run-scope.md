# Flows scope — the durable run engine (how a flow runs, suspends, resumes)

Status: scope (the ask). Promotes to `public/flows/` once shipped.

A flow **run** is how a node-graph turns into work: a durable thing that survives restart,
resumes idempotently, can pause → patch → resume, and exposes its progress over MCP. This doc
owns the **execution engine** — not the node model, not the editor. The spine
(`flows-scope.md`) owns the canonical **Decisions (v1) 1–7**; sub-docs reference them by number
rather than re-deciding. The headline still holds: **this is not a new engine.** A run is a
`flow_run` **coordinator record + one `lb-jobs` job per node** (a `flow-step` kind), driven by the
`chains` frontier driver, ported verbatim (`../rules/rule-chains-scope.md`;
[Decision 8](flows-scope.md)). We add the run-store records, the version-pinning resume contract,
and the `flows.*` run surface — nothing else.

## Goals

- A run is a durable **`flow_run` coordinator + one `lb-jobs` job per node** (a `flow-step` kind,
  [Decision 8](flows-scope.md)) — restart-survivable, resume-idempotent,
  suspend/resume/cancel-able through `lb-jobs`' own verbs.
- The **frontier driver ported from `chains` verbatim** drives node scheduling: validate-at-save,
  in-degree-0 frontier, **enqueue a node's job when its in-degree hits 0**, `on_step_done` release,
  `FailurePolicy = Halt | Continue`, CAS step-claim. **Concurrency is independent branch jobs
  competing for the `lb-jobs` semaphore** — not async tasks inside one worker walking a cursor.
- **Run-store records** mirroring `chain_run` / `chain_step_output`, carrying the **pinned
  `flow_version`** (Decision 1) so a live run is immune to edits.
- **Pause → edit → resume** as a config-only `flows.patch_run`; a structural edit becomes a new
  version; a drifted resume fails cleanly with `ResumePointDrift`.
- The **`flows.*` run surface** — `run` (batch-as-job), `runs.get`, `watch` (SSE),
  `suspend`/`resume`/`cancel`, `patch_run` — one cap per verb.
- **No capability widening:** `flows.run` plus every node-tool's own gate under `caller ∩ grant`.
- A **fan-out posture** (the `coalesce` enum defined here) so a chatty source can't storm the queue, and
  **must-deliver node effects through the outbox**.

## Non-goals

- **A new runtime, scheduler, or persistence layer.** Reuse `lb-jobs` + the `chains` driver +
  SurrealDB run-store. No second queue, no bespoke worker pool, no new store.
- **The node model / descriptor / config schema** — `node-descriptor-scope.md`.
- **Extension-node execution + install-grant narrowing** — `extension-nodes-scope.md` (this doc
  gives only the no-widening *run* gate; the per-extension narrowing lives there).
- **Trigger definitions** (manual/cron/event/inject/boot) — `triggers-lifecycle-scope.md`. This
  doc carries only the run-side fan-out *posture* (and defines the canonical `coalesce` enum).
- **Streaming sub-node/token events.** A node is the unit of the live feed; finer projections are
  a deferred, additive follow-up (as in chains).

## Intent / approach

**A run is a coordinator + one job per node; the driver is chains' ([Decision 8](flows-scope.md)).**
`flows.run {id,params}` validates the pinned graph, writes a `flow_run` coordinator record via
`lb-jobs::create`, and returns `{run_id}` **immediately** — never a blocking loop (§6.1
batch-as-job; the run *is* the durable coordinator). The frontier driver, ported verbatim from
`chains`, seeds the in-degree-0 frontier and **enqueues one `lb-jobs` job per node** (a `flow-step`
kind, generalising `chains`' `Job::WorkflowStep`) as each node's in-degree hits 0. **Concurrent
branches are independent jobs competing for the `lb-jobs` semaphore** — not async tasks inside one
worker walking a single cursor. This is what makes "ported verbatim from chains" honest, and it
makes [Decision 6](flows-scope.md)'s `chains.*` alias true at the **execution** layer, not just the
MCP layer.

**Exactly-once has two owners, two layers ([Decision 8](flows-scope.md)).** They are *not*
redundant guards of the same thing: the **CAS claim** `Enqueued→Running` on `flow_step_output`
owns **cross-node** exactly-once under restart-redelivery (a lost claim no-ops, so no double
node-run); **`append_event` idempotency** owns **within-a-single-step** replay — for a node that is
itself a multi-turn job, `lb-jobs::append_event(store,ws,id,index,event)` upserts at `steps[index]`
(`cursor = max(cursor, index+1)`), so a re-applied turn on resume is a no-op.

**Each node's job claims via CAS, runs, records, releases.** The run-store's step record holds the
CAS claim; `on_step_done` records the outcome and releases dependents that reach in-degree 0;
`FailurePolicy::Halt` prunes the failed node's subtree (those nodes `Skipped`), `Continue` releases
them with a `null` upstream output.

**Version-pinning is the spine of resume (Decision 1).** `flows.run` pins `flow_version` into
the run-store. Editing the flow writes a **new version** — the suspended run keeps its pinned
graph, untouched. "Pause → tweak → resume" is `flows.patch_run {run_id, node, config}`: a
**config-only** patch to an **unexecuted** node of the pinned run. A **structural** change
(add/remove/retype a node, rewire an edge) is **rejected for a live run** and becomes a new
version for the next run. On resume, the engine validates the next-frontier nodes still exist
with the **same type + ports**; a mismatch fails cleanly as `ResumePointDrift`, surfaced in
`flows.runs.get`. A `patch_run` validates against the **pinned** schema, not the latest descriptor
([Decision 12](flows-scope.md)).

**A subflow node waits on a child run — the one new coordination pattern ([Decision 11](flows-scope.md)).**
A `subflow` node enqueues a **pinned** child `flow_run` and its parent step **parks (suspends)**
until the child reaches terminal, then maps the child's output nodes → the parent node's ports by
name (the [Decision 4](flows-scope.md) binding grammar — one whole-value `${…}` reference or a
literal per binding, no templating). A **child failure** surfaces as the parent node's
`Outcome::Err`, handled under the **parent's** `FailurePolicy` (Halt prunes the parent subtree,
Continue passes `null`); a **parent suspend cascades** to the child run; parent and child **pin
versions independently**. This "a step waits on a child run" is the single genuinely new pattern
the engine adds over the linear-chain frontier: the frontier driver **resumes the parent on
child-run terminal** — a reactor on child completion, not a poll. *Rejected:* inlining the child
graph into the parent run (loses independent versioning + the clean parent/child wall).

**Runs are one-shot; retained inputs are the read-side ([Decision 9](flows-scope.md)).** The
frontier runs to terminal — there is no long-lived parked run awaiting input. A run **reads**
retained input values from `flow_input:{ws}:{flow}:{node}` (set on a *retained* input node via
`flows.inject`); a run is **started by a firing trigger**, never by a retained inject (an inject
into a retained node updates state and starts nothing). The inject mechanics live in
`triggers-lifecycle-scope.md` + `dashboard-binding-scope.md`; this doc owns only the run engine's
**read-side**: every run consults the current retained values, so a control loop is retained inputs
+ event-triggered one-shot runs, not a parked run.

*Rejected:* a blocking `run` that loops over nodes in the handler (no resume, ties up the call,
breaks past some N — the §6.1 smell). *Rejected:* **one job walking a single linear
transcript+cursor** (it cannot run concurrent branches — the framing an earlier draft used, now
corrected per [Decision 8](flows-scope.md)). *Rejected:* mutating the running graph in place
(rewrites append-addressed step history; [Decision 1](flows-scope.md)). *Rejected:* a second
run-store distinct from chains' (duplicate machinery — [Decision 6](flows-scope.md) says one
engine, `chains.*` becomes a thin alias).

## How it fits the core

- **Tenancy / isolation:** the run job, its steps, its run-store records, its outbox effects, and
  its series are all `…:{ws}:…` in the workspace namespace; `flows.run` **selects the caller's
  namespace** from the token (un-spoofable, set at create, not by the worker). ws-B cannot run,
  watch, get, suspend, resume, cancel, or patch a ws-A run. **Isolation tested across store AND
  MCP.**
- **Capabilities — composition, never widening (Decision 7; the `query.run` precedent):** running
  a flow needs `mcp:flows.run:call` **AND** every Tool node passes its **own** gate under
  `caller ∩ grant`. Holding `flows.run` alone **cannot** reach a tool the caller lacks. One cap
  per verb (`mcp:flows.run:call`, `mcp:flows.runs.get:call`, `mcp:flows.runs.list:call`,
  `mcp:flows.watch:call`, `mcp:flows.suspend:call`, `mcp:flows.resume:call`,
  `mcp:flows.cancel:call`, `mcp:flows.patch_run:call`). **Deny matrix:**
  - (a) caller holds `flows.run` but **not** a Tool node's underlying cap → **that node is
    denied**, the run records the deny (`Err` outcome) on its step, **no widening**, and the run
    continues under `FailurePolicy` (Halt prunes the subtree; Continue passes `null`).
  - (b) extension-node cases (install-grant `caller ∩ install-grant` narrowing) are in
    `extension-nodes-scope.md` (link) — this doc owns only the caller-side `flows.run` gate.
- **Placement:** `either` (symmetric). Driver + run-store are placement-free; a run executes on
  the node that owns its workspace authority, like any job. No `if cloud`.
- **MCP surface (§6.1):**
  - **Run (batch-as-job):** `flows.run {id, params} → {run_id}` — returns **immediately**; the
    run is the durable job. No blocking loop.
  - **Get:** `flows.runs.get {run_id}` — a snapshot of per-node status + outcomes + the pinned
    `flow_version`, rebuilt from the run-store records (also the late-join / `ResumePointDrift`
    surface).
  - **List (reattach):** `flows.runs.list {flow_id, status?}` — the runs of a flow (optionally
    filtered by lifecycle status). The canvas/dashboard hold a **`flow_id`** but `flows.watch` /
    suspend / resume / cancel all key on **`run_id`**; `runs.list` is how a reopened surface
    finds the **active `run_id`** to reattach to. `flows.get` may also return active run ids
    inline. One cap `mcp:flows.runs.list:call`.
  - **Live feed:** `flows.watch {run_id}` → SSE stream of **node-status transitions**
    (Enqueued → Running → Ok/Err/Skipped, fan-out/fan-in) over the gateway SSE route — **reuse
    the named `chains.watch` follow-up** verbatim. Motion, not a polled `list`; a dropped watcher
    re-reads the run records via `runs.get` (the stream is never the record, §3 rule 3).
  - **Lifecycle:** `flows.suspend {run_id}` / `flows.resume {run_id}` (`lb-jobs` `suspend()` /
    `unsuspend()`); `flows.cancel {run_id}` (`lb-jobs` `cancel()` — **non-resumable**, terminal);
    `flows.patch_run {run_id, node, config}` (config-only patch to an unexecuted node of the
    pinned run). The patched config is validated against the run's **pinned node schema** (the
    pinned `flow_version`'s descriptor), **never the current descriptor** —
    [Decision 12](flows-scope.md); a live run must accept exactly the fields the pinned form
    offered, not those a newer descriptor moved.
  - **CRUD N/A here** — flow CRUD is `flows.save`/`list`/`delete` on the spine; a run is not
    independently created/edited, it is started, watched, and steered. (A run *is* listed —
    `flows.runs.list` above is the reattach surface, not a flow-CRUD verb.)
- **Data (SurrealDB) — no new persistence ([Decision 6](flows-scope.md)):** each node's execution
  is its own `lb-jobs` job (a `flow-step` kind); the run-store mirrors chains':
  - `flow_run:{ws}:{run_id}` — the **coordinator** record: lifecycle
    `Pending → Success | PartialFailure | Failed`, the **pinned `flow_version`**, and the frontier
    state the driver advances. (Compare `chain_run`.)
  - `flow_step_output:{ws}:{run_id}:{node_id}` — one record per node (so concurrent **branch jobs**
    don't contend): the CAS claim state `Enqueued→Running` (the **cross-node** exactly-once owner,
    [Decision 8](flows-scope.md)) + outcome `Ok | Err | Skipped` + the recorded output downstream
    bindings read. (Compare `chain_step_output`.)
- **Bus (Zenoh):** node-status chatter is **fire-and-forget motion** on a per-run subject
  (`ws/{ws}/flow/{run}/**`) — a dropped watcher rebuilds from the records. **Must-deliver node
  effects go through the OUTBOX** (transactional with the step, `idempotency_key`, backoff,
  dead-letter), **never raw pub/sub** — `../inbox-outbox/outbox-scope.md`.
- **Sync / authority:** an **edge** flow run survives the edge disconnecting; on reboot/reconnect
  `lb-jobs` resumes the coordinator and its in-flight node jobs, and the **CAS claim** re-applies
  un-run nodes **exactly once** cross-node while **idempotent `append_event`** re-applies a
  multi-turn node's own replay ([Decision 8](flows-scope.md)). The outbox **relays effects on
  reconnect**, at-least-once + dedup. The run is authoritative on its hosting node.
- **Observability:** `flow_run` / `flow_step_output` carry the **per-node outcome plus the
  deny / drift reason** (the why-didn't-it-fire / why-was-this-node-denied / why-did-resume-drift
  story). These are surfaced read-side by `flows.runs.get` / `flows.watch` and emitted into the
  cross-cutting `observability/` traces + `audit/` deny-ledger seam at the host-dispatch chokepoint
  — the run-store *is* the record, not a parallel log.
- **Secrets:** none new — a node reaching an external source does so through the gated tool
  (`federation.query`, etc.); the run never sees a DSN.
- **SDK/WIT impact:** none — host crate + MCP verbs + `lb-jobs`/run-store reuse. No wasm/native
  ABI change. (The `[[node]]` manifest block is the spine's, not this doc's.)

## Fan-out posture (the genuine risk)

A **chatty source** spawning one run per event could storm the queue (the build-time risk the
spine flags). Two disciplines, both reused, never new machinery:

- **Fire-once-then-skip** — carry the **reminders** discipline: a coalesced trigger fires one run
  and skips redundant re-fires within the window, rather than enqueuing a run per tick.
- **Coalesce config** on the **event-trigger node** — this doc is the **canonical** place the
  coalesce vocabulary is defined. One enum, used everywhere a stream is throttled:

  ```
  coalesce: { strategy: latest | leading | trailing | sample, window_ms }
  ```

  (`latest` = latest-wins within the window; `leading` = fire on the first edge then suppress;
  `trailing` = fire once at window end; `sample` = fire at most once per `window_ms`.)
  `triggers-lifecycle-scope.md` (the event trigger) and `dashboard-binding-scope.md`
  **reference this definition** rather than redefining it. The trigger node itself is defined in
  `triggers-lifecycle-scope.md` (link); this doc owns only the run-side consequence: the queue is
  protected at the trigger, before a run is created.

## Example flow

A 4-node flow `A → B → C → D` (linear, one Tool node each), run by `acme`.

1. **Run.** `flows.run {id, params}` validates the pinned graph (it validated at save —
   re-checked cheaply), pins `flow_version = 7` into `flow_run:acme:R1` (Pending), seeds the
   in-degree-0 frontier (`A`), and **returns `{run_id: R1}` immediately**. `flow_run:acme:R1` is
   the coordinator; the driver enqueues a `flow-step` job for `A`.
2. **A, B run.** `A`'s job claims `A` (CAS `Enqueued→Running` on `flow_step_output:acme:R1:A`),
   runs its Tool (gated under `caller ∩ grant`), records `Ok` + output, `on_step_done` releases
   `B` and the driver enqueues `B`'s job. `B` runs the same way. `C` is the next frontier. The
   canvas (over `flows.watch` SSE) shows `A`,`B` green, `C` enqueued.
3. **Suspend.** An operator calls `flows.suspend {run_id: R1}` (`lb-jobs::suspend`). The
   coordinator stops enqueuing the next frontier (the unexecuted `C`); `flow_run` stays open.
4. **Config patch.** The operator realises `D`'s Tool needs a different argument. `D` is
   **unexecuted**, so `flows.patch_run {run_id: R1, node: D, config: {…}}` is accepted — a
   config-only patch to the pinned run R1 (the flow record itself is untouched).
5. **Resume — exactly-once.** `flows.resume {run_id: R1}` (`lb-jobs::unsuspend`). The engine
   validates the next-frontier node `C` still exists with the same type + ports (it does — R1's
   graph is pinned). `C` then `D` run; `D` uses the patched config. Had a duplicate redelivery hit,
   `C`'s CAS claim would no-op it (cross-node exactly-once). `flow_run` finalizes `Success`.
6. **Variant — structural edit during the suspend.** While R1 is suspended, an author opens the
   flow and **deletes `C`**. This is **structural**, so it is **rejected as a patch to the live
   run** and instead **writes flow `version 8`**. R1 keeps its pinned `version 7` and **finishes
   on it** unaffected; the *next* `flows.run` uses `version 8`. (Had the author somehow forced a
   resume against a drifted graph, the next-frontier validation would fail the run cleanly as
   `ResumePointDrift`, surfaced in `flows.runs.get`.)
7. **Variant — crash mid-step.** Suppose the node crashes while `C` is `Running`. On reboot
   `lb-jobs` resumes the coordinator and re-attempts `C`'s `flow-step` job: its CAS claim is
   re-attempted — if `C`'s effect already committed (claim already `Running`/done), the re-attempt
   **no-ops** and the run moves on; if not, `C` re-runs once (and were `C` itself a multi-turn job,
   its own `append_event` turns re-apply idempotently). Either way the result is **exactly-once** —
   no double effect, and any must-deliver effect already in the outbox **relays on reconnect**
   (at-least-once + dedup).

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`) — the gate. **No mocks**: the **real
`lb-jobs` queue**, real store (`mem://`), real caps, a real bus subject for `watch`, the **real
outbox**, and the **real gateway** for the SSE feed; flows seeded as real `flow` records. The
only sanctioned fake is a true external behind one extension trait (the MQTT broker / a provider
HTTP API).

- **Capability-deny — the no-widening matrix (§2.1):** each `flows.<verb>` denied without its cap;
  a run whose Tool node calls a tool the **caller** lacks → **that node denied, the run records
  the deny, no widening** (holding `flows.run` alone reaches nothing extra); the run then behaves
  per `FailurePolicy` (Halt prunes subtree / Continue passes `null`).
- **Workspace-isolation (§2.2):** ws-B cannot run / get / list / watch / suspend / resume /
  cancel / patch a ws-A run; a ws-B caller cannot read ws-A's `flow_run` / `flow_step_output` /
  `flow_input`; `flows.runs.list {flow_id}` never returns another ws's runs; the run job's
  `workspace` is un-spoofable — proven **across store AND MCP**.
- **Offline / sync — the headline (reuses the `lb-jobs` resume shape, §2.3):** kill the node
  mid-run across a disconnect → on restart the run **resumes and finishes exactly once**; assert
  **both** exactly-once layers ([Decision 8](flows-scope.md)) — a re-applied node is a no-op via
  the **CAS claim** (cross-node), and a multi-turn node's replay is a no-op via idempotent
  **`append_event`** (within-step); a must-deliver effect in the outbox **relays on reconnect**
  at-least-once, idempotently.
- **Validate-at-save (port `chains` `validate` verbatim):** **cycle** (Kahn) / **dangling edge**
  / **duplicate id** / **self-edge** / **size cap** each rejected **before any node runs**.
- **Diamond frontier:** a valid fan-out + fan-in schedules in dependency order; fan-in fires only
  when in-degree hits 0; **independent branches run concurrently as separate `flow-step` jobs
  competing for the `lb-jobs` semaphore** (not async tasks inside one worker) — assert genuine
  job-level concurrency, [Decision 8](flows-scope.md).
- **Halt subtree-skip:** under `Halt`, a failed node's transitive subtree is `Skipped` while
  independent branches finish (run = `PartialFailure`); under `Continue`, dependents release with
  the failed output as `null`.
- **Suspend → patch_run(config) → resume exactly-once:** the suspend stops the coordinator
  enqueuing the next frontier; a
  config-only patch to an **unexecuted** node is accepted; resume runs the remaining frontier once
  with the patched config; a re-delivery no-ops.
- **Structural-edit-during-suspend → new version:** a structural edit during a suspended run is
  rejected as a live-run patch, writes a **new flow version**, and the **live run finishes on its
  pinned version**; the next run uses the new version.
- **ResumePointDrift:** a resume whose next-frontier node no longer matches type + ports fails the
  run cleanly with `ResumePointDrift`, surfaced in `flows.runs.get`.
- **Subflow parks on child run ([Decision 11](flows-scope.md)):** a `subflow` node enqueues a
  pinned child `flow_run` and the parent step **parks** until the child reaches terminal, then maps
  child outputs → parent ports; a **child failure** drives the parent node `Outcome::Err` under the
  parent's `FailurePolicy`; a **parent suspend cascades** to the child; the frontier driver resumes
  the parent on child-run terminal.
- **Reattach (the `runs.list` surface):** `flows.runs.list {flow_id, status?}`
  returns a flow's runs (status-filtered); a reopened canvas holding only `flow_id` finds the
  **active `run_id`** and reattaches `flows.watch` to the live stream.
- **Patch against pinned schema ([Decision 12](flows-scope.md)):** a `patch_run` on a live run
  pinned to an old version is validated against the **pinned** node schema, accepting the pinned
  form's fields and rejecting a field only a newer descriptor introduced.
- **Live feed (real gateway):** a `flows.watch` `*.gateway.test.tsx` shows a late watcher get a
  snapshot rebuilt from the run records, then live node-status transitions — against a real
  spawned node.

## Risks & hard problems

- **Resume-point drift UX.** `ResumePointDrift` must be *rare and obvious*, not a surprise — the
  editor's draft-vs-pinned distinction (Decision 1; `flows-canvas-scope.md`) is what keeps it so;
  the run-side contract here is fail-clean + surface in `runs.get`, never silent mis-execution.
- **High-frequency fan-out.** The genuine load risk; bounded at the trigger (the `coalesce` enum +
  fire-once-then-skip) **before** a job is created — see the fan-out posture above.
- **Patch_run scope creep.** "Config-only to an unexecuted node" is the hard line; anything
  structural is a new version, never an in-place mutation of a live run. Enforced server-side, not
  trusted to the caller.
- **One responsibility per file (FILE-LAYOUT).** The implied code is a folder-of-verbs:
  `run.rs` (create the coordinator), `runs_get.rs`, `runs_list.rs`, `watch.rs`, `suspend.rs`,
  `resume.rs`, `cancel.rs`, `patch_run.rs` for the MCP surface; `frontier.rs` (ported driver),
  `enqueue_node.rs` (one `flow-step` job per node), `claim.rs` (CAS), `subflow_park.rs` (the
  wait-on-child-run reactor), and a `run_store/` (`flow_run.rs`, `flow_step_output.rs`,
  `flow_input.rs`) for the store — never a `flow_run_utils.rs`.

## Related

- `flows-scope.md` — the spine; **Decisions (v1) 1–13** (this doc owns the run-side of 1, 6, 7, 8,
  9, 11, 12).
- `node-descriptor-scope.md` — the node model + config schema a step runs.
- `extension-nodes-scope.md` — extension-node execution + the install-grant narrowing (deny
  matrix case (b)).
- `triggers-lifecycle-scope.md` — the event-trigger node; **references this doc's `coalesce` enum**
  + the retained-inject mechanics ([Decision 9](flows-scope.md)).
- `dashboard-binding-scope.md` — the dashboard read/write bridge; **references this doc's `coalesce`
  enum** for control debounce.
- `../rules/rule-chains-scope.md` — the frontier driver + run-store this ports verbatim
  (`chain_run`/`chain_step_output` → `flow_run`/`flow_step_output`).
- `../jobs/jobs-scope.md` — `lb-jobs` (the job a run is): `create`/`load`/`append_event`/
  `suspend`/`unsuspend`/`complete`/`cancel`; the resume/idempotency this inherits.
- `../inbox-outbox/outbox-scope.md` — the outbox a must-deliver node effect routes through.
- README `§6.9` (jobs), `§6.10` (inbox/outbox), `§6.13` (the three gates / gateway SSE),
  `§6.5` (MCP), `§3` (rules 1/3/5/6/7).
</content>
</invoke>
