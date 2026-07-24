# Session — harden PR #100's direct query path (data-loss + LIMIT-pushdown fixes)

**Date:** 2026-07-24
**Branch:** `pr100` (= PR #100 `fix/datapath`, `refs/pull/100/head`) — worked in place on the shared
checkout at the user's explicit direction; **no commits/stash/branch ops** (left to the user).
**Area:** federation (query engine) — the direct fast path PR #100 added.

## The ask

PR #100 adds a direct query path that bypasses DataFusion for any single-source query (a query not
touching the synthetic `information_schema` views), keeping DataFusion only for those views. Review it,
close the two regressions it introduced, prove the dashboard uses the direct path by default, and
leave real tests + docs.

Context going in (from the review that spawned this session): DataFusion earns its keep for exactly
three jobs — `information_schema` synthesis, uniform Arrow-typed conversion, and remote-LIMIT
pushdown. The PR keeps job 1 (info_schema still routes through DataFusion) but its direct path
regressed jobs 2 and 3. Fix those; keep the fast path; keep the split automatic per-query.

## What shipped

### 1. Data-loss hole closed (`source/postgres.rs`)
The direct path's Postgres Arrow→JSON converter (`cell_to_value`) ended in `_ => Null`, silently
dropping every Arrow type without an explicit arm (`jsonb`, `uuid`, `int[]`, `bytea`, …). Changed the
catch-all to `stringify_cell(...)` — best-effort text via `arrow::util::display::ArrayFormatter` (a
genuinely-null cell is still null, handled before dispatch). See
[debugging entry](../../debugging/federation/direct-path-nulled-wide-postgres-types.md).

**Second bug found while testing:** the direct path rendered `timestamptz` as `+00:00` where the
DataFusion path (arrow_json) emits `Z`. Same instant, divergent string. Aligned to the DataFusion wire
form with `to_rfc3339_opts(SecondsFormat::AutoSi, /*use_z=*/true)`.

### 2. Remote LIMIT pushed into the source (`query.rs`, `validate.rs`)
The direct path did `json_rows.truncate(ROW_CAP)` *after* the source computed and shipped the full
result. Added `validate::cap_direct_sql` — wraps the validated SELECT under an outer `LIMIT ROW_CAP`
(`SELECT * FROM (<q>) AS lb_capped LIMIT 10000`). Dialect-agnostic, and the outer LIMIT clamps whatever
the inner produced (a user `LIMIT 50` still wins; none/`LIMIT huge` clamps to ROW_CAP). See
[debugging entry](../../debugging/federation/direct-path-limit-not-pushed-to-source.md).

Also refactored the direct branch of `register_and_run` into its own `run_direct_path` fn (one
responsibility per function, FILE-LAYOUT).

### 3. Dashboard uses the direct path by default — confirmed, not changed
The path is chosen **automatically per-query** in `validate.rs`: `is_simple` = "doesn't reference
information_schema". A `viz.query` panel target reaches `federation.query` through the generic
`call_tool_at_depth` (opaque tool id — no extension-id branching, CLAUDE.md §10), which calls
`run_query_cached → run_query_with → register_and_run`, routing on `is_simple`. So a normal panel query
already takes the direct path with **no** DataFusion plan/registration phases. Proven by
`normal_panel_query_takes_direct_path_no_datafusion_phases`: for a plain panel SELECT, the phase
timings `info_schema_reg_ms`/`table_reg_ms`/`plan_ms` are all 0; an `information_schema` probe is
asserted NON-simple (still correctly DataFusion). **No UI toggle added** — the selection stays
automatic per-query, as required.

## Tests (`rust/crates/federation/tests/direct_path_pg_test.rs`)

Real seeded Postgres (dev TimescaleDB container on :5433; skips loudly if unreachable — never a silent
green, testing-scope §0). All green:

- `direct_path_preserves_wide_types` — wide types return real values (concrete assertions), null row
  stays null. **Verified fail-before/pass-after** by reverting the catch-all to `_ => Null`.
- `direct_path_matches_datafusion_for_common_types` — direct vs DataFusion agree cell-for-cell on the
  types both carry (this caught the timestamptz divergence).
- `direct_path_pushes_limit_into_source` — exactly ROW_CAP rows AND `EXPLAIN` shows a `Limit` node;
  a user `LIMIT 5` still wins.
- `normal_panel_query_takes_direct_path_no_datafusion_phases` — the routing guarantee.
- `cap_wrap_shape` — unit check on the SQL wrap (no DB).
- `perf_direct_vs_datafusion` — `#[ignore]`d measurement.

A note on the oracle: the DataFusion per-table provider **refuses a table containing `jsonb`**
("unsupported data type: jsonb"), so it is NOT a valid oracle for the wide types — the direct path is
strictly better there. Hence test 1 asserts concrete values; the parity test covers only the types
both paths can carry. (This also means the type-parity numbers in the original review's framing were
optimistic: DataFusion doesn't merely render these types differently, it can't carry them at all.)

Two test-only seams were added to the real code (no fakes): `Source::exec_raw_for_test` /
`explain_for_test` (real writes/EXPLAIN through the actual pool) and `query::run_via_direct_for_test` /
`run_via_datafusion_for_test` / `run_with_phases_for_test` (thin wrappers over the real paths).

## Measured perf (real, warm, 10k rows)

`direct ≈ 28–40 ms` vs `datafusion ≈ 82–113 ms` (3 runs) — the direct path is ~2.5–3.5× faster on a
bounded query, and far more on an unbounded large table (the source no longer materializes the whole
result before the cap). These are measured here, not the PR's narrated numbers.

## Environment gotcha (worth remembering)

The `PostgresConnectionPool` parses its `connection_string` by splitting on whitespace into
`key=value` pairs — it does **not** understand a `postgresql://` URL, which collapses to an empty
config and fails with a bare "invalid configuration". Tests must use libpq form:
`host=localhost port=5433 user=lb password=lb_secret dbname=lb`. This initially masked as the whole PG
test suite *skipping* (a silent green — the exact trap testing-scope §0 warns about); caught by making
the skip path loud and probing the real connect error.

## Shared-checkout note (for the user)

`cargo fmt` reformatted several files another concurrent session has uncommitted in this shared tree
(`event.rs`, `host/src/viz/query.rs`, `main.rs`, `sqlite.rs`, `host/src/federation/tool.rs`,
`docs/scope/README.md`) — their compact one-line `if`s were already unformatted per the repo's fmt
rule. The changes are cosmetic-only and don't alter behavior, but they now sit in the working tree
mixed with that session's logic. Flagged so the commit split is deliberate, not a surprise.

## Open / follow-ups

- The pre-existing federation e2e stack overflow
  ([entry](../../debugging/extensions/federation-postgres-e2e-stack-overflow.md)) is unrelated and
  untouched.
- `numeric(40,0)` and `inet` hard-error in the connector's Postgres→Arrow layer on **both** paths (a
  loud failure, not the silent-null this session fixed) — out of scope here, noted for a future
  type-coverage pass.
