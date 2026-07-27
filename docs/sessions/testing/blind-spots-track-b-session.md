# Closing test blind spots — track B: multi-batch loops + width axes

- Issue: [#108](https://github.com/NubeDev/lb/issues/108) (track B; track A is
  `blind-spots-track-a-session.md`)
- Scope: `docs/scope/testing/testing-scope.md` §3.2, rows 2 (**a loop enumerates the outcomes that
  existed when it was written**) and 3 (**one configuration is exercised, not the axis**)
- Sibling history: `docs/debugging/ingest/filtered-batch-stops-the-drain-loop.md`,
  `docs/sessions/ingest/series-normalize-session.md`
- Status: **done**, green (output below). No new production bug found; no `debugging/` entry needed.

## The ask

Two of the four §3.2 blind spots, closed in the suite rather than described:

3. **Multi-batch by default.** Bug #1 of #108 was a drain-loop stall that *every* single-batch test
   passed against — the loop broke before it had to iterate. Audit every test that drives a loop over
   batches or pages and raise it past that loop's batch/page size.
4. **Axis assertions for width-keyed behaviour.** Bug #3 was `method` resolving only at a tier's
   EXACT `width_ms`. Apply `series_normalize_method_test.rs`' shape — loop the axis, assert the
   property at every point — to every other place that resolves something from config by exact match.

## Item 3 — the audit

Batch/page sizes that actually drive a loop, and what runs against them:

| Test | Loop / page size | Was | Now | Verdict |
|---|---|---|---|---|
| `crates/ingest/tests/series_retention_test.rs` → `retention_gc_rolls_up_then_evicts_and_buckets_merge_rollups` | `commit_batch(…, 256)` in `seed` | 200 | **700** | Raised. Under 256 the seed drain loop never went round. Window/horizon/assertions scaled with it (evict 600, 60 rollup rows, 35 buckets, 55 tier-evicted). |
| `crates/ingest/tests/durable_redrain_test.rs` → `restart_redrains_staged_samples_exactly_once` | `drain_all` loop, 256 | 5 | **700** | Raised (via `examples/crash_ingest.rs`). Restart recovery now has to iterate three batches. |
| `crates/ingest/tests/durable_redrain_test.rs` → `committed_batch_survives_kill_without_double_commit` | same | 5 staged / 5 committed | **700 staged / 256 committed** | Raised **and strengthened**: the kill now lands MID-backlog, so the test asserts the surviving batch, the still-staged remainder, and an exactly-once re-drain of the rest. A single-batch backlog could not express "mid-backlog" at all. |
| `crates/ingest/tests/durable_redrain_test.rs` → `drain_all` helper | — | `pass.committed == 0` | `pass.drained() == 0` | **Fixed.** This helper still carried the exact termination condition the drain-loop stall was; the sub-256 seed meant it could never say so. (The sibling helper in `series_retention_test.rs` was fixed during the series-normalize slice; this one was missed.) |
| `crates/ingest/tests/series_plane_test.rs` → `pushdown_is_o_buckets_not_o_rows` | `SCAN_CHUNK` = 10 000 in `read_buckets_fold` | 10 000 | **12 000** | Raised. At exactly 10 000 the keyset loop's second page came back EMPTY — the loop iterated but no row was ever folded across a chunk boundary. |
| `crates/ingest/tests/series_cap_multibatch_test.rs` (**new file**) | `CAP_EVICT_BATCH` = 5 000 in `cap_series` | *(no test existed)* | **5 100 / 5 550 over cap** | New. `cap_series` loops until at/under the bound; nothing anywhere forced a second eviction slice. Two tests: the primitive, and the `run_gc` pass that the retention reactor calls. |
| `crates/insights/src/table_scan.rs` (**new `#[cfg(test)]`**) | `MAX_SCAN_LIMIT` = 200 in insights' own `scan_all` | *(no test existed)* | **250 rows** | New. Insights has a *second* `scan_all` (its own, with a `MAX_ROWS` backstop) that no test paged past. A unit test rather than an integration one because the module is private. |
| `crates/host/tests/ingest_drain_bound_test.rs` (5 tests) | `COMMIT_BATCH` = 256 | 2 000 / 600 / 1 000 / 300 / 900 | unchanged | **Left alone — already correct.** The bound assertion ("one call commits at most `COMMIT_BATCH`") runs against a 2 000-row backlog, which is the point. |
| `crates/host/tests/series_normalize_test.rs` → `a_fully_filtered_backlog_drains_completely…` | 256 | 700 | unchanged | **Left alone — the model.** The regression test for the original stall. |
| `crates/host/tests/roster_scan_paging_test.rs` (3 tests) | `MAX_SCAN_LIMIT` = 200 | 250 / 240 / 240 | unchanged | **Left alone — already past one page.** |
| `crates/host/tests/flows_scan_paging_test.rs` (2 paging tests) | 200 | 240 / 240 | unchanged | **Left alone — already past one page.** |
| `crates/ingest/tests/series_plane_test.rs` → `keyset_paging_walks_every_row_exactly_once` | explicit `limit: 7` | 50 rows | unchanged | **Left alone.** Runs against a per-call limit, ~8 cursor pages. |
| `crates/host/tests/series_plane_host_test.rs` → `paged_read_walks_chain_via_mcp` | explicit `limit: 10` | 25 samples | unchanged | **Left alone.** 3 pages. |
| `crates/host/tests/ingest_test.rs` | — | — | unchanged | **Left alone — no fixed-count loop.** Deny/isolation/round-trip only; nothing here drives a batch loop. |
| `crates/jobs/tests/retain_test.rs` → `pending_is_indexed…at_scale` | `MAX_PENDING` = 10 000 | 5 000 | unchanged | **Left alone — not a loop.** `jobs::pending` is one capped query, not a paging loop; there is nothing to iterate. |
| `crates/outbox` relay | — | — | unchanged | **Nothing to raise.** The relay has no batch size: `pending`/`due` is an unbounded query. Noted below as a future trap. |

Ruled out after checking (not batch/page loops): `crates/store/tests/*` compaction loops (write
volume), `flows_retention_test.rs` + `jobs/retain.rs` (single `DELETE` statement), telemetry's
`MAX_PAGE` (a query cap, no cursor loop), `federation/sample.rs` `MAX_ROWS` (single query),
`store::write_batch` `MAX_BATCH` (rejects over-cap, never loops).

## Item 4 — the axis assertions

Exact-match config lookups found in the host + ingest crates, and what now sweeps them:

| Lookup | Where | Axis test |
|---|---|---|
| `Policy::tier_at` — `.find(\|t\| t.width_ms == width_ms)` | `crates/ingest/src/retention.rs:58` | `series_width_axis_test.rs` → `every_method_governs_a_read_at_every_width_not_just_the_tiers_own`, `a_multi_tier_policy_resolves_its_method_at_every_width` |
| `merge_rollups` — `.filter(\|r\| r.width_ms == finest)` | `crates/ingest/src/bucket.rs:313` | `series_width_axis_test.rs` → `a_bucketed_read_covers_the_whole_history_at_every_width` |
| `apply_method` over rollup-backed buckets | `crates/ingest/src/method.rs` | same file, all eight methods × nine widths |
| `effective_width` (engine, **rejects** over-cap) vs `resolution::derive_width` (dashboard, **clamps** to its own local `MAX_BUCKETS` mirror) | `crates/ingest/src/bucket.rs:84` / `crates/host/src/viz/resolution.rs:73` | `viz_resolution_axis_test.rs` → `every_zoom_level_yields_a_width_the_engine_accepts` |
| the resolution LADDER → `Policy::method_for` end to end | `crates/host/src/viz/resolution.rs` → `crates/host/src/ingest/read.rs:80` | `viz_resolution_axis_test.rs` → `the_configured_method_governs_at_every_ladder_width` |

Two new files:

- **`rust/crates/ingest/tests/series_width_axis_test.rs`** — seeds 600 samples, GCs so five sixths of
  the history is rollup-backed (the only state in which the rollup merge is on the read path at all),
  then sweeps nine widths from 1 s to 15 min — finer than the stored tier, at it, coarser, and widths
  that are not tier widths at all. Asserts at **every** width: total bucket `count` == 600 (no
  history silently lost), the true min/max survive, buckets stay on the absolute width grid, every
  one of the eight methods resolves and produces a `value`, and `first`/`nearest` name the earliest
  sample. Plus multi-tier policies, which a single-tier test cannot express — including the case
  where a tier at the read's exact width declares **no** method and must fall through rather than
  shadow the configured one.
- **`rust/crates/host/tests/viz_resolution_axis_test.rs`** — the same shape at the layer where bug #3
  was actually observed. Sweeps 8 zoom levels × 2 budgets × 3 `minInterval`s through the real
  `viz.query` (48 real panel resolutions) asserting the derived width is one the engine *accepts*
  (an empty frame is the drift signal — nothing on screen says the read was rejected) and the budget
  ceiling holds everywhere, not just at the tested range. Then sweeps all **17** ladder steps through
  `series.read`, asserting a coil configured `last` still reports `method: "last"`, carries a `value`
  on every bucket, and never leaves the coil's `{0,1}` domain — i.e. it was sampled, not averaged.

**No axis assertion failed.** The one red during development was my own wrong expectation about
`method_for` precedence when two tiers disagree (the exact-width tier correctly wins at its own
width); the test was corrected, not the code. So there is no `docs/debugging/` entry from this
session and `docs/debugging/README.md` is unchanged.

### Decisions, and what was rejected

- **A new file for the cap-eviction tests, not an append.** `series_retention_test.rs` is the natural
  home, but the FILE-LAYOUT ratchet fails on a grandfathered file growing and the rule is one
  responsibility per file anyway. Rejected: appending to the existing retention suite.
- **`viz::resolution` is a private module, so the derived-width axis is asserted END TO END through
  `viz.query`** rather than as a unit test. Rejected: making the module `pub` for testability (it
  would widen a host API for a test), and rejected adding the sweep to `resolution.rs`'s own `mod
  tests` — the file is at 387 lines and any addition pushes it over the 400 hard limit into a *new*
  violation. Going through `viz.query` is also the Rule 9 answer: it is the path a dashboard takes.
- **The ladder constant is duplicated in the test, deliberately.** A test that imported the private
  constant could not notice the ladder and the engine's cap drifting apart — which is the failure the
  test exists to catch.
- **Scaled `series_retention_test`'s arithmetic rather than rewriting the test.** Every count moves
  by the same factor (200→700, cutoff 100 k→600 k), so the assertions still say what they said.
- **`durable_redrain`'s second phase was strengthened, not just enlarged.** With 700 staged and one
  256-row batch committed before the abort, "killed after a commit" finally means *mid-backlog*,
  which is the case a restart actually meets.

## Green

```
$ cd rust && cargo fmt && cargo build --workspace
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 9.85s

$ bash rust/scripts/check-file-size.sh
FILE-LAYOUT: OK — 2408 files checked, 114 grandfathered

$ cargo test -p lb-ingest --test series_width_axis_test --test series_cap_multibatch_test \
    --test series_retention_test --test series_plane_test --test durable_redrain_test --no-fail-fast
durable_redrain_test    3 passed; 0 failed
series_cap_multibatch_test  2 passed; 0 failed
series_plane_test      13 passed; 0 failed
series_retention_test   3 passed; 0 failed
series_width_axis_test  3 passed; 0 failed

$ cargo test -p lb-insights --lib --no-fail-fast
table_scan::tests::scan_all_drains_every_page_not_just_the_first ... ok
1 passed; 0 failed

$ cargo test -p lb-host --test viz_resolution_axis_test --test viz_resolution_test \
    --test series_normalize_method_test --test series_normalize_test --test ingest_drain_bound_test \
    --test ingest_test --test series_cap_reactor_test --test flows_scan_paging_test \
    --test roster_scan_paging_test --no-fail-fast
viz_resolution_axis_test      2 passed; 0 failed
viz_resolution_test           6 passed; 0 failed
series_normalize_method_test  3 passed; 0 failed
series_normalize_test         4 passed; 0 failed
ingest_drain_bound_test       5 passed; 0 failed
ingest_test                   7 passed; 0 failed
series_cap_reactor_test       4 passed; 0 failed
flows_scan_paging_test        4 passed; 0 failed
roster_scan_paging_test       3 passed; 0 failed
```

Nothing red anywhere in the binaries touched by this work.

## Left undone

- **`crates/outbox`**: the relay has no batch bound at all (`pending`/`due` return everything due and
  `relay.rs` iterates the lot), so there is nothing to page past today — but there is also no test
  that would catch a one-batch stall the day a bound is added. Worth a note on the outbox scope
  rather than a speculative test now.
- **Telemetry's console read** (`MAX_PAGE` = 200) and **`insights::table_scan`'s `MAX_ROWS`
  truncation branch** remain untested at the boundary. The insights *paging loop* is now covered
  (250 rows); the 10 000-row backstop is not — seeding 10 001 rows to prove a truncation guard is a
  poor trade in a suite this size.
- `docs/scope/testing/testing-scope.md` §3.2 still points at #108 as standing work; rows 2 and 3 are
  now materially closed for the ingest/viz surfaces but not for the whole workspace, so the row text
  was deliberately left as-is.
