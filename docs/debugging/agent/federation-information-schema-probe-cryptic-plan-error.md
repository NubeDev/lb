# Agent probed `information_schema` via federation.query — cryptic DataFusion "table not found"

> **Superseded (same day):** the steering-rejection half of this fix was replaced — the probes are
> now ANSWERED read-only from the source's real catalog. See
> [../federation/information-schema-now-answered-read-only.md](../federation/information-schema-now-answered-read-only.md).
> The `federation.schema` descriptor half stands.

**Area:** agent (in-house runtime) + federation (host gate + sidecar validator)
**Date:** 2026-07-05
**Symptom:** A widget-builder run trying to find meter data issued
`SELECT … FROM information_schema.tables` through `federation.query` and got back an opaque
DataFusion plan error ("table not found"). With no discovery path it could actually use, the model
then **guessed** a table name (`meter_readings`) and failed again. The correct discovery verb —
`federation.schema` — was advertised **name-only** (no arg schema), so the model could not form the
call.

## Root cause

Two halves:

1. **Catalog schemas are unreachable by design** — the federation planner never registers
   `information_schema` / `pg_catalog`, so a probe fails deep in DataFusion with a cryptic plan
   error instead of a message naming the right verb. Every OpenAI-tool-schooled model tries the
   `information_schema` probe first; the error taught it nothing.
2. **`federation.schema` had no arg-schema descriptor.** Inventory-row tools are advertised
   name-only (the round-1 catalog fix widened *names*, not *schemas*), so the model had the verb in
   its menu but no way to know it takes `{source, table?}` — and fell back to guessing table names
   in SQL.

## Fix

- **Steering, both layers:** catalog-schema SQL is now rejected with a message that names the right
  verb — `rejected sql: catalog schemas (information_schema / pg_catalog) are not queryable through
  federation.query; call the federation.schema tool instead — {source} lists the source's tables,
  {source, table} lists a table's columns`. Host gate in
  `rust/crates/host/src/federation/validate.rs` (verified live) + the sidecar's parser in
  `rust/extensions/federation/src/validate.rs` (defense in depth).
- **A real descriptor for `federation.schema`** (`{source, table?}`,
  `rust/crates/host/src/federation/schema.rs`, registered in
  `rust/crates/host/src/tools/descriptor.rs`) so the model can form the call.

**Verified live (2026-07-05, GLM-4.6, widget-builder persona):** the retest run led with
`datasource.list → federation.schema` and issued zero `information_schema` probes and zero guessed
table names; a direct `federation.query` with `information_schema.tables` returned the steering
message.

## Regression tests

- Host gate: steering tests in `rust/crates/host/src/federation/validate.rs` (federation_test suite).
- Sidecar: steering tests in `rust/extensions/federation/src/validate.rs` unit tests.

## Follow-up (recorded)

Other inventory-row verbs still have no arg schemas — the same "model guesses args" failure mode
awaits any verb a persona leans on. Add descriptors for the agent-critical verbs next:
`viz.query`, `dashboard.save`, `store.query`, `query.run`.
