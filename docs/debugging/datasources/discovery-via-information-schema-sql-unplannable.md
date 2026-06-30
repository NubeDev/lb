# Datasources page can't connect: `table 'datafusion.information_schema.tables' not found`

**Area:** datasources · **Status:** resolved · **Date:** 2026-06-29

## Symptom

Opening a datasource detail page (e.g. `#/t/acme/datasources/timescale`) failed to
list tables. The sidecar returned:

```
extension error: supervisor: child returned an error:
plan: Error during planning: table 'datafusion.information_schema.tables' not found
```

The DB (`lb-timescaledb`) was up, healthy, and seeded — connectivity was never the
problem.

## Root cause

The federation engine is **not** a Postgres pass-through. For each query it parses the
SQL, extracts the referenced tables, registers *each* as a DataFusion `TableProvider`,
then runs the query through a `SessionContext`. Only the explicitly-referenced tables
exist in that context — there is **no `information_schema` / `pg_class` catalog**.

The UI's discovery path (`useDatasourceQuery`, and a second copy in the command
palette's `useSqlSchema`) hand-wrote catalog SQL —

```sql
SELECT t.table_name ... FROM information_schema.tables t
LEFT JOIN pg_class c ON c.relname = t.table_name ...
```

— and shipped it through `federation.query`. DataFusion has no provider named
`information_schema.tables`, so **planning failed** before any DB round-trip.

This was also a layering violation: SQL-dialect knowledge (postgres vs sqlite catalog
shapes) lived in the **browser**, duplicated across two hooks, and the palette caller
even hardcoded `kind="sqlite"` for every source.

## Fix

Discovery now goes through the **native `federation.schema` verb**, which already
existed backend-side exactly for this (`crates/host/src/federation/schema.rs` →
sidecar `query::discover_tables` / `describe_table`). It registers the source's *own
catalog table* (`pg_catalog.pg_tables`) as a provider under a bare alias so DataFusion
*can* plan the listing, and reads columns straight off the provider's real Arrow
schema (no catalog SQL at all). Dialect selection lives in one place backend-side
(`query::list_tables_plan`).

UI changes:
- Added `discoverTables(source)` / `describeTable(source, table)` to
  `lib/datasources/datasource.api.ts` (both call `federation.schema` over the MCP
  bridge). The DB connection stays entirely backend — the browser only sends an MCP
  tool call; the DSN never leaves the host.
- Rewired `useDatasourceQuery` and `useSqlSchema` onto those verbs; deleted both
  copies of the hand-written catalog SQL and the now-dead `kind` params/props
  (`useDatasourceQuery(source)`, `useSqlSchema(source)`, `<SqlArg>` lost its `kind`).
- `federation.query` is still used for ad-hoc/preview SELECTs (those reference real
  tables, so they plan fine).

## Regression coverage

`crates/host/tests/federation_test.rs::federation_end_to_end_postgres` already drives
`federation.schema` against a real seeded Postgres + the supervised sidecar and asserts
both discovery shapes (`{tables}` and a table's `{columns}`) return real data — this is
the long-term lock on the backend path. UI unit suites stay green.

## Lesson

The federation engine only knows the tables a query *names*. Any "browse the source"
need (tables, columns) must use the native `federation.schema` discovery verb — never
catalog SQL through `federation.query`. Keep dialect knowledge backend-side; the UI
asks "what's here?", it does not write catalog SQL.
