# Datasources scope — single-source query pushdown for `federation.query`

Status: scope (the ask). Promotes to `public/datasources/datasources.md` once shipped.

`federation.query` is 8–10× slower than the source engine it queries. A 4-table
JOIN + GROUP BY over the demo SQLite source (`demo-buildings`, ~956k `point_reading` rows)
takes **3–4 s through the sidecar vs ~0.4 s in `sqlite3` directly**. The cause is not
overhead in the host path (auth + resolve + DSN mediation are milliseconds): the sidecar's
`run_query` registers each referenced table as a plain DataFusion `TableProvider` and lets
DataFusion execute the plan itself — so every base-table row streams out of the source into
Arrow and the JOINs/aggregation run in the sidecar, when the source engine could have run the
whole statement and returned 5 rows. This scope makes the sidecar **push the entire validated
SELECT down to the source engine** whenever every table in the plan comes from one source —
which for `federation.query {source, sql}` is *always*, since the verb takes exactly one source.

> Read with: `datasources-scope.md` (the parent — the `federation` extension, verbs, caps),
> `federation-paging-scope.md` (slice D of page-chaining — pushdown for *paging predicates*;
> this scope generalizes to the whole statement), `sqlite-datasource-demo-scope.md` (the demo
> source this was measured on), README §3 rules 2/5, §6.3.

## Goals

- A single-source `federation.query` executes **inside the source engine** (SQLite, Postgres,
  Timescale); only the result rows cross the provider boundary. Target: the demo query above
  within ~2× of direct-engine time (well under 1 s), not 8–10×.
- **No contract change.** Same verb, same `{source, sql}` input, same `{columns, rows}`
  envelope, same `ROW_CAP` bound, same SELECT-only validation (both host- and sidecar-side).
  Callers — the agent, Data Studio, dashboards, rules `source(...)` — see the same answers,
  faster.
- The `federation.schema` / `federation.sample` discovery paths and the `information_schema`
  synthesis keep working unchanged.
- The fix lives **entirely inside the `federation` extension** (Cargo features + the
  `SessionContext` wiring in `query.rs`). No core crate changes.

## Non-goals

- **Cross-source federation** (joining two registered sources in one query). The verb is
  single-source by design; this scope doesn't change that.
- **Connection/pool reuse across calls.** `run_query` reconnects per call today (~ms for
  SQLite, more for Postgres). Real, but a separate, stateful concern — the sidecar is
  supervised and currently stateless per call; a pool cache is its own small scope if the
  per-call connect ever dominates. Out of scope here.
- Paging/cursor semantics — `federation-paging-scope.md` owns those; this scope just makes
  the pushdown machinery they assumed actually exist for whole statements.
- MySQL/other kinds beyond what's shipped (`sqlite`, `postgres`/`timescale`).

## Intent / approach

Use the pushdown support **already present in the pinned dependency** rather than writing any
translation ourselves. `datafusion-table-providers` 0.11 ships per-engine federation features
(`sqlite-federation`, `postgres-federation`) that wrap each `TableProvider` in a
`datafusion-federation` adaptor; with the federation optimizer rule installed in the
`SessionState`, DataFusion detects that every table in a plan belongs to the same source,
**unparses the plan back to that engine's SQL dialect, and executes it remotely** — the
sidecar only shapes the (small) result batches. Concretely:

1. `rust/extensions/federation/Cargo.toml` — add `sqlite-federation` to the
   `datafusion-table-providers` feature list (and `postgres-federation` under the existing
   `postgres` feature), plus the matching `datafusion-federation` dependency for the
   session-state wiring. Heavy deps stay quarantined in the extension (parent-scope rule).
2. `query.rs::register_and_run` (and `catalog_rows`) — build the `SessionContext` from a
   federation-enabled `SessionState` (analyzer rule + query planner) instead of
   `SessionContext::new()`, and register the factory's **federated** provider variant. The
   existing `df.limit(0, Some(ROW_CAP))` stays — under pushdown it unparses to a `LIMIT`
   executed remotely, which is strictly better than the current client-side cap.
3. The `Source` trait grows a federated-provider accessor (or the per-kind sources return
   already-wrapped providers under the feature) — one small seam, both engines behind it.

**Alternative rejected:** bypass DataFusion for single-source calls and hand the validated SQL
string straight to rusqlite/tokio-postgres. Simpler on paper, but it forks the execution path
per engine (two dialects, two row-shaping codepaths, two `ROW_CAP` enforcements), silently
drops the `information_schema` synthesis, and re-opens the SELECT-only guarantee to per-engine
dialect quirks the validator wasn't written for. The engine-agnostic seam is the parent
scope's whole design; pushdown keeps it and removes only the wasted row movement.

**Known wrinkle to verify, not assume:** the existing `COUNT(*)` zero-column-scan workaround in
`register_and_run` (the "Physical input schema should be the same" steer) exists *because* of
the non-pushdown provider. Under pushdown, `COUNT(*)` unparses to remote SQL and should just
work — the build must test it and either delete the steer or keep it for any residual
non-pushdown fallback path.

## How it fits the core

- **Tenancy / isolation:** unchanged. Source resolution is workspace-pinned in the host
  (`federation/record.rs::resolve`) before the sidecar ever sees the call; pushdown changes
  *where the plan executes*, not *who may reach the source*.
- **Capabilities:** unchanged — `mcp:federation.query:call` gates the verb; `net:*` gates the
  endpoint; the DSN is mediated under the federation extension's own grant. Deny paths are
  untouched (and re-asserted by the existing tests).
- **Placement:** either role — the sidecar runs wherever the node runs; symmetric (rule 1).
- **MCP surface:** no new verbs, no schema change. Read-only `federation.query` stays the one
  shape; `federation.schema`/`sample` unchanged. (§6.1: pure read, no CRUD/feed/batch added.)
- **Data (SurrealDB):** untouched. SurrealDB is still never a DataFusion source (rule 2) —
  pushdown only concerns the *external* engine's own tables.
- **Bus / Sync / Secrets:** N/A / N/A / unchanged (DSN handling identical).
- **SELECT-only, both gates:** the host validates, the sidecar validates, *then* plans. The
  unparser emits SQL derived from the already-validated logical plan — no widening. The
  regression suite must confirm write statements still refuse identically.
- **SDK/WIT impact:** none — native sidecar internals, no plugin-boundary change.
- **Skill doc:** N/A — no new drivable surface; `federation.query` is unchanged for callers.
  (Existing datasource skills/docs need no edit beyond the perf note on ship.)

## Example flow

1. Caller posts `federation.query {source: "demo-buildings", sql: "SELECT s.name, AVG(r.value) … 4-table JOIN … GROUP BY …"}`.
2. Host authorizes, resolves the source in the caller's workspace, validates SELECT-only,
   enforces `net:*`, mediates the DSN, hands `{kind, dsn, sql}` to the sidecar (all unchanged).
3. Sidecar re-validates, connects, registers the four referenced tables as **federated**
   providers in a federation-enabled `SessionContext`.
4. DataFusion plans; the federation analyzer sees one source for the whole plan and unparses
   it (JOINs + GROUP BY + ORDER BY + the `ROW_CAP` LIMIT) into a single SQLite statement.
5. SQLite executes it in-engine (~0.4 s) and returns **5 aggregated rows**; the sidecar shapes
   them into `{columns, rows}`. Previously step 4–5 moved ~956k rows into Arrow and joined
   them in the sidecar (3–4 s).

## Testing plan

Per `scope/testing/testing-scope.md` — all against the **real** engines (mandatory categories
first):

- **Capability deny:** existing `federation.query` deny tests must stay green (no cap change).
- **Workspace isolation:** existing cross-tenant resolve tests must stay green.
- **Correctness (the heart of it):** in `federation_sqlite_test.rs` (no-Docker path), run the
  demo-shaped JOIN + GROUP BY + ORDER BY query pre-seeded into a real SQLite file and assert
  the exact `{columns, rows}` answer matches the non-pushdown expectation. Repeat for the
  Postgres path under the Docker-gated test if available in the session.
- **Pushdown is actually happening:** assert via the plan (`EXPLAIN` through the context, or
  the federation adaptor's physical-plan node type) that a multi-table single-source query
  produces one federated scan — not a perf-timing assertion (flaky), a *structural* one. Add
  a coarse sanity timing to the session log, not the test.
- **ROW_CAP:** a query matching > `ROW_CAP` rows still returns exactly `ROW_CAP` (now via
  remote LIMIT).
- **SELECT-only regression:** `INSERT`/`UPDATE`/`DELETE`/multi-statement inputs still refuse
  with the same errors.
- **`COUNT(*)` wrinkle:** test bare `COUNT(*)` under pushdown; delete or re-justify the
  steer message accordingly (this is a behavior change to document either way).
- **Discovery unchanged:** `federation.schema` list/describe and `federation.sample` green on
  both kinds.

## Risks & hard problems

- **Unparser dialect gaps.** The plan→SQL unparser may emit SQL a given engine rejects for
  exotic constructs (functions, casts). Mitigation: the adaptor falls back to per-table scans
  when it can't push down — verify the fallback path still answers correctly (it's today's
  behavior), and log which shape fell back.
- **Version lock.** `datafusion-federation` must match datafusion 53 / providers 0.11 exactly;
  a mismatch drags a second arrow/sqlparser into the build. Pin explicitly.
- **Answer drift.** In-engine execution can differ from DataFusion execution at the margins
  (float aggregation order, NULL sort position, collation). The correctness tests pin the
  demo answers; any drift found is a finding to document, not silently accept.
- **`information_schema` synthesis** registers *synthesized* in-memory views next to federated
  providers — a mixed plan can't fully push down. Confirm mixed queries still work (fallback),
  since `validate.rs` already flags `wants_info_*`.

## Open questions

1. Does providers 0.11's `SqliteTableFactory` expose the federated provider directly, or do we
   wrap with `datafusion-federation`'s `FederatedTableProviderAdaptor` ourselves? (Answer at
   build time from the crate source; both are small.)
2. Keep or delete the `COUNT(*)` steer once pushdown lands? (Decide from the test result —
   keep it only if the fallback path can still hit the upstream bug.)
3. Should `catalog_rows` (discovery) also go federated, or stay on plain providers? (Cheap
   either way — discovery reads tiny catalogs; do it only if it falls out for free.)

## Related

- `datasources-scope.md` — the parent extension scope (verbs, caps, doctrine).
- `federation-paging-scope.md` — slice D of page-chaining; assumed predicate pushdown, this
  delivers statement pushdown.
- `sqlite-datasource-demo-scope.md` — the `demo-buildings` source the regression was measured on.
- `public/datasources/datasources.md` — gains the perf note on ship.
- Code: `rust/extensions/federation/src/query.rs`, `src/source/{mod,sqlite,postgres}.rs`,
  `rust/crates/host/src/federation/query.rs` (unchanged, for context),
  `rust/crates/host/tests/federation_sqlite_test.rs`.
- README §3 (rules 2/5), §6.3.
