# Node pegs a full CPU core; API responses feel progressively slower

**Area:** jobs · **Status:** open (diagnosed; fix scoped in
`scope/jobs/job-retention-scope.md`, not yet built)

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

## Fix (scoped, not yet applied)

See `scope/jobs/job-retention-scope.md`:
1. Replace the full-walk in `pending` with an **indexed status/kind query** (`WHERE data.kind
   = $kind AND data.status IN [...]`, backed by a `(kind,status)` index) so the drain scan is
   O(pending), not O(table).
2. **Bounded retention** for terminal `job` / `flow_run` / `flow_step_output` rows (reusing the
   `store/capped.rs` transactional-trim precedent), workspace-scoped, threshold in config/prefs.

**Immediate relief** (no code): wipe `.lazybones/data/dev-store` (dev seed data) and
`make kill && make dev`, or add a `make purge-store`. The CPU drops to idle.

## Guard (required on fix)

A performance regression test: seed N (e.g. 5,000) terminal jobs + a few resumable ones into a
**real** `mem://` ws and assert `pending` returns exactly the resumable set with cost **not**
scaling with N (measure row-reads / index use, not wall-clock, so it's deterministic in CI).
Plus the mandatory workspace-isolation test (a ws-B retention pass never touches ws-A rows) and
a correctness test (a resumable job is never trimmed) — keeping `pending.rs`'s existing
first-page-only guarantee (a late-sorting pending job is still found) intact.

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
