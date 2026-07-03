# Jobs scope — reactor drain scan + terminal-job retention

Status: **shipped** (2026-07-03) → `public/jobs/jobs.md`. Session:
`sessions/jobs/job-retention-session.md`. Debug entry closed:
`debugging/jobs/node-pegs-cpu-reactor-rescans-job-table.md`. Open questions resolved (below).

A node with a long-lived workspace can peg a full CPU core doing nothing but re-scanning its
own `job` table. The channel-agent reactor (`spawn_agent_reactors`, 2s tick) and the flow
reactor (`spawn_flow_reactors`, 5s tick) each call `lb_jobs::pending`, which **walks every
page of the `job` table** and filters kind/status in Rust. The table accumulates a terminal
row per flow run, agent run, and every other job kind **forever** — nothing purges `done`/
`failed`/`cancelled` jobs. Once it holds a few thousand rows, one `pending()` pass costs more
than the tick period, so the reactors scan back-to-back with no idle, indefinitely. We want
the drain scan to stay cheap regardless of table age, and terminal jobs (and their sibling
`flow_run`/`flow_step_output` rows) to stop growing without bound.

> Read with: `jobs-scope.md` (the S0 native-queue decision + the S5 record shape),
> `../flows/flip-flop-node-scope.md` / `../flows/flow-run-scope.md` (the reactors that call
> `pending`), the store `capped.rs` bounded-retention primitive (the trim precedent), and the
> new debugging entry `../../debugging/jobs/node-pegs-cpu-reactor-rescans-job-table.md` (the
> diagnosis this scope fixes). The prior full-walk change in `pending.rs` (which this scope
> replaces with an indexed query) is documented only in that file's own doc-comment — it never
> got a debug entry.

---

## Goals

- **The reactor drain scan is O(pending), not O(table).** A workspace with 100k terminal jobs
  and 3 running ones costs about the same per tick as one with 3 jobs total. No unbounded
  full-table walk on a recurring timer.
- **Terminal jobs stop accumulating without bound.** A retention policy purges `done`/
  `failed`/`cancelled` jobs past a bound (age and/or count), workspace-scoped. The
  large sibling tables the demo flow inflates — `flow_run`, `flow_step_output` — get the same
  bounded retention (they are the actual bulk: ~6k `flow_step_output` for ~2.9k runs in the
  reproduced case).
- **No behaviour regression.** The reactors still see every genuinely-pending job (the property
  the current full-walk was written to guarantee — see the `pending.rs` first-page-only note).
  Retention never deletes a resumable (`Running`/`Suspended`) job.

## Non-goals

- **Not** a rewrite of the job queue or a move to an external backend — the S0 decision
  (`jobs-scope.md`) stands: native SurrealDB queue.
- **Not** LIVE-query push pickup for reactors (deferred in `jobs-scope.md` open questions). This
  scope keeps the tick-scan model; it just makes the scan and the table cheap. LIVE pickup can
  land later and would compose (it removes the scan entirely), but it is a bigger change and not
  required to stop the CPU burn.
- **Not** a general TTL/GC framework for every table. This scope names the three tables that
  actually grow unbounded from routine reactor traffic (`job`, `flow_run`, `flow_step_output`);
  other tables adopt the same primitive when they show the same symptom, not pre-emptively.
- **Not** changing the durable-resume or idempotency contract (`jobs-scope.md` §S5) — retention
  only touches rows already in a terminal state.

## Intent / approach

Two independent, composable changes:

**1. A status-filtered drain query (`pending` becomes cheap).** Instead of scanning the whole
table and filtering `kind`/`status` in Rust, push the predicate into SurrealDB:
`SELECT … FROM job WHERE data.kind = $kind AND data.status IN $resumable`, workspace-scoped
via `query_ws`, backed by an index on `(kind, status)`. The store already has the precedent —
`list.rs` runs `WHERE data.{field} = $value` today; this is the same shape with a
two-field predicate and a defined index so the query is a lookup, not a table scan. The result
set is the handful of actually-pending jobs, so cost tracks pending count, not table size. The
existing paging/`MAX_PENDING_PAGES` ceiling stays as a self-protection backstop but is now
never approached in practice.

*Rejected alternative:* keep the full walk but cache it between ticks. That hides the growth
instead of fixing it, still O(table) on the first tick after any write, and adds a staleness
window where a freshly-enqueued job waits a cache generation. The index is simpler and correct.

**2. Bounded retention for terminal rows.** A terminal job is a fire-once record with no resume
value once drained — it is exactly the "bounded ring" `capped.rs` was built for. Apply
count-bounded (and optionally age-bounded) retention per workspace to `job`, `flow_run`, and
`flow_step_output`. Two placement options, pick during implementation (open question):
  - **(a) Transactional trim at write-time** — extend the terminal-transition write (the
    `complete`/`finalize` path) to trim oldest terminal rows in the same transaction, exactly as
    `capped_insert` does. Guarantees the bound is never exceeded, no reaper needed. Preferred if
    the terminal transition is a single chokepoint.
  - **(b) A retention pass on the reactor tick** — a cheap `DELETE … WHERE status IN
    ($terminal) AND <string>id < $cutoff` bounded per tick, run alongside the (now cheap) drain
    scan. Simpler to add across three tables; overshoots the bound between sweeps (acceptable for
    a *retention* bound, unlike a correctness-critical ring cap). `capped.rs`'s own docs call the
    reaper "an optional secondary safety net" — here it is the primary mechanism because the bound
    is soft.

Both reuse existing store primitives (`write_tx` / the `DELETE … WHERE` idiom); neither adds a
new persistence layer. Retention thresholds are **config/prefs, not baked constants** (per
`capped.rs`'s "defaults live in the caller" rule) — a dev node can keep a small window, a
production hub a larger one.

## How it fits the core

- **Workspace is the hard wall (§3.6):** every query and every delete is `query_ws`-scoped; a
  ws-B retention pass physically cannot see or purge a ws-A job. This is a **mandatory isolation
  test** (a ws-B trim leaves ws-A's terminal rows untouched).
- **Capability-first (§3.5):** the drain scan and retention are **raw store/host-internal verbs**,
  like every `lb-jobs` verb today (`jobs-scope.md` §S5) — the reactor holds its own node-internal
  authority (`node:reactor`), not a user principal. No new user-facing MCP cap. Retention is not a
  principal-facing surface; it is the node maintaining its own durable state. (If we later expose a
  manual "purge terminal jobs now" admin verb, *that* gets a cap — flagged as an open question, not
  built here.)
- **Symmetric nodes (§3.1):** identical on edge and hub; the only difference is the retention
  threshold, which is config/prefs, never a code branch. No `if cloud`.
- **One datastore (§3.2):** SurrealDB only. The index and the trim are SurrealDB features; no new
  store, no external queue.
- **State vs motion (§3.3):** jobs are **state** (SurrealDB). This scope does not touch the bus.
  The reactor tick reading state is exactly the model `jobs-scope.md` chose; we make the read cheap,
  we do not move it to motion. (A future LIVE-query pickup would be the "motion" version — out of
  scope, noted.)
- **MCP surface (API shape §6.1):** no new **write** verbs for callers (retention is internal). The
  read verb `pending` keeps its signature; its implementation changes from full-scan to
  indexed-filter. This is a **read** shape change only. No live-feed, no batch, no job-of-its-own —
  retention is a bounded maintenance sweep, not a long batch.
- **Data (SurrealDB):** touches `job`, `flow_run`, `flow_step_output`. Adds a `DEFINE INDEX` on
  `job` over `(data.kind, data.status)` (and equivalents on the flow tables if option (b) filters by
  status there). No new tables.
- **Durability:** N/A — deleting an already-terminal, already-drained row has no must-deliver
  effect; it goes through no outbox. The correctness guard is that a *resumable* job is never in the
  delete set.
- **One responsibility per file (FILE-LAYOUT):** the indexed `pending` stays in `jobs/pending.rs`
  (rewritten body, same responsibility). Retention is its **own** verb file(s) —
  e.g. `jobs/retain.rs` (and a flow-side `flows/retain_runs.rs` if the flow tables are trimmed there),
  not folded into `pending.rs` or a `utils`. The index definition lives with the store's schema/open
  path, beside the other `DEFINE`s.

## Example flow

The reproduced failure, and the fixed path:

1. A `flipflop`/`cron` demo flow runs unattended for days. Each firing mints a `flow-run` job
   (terminal `done` after it completes), a `flow_run` row, and several `flow_step_output` rows.
   Nothing purges them; the `job` table reaches ~2,900 rows, `flow_step_output` ~6,300.
2. **Today (broken):** every 2s the agent reactor calls `pending(store, ws, "channel-agent-run")`,
   which `scan`s the whole `job` table page by page (200/page) and filters in Rust. At ~0.65s per
   500-row page in a debug build, one pass exceeds the 2s tick; the 5s flow reactor's `pending`
   +`reconcile` pile on. The node scans back-to-back at 100% CPU; every gateway request contends for
   the runtime, so responses feel slower and slower as the table grows.
3. **Fixed — drain scan (change 1):** `pending` runs `SELECT … WHERE data.kind = $kind AND
   data.status IN ['running','suspended']` against the `(kind,status)` index. With 3 pending jobs it
   returns 3 rows in ~microseconds regardless of the 2,900 total. The tick finishes with time to
   spare; the node idles between ticks.
4. **Fixed — retention (change 2):** on the terminal transition (or on the tick), terminal `job`
   rows past the workspace's retention window are trimmed, and the same for `flow_run` /
   `flow_step_output`. The tables stabilise at the bound instead of growing forever — so even a
   naïve full scan would stay cheap, and the store stops bloating on disk.
5. A ws-B reactor ticking at the same time scans and trims only ws-B's tables; ws-A's terminal rows
   are invisible to it (the hard wall) — asserted by the isolation test.

## Testing plan

Per `scope/testing/testing-scope.md`, against the **real** store/reactor (no mocks, rule 9 —
seed real job/flow rows into a `mem://` store):

- **Performance / cost (the regression this exists to prevent):** seed N terminal jobs (e.g.
  5,000) + a few resumable ones into a real ws; assert `pending` returns exactly the resumable
  set and that its cost does **not** scale with N (query plan uses the index / bounded row-reads —
  measure work, not wall-clock, so it is deterministic in CI). This is the **regression test** the
  debugging entry demands.
- **Mandatory capability path:** N/A for a user cap (raw internal verb) — **but** assert the negative
  explicitly: `pending`/retention require no user grant and are unreachable as a user-facing MCP verb
  (there is no `mcp:jobs.pending` a principal can call), so the "no new capability surface" claim is
  tested, not asserted.
- **Mandatory workspace-isolation:** a ws-B retention pass leaves ws-A's terminal rows intact;
  `pending` for ws-B never returns a ws-A job. (Extends the existing `jobs-scope.md` isolation test.)
- **Retention correctness:** a resumable (`Running`/`Suspended`) job is **never** deleted, even when
  older than the window; only terminal rows are trimmed; the bound is respected (count and/or age);
  and — the load-bearing safety property — draining a run and then trimming never removes a job the
  reactor still needs (compose with the idempotent-resume test in `jobs-scope.md` §S5).
- **Concurrency (if option (a), transactional trim):** two concurrent terminal transitions do not
  over- or under-evict — reuse the `capped.rs` concurrency-test pattern.
- **No behaviour regression:** the first-page-only scenario `pending.rs` guards against still passes —
  a genuinely-pending job whose id sorts late is still found (the index query returns it directly,
  which is *strictly* safer than the paged walk it replaces).

## Risks & hard problems

- **Under-deleting a live job = double-run.** The one unacceptable failure. Retention's delete
  predicate must be `status IN (terminal)` and nothing else; a resumable job outside the window is
  kept forever, by design. Any age/count bound is applied **within** the terminal set only. The
  isolation + correctness tests are the guard.
- **Index correctness on the stored shape.** Job rows are stored under a `data` wrapper in some paths
  (`pending.rs` already unwraps defensively); the index must target the same field path the query
  filters on (`data.kind`/`data.status`), or the query silently falls back to a full scan and we've
  fixed nothing. Verify the query plan actually uses the index (assert row-reads, not just results).
- **Flow tables are the real bulk.** Fixing only `job` leaves `flow_step_output` (~2× the runs)
  growing on disk. The scope must trim all three or the disk/scan pressure only half-lifts.
- **Retention window as data loss.** Trimming terminal runs discards run history a user might want in
  the UI. The window is a **policy** (config/prefs) with a sane default, and the default must be large
  enough that ordinary inspection isn't lost — the goal is bounding runaway growth, not aggressive GC.
  Flagged as an open question (what default, and whether the UI should warn when it's showing a
  truncated history).
- **This is dev-visible now, production-fatal later.** Reproduced as a dev-node annoyance (a demo
  flow left running), but the same unbounded growth hits any long-lived production workspace far
  harder. Treat it as a correctness/scaling fix, not a dev-QoL nicety.

## Open questions — RESOLVED (2026-07-03)

- **Retention placement:** **option (b) reactor-tick sweep for all three tables.** `job` reaches
  terminal through **two** verbs (`complete`, `cancel`) — no single chokepoint for (a). `flow_run` has
  a single chokepoint (`set_run_status`), but its `flow_step_output` rows are keyed `{run_id}:{node}`
  and written by a different verb, so a transactional trim at the run transition **cannot reach** the
  step bulk; they must be purged in tandem, keyed by the purged run ids — which only a sweep does. So
  (b) uniformly, for concrete per-table reasons (not convenience). Swept on the flow reactor tick,
  throttled to every 30th tick (`flows/retention_sweep.rs`), fires on tick 0 for immediate reclaim.
- **Bound shape and defaults:** **count-bounded per workspace, default 500** terminal jobs and 500
  finished runs (`DEFAULT_TERMINAL_JOB_CAP`, `DEFAULT_FINISHED_RUN_CAP`). Generous so ordinary flow-run
  history the UI shows isn't lost; the goal is bounding runaway growth, not aggressive GC. Age-bounding
  deferred (count alone bounds the tables; add age if a time-window policy is later needed).
- **Where retention config lives:** **a compiled caller-owned default.** Prefs here is a **closed
  typed-axis** system (language/timezone/…) with no numeric key→value getter, so there is nowhere to
  resolve a retention number from — the `capped.rs` "defaults live in the caller" default is the
  mechanism; an operator override slots in at the constant. (Reopen if a numeric-prefs axis is added.)
- **Manual purge verb:** **deferred, nothing reserved.** Retention is a raw node-internal verb with no
  user-facing MCP surface (asserted by construction). A manual "purge now" admin verb, if ever wanted,
  gets its own cap then — not built, no name reserved now.
- **Immediate dev relief:** **shipped** as `make purge-store` (wipes `.lazybones/data/dev-store` only,
  no rebuild, keys/extensions untouched).

## Related

- `jobs-scope.md` — the native-queue decision, the S5 record shape, the deferred LIVE-query pickup
  this scope deliberately does not pull forward.
- `../flows/flip-flop-node-scope.md`, `../flows/flow-run-scope.md`,
  `../flows/flow-plc-reliability-scope.md` — the reactors that drive `pending` on a tick and mint the
  terminal rows.
- Store `capped.rs` — the bounded-retention primitive and its transactional-trim invariant; the model
  for change 2. `list.rs` — the `WHERE data.{field}` filtered-read precedent for change 1.
- `../../debugging/jobs/node-pegs-cpu-reactor-rescans-job-table.md` — the diagnosis this scope fixes.
  `crates/jobs/src/pending.rs`'s doc-comment records the earlier first-page-only fix (which introduced
  the full-walk) and already flagged the "table grows forever" hazard this scope closes out.
- README `§6.9` (jobs), `§6.10` (outbox — why retention needs none), `§7` (workspace wall),
  `§3` rules 1/2/3/5/6.
