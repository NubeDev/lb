# `type::thing("series:node.cpu_temp")` mis-parses a dotted entity id (tag add fails)

- Area: tags
- Status: resolved
- First seen: 2026-06-27
- Resolved: 2026-06-27
- Session: ../../sessions/tags/tags-session.md
- Regression test: rust/crates/host/tests/tags_test.rs (`series_find_discovers_by_tags`)

## Symptom

`tags.add(entity = "series:node.cpu_temp", …)` failed with "The query was not executed due to a failed
transaction". Tagging `series:cpu` (no dot) worked; the dotted series name broke it.

## Reproduce

```sql
UPSERT type::thing('tagged', […]) SET in = type::thing($entity), …  -- $entity = "series:node.cpu_temp"
```
Ingest series names are dotted (`node.cpu_temp`), so the entity ref is `series:node.cpu_temp`.

## Investigation

- Only dotted ids failed → the one-arg `type::thing("table:id")` form parses the string and the `.` in
  the id is read as a field-access idiom, not part of the record id.
- The two-arg `type::thing($table, $id)` form takes the id as an opaque string, so dots are safe.

## Root cause

Building the entity record link from a single `table:id` string; a dotted id is mis-parsed by the
one-arg `type::thing`.

## Fix

Parse the entity into `(table, id)` and build the link two-arg. Also store the **raw** entity string on
the edge (`ent = $entity`) and return that from `find`, so a dotted id round-trips verbatim instead of
coming back backtick-escaped from `<string>in` (`series:`node.cpu_temp``).

- `rust/crates/tags/src/entity.rs` — `entity_parts(entity) -> (table, id)`.
- `add.rs` — `in = type::thing($etb, $eid)`, `ent = $entity`. `of.rs`/`remove.rs` filter with the
  two-arg form; `find.rs` selects `ent AS entity`.

## Verification

`cargo test -p lb-host --test tags_test` — `series_find_discovers_by_tags` tags `series:node.cpu_temp`
and gets it back verbatim from `series.find`.

## Prevention

Always build entity links two-arg and round-trip the raw `ent`; the regression uses a dotted series name
so a reversion fails.
