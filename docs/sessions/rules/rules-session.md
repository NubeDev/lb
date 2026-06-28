# Rules engine + rule chains — session

- Date: 2026-06-28
- Scope: ../../scope/rules/rules-engine-scope.md + ../../scope/rules/rule-chains-scope.md
  (+ ../../scope/datasources/datasources-scope.md, built the same session — see
  ../datasources/datasources-session.md)
- Stage: post-S8 platform capability (rules plane); see STATUS.md
- Status: done (rules + chains green; federation a sibling session)

## Goal

Build the **lazybones-native rules engine** — a sandboxed Rhai script a workspace authors and runs
that reads data, transforms it (a lazy column-oriented `Grid` + timeseries plan-builders), calls AI
(metered + fenced), and emits findings/alerts — exposed as `rules.*` MCP verbs; and **rule chains**,
a DAG of saved rules driven over `lb-jobs` + SurrealDB, exposed as `chains.*`. Both ported from the
working `rubix-cube` engine, **re-seamed onto the lazybones chokepoints**.

## Source / attribution (MIT/Apache-2.0)

Ported from `rust/rubix-cube/rbx-server/src/rules/` (read 2026-06-28), MIT/Apache-2.0, same repo
lineage. The attribution is recorded crate-level in `crates/rules/src/lib.rs` and per-file in each
ported module's header.

**Lifted verbatim:** the rhai sandbox + governors (`sandbox.rs` ← `rules/sandbox.rs`), the lazy
`Grid`/`Col`/`GroupedGrid` plan model (`grid.rs` ← `rules/grid.rs`), the timeseries plan-builders
(`verbs/timeseries.rs`), the `AiMeter` budget (`meter.rs` ← `rules/ai.rs`), the nsql re-validation
fence (in `verbs/ai.rs`), the DAG model + `validate` (Kahn) + binding resolver
(`workflow/model.rs`+`workflow/context.rs` ← `rules/workflow/`).

**Re-seamed (the three boundaries the scope named):**
1. **Grid `collect`** — rubix-cube collected through a local DataFusion engine; lb-rules calls the
   host `DataSeam` trait → `store.query`/`series.*` (platform, SurrealDB) or `federation.query`
   (external). No DataFusion in core (`lb-rules` links only rhai + serde).
2. **`ai.*`** — re-points rubix-cube's `AiBackend` at the host `AiSeam` (the AI-gateway / a
   deterministic test model). The budget meter + the nsql fence transfer exactly.
3. **`alert`** — resolves rubix-cube's stage-03 TODO: an `alert` finding raises a real **inbox** item
   and routes a must-deliver notification through the **outbox** (host-side, after the run).

**Re-keyed:** `project_id` → `workspace` throughout; `allowed_datasets` → the workspace's granted
sources (host-resolved). `Workflow` → `Chain`. The actix `POST /rules/run` + workflow routes →
`rules.*` / `chains.*` MCP verbs (rule 7). The `InMemoryRunStore`/`JobQueue`/cron-thread are
**dropped** — replaced by `lb-jobs` + a SurrealDB run-store behind rubix-cube's trait shape (rule 1).

## What changed

### `crates/rules` — the pure engine library (rhai + serde only)
- `sandbox.rs` — `RuleLimits` + `build_engine`: every governor set, `eval`/`import` stripped,
  wall-clock deadline via `on_progress`. The cage = "absence of capability + presence of limits".
- `grid.rs` — the lazy `Grid` carrying `(SourceKind, source, sql, ctx)`; plan-builders compose by
  wrapping in a subquery; `collect` calls the host data seam. A platform grid composes **SurrealQL**,
  a federation grid **ANSI SQL** — one author surface, two dialects.
- `seam.rs` — `DataSeam` (resolve/collect/schemas) + `AiSeam` (complete/propose_sql/embed) + the
  `SourceKind` (Platform | Federation) split. The host implements these.
- `verbs/` — `data` (`source`/`query`/`history`/`span`/`last`/`param`), `timeseries`
  (`rollup`/`lag`/`delta`/`rate`/…, dialect-aware), `emit` (`emit`/`alert`/`log`), `ai`
  (`ask`/`complete`/`classify`/`embed`) with the budget + fence.
- `engine.rs` — `RuleEngine::run`: fresh engine per run, register verbs, eval on the caller's thread
  (the host runs it on a blocking thread), drain collectors, classify `RuleOutput`.
- `workflow/` — `Chain`/`Step`/`Trigger`/`FailurePolicy` + `validate` (Kahn) + `RunContext`
  binding/result logic (whole-value `${...}` references only). Pure DAG math; the durable backend is
  the host's.

### `crates/caps` — `Net` surface + `Connect` action (grammar addition)
The datasources sibling needs `net:tls/host/port:connect`. Added `Surface::Net` + `Action::Connect`
to the grammar (additive; existing wildcard/segment tests still green).

### `crates/secrets` — capability-mediated secret store (was an S0 placeholder)
`lb_secrets::get`/`set` over `secret:{ws}:{path}` records, gated by the `Secret` surface. Mediated
(the value is for the host/supervisor, never a caller surface). NOTE: envelope-encryption-at-rest is
its own dedicated stage; this lands the capability-mediation + workspace-walled storage the
datasources DSN needs.

### `crates/host/src/rules` — the `rules.*` service (the re-seam, wired)
- `seam.rs` — `HostDataSeam` (resolve series/store→Platform, a registered `datasource:{ws}:{name}`→
  Federation; collect via `store_query_run` or the `federation.query` MCP verb, `block_on` from the
  blocking rule thread) + `HostAiSeam` over a `RuleModel` (the AI-gateway / test model — the one
  sanctioned external-behind-a-trait, testing §0).
- `run.rs` — `rules.run`: build seams pinned to (caller, ws), run on `spawn_blocking`, route `alert`
  findings to inbox + outbox.
- `save`/`get`/`list`/`delete` — CRUD over `rule:{ws}:{id}` (tombstone delete, idempotent).
- `mod.rs` — `call_rules_tool` bridge; `config.rs` — limits from `LB_RULES_*` env.

### `crates/host/src/chains` — the `chains.*` service (DAG over lb-jobs + SurrealDB)
- `record.rs` — `chain:{ws}:{id}` (DAG), `chain_run:{ws}:{run_id}` (lifecycle),
  `chain_step:{ws}:{run_id}:{step_id}` (per-step CAS claim + outcome + output — per-step rows so
  concurrent writes don't contend).
- `run_store.rs` — the durable backend behind rubix-cube's trait shape: `create_run`, `claim_step`
  (CAS `Pending|Enqueued→Running`, the idempotency guard), `record_outcome`, `ready_dependents`,
  `skip_subtree`, `finalize_if_complete`, `resolve_bindings`.
- `coordinator.rs` — `start` (seed + frontier) + `drive` (claim → resolve bindings → run the saved
  rule via `lb-rules` → record → release dependents / Halt-prune → finalize). Resumable: a re-drive
  reads the durable state and the CAS claim makes a redelivered step a no-op.
- `run.rs` — `chains.run` (create an `lb-jobs` `Job` of kind `chain`, start, drive) + `chains.resume`
  (re-drive from durable state).
- `save`/`get`/`delete` + `chains.runs.get` (rebuild the DAG-canvas snapshot from the records).

### Dispatch wiring
`tool_call.rs`: `rules.` + `chains.` added to `is_host_native` + a dispatch branch each; crate-root
`pub use` in `lib.rs`. `Node::boot_with_store` added (a clean restart-test seam — re-open the same
on-disk store).

## Decisions (with the alternative rejected)

- **Platform grids compose SurrealQL, not DataFusion SQL.** The scope says "re-target the collect
  path." Platform data is SurrealDB-native and `store.query` is SurrealQL, so the platform Grid emits
  SurrealQL (`time::group` bucketing, `math::*` reductions) and `rollup` is dialect-aware. *Rejected:*
  keeping DataFusion SQL and translating — that would re-introduce the federation engine into the
  platform path and fork the authority (rule 2).
- **Platform `history()` drops the `now()-window` filter.** A committed series carries a LOGICAL `ts`
  (a sample timestamp, not wall-clock), so a `now()-window` filter is both nondeterministic (testing
  §3) and semantically wrong for replayed/seeded data. `history` returns the ordered series and the
  author filters the window explicitly. The federation path keeps the wall-clock window (its `ts` is
  a real timestamp). *Recorded* so it isn't read as an omission.
- **The committed series numeric is `payload`, normalized to `value`.** The series row stores the
  scalar under `payload` (ingest scope); `history` selects `payload AS value` so the timeseries verbs
  (which speak `value`) compose uniformly across platform + federation.
- **A chain is driven inline-but-durably, not by a worker pool.** `lb-jobs` is a durable resumable
  session, not a multi-worker queue (the queue is deferred there). So `chains.run` creates the job
  record + drives the frontier to completion, with each step's CAS claim + recorded output making a
  restart-`resume` exactly-once. *Rejected:* porting rubix-cube's `JobQueue` + cron-thread — that
  duplicates `lb-jobs` and re-opens the durability gap (rule 1). The trait shape is kept; the backend
  is ours.
- **`ai.classify` returns the source grid in v1 (documented limitation).** The label-join back into a
  lazy grid needs a literal-rows source the platform doesn't expose yet; `classify` still charges the
  budget + bounds context (the tested invariants). A literal-rows grid is additive later.
- **A default `DisabledModel` for the bridge path.** A `rules.run`/`chains.run` reached over the
  generic MCP bridge has no model wired, so `ai.*` errors clearly ("AI not configured") — rubix-cube's
  posture. A role that wires the gateway, and the tests, inject a real `RuleModel`.

## Tests (all green — see output below)

### `crates/rules` unit (28) — `cargo test -p lb-rules`
- **cage** (`cage_test.rs`): `eval`/`import` rejected; `max_operations` trips an infinite loop;
  wall-clock deadline aborts a slow run; oversized array rejected.
- **grid** (`grid_test.rs`): filter→WHERE subquery; rollup→`time::group`+`math::max`; `col.max`→
  scalar; returned grid materializes; emit→Findings; alert marked; empty→Nothing.
- **AI fence + budget** (`ai_fence_test.rs`): a malicious proposed `SELECT … payroll` is re-validated
  through the collect gate and rejected (the fence — never reached execution); `AiMeter` caps calls +
  summed tokens; an ungranted `source()` denied mid-run.
- **DAG + bindings** (`dag_test.rs`): diamond valid + frontier; cycle/dangling/dup/self-edge/size cap
  rejected; `${params.x}`/`${steps.x.output}`/`${steps.x.findings}` resolve by value; embedded `${`
  is a literal; failed upstream → null.
- duration parser unit tests.

### `crates/host` integration — `cargo test -p lb-host --test rules_test --test chains_test`
- **rules (6):** each verb denied without its cap; a rule reading an ungranted source denied mid-run;
  the full e2e (seed real series via ingest+commit → run a rollup+`alert` rule → the alert raises a
  real inbox item); ws-B cannot get a ws-A saved rule; AI budget caps a loop; a saved rule survives a
  node restart (persistent store re-open).
- **chains (6):** each verb denied; a cyclic DAG rejected at save; a diamond runs all steps to
  success in order; **Halt** skips the failed step's subtree (PartialFailure); ws-B cannot run a ws-A
  chain; a run **resumes exactly once after a restart** (re-open store + `chains.resume`, no
  double-run).

### `crates/caps` + `crates/secrets` — grammar unchanged-green + secret mediation/isolation/deny.

### Green output (2026-06-28)

```
$ cargo test -p lb-rules
running 2 tests   (duration unit) ............ ok. 2 passed
running 4 tests   (ai_fence_test) ............ ok. 4 passed
running 5 tests   (cage_test) ................ ok. 5 passed
running 10 tests  (dag_test) ................. ok. 10 passed
running 7 tests   (grid_test) ............... ok. 7 passed
  -> 28 lb-rules tests, 0 failed

$ cargo test -p lb-host --test rules_test --test chains_test
test each_chains_verb_is_denied_without_its_cap ... ok
test save_rejects_a_cyclic_dag_before_any_run ... ok
test ws_b_cannot_run_a_ws_a_chain ... ok
test halt_skips_the_subtree_of_a_failure ... ok
test diamond_runs_all_steps_to_success ... ok
test run_resumes_exactly_once_after_restart ... ok
  chains: ok. 6 passed; 0 failed
test each_rules_verb_is_denied_without_its_cap ... ok
test ai_budget_caps_a_loop ... ok
test rule_reading_an_ungranted_source_is_denied_mid_run ... ok
test ws_b_cannot_get_a_ws_a_saved_rule ... ok
test run_rollup_alert_rule_raises_inbox_item ... ok
test saved_rule_survives_a_restart ... ok
  rules: ok. 6 passed; 0 failed

$ cargo test -p lb-caps -p lb-secrets   # grammar + secret mediation/deny/isolation -> all green
$ cargo build --workspace               # Finished — green
```

## Follow-ups / open

- `chains.watch` SSE (the live DAG-canvas feed) + Cron/Event triggers (the S6 reactor + `bus.watch`)
  are wired in the run-store/coordinator design but the live-feed transport + the reactor tick are a
  thin additive slice — `chains.runs.get` gives the snapshot today. Tracked in the scope.
- `ai.classify` label-join (above).
- A Playground `*.gateway.test.tsx` (frontend) — the backend e2e proves the path; the UI page is the
  remaining frontend slice.
