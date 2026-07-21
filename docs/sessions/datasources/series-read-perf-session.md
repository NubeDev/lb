# Session: series-read performance — pushdown decimation + batched latest

**Scope:** `docs/scope/datasources/series-read-perf-scope.md` ·
**Status:** implemented, tested green on the final code — `lb-ingest --test series_plane_test` **20/20**
and `lb-host --test ingest_test --test ingest_isolation_test` (**7 + 3**) both pass on a clean run;
`cargo build --workspace` is green. Live-latency verification against a real node is deferred — the
O(buckets) contract is pinned by a crate integration test (`pushdown_is_o_buckets_not_o_rows`), not yet
timed on a box.

> **Note on a pre-existing, unrelated test failure:** `cargo test --workspace` cannot *compile*
> `lb-cli`'s `ext_publish_test` in this environment because it needs a prebuilt wasm fixture
> (`extensions/hello-v2/target/wasm32-wasip2/release/hello_v2_ext.wasm`) produced by an extension
> `build.sh` that hasn't run here. This aborts the whole-workspace test *compile* before any test runs,
> so verify the affected crates directly (the two commands above) rather than via `--workspace`. This is
> environmental, not caused by this change — no `role/cli` or `extensions/*` files were touched.

Closes the two measured slownesses in the series read plane, both without a wire break:

1. **Bucketed `series.read` (`mode:"buckets"`) becomes a pushed-down `GROUP BY`** — the read-time
   aggregate the [`series-decimation-scope`](../../scope/datasources/series-decimation-scope.md)
   always intended. The chunked in-Rust fold that shipped first moved every raw row of the window
   into the host (a 10 k-sample window → 10 k rows over the store boundary to emit ~240 buckets); it
   now aggregates where the data lives and returns ≤ budget bucket rows.
2. **`series.latest_many`** — a new list-shaped read verb that collapses a K-series fleet snapshot
   from K authorize + query round-trips into one.

## Verified first, then built

The scope's whole design rests on two SurrealDB claims the decimation scope had recorded as
blockers. Before writing anything I re-ran a throwaway probe against the real `mem://` store
(SurrealDB 2.6.5) and confirmed both:

- `array::last()` over an `ORDER BY ts ASC, seq ASC` subquery **is** an exact chronological `last`,
  including a **non-numeric** last (`"offline"` carried verbatim).
- `math::min/max/sum` **skip** non-numeric values natively, and `type::is::number(payload)` in the
  predicate makes `count()` the numeric count — so `avg = sum/num_count` is exact, and a mixed bucket
  aggregates its numbers while still counting/carrying its non-numbers.

The probe was deleted once the parity test replaced it as the standing guard.

## What shipped

**`crates/ingest/src/bucket.rs`** — the fold is replaced as the production path by a pushdown, and
kept as the test oracle:

- `read_buckets` (production) → `raw_bucket_query` + the shared `merge_rollups` + `finish`.
- `raw_bucket_query` issues **one `query_ws` two-statement call** (one snapshot, so a concurrent
  commit cannot split Query N from Query L):
  - **Query N** — `math::floor(time::millis(ts)/$width) AS b`, `count() AS num_count`,
    `math::min/max/sum` over `WHERE … type::is::number(payload) …`, `GROUP BY b`.
  - **Query L** — `count() AS count`, `array::last(p) AS last`, `array::last(t) AS last_ts` over an
    ordered subquery, `GROUP BY b`.
  It joins the two by bucket index in a `BTreeMap` (O(buckets), never O(rows)).
- `read_buckets_fold` — the original chunked fold, **retained only as the parity oracle** and
  exported for the test.
- `merge_rollups` + `finish` — factored out so the pushdown and the fold aggregate the post-GC rollup
  tail and finalize the wire shape **identically**.

**`crates/ingest/src/latest.rs`** — `latest_many(store, ws, &[String]) -> Vec<(String,
Option<Sample>)>`: one `WHERE series IN $names ORDER BY ts DESC, seq DESC` scan; every requested name
pre-seeded to `None` (dedup-safe, order preserved) then filled by the first row seen per series (=
its newest, same "newest" as single `latest`).

**`crates/host/src/ingest/read.rs`** — `series_latest_many` wrapper: authorizes
`mcp:series.latest:call` **once** for the whole batch (no new cap), then calls the store method.
Re-exported through `ingest/mod.rs`.

**`crates/host/src/ingest/tool.rs`** — a `series.latest_many` dispatch arm returning
`{ latest: { "<name>": Sample|null, … } }`, plus a `string_arr` arg helper. Routing is by the
existing `"series."` prefix in `tool_call.rs`, so no router change.

**`crates/host/src/system/catalog.rs`** — one `HostTool` entry for `series.latest_many`.

## The one real bug the parity oracle caught

The first pushdown keyed buckets on `floor((ts - from)/width)` (a `from`-relative index) and mapped
back with `t = floor(from) + b*width`. That splits an absolute bucket across two `from`-relative ones
whenever `from` is **not** width-aligned — the fold keys on the absolute `floor(ts)`. The
`pushdown_handles_an_unaligned_from` parity test (`from=17_000`, width `60_000`) failed immediately:
`max` 76 vs the fold's 59. Fix: key on the **absolute** `floor(ts/width)` and map `t = b*width`,
matching the fold exactly. This is precisely the "bucket-index vs floor alignment" risk the scope
flagged — the oracle made it executable.

## Tests (no mocks — real `mem://`, seeded via `write`+`commit_batch`)

Crate (`crates/ingest/tests/series_plane_test.rs`):
- `pushdown_buckets_are_byte_identical_to_the_fold` — headline: pushdown vs fold byte-for-byte over a
  nasty window (numeric + non-numeric, in-bucket spike, same-`ts` broken by `seq`, non-numeric last,
  sparse gap).
- `pushdown_handles_an_unaligned_from` — the alignment seam, 300 samples, asserts the absolute grid.
- `pushdown_is_o_buckets_not_o_rows` — the regression guard the scope demands: 10 k samples decimate
  under budget, with fold parity so speed didn't cut a corner.
- `latest_many_covers_every_name_and_scopes_by_workspace` — order/absent-null/newest/non-numeric/
  single-latest parity + ws scoping.

Host (`crates/host/tests/ingest_test.rs`): `latest_many_batches_the_snapshot_and_parities_single_latest`,
`latest_many_denied_without_the_single_latest_cap` (whole-batch deny), `latest_many_is_workspace_scoped`.
The mandatory capability-deny and workspace-isolation categories are covered at the MCP bridge.

## Core-rules check

- **Rule 2 (one datastore) / 3 (state vs motion):** both are read-time SurrealDB reads of committed
  state; no new table, no stored aggregate, no bus. The only stored tier stays GC's rollup.
- **Rule 5 (capability-first) / 6 (workspace wall):** both go through `query_ws`, ws-first; bucketed
  read keeps `mcp:series.read:call`, the batch reuses `mcp:series.latest:call` checked once.
- **Rule 1 (symmetric):** `either` placement, no `if cloud`.
- **Wire shape:** `series.read`'s `{buckets, width_ms}` is byte-for-byte unchanged (execution swap
  only); `series.latest_many` is additive.

## Open questions

None new. The scope's three (pushdown vs fold, one query or two, result shape) were resolved in the
scope and hold in the implementation. Only follow-up: time the pushdown on a real node/box to confirm
the "2.9 s → low tens of ms" target (the O(buckets) *shape* is already test-pinned).

## Session housekeeping — a build-lock incident (not a code issue)

While verifying, background `cargo` runs from this session (`cargo test -p lb-ingest -p lb-host`, then a
`cargo build -p federation --features postgres`) were stopped at the session layer but **kept running as
OS processes**, each holding `rust/target/debug/.cargo-lock`. Because the product host (`rubix-ai`)
builds into the **shared** `lb/rust/target` directory, its `make dev` sat at *"Blocking waiting for file
lock on build directory"* behind them. Diagnosed with `fuser -v rust/target/debug/.cargo-lock` (prints
the holding PID), killed the orphans, and confirmed the lock released. No source or config was involved —
purely orphaned processes. Takeaway for future sessions: stopping a background task in the harness does
not always reap the spawned `cargo`/`rustc`; check `fuser` on the lockfile before assuming a build is
wedged, and prefer foreground/`timeout`-bounded cargo runs for verification.
