# Node pegs a full CPU core; API responses feel progressively slower

**Area:** jobs · **Status:** **resolved** (2026-07-03 — indexed drain scan + bounded retention shipped;
scope `scope/jobs/job-retention-scope.md`, session `sessions/jobs/job-retention-session.md`)

## Symptom

A long-running dev `node` sits at a **constant 100% of one core** even when idle. Gateway
requests still return sub-millisecond in isolation, but under the constant CPU load the whole
runtime feels sluggish, and it got worse the longer the node ran ("responses feel slower since
around when I added the external agent"). The user suspected the new external-agent code.

## Reproduce

1. Take a dev store that has run the `flipflop`/`cron` demo flow (`flow-1782857109253`) for a
   while. In the reproduced case the store held ~2,900 `job` rows, ~2,900 `flow_run`, ~6,300
   `flow_step_output` (33 MB store).
2. Boot a node against a **copy** of that store. It immediately pegs 100% CPU.
3. Boot a node against a **fresh/empty** store with the identical binary + `external-agent`
   feature: it idles at ~2%.

The differentiator is the accumulated data, not the code path or the feature flag.

## Observed (not guessed)

- `pidstat` on the live node: ~100% CPU, steady. I/O counters (`/proc/pid/io`) barely move —
  it is CPU, not disk. `perf`/`strace`/ptrace were all blocked in this environment, so I
  reproduced on a store copy on a spare port and experimented there instead.
- Disabling the demo flow (via `/flows/{id}/enable {enabled:false}`) did **not** drop the CPU —
  ruling the flow's *firing* out as the cost; it is the **scanning**, not the running.
- A fresh empty store idles; a copy of the bloated store pegs. → the cost scales with table
  size, i.e. a scan.
- `/store/tables` on the copy: `series` 10,693, `flow_step_output` 6,295, `job` 2,898,
  `flow_run` 2,891 (rest tiny). Timing a single 500-row `job`/`flow_run`/`flow_step_output`
  page over the gateway: **0.64–0.88 s each** in the debug build.

## Root cause

`spawn_agent_reactors` (2 s tick, `crates/host/src/agent_reactor.rs`, added 2026-07-01
"added in channel agent") and `spawn_flow_reactors` (5 s tick) both call `lb_jobs::pending`
(`crates/jobs/src/pending.rs`). `pending` **walks the entire `job` table page-by-page**
(`MAX_SCAN_LIMIT` 200/page, up to `MAX_PENDING_PAGES` 50) and filters `kind`/`status` in Rust
— there is no indexed status/kind query. The `job` table accumulates one terminal row per flow
run / agent run / every kind **forever**; nothing purges `done`/`failed`/`cancelled` jobs.

At ~2,900 rows a single `pending` pass costs more than the 2 s tick period, so the reactors
scan **back-to-back with no idle**, permanently. The `flow_run`/`flow_step_output` growth
compounds the on-disk/scan pressure. The external-agent runtime code (`role/external-agent`)
is **per-run only** and was not the cause — the reactor that *scans for* channel-agent runs
was, and it shipped in the same batch of work, which is why the timing felt related.

The full-walk in `pending` was itself an earlier fix — a first-page-only read had missed
late-sorting jobs (recorded only in `pending.rs`'s own doc-comment, never a debug entry). That
same comment already flagged the unbounded-growth hazard: *"a workspace's `jobs` table
accumulates rows from every kind forever."* This entry is that hazard coming true.

## Fix (applied 2026-07-03)

Both changes shipped — see `sessions/jobs/job-retention-session.md`:
1. `pending` (`crates/jobs/src/pending.rs`) is now an **indexed** `SELECT data FROM job WHERE
   data.kind = $kind AND data.status IN ['running','suspended']`, backed by
   `DEFINE INDEX job_kind_status ON TABLE job COLUMNS data.kind, data.status`
   (`crates/jobs/src/schema.rs`, ensured lazily per-namespace on first `create`). O(pending), not
   O(table); strictly safer than the paged walk on the first-page property (no pages to fall off).
2. **Bounded retention**: `crates/jobs/src/retain.rs` (`retain_terminal`, `job`) and
   `crates/host/src/flows/retain_runs.rs` (`retain_runs`, `flow_run` + `flow_step_output` in tandem),
   count-bounded per workspace (default 500 each), run on the flow reactor tick throttled to every 30th
   tick (`crates/host/src/flows/retention_sweep.rs`). Delete predicate is `status IN (terminal)` and
   nothing else — a resumable job/run is never trimmed. Reuses `capped.rs`'s safe-delete idiom. Config
   is a compiled caller-owned default (no numeric prefs axis exists — see the session doc).

**Immediate relief** (no rebuild): `make purge-store` (added this session — wipes
`.lazybones/data/dev-store` only) then `make kill && make dev`. The CPU drops to idle.

## Guard (shipped — regression tests)

- **Performance regression:** `crates/jobs/tests/retain_test.rs::pending_is_indexed_and_returns_only_resumable_at_scale`
  — 5,000 terminal + 2 resumable into a real `mem://` ws; `pending` returns exactly the 2 resumable, an
  index-backed `count()` over the drain predicate equals 2 (not N), and `INFO FOR TABLE` confirms the
  index exists. Deterministic (measures the DB-side filter, not wall-clock).
- **Never-trim-resumable:** `retention_never_trims_a_resumable_job` (job) +
  `flows_retention_test.rs::retain_runs_never_trims_a_live_run_and_trims_step_rows` (flow run).
- **Workspace isolation:** `retention_is_workspace_scoped` (both crates).
- **Bound respected / newest kept + step-row tandem:** the remaining `retain_test` / `flows_retention_test`
  cases. The first-page-only guarantee is preserved (a late-sorting resumable job is returned directly).

## Lessons

- A **recurring full-table scan on a timer** is a latent O(table) cost bomb: fine on an empty
  dev store, fatal on a long-lived one. Any reactor tick that reads "what's pending" needs an
  indexed predicate, not a scan-and-filter.
- **Any table written on every routine event needs a retention policy from day one.** "Terminal
  rows accumulate forever" was written in a comment and shipped anyway; the growth is invisible
  until a scan reads it back on a hot loop.
- Trust the repro over the suspect: the user (and I, initially) eyed the external-agent code;
  reproducing on a fresh vs. copied store isolated it to **data growth + a scanning reactor**,
  not the new runtime.
