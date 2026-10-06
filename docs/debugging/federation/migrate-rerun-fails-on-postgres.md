# `federation.migrate` fails the second time on Postgres

- Date: 2026-10-06
- Area: federation (`crates/federation/src/migrate.rs`, `source/dialect.rs`, `source/postgres.rs`)
- Status: fixed

## Symptom

Applying the same design twice to a Postgres source: the first run creates the tables, the second
fails with `apply ddl: db error` — Postgres logs `relation "<table>" already exists`. The first run
had also created every FK twice (a named one and an unnamed `<table>_<col>_fkey`). Found preparing an
extension that migrates its own tables at every start-up; there was no Postgres migrate test.

## Root cause

1. Postgres had no `list_columns_with_types`; the trait default reads a table provider's Arrow
   schema, which answered EMPTY for an existing table. The planner reads "no columns" as "no table"
   and planned CREATE TABLE again.
2. The planner never read live FKs (`LiveCatalog` had no field for them) and re-emitted every
   `ADD CONSTRAINT`.
3. `plan_migrate` inlined FKs in CREATE TABLE for every engine, though its own doc says sqlite only;
   Postgres then added them again as `ADD CONSTRAINT`.

## Fix

`source/pg_catalog.rs` reads columns and FK names from `information_schema` (`current_schema()`);
`LiveCatalog.fk_names` makes the planner skip existing FKs; FKs are inlined only for sqlite.
`postgres.rs` and `dialect.rs` shrank below their FILE-LAYOUT baselines (`pg_value.rs`,
`live_types.rs` split out).

## Regression tests

- `tests/migrate_pg_test.rs` — applies a design twice against a real Postgres; the second run must
  plan nothing. Failed before (verified), passes now.
- `source/dialect_reapply_tests.rs` — an existing FK is not added again; a new Postgres table does not
  inline its FKs.
