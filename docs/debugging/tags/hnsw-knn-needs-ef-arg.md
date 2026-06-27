# HNSW `<|K|>` knn search returns nothing; the two-arg `<|K,EF|>` form is required

- Area: tags
- Status: resolved
- First seen: 2026-06-27
- Resolved: 2026-06-27
- Session: ../../sessions/tags/tags-session.md
- Regression test: rust/crates/tags/tests/tags_addons_test.rs (`vector_search_returns_nearest_and_rejects_dim_mismatch`)

## Symptom

Vector nearest-neighbour search over the HNSW index returned an empty set even with stored embeddings.

## Reproduce

```sql
SELECT <string>id AS id, vector::distance::knn() AS dist FROM tag_vector
WHERE embedding <|2|> $q ORDER BY dist ASC          -- → []
```

## Investigation

- The rows existed (a plain `SELECT … FROM tag_vector` returned them).
- The single-arg knn operator `<|K|>` returned nothing; the two-arg `<|K,EF|>` form (K neighbours, EF
  search breadth) returned the expected ordered neighbours.
- With `<|K,EF|>` the results come back already ordered by ascending distance, so the explicit
  `ORDER BY vector::distance::knn()` (which itself tripped the selected-idiom rule) is unnecessary.

## Root cause

The single-arg `<|K|>` HNSW operator does not drive the index search on this engine build; the EF
parameter is required.

## Fix

- `rust/crates/tags/src/vector.rs` — `find_similar` uses `embedding <|{k},{ef}|> $q` (ef = `max(k*4,40)`),
  no ORDER BY; returns the caller's logical `vid` (stored alongside the embedding) rather than the
  composite record id.

## Verification

`cargo test -p lb-tags --test tags_addons_test` — `vector_search_returns_nearest_and_rejects_dim_mismatch`
gets `a` then `c` nearest to `[1,0,0]`, and a wrong-dimension write is rejected (not stored).

## Prevention

Always use `<|K,EF|>` for HNSW search; the regression asserts ordered neighbours, so the single-arg form
(empty result) fails it.
