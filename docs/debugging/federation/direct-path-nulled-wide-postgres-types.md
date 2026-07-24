# Direct query path silently nulled wide Postgres types (jsonb/uuid/array/bytea)

**Area:** federation (query engine — the direct fast path added by PR #100)
**Date:** 2026-07-24
**Symptom:** A dashboard panel querying a table with a `jsonb`, `uuid`, `int[]`, `bytea`, or other
"wide" Postgres column showed those cells as **empty/null** — even though the data was present in the
source. No error, no log: the value just vanished. Only single-source queries (the ones that take the
direct path) were affected; a query routed through DataFusion (any `information_schema` probe) showed
the value fine.

## Root cause

PR #100 added a direct query path (`rust/crates/federation/src/query.rs::run_direct_path`) that
bypasses DataFusion for any query not touching the synthetic `information_schema` views — the 90%
case, for a real perf win. For Postgres it uses a hand-written Arrow→JSON converter
(`source/postgres.rs::cell_to_value`) instead of the DataFusion path's `arrow_json::ArrayWriter`.

That converter had an explicit arm per Arrow `DataType` and ended in:

```rust
_ => serde_json::Value::Null,
```

So **any Arrow type without an explicit arm** — the Arrow representations the connector produces for
`jsonb`, `uuid`, array types, `bytea`, enums, etc. — fell through to `Null`. The cell was silently
dropped. The DataFusion path never had this hole (`arrow_json` renders every Arrow type), so the two
paths diverged: same query, same source, different (lossy) answer depending on which path ran.

## Fix

`_ => stringify_cell(col, row)` — a best-effort TEXT rendering via `arrow::util::display::
ArrayFormatter` (the same machinery Arrow's pretty-printer uses), which handles lists/structs/
decimals/etc. A genuinely-null cell is still `Null` (handled at the top of `cell_to_value` before
dispatch), so the fix never fabricates a value for a real null — it only stops **losing** a real value.

Unknown types now surface as readable text instead of disappearing. `numeric(40,0)` and `inet` are a
*different* failure mode — the connector's Postgres→Arrow layer refuses to deserialize them and the
whole query errors **loudly** on both paths; that is out of scope for this hole (a loud error is not
silent data loss).

## Second bug found while testing: timestamptz `+00:00` vs `Z`

The direct path's `Timestamp` arm rendered a tz-aware value with `dt.to_rfc3339()` → `…05+00:00`,
while the DataFusion path (arrow_json) emits `…05Z`. Same instant, different string — a spurious
"value changed" for any dashboard doing string comparison or snapshotting. Aligned the direct path to
the DataFusion wire form with `to_rfc3339_opts(SecondsFormat::AutoSi, /*use_z=*/true)`. Caught by
`direct_path_matches_datafusion_for_common_types`.

## Regression tests

`rust/crates/federation/tests/direct_path_pg_test.rs` (requires `--features postgres` + a reachable
Postgres — the dev TimescaleDB container on :5433; skips loudly otherwise):

- `direct_path_preserves_wide_types` — seeds jsonb/uuid/numeric/array/bytea/timestamptz + an all-null
  row; asserts the direct path returns their **real values** (concrete expected values) and leaves the
  null row null. **Fails against `_ => Null`** (every wide cell comes back null), passes after the fix.
- `direct_path_matches_datafusion_for_common_types` — direct vs DataFusion agree cell-for-cell on the
  types both paths carry; this is what caught the timestamptz divergence.

Verified fail-before/pass-after by reverting the catch-all to `_ => Null` (test went red at
"jsonb dropped") and restoring the fix (green).
