# `series.read(seq <= u64::MAX)` returns nothing (huge int coerces to a float and mis-compares)

- Area: ingest
- Status: resolved
- First seen: 2026-06-27
- Resolved: 2026-06-27
- Session: ../../sessions/ingest/ingest-session.md
- Regression test: rust/crates/ingest/tests/ingest_test.rs (`write_commit_read_round_trips_typed`, reads with open bounds) + durable_redrain_test.

## Symptom

`series.read` with an open upper bound expressed as `to_seq = u64::MAX` returned an empty set even
though committed samples existed. A range of `(0, 100)` worked; `(0, u64::MAX)` returned 0 rows.

## Reproduce

```rust
read(&store, "acme", "m", 0, u64::MAX).await  // → [] even with committed samples
```
The query was `... WHERE seq >= $from AND seq <= $to` with `$to` bound to `u64::MAX`.

## Investigation

- `(0, 100)` matched; only the `u64::MAX` upper bound failed → not a binding/namespace problem.
- SurrealDB coerces a near-`2^64` integer toward a float; the `seq <= <that value>` comparison then
  mis-evaluates to false for ordinary `seq` values (the sentinel never behaves as "+∞").

## Root cause

Using `u64::MAX` as a sentinel for "no upper bound" — a value the engine cannot represent exactly as an
integer in the comparison, so the predicate silently excludes everything.

## Fix

Make the range bounds `Option<u64>` and **omit the clause entirely** when a bound is open, rather than
bind a sentinel:

- `rust/crates/ingest/src/read.rs` — `from_seq`/`to_seq: Option<u64>`; build `seq >=`/`seq <=` only when
  `Some`.
- `rust/crates/host/src/ingest/{read.rs,tool.rs}` — pass `None` when `from_seq`/`to_seq` are absent
  from the MCP input (never a `u64::MAX` default).

## Verification

`cargo test -p lb-ingest` — round-trip + durable re-drain tests read with open bounds (`None,None`) and
return the full committed set.

## Prevention

Never use a max-int sentinel for "unbounded" against this engine — express open bounds as the absence of
the clause. The regression tests read with open bounds, so a re-introduced sentinel fails.
