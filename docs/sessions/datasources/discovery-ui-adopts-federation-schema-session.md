# Session — Datasources UI adopts the native `federation.schema` discovery verb

**Date:** 2026-06-29
**Area:** datasources (frontend) / federation
**Symptom (UI):** Opening a datasource detail page
(`#/t/acme/datasources/timescale`) failed to list tables with
`plan: Error during planning: table 'datafusion.information_schema.tables' not found`.
The DB (`lb-timescaledb`) was up, healthy, and seeded.

## The ask

"Help me debug why the datasources page can't connect to the db." Constraint the
user added mid-session: the DB connection must stay **backend**, working over
REST/MCP, and the fix should be **good long-term**.

## Diagnosis

Not a connectivity problem. The federation engine only registers the tables a query
*references* as DataFusion `TableProvider`s — there is no `information_schema` /
`pg_class` catalog in the `SessionContext`. The UI's discovery hooks
(`useDatasourceQuery`, and a second copy in the palette's `useSqlSchema`) hand-wrote
catalog SQL and shipped it through `federation.query`, so planning failed before any
DB round-trip. SQL-dialect knowledge was duplicated in the browser; the palette
caller even hardcoded `kind="sqlite"`.

The correct backend path already existed: the native `federation.schema` verb
(`crates/host/src/federation/schema.rs` → sidecar `discover_tables`/`describe_table`),
shipped + made correct in the earlier
[federation-discovery-crash-session](federation-discovery-crash-session.md). The UI
just never adopted it.

## What changed (frontend only)

- `lib/datasources/datasource.api.ts`: added `discoverTables(source)` and
  `describeTable(source, table)` — both call `federation.schema` over the MCP bridge.
  The DB connection stays backend; the browser only sends an MCP tool call, the DSN
  never leaves the host. Exported from the lib barrel.
- `useDatasourceQuery(source)` and `useSqlSchema(source)`: rewired onto the new verbs;
  deleted both copies of the hand-written catalog SQL and the now-dead `kind`
  param/prop (also dropped `kind` from `<SqlArg>` and its `CommandPalette` call site).
- `federation.query` still backs ad-hoc/preview SELECTs (they reference real tables).

This makes the backend the single source of truth for catalog access and removes the
browser-side dialect duplication — the long-term posture the user asked for.

## Tests

- Backend: `cargo test -p lb-host --test federation_test` →
  `federation_end_to_end_postgres ... ok` (drives `federation.schema` against a real
  seeded Postgres + sidecar; asserts both `{tables}` and `{columns}` shapes). This is
  the regression lock and it already existed.
- UI unit: `pnpm test` → 167 passed; `tsc --noEmit` clean. The 4 eslint errors in
  `SqlArg.tsx` (raw `<textarea>`/`<button>`) are **pre-existing** (verified via
  `git stash`), unrelated to this change.

## Debugging entry

[datasources/discovery-via-information-schema-sql-unplannable.md](../../debugging/datasources/discovery-via-information-schema-sql-unplannable.md)
(+ `docs/debugging/README.md` index row).

## Follow-up (not done here)

`SqlArg.tsx` should migrate its raw `<textarea>`/`<button>` to the shadcn primitives
(pre-existing ui-standards lint debt).
