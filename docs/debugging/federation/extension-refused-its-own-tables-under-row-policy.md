# An extension is refused its own table and settings under an enforced row policy

- Date: 2026-10-07
- Area: federation / auth-caps (`host/src/federation`, `host/src/store_query`, sidecar `row_policy`)
- Status: fixed

## Symptom

On a node whose estate datasource had an enforced row policy (Frasers), the `waste` extension never
set up: every call it made was `host denied the call (capability/workspace gate)`. With the
extension's own token, `datasource.list` returned 200 and `store.query` 403. On a laptop with no row
policy the same extension and lb worked.

## Root cause

A native extension's backend calls lb as itself, `ext:<id>` (`native/spec.rs`). It is not a
workspace admin and no menu or grant names a site for it, so `entity_scope` gives it "nothing".
Under an enforced policy that refused, one after another:
1. `store.query` — reading its own setting (`waste_config`): any restricted caller is refused.
2. `federation.migrate` / `write` / `delete` — creating and writing its own table:
   `refuse_if_restricted`.
3. `federation.query` on its own table: the rewrite allows only policy tables.
4. `federation.query` on `point_meta_tags` (the board's site): narrowed to its sites — none.

Present since lb#221 introduced the policy (2026-09-22); no extension wrote its own data under one
before.

## Fix

- The tables an extension's migrate CREATES are recorded as its own (`federation/owned.rs`,
  reserved table `federation_table_owner`, atomic claim). On a restricted source it may migrate,
  write and delete only those, and its reads carry `own_tables`, which the sidecar leaves
  unfiltered. An existing table can never be claimed (`federation/migrate_owned.rs`).
- New `store.get {table, id}`, gated on `store:<table>:read`; an extension whose caps name the table
  exactly is not entity-limited.
- `datasource_row_policy` is now a reserved table too: before, `store.write` with `store:*:write`
  could rewrite a policy.
- (4) is not the extension's data: the `waste` tile now reads the site as the viewer.

## Regression test

`host/tests/federation_ext_own_tables_test.rs` (4; three fail without the host change),
sidecar `row_policy/own_tables_tests.rs` (4), `migrate_owned` unit tests (2), and the reserved-table
drift test now covers both tables.
