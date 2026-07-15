# Session — rules 10x: long-running job-backed runs (pause/resume) + the data stdlib

Status: in-progress. Date: 2026-07-15.

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

(continues as the session progresses)

## Test output

(pasted at the end of the session)
