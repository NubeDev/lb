# `DEFINE TABLE … AS SELECT … GROUP` defines but never populates on SurrealKV (tag_counts empty)

- Area: tags
- Status: resolved (degraded to per-query)
- First seen: 2026-06-27
- Resolved: 2026-06-27
- Session: ../../sessions/tags/tags-session.md
- Regression test: rust/crates/tags/tests/tags_addons_test.rs (`materialized_counts_are_per_dimension`)

## Symptom

The materialized per-dimension count view `tag_counts` always read empty, even though the equivalent
ad-hoc `GROUP BY` over the same edges returned correct counts.

## Reproduce

```sql
DEFINE TABLE tag_counts AS SELECT count() AS n, tkey AS key FROM tagged GROUP BY key;
-- add edges via UPSERT (with in/out) …
SELECT * FROM tag_counts;                                    -- → []
SELECT count() AS n, tkey AS key FROM tagged GROUP BY key;   -- → [{key:region,n:2},{key:kind,n:1}] ✓
```
Defining the view AFTER the edges exist (backfill) was also empty.

## Investigation

- The slice-0 spike marked the `DEFINE TABLE … AS SELECT … GROUP` statement AVAILABLE — it parses and
  defines without error. The gap is **population**, not definition.
- Neither incremental maintenance (on edge UPSERT) nor backfill-at-define populated the view on
  SurrealKV (surreal 2.6, `kv-surrealkv`). The edges are written via UPSERT carrying `in`/`out`; the
  view's computed rows never appeared.

## Root cause

Materialized AS-SELECT view maintenance does not fire for our edge writes on this engine build — a
DEGRADABLE-feature reality the spike's "defines OK" check could not see (it tested DEFINE, not
populate).

## Fix

Per the spike's degrade rule and the tags scope's open question ("`tag_counts` live view vs periodically
rebuilt — measure"), compute per-dimension counts **per-query** with the working `GROUP BY`:

- `rust/crates/tags/src/counts.rs` — `count_by_key` runs `SELECT count() AS n, tkey AS key FROM tagged
  GROUP BY key`; `define_counts_view` is kept as the idempotent setup seam (ensures the edge table
  exists) where a materialized view would be (re)defined once an engine populates one — no caller change
  needed then.

## Verification

`cargo test -p lb-tags --test tags_addons_test` — `materialized_counts_are_per_dimension` asserts
region=2, kind=1 via `count_by_key` and passes.

## Prevention

The regression asserts real counts through the public verb, so the per-query path stays correct; if a
future engine populates the view, only `count_by_key`'s body changes. Note (do not oversell): these are
per-dimension counts only — faceted intersection counts are computed by the `find` traversal, never a
view.
