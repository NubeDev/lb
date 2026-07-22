# Session — the store migration must run on an in-place UPGRADE (fix)

Date: 2026-07-22. Follows `docs/sessions/packs/store-datasource-session.md` (the keystone). Scope:
rubix-ai `docs/scope/packs/pack-store-datasource-scope.md` §Migration. Consumed downstream in rubix-ai
via the local `.cargo` `[patch]` (no tag this session — the release bundles this with the keystone).

## The bug

The store-seed + `migrate_sqlite_entities` path in `crates/host/src/pack/apply.rs::apply_datasource`
was gated behind `first_apply` (`= run_rules`). `run_rules` is `false` on a `Decision::Upgrade`, so a
workspace that already applied a sqlite-entity vN (has a receipt) and bumps to a store-backed vN+1 —
**the one real-world migration path** — never ran the migration. The keystone's `_5b_` test only
exercised a first apply over a pre-staged sqlite file, so the gap was invisible.

## The fix (two parts — the second was uncovered by proving the first)

1. **`apply.rs` — gate on `first_apply || upgrade`.** The migration/seed already only write into an
   EMPTY store table (`store_table_empty` per table), so running them on an upgrade preserves
   seed-ownership without the first-apply gate: a later vN+1→vN+2 upgrade over already-owned tables is
   a safe no-op, and a plain re-apply (neither first nor upgrade) still never enters the block. One
   condition changed (`let seed_this_run = first_apply || upgrade;`), plus the run-once comments.

2. **`sqlite.rs::parse_create_tables` — strip SQL comments before the `CREATE TABLE` scan.** Proving
   #1 live surfaced a DORMANT bug: `reconcile_schema` (which runs ONLY on an upgrade) scans the pack's
   `schema.sql` for the substring `create table` without stripping comments. A pack's schema header
   commonly narrates *"kept to CREATE TABLE only — …"*; the scanner read `only` as a table name, found
   the next `(`, and emitted bogus DDL that failed on `execute_batch` (near the em-dash). A fresh apply
   runs the schema through sqlite directly (comments fine), so it only bit the upgrade path. Added a
   char-based (UTF-8-safe) `strip_sql_comments` (line `--`…EOL + block `/* … */`) called at the top of
   `parse_create_tables`. This fixes `packs/bas` AND `packs/ems` (identical header comment).

## Tests (all green)

- `crates/host/tests/pack_store_test.rs::the_migration_runs_on_an_in_place_upgrade` — NEW. Applies a
  store pack v1 (establishes a receipt, store empty), stages the operator's live sqlite rows, then
  UPGRADES to v2 with `migrate_from` → asserts the `Upgrade` decision reaches the migration and the
  operator rows land in the store (seed does not clobber). A `MIGRATE_LB_DIR_LOCK` serializes the two
  tests that set the process-global `LB_DIR`.
- `crates/host/src/pack/sqlite.rs::a_create_table_inside_a_comment_is_ignored` — NEW. A `CREATE TABLE`
  in a line- or block-comment is not parsed; the real table still is.
- `cargo test -p lb-host --test pack_store_test` (8 passed) + `... --lib pack::sqlite` (8 passed) +
  `cargo fmt --all --check` clean.

## Proven live (rubix-ai, via the gateway)

Fresh isolated node (`:8199`, ws `upgws`, federation on). Applied a minimal **bas v4** (sqlite
entities: `site`/`meter`/`point` in the `demo-buildings` sqlite datasource) → store empty. Then applied
the real **`packs/bas` v5** (store) → `Decision::Upgrade`:
- `APPLIED datasource bas-readings` (was FAILED before the comment-strip fix — the reconcile parser
  error is gone);
- `migrated 3 entity row(s) from the prior sqlite 'demo-buildings' into the store — meter:1, point:1,
  site:1`;
- the store now holds the v4 operator rows (`site-001` = "V4 OPERATOR Site", etc.) — carried on the
  BUMP, seed did not clobber; `bas-readings` (288 `point_reading` rows) queryable via `federation.query`.

## Release (WORKFLOW-LB §4a — the standing follow-up)

This ships together with the store keystone from `store-datasource-session.md` (still only in the
`[patch]`, not yet tagged). Cut `node-vX.Y.Z` covering both, bump the rubix-ai pin, drop the `[patch]`.
