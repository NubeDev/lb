# Session — rules 10x: long-running job-backed runs (pause/resume) + the data stdlib

Status: done. Date: 2026-07-15. Shipped on `master` via the [#70](https://github.com/NubeDev/lb/pull/70)
merge (`2e38abc`) — the slice was authored on `feat/store-online-compaction-67` and rode that branch in.

Scopes built:
- `docs/scope/rules/long-running-rules-scope.md` (written this session — the ask for job-backed
  runs, checkpoints, suspend/resume/cancel)
- `docs/scope/rules/data-stdlib-scope.md` (pre-existing — Phase 1 pure families + Phase 2 polars
  `Frame`, previously scaffolding-only)

## The ask (user)

"10x the rules system: longer running jobs, pausing a job and then resume, more helpers,
improvements for data science / dataframes."

## What existed before

- `rules.run`/`rules.eval` synchronous only, 10 s / 5 M-op governors; no background form.
- Pause/resume existed for **flows** (`flows.suspend/resume`) and agent jobs (`lb-jobs`
  `suspend/unsuspend`) but a single rule body was un-pausable and un-resumable.
- The data stdlib was designed (~180 fns in the scope) but **unbuilt**: `lb-frame` was Phase 0
  (JSON boundary + limits type, zero rhai surface); no `time`/`json`/`stats`/`mathx` families.

## Design decisions made this session

- **Resume = replay over checkpoints, never VM snapshotting.** Pause aborts the eval at a governor
  tick; resume re-runs the body with persisted `job.step`/`job.set` state folded back in. Safe
  because messaging writes are deterministic-id upserts (rules-messaging contract) — replayed
  effects land on the same ids. Rejected: snapshotting a live rhai VM (dishonest/impossible).
- **Checkpoints ride the `lb-jobs` transcript** via two additive `#[non_exhaustive]`
  `TranscriptEvent` variants: `Checkpoint {key, value(JSON string)}`, `Progress {pct?, msg}`.
- **Cooperative control**: `RunControl` (AtomicU8) shared between the cage's `on_progress`
  governor and the host `rules.runs.suspend/cancel` verbs. Cancel outranks pause. Typed abort
  tokens map to `RuleError::Paused/Cancelled`.
- **Owner verbs (`rules.runs.*`), not raw `jobs.*`** — honors job-control-scope's chokepoint rule.
- **No auto-resume of orphans** (would need a persisted principal — refused); orphans are
  `live:false` and caller-resumable under the resumer's caps.
- **One `job` handle in every run** — durable when job-backed, ephemeral in sync runs; one body
  works in both modes.
- **`register()` refactored to a `RunWiring` struct** (the positional list outgrew itself when the
  stdlib + job handle landed).
- **Catalog** became `LazyLock<Vec<FnEntry>>` chaining per-family consts defined beside their
  `register_fn` sites (keeps catalog.rs from blowing the 400-line ceiling as ~180 entries land).
- **rhai `timestamp()` disabled** in `build_engine` (shadowed with an author error) — the
  data-stdlib determinism contract.
- **`RuleLimits` gained `max_frame_rows`/`max_frame_cells`** (200k/2M defaults) — the polars bound
  moves to inputs because the deadline can't interrupt a native call.

## Work log

- Explored current state (3 parallel read agents): rules crate map, jobs/flows map, frame map.
- Wrote `long-running-rules-scope.md`.
- lb-rules shared wiring: `control.rs` (RunControl), sandbox control + timestamp kill + frame
  limits, `seam.rs` `JobSeam`, `verbs/job.rs` (the handle, full), stubs for
  `time/json/stats/window/mathx/frame`, `verbs/mod.rs` RunWiring rework, `engine.rs`
  `run_with(RunOptions)`, `runtime.rs` `Paused/Cancelled`, catalog restructure, chrono dep.
- lb-jobs: added `Checkpoint`/`Progress` transcript variants (additive).

- lb-jobs: `list_kind` (kind-scoped observe read, terminal rows included — `pending` stays the
  reactor drain).
- Host slice B (`host/src/rules/runs/`, one verb per file): `start.rs` (`rules.run_async` — seed
  job synchronously, spawn named drive task; the `flows_run_async` pattern), `worker.rs` (drive one
  eval under the job governor profile; settle Done/Suspended/Cancelled/Failed; route alerts on
  success only — replay makes that exactly-once), `seam.rs` (`HostJobSeam` over the transcript),
  `registry.rs` (`RuleRunMap` on `Node`, the `sidecars` precedent), `payload.rs` (pinned
  body/params/now/route + checkpoint fold), `get/list/suspend/resume/cancel.rs`.
- Host config: job governor profile (`LB_RULES_JOB_TIMEOUT_MS` 10 min, `LB_RULES_JOB_MAX_OPERATIONS`
  500 M, `LB_RULES_JOB_AI_MAX_CALLS` 64, `LB_RULES_JOB_AI_MAX_TOKENS` 200 k,
  `LB_RULES_JOB_MAX_WRITES` 256).
- Dispatch arms + system catalog rows for the six verbs; member built-in role gains the six caps.
- Tests written: `crates/rules/tests/longrun_test.rs` (cage: handle modes, memoize/replay, budgets,
  pause/cancel mapping) and `crates/host/tests/rules_longrun_test.rs` (cap-deny per verb, read ≠
  control, ws-isolation, suspend-mid-run→resume without re-spend + exactly-once channel post,
  cooperative cancel + D2, progress/result shapes, restart-resume over a disk store).
- Delegated the four stdlib families to parallel agents (time+duration, json+mathx, stats+window,
  polars Frame) with the shared wiring pre-stubbed so their files stay disjoint.

- Delegated the four stdlib families to parallel agents (time+duration, json+mathx, stats+window,
  polars Frame) with the shared wiring pre-stubbed so their files stay disjoint.
- **Close-out pass (post-merge, on `master`)**: re-verified every claim below by running the suites
  rather than trusting the handover; removed two dead `pub use RULE_RUN_KIND` re-exports (the last
  warnings in `lb-host`); wrote the STATUS.md entry.

## Test output

All re-run on `master` at close-out (not copied from the working branch). `cargo fmt --check` clean
for every file this slice touched.

### `lb-host` — the headline suite

```
$ cargo test -p lb-host --test rules_longrun_test
running 9 tests
test each_runs_verb_is_denied_without_its_cap ... ok
test read_caps_do_not_grant_control ... ok
test cancel_bites_mid_run_and_is_idempotent ... ok
test cancel_of_a_suspended_run_works ... ok
test a_failing_body_settles_failed_with_the_error_recorded ... ok
test progress_and_result_surface_in_get_and_list ... ok
test ws_b_cannot_see_or_control_a_ws_a_run ... ok
test suspend_mid_run_then_resume_finishes_without_respending_steps ... ok
test suspended_run_resumes_after_a_restart ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.66s
```

Rule-9 mandatory coverage is in that list: capability-deny (`each_runs_verb_is_denied_without_its_cap`,
`read_caps_do_not_grant_control`) and workspace-isolation (`ws_b_cannot_see_or_control_a_ws_a_run`).

### The rest

| suite | result |
|---|---|
| `lb-rules` (`--features frames`) | **80 lib + 89 integration**, all green |
| `lb-frame` | **53** across 9 files (incl. `sql_security_test` — keep it) |
| `lb-host` `rules_test` | 22 |
| `lb-host` `rules_ai_wiring_test` | 8 |
| `lb-host` `rules_workflow_convergence_test` | 14 |
| `lb-host` `rules_buildings_examples_test` | 1 |

`lb-host` rules-adjacent total **54**, no regressions.

### Flake seen at close-out (not a regression)

One `rules_longrun_test` run came back `FAILED. 8 passed; 1 failed` at **11.01s** against a normal
**~0.6s**. Re-ran 3× at `-j 1`: **9/9 green every time** (0.58s / 0.61s / 0.56s). The only edit in play
was deleting two unused re-exports, which cannot change runtime behaviour. This is the known
load-contention flake for these suites ([`rules_test` hangs under load]) — a red line in a sweep run
against a busy box is a harness artifact, not a signal. Recorded here rather than dropped so the next
reader doesn't re-diagnose it.

### Counts that differ from the working-branch handover

- integration tests for `lb-rules` measured **89**, not the 62 the handover claimed (it under-listed
  the test files).
- the handover named `rules_ai_wiring` / `workflow_convergence`; the real targets are
  `rules_ai_wiring_test` / `rules_workflow_convergence_test`. Cargo answers a wrong `--test` name by
  printing the target list and exiting 101 — which reads like a failure but is "no such target".
- `cargo build --workspace` still un-run (OOMs at default `-j` with polars + a concurrent session);
  every per-crate check passes. Left to CI, or re-run with `-j 2`.
