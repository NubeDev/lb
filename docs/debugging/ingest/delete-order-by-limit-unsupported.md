# `DELETE … ORDER BY … LIMIT 1` is unsupported (drop-oldest eviction fails to parse)

- Area: ingest
- Status: resolved
- First seen: 2026-06-27
- Resolved: 2026-06-27
- Session: ../../sessions/ingest/ingest-session.md
- Regression test: rust/crates/ingest/tests/ingest_test.rs (`best_effort_overflow_drops_oldest`)

## Symptom

The best-effort overflow eviction (drop the oldest staged row) errored:

```
Parse error: Unexpected token `ORDER`, expected Eof
 | DELETE ingest_staging ORDER BY sample.ts ASC, sample.seq ASC LIMIT 1
```

## Reproduce

Run `enforce_bound` with a full best-effort staging using the original eviction query
`DELETE <table> ORDER BY … LIMIT 1`.

## Investigation

- SurrealDB's `DELETE` does not accept `ORDER BY … LIMIT` to bound which rows are removed.
- A second quirk surfaced en route: selecting the row id and round-tripping it back as
  `serde_json::Value` fails (`invalid type: enum`) — a `Thing` id does not deserialize into JSON (same
  enum-tag mismatch as the store `record` envelope).

## Root cause

Two engine constraints: `DELETE` has no ordered-limit form, and a record id can't be carried through
host JSON.

## Fix

Delete the rows returned by a **subquery** that selects the single oldest id, with the order keys in the
projection (the selected-idiom rule), so the id never leaves the engine:

```sql
DELETE (SELECT id, sample.ts AS _ts, sample.seq AS _seq FROM ingest_staging
        ORDER BY _ts ASC, _seq ASC LIMIT 1)
```

- `rust/crates/ingest/src/overflow.rs` — `drop_oldest` rewritten as the subquery-DELETE.

## Verification

`cargo test -p lb-ingest` — `best_effort_overflow_drops_oldest` (bound 2, third sample evicts the
oldest; staging stays at the bound) passes.

## Prevention

Eviction by subquery-of-ids is the pattern for any "delete the N oldest" on this engine; never
`DELETE … ORDER BY … LIMIT`, and never round-trip a record id through host JSON.
