# Session — jobs: indexed drain scan + bounded terminal retention

Status: **done** (2026-07-03). Scope: [`scope/jobs/job-retention-scope.md`](../../scope/jobs/job-retention-scope.md).
Fixes debug entry: [`debugging/jobs/node-pegs-cpu-reactor-rescans-job-table.md`](../../debugging/jobs/node-pegs-cpu-reactor-rescans-job-table.md).
Public: [`public/jobs/jobs.md`](../../public/jobs/jobs.md) → "Drain scan + retention".

## The ask

A long-lived node pegs a CPU core re-scanning its own `job` table: the agent reactor (2 s) and flow
reactor (5 s) both call `lb_jobs::pending`, which **walked every page** of `job` and filtered in Rust.
Terminal rows (`done`/`failed`/`cancelled`) accumulate forever — plus the `flow_run` / `flow_step_output`
rows the demo flow mints — so one `pending` pass grows past the tick and the reactors scan back-to-back.

Two independent changes: (1) make `pending` an **indexed** O(pending) query; (2) **bounded retention**
for terminal `job` / `flow_run` / `flow_step_output` rows.

## What I built

### 1. Indexed drain scan (`pending` is O(pending), not O(table))

- New [`crates/jobs/src/schema.rs`](../../../rust/crates/jobs/src/schema.rs) — `define_job_index`, a
  `DEFINE INDEX IF NOT EXISTS job_kind_status ON TABLE job COLUMNS data.kind, data.status`. Field path is
  `data.kind`/`data.status` (not `kind`/`status`): every store write nests the host body under `data`
  (`lb_store::record`), so the index must target the stored path or SurrealDB silently scans.
- Ensured **lazily, per-namespace, idempotent** — called at the top of `create` (first-touch), matching
  the prefs/tags convention. There is **no** global boot-time schema pass in this codebase, and a
  `DEFINE` runs in whichever namespace `query_ws` selected, so per-ws first-touch is the correct hook.
- Rewrote [`pending.rs`](../../../rust/crates/jobs/src/pending.rs): the paged `scan` + Rust filter →
  one `SELECT data FROM job WHERE data.kind = $kind AND data.status IN ['running','suspended']`. The
  resumable statuses are the kebab-case serde values. This is **strictly safer** than the paged walk on
  the first-page-only property it guarded (no pages to fall off — the index returns every match direct).
  A `LIMIT 10_000` self-protection backstop stays, never approached now that retention bounds the table.

### 2. Bounded retention

- **`job`** — [`crates/jobs/src/retain.rs`](../../../rust/crates/jobs/src/retain.rs) `retain_terminal`:
  trims the terminal set to the newest `cap` per ws. Predicate is `data.status IN (done|failed|cancelled)`
  **and nothing else** — a `Running`/`Suspended` job outside the window is kept forever (the one
  unacceptable failure is trimming a resumable job → double-run). Reuses `capped.rs`'s safe-delete idiom
  (`LET $keep = (SELECT … LIMIT n); DELETE … WHERE … NOT IN $keep`), never the inline `NOT IN (subquery)`
  form SurrealDB mis-evaluates.
- **`flow_run` + `flow_step_output`** —
  [`crates/host/src/flows/retain_runs.rs`](../../../rust/crates/host/src/flows/retain_runs.rs)
  `retain_runs`: trims finished runs (`success`/`partialFailure`/`failed`/`cancelled`; never
  `pending`/`suspended`) to the newest `cap`, and **deletes the step rows of every purged run in tandem**
  (keyed on `data.run_id`) — the step table is ~2× the runs and the real disk bulk.
- **Sweep wiring** —
  [`crates/host/src/flows/retention_sweep.rs`](../../../rust/crates/host/src/flows/retention_sweep.rs)
  runs both trims on the flow reactor tick, **throttled** to every 30th tick (~2.5 min at the 5 s
  cadence; fires on tick 0 so a freshly-booted node reclaims a bloated store immediately). Errors
  logged, never fatal; ws-scoped (the hard wall).
- **Immediate dev relief** — `make purge-store` (wipes `.lazybones/data/dev-store` only, no rebuild, keys
  and extensions untouched).

## Open questions — resolved (recorded in the scope doc)

- **Retention placement:** *option (b) reactor-tick sweep for all three tables.* `job` reaches terminal
  through **two** verbs (`complete`, `cancel`) — no single chokepoint for option (a). `flow_run` *does*
  have a single chokepoint (`set_run_status`), but its `flow_step_output` rows are keyed `{run_id}:{node}`
  and written by a different verb, so a transactional trim at the run transition **cannot reach** the
  step rows (the real bulk); they must be purged in tandem, keyed by the purged run ids — which only a
  sweep does. So (b) uniformly, for concrete per-table reasons, not convenience.
- **Bound shape / default:** *count-bounded per workspace.* Default **500** terminal jobs and **500**
  finished runs per ws — generous so ordinary run history the flow UI shows isn't lost; the goal is
  bounding runaway growth, not aggressive GC.
- **Where config lives:** *a compiled caller-owned default* (`DEFAULT_TERMINAL_JOB_CAP`,
  `DEFAULT_FINISHED_RUN_CAP`), per `capped.rs`'s "defaults live in the caller". **Not** a prefs key: prefs
  here is a **closed typed-axis** system (language/timezone/…) with no numeric key→value getter, so there
  is nowhere to resolve a retention number from today. An operator override would slot in at the constant.
- **Manual purge verb:** *deferred, nothing reserved.* Retention is a raw node-internal verb like every
  `lb-jobs` verb; there is no user-facing MCP surface (asserted by construction — no `mcp:jobs.*`). If a
  manual "purge now" admin verb is ever wanted, *that* gets a cap; not built, no name reserved.
- **Immediate dev relief:** *shipped* as `make purge-store`.

## Tests (all real infra, `mem://` store, rule 9 — no mocks)

New: [`crates/jobs/tests/retain_test.rs`](../../../rust/crates/jobs/tests/retain_test.rs) (4),
[`crates/host/tests/flows_retention_test.rs`](../../../rust/crates/host/tests/flows_retention_test.rs) (2).
Existing `pending_test.rs` (2) still green against the rewritten indexed `pending`.

- **Performance/cost (the regression):** seed 5,000 terminal jobs + 2 resumable (one sorting before all
  terminal, one after) → `pending` returns exactly the 2 resumable; an index-backed `count()` over the
  drain predicate equals 2 out of 5,002; `INFO FOR TABLE job` confirms the `job_kind_status` index
  exists. Deterministic — measures the query's own DB-side filter, not wall-clock.
- **Never-trim-resumable:** a Running + a Suspended job with the LOWEST ids survive a trim to cap 5 that
  evicts 45 of 50 terminal — the load-bearing safety property.
- **Bound respected / newest kept:** trim to 3 leaves exactly the 3 newest terminal ids.
- **Workspace isolation:** a ws-B trim leaves every ws-A row intact (job **and** flow-run variants).
- **Flow-run + step tandem:** trimming 30 runs to 5 deletes 25 runs *and* their 50 step rows; the 2
  live (`pending`/`suspended`) runs and their steps survive.

### Green output

```
cargo test -p lb-jobs
  pending_test:  2 passed
  resume_test:   6 passed
  retain_test:   4 passed   (pending_is_indexed_and_returns_only_resumable_at_scale,
                             retention_never_trims_a_resumable_job,
                             retention_keeps_the_newest_terminal_rows,
                             retention_is_workspace_scoped)

cargo test -p lb-host --test flows_retention_test
  retain_runs_never_trims_a_live_run_and_trims_step_rows ... ok
  retain_runs_is_workspace_scoped ... ok
  test result: ok. 2 passed
```

Full `cargo test --workspace --no-fail-fast`: green except the three **pre-existing** master failures
unrelated to this change (`agent_routed_test::an_edge_invokes_the_hub_agent_over_the_routed_namespace`
— "no in-house model configured"; `SystemView.gateway`, `sqlSource.gateway`). `cargo build --workspace`
+ `cargo fmt` clean. No UI touched (`pnpm test` not required).

## Files

New: `jobs/src/schema.rs`, `jobs/src/retain.rs`, `host/src/flows/retain_runs.rs`,
`host/src/flows/retention_sweep.rs`, `jobs/tests/retain_test.rs`, `host/tests/flows_retention_test.rs`.
Changed: `jobs/src/pending.rs` (indexed), `jobs/src/create.rs` (first-touch index), `jobs/src/lib.rs`,
`host/src/flows/mod.rs`, `host/src/flows/reactor_loop.rs` (sweep wiring), `host/src/lib.rs` (test seam),
`Makefile` (`purge-store`).
