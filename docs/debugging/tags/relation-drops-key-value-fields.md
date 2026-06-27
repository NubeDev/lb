# A `tagged` edge silently drops fields literally named `key` / `value`

- Area: tags
- Status: resolved
- First seen: 2026-06-27
- Resolved: 2026-06-27
- Session: ../../sessions/tags/tags-session.md
- Regression test: rust/crates/tags/tests/tags_test.rs (`add_then_of_returns_the_tag`, `find_exact_key_only_and_faceted`)

## Symptom

After `tags.add`, reading the edge back showed `key: null` and `value: null` even though the UPSERT did
`SET key = $key, value = $value`. `of`/`find` then failed to deserialize ("expected a string, found
None") or returned empty.

## Reproduce

```sql
UPSERT type::thing('tagged', [$entity,$key,$value,$source]) SET
  in = type::thing($entity), out = type::thing('tag',[$key,$value]),
  key = $key, value = $value, at = $at, by = $by, source = $source, ...;
SELECT key, value FROM tagged;   -- key/value come back null
```
The same `SET key=…, value=…` on the `tag` NODE table (no `in`/`out`) persists correctly.

## Investigation

- Only `key` and `value` were null; `at`/`by`/`source`/`confidence` persisted fine.
- The differentiator is `in`/`out`: a row carrying them is treated as a graph edge, and the field names
  `key`/`value` collide with edge-internal semantics — they are dropped on write.
- Renaming the same data to `tkey`/`tval` on a plain table (and on the edge) persisted correctly.

## Root cause

`key`/`value` are reserved/special on an edge row (one with `in`/`out`); a user `SET key = …` is
silently discarded.

## Fix

Denormalize the tag key/value onto the edge under non-colliding names `tkey`/`tval`, and alias them back
to `key`/`value` on read:

- `rust/crates/tags/src/add.rs` — edge sets `tkey = $key, tval = $value` (the `tag` node keeps
  `key`/`value`).
- `of.rs` — `SELECT tkey AS key, tval AS value …`. `find.rs` / `remove.rs` / `counts.rs` filter/group on
  `tkey`/`tval`.

## Verification

`cargo test -p lb-tags` — `add_then_of_returns_the_tag` and `find_exact_key_only_and_faceted` pass; the
edge round-trips key/value.

## Prevention

Never name an edge (RELATE/`in`-`out`) field `key` or `value`; the regression tests read the tag back
through `of`/`find`, so a reversion fails loudly.
