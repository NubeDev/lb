# Direct query path fetched the full result then truncated (ROW_CAP not pushed to the source)

**Area:** federation (query engine — the direct fast path added by PR #100)
**Date:** 2026-07-24
**Symptom:** A direct-path query over a large table did the FULL source-side work (scan, sort,
aggregate over every row) and shipped every row across the wire, only to discard everything past
`ROW_CAP` (10 000) on the sidecar. On an unbounded `SELECT * FROM huge` this is the source computing
and transmitting millions of rows to keep 10 000 — the exact cost the row cap exists to avoid.

## Root cause

`rust/crates/federation/src/query.rs`, the direct branch:

```rust
let (columns, mut json_rows) = source.query_direct_json(sql).await?;
json_rows.truncate(ROW_CAP);   // <-- client-side, AFTER the source did all the work
```

The DataFusion path caps *before* execution via `df.limit(0, Some(ROW_CAP))`, which the federation
unparser turns into a **remote `LIMIT`** the source engine applies — so the source returns only capped
rows. The direct path skipped that and truncated after the fact, losing the pushdown.

## Fix

Wrap the validated SQL under an outer `LIMIT ROW_CAP` before sending it
(`validate.rs::cap_direct_sql`):

```sql
SELECT * FROM ( <validated user SELECT> ) AS lb_capped LIMIT 10000
```

Chosen over editing the inner query's own `LIMIT` node because it is **dialect-agnostic** (Postgres
and SQLite both accept a derived-table + outer LIMIT — no per-engine `LimitClause` surgery) and the
outer LIMIT **clamps whatever the inner produced**: a user `LIMIT 50` yields ≤50 (inner wins), a user
`LIMIT 1000000` or none is clamped to ROW_CAP (outer wins). "Use the smaller" falls out of the nesting
— no literal comparison needed. The input is already `validate_select`-approved (exactly one read-only
SELECT), so the wrap can't smuggle a second statement or a write.

## Regression test

`direct_path_pushes_limit_into_source` (`tests/direct_path_pg_test.rs`): seeds `ROW_CAP + 50` rows,
asserts exactly `ROW_CAP` come back, AND `EXPLAIN`s the exact wrapped SQL the direct path sends to
prove the plan Postgres executes carries a `Limit` node (the bound reached the engine, not the
sidecar). Also asserts a user `LIMIT 5` still wins. Green.

## Measured effect

Not the point of this bug (correctness/cost, not latency), but for the record a warm 10k-row query
now runs **direct ≈ 28–40 ms vs DataFusion ≈ 82–113 ms** (`perf_direct_vs_datafusion`, `--ignored`).
On an *unbounded* large table the win is far larger, since the source no longer materializes the whole
result.
