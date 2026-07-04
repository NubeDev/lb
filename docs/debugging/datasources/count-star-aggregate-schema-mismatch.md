# `federation.query` with a column-less aggregate (`count(*)`) fails + crash-loops the sidecar

- Area: datasources (federation extension — DataFusion / `datafusion-table-providers` Postgres pushdown)
- Status: **open** (root-caused; interim workaround known; complete fix pending)
- First seen: 2026-07-04
- Session: ../../sessions/testing/datasources-dashboard-charts-session.md
- Regression test: `rust/crates/host/tests/federation_test.rs::federation_count_star_columnless_aggregate`
  (`#[ignore]` — a fails-until-fixed dead-drop; run with `--ignored`; **verified fails-before** 2026-07-04)

## Symptom

Driving a real seeded TimescaleDB through `federation.query` (the datasources e2e runbook), a bare
`count(*)` over a datasource table returns an internal error instead of a row count:

```
$ curl -s -X POST $BASE/mcp/call -H "$A" -H "$C" \
    -d '{"tool":"federation.query","args":{"source":"timescale","sql":"SELECT count(*) AS n FROM point_reading"}}'
extension error: supervisor: child returned an error: execute: Internal error: Physical input
schema should be the same as the one converted from logical input schema. Differences:
	- Different number of fields: (physical) 1 vs (logical) 0.
This issue was likely caused by a bug in DataFusion's code. ...
```

Retried a few times (e.g. a page that re-issues the query), the child panics repeatedly and the
supervisor gives up:

```
extension error: supervisor: restart budget exhausted after 5 restarts
```

The sidecar recovers once a *good* query succeeds (restart budget resets), so it is transient, but a
"total count" dashboard tile bound to a datasource is dead, and a retrying client can briefly take the
whole federation sidecar down.

## Reproduce

1. `docker/postgres` up + `./seed.sh` (or any Postgres datasource), `make dev`, log in, mint `$TOKEN`.
2. Call `federation.query` on the `timescale` source with each SQL below.

| SQL | Result |
|---|---|
| `SELECT count(*) AS n FROM point_reading` | ❌ Internal error (physical 1 vs logical 0) |
| `SELECT count(1) AS n FROM point_reading` | ❌ same |
| `SELECT sum(1) AS n FROM point_reading` | ❌ same (and exhausts restart budget on retry) |
| `SELECT count(*) FROM (SELECT 1 AS c FROM point_reading) t` | ❌ same (optimizer re-prunes to empty) |
| `SELECT count(value) AS n FROM point_reading` | ✅ `475974` |
| `SELECT point_id, count(*) AS n FROM point_reading GROUP BY point_id` | ✅ (grouped keeps a scanned column) |
| `SELECT avg(value)`, `max(value)`, `min(value)` | ✅ |

The one common factor of the failures: **the aggregate references no table column**, so the scan
handed to the Postgres `TableProvider` projects **zero** columns.

## Investigation

- Not a false-bug: DB seeded (`SELECT count(*) FROM point_reading` in `psql` → `475974`), federation
  enabled (`datasource.list` shows `timescale`), node freshly rebuilt, time unit fine (the value/time
  reads render correctly — see the session doc). Ruled all four Step-4 causes out first.
- The trigger is a **column-less aggregate**. DataFusion's optimizer prunes the base-table scan to an
  empty projection for `count(*)` / `count(<literal>)` / `sum(<literal>)`. The pushed-down Postgres
  provider (`datafusion-table-providers` 0.11, datafusion 53.1) then yields a physical input schema
  with 1 field while the logical input has 0 — datafusion 53's **new aggregate schema verifier**
  rejects the mismatch (`Physical input schema should be the same …`). That verifier's own doc
  (`datafusion-common` `execution.skip_physical_aggregate_schema_check`) says it exists to catch
  planner bugs and the flag is the sanctioned workaround.
- **Flipping `skip_physical_aggregate_schema_check = true` is necessary but NOT sufficient.** Past the
  verifier, the empty-projection scan reaches Arrow's `BatchCoalescer`, which then fails with
  `Batch has 0 columns but BatchCoalescer expects 1`. Confirmed live: with the flag on, the error
  changes from the schema-verifier message to the coalescer message — the empty scan is the real
  defect, not just the check.
- **Wrapping the table** (`… FROM (SELECT 1 AS c FROM t)`) does **not** help: the optimizer re-prunes
  the inner projection to empty for a `count(*)`. The **only** shape that works is one where the
  aggregate argument is a real column (`count(value)`), which forces the scan to keep that column.

## Root cause

`datafusion-table-providers`' Postgres provider does not correctly handle an **empty-projection scan**
(the plan `count(*)` produces): it reports a 1-field physical schema against a 0-field logical input
and emits a 0-column batch the coalescer rejects. It is upstream of our code (in the provider /
datafusion planner interaction), surfaced through our `federation.query` path
(`rust/extensions/federation/src/query.rs::register_and_run`).

## Fix (interim + planned)

- **Interim workaround (documented, no code):** rewrite a "count all rows" query to reference a
  NOT-NULL column — `SELECT count(<pk_or_notnull_col>) FROM t` — which returns the correct count.
  For `point_reading` use `count(value)`; for the general case any NOT NULL column (NULL semantics of
  `count(col)` vs `count(*)` differ only on nullable columns).
- **Planned complete fix (not landed this session):** in `register_and_run`, detect a column-less
  aggregate (via the already-parsed sqlparser AST from `validate_select`) and rewrite the aggregate
  argument to a concrete NOT-NULL column from the table's Arrow schema (which the provider already
  exposes via `describe_table`), combined with enabling `skip_physical_aggregate_schema_check`. This
  keeps `count(*)` semantics (row count) while guaranteeing a non-empty scan. Deferred here because a
  correct, general AST rewrite (WHERE/HAVING/DISTINCT/multiple aggregates, NOT-NULL column selection)
  is more than a drive-by change and warrants its own scoped session + tests.

## Regression test

`rust/crates/host/tests/federation_test.rs::federation_count_star_columnless_aggregate` — spawns the
real `postgres:16-alpine` fixture, asserts `count(seq)` (working shape) returns 5, then asserts bare
`count(*)` also returns 5. **Marked `#[ignore]`** (fails-until-fixed) so the assumed-green default run
is undisturbed; verified to **fail-before** on 2026-07-04 with the exact `Physical input schema …
(physical) 1 vs (logical) 0` panic. When the planned fix lands, drop `#[ignore]` and it passes.
