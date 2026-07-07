# Query-builder-common scope — one builder, N dialect emitters

Status: **SHIPPED (2026-07-06)** — the Query Builder is common across dialects. A federation target
gets the same Builder⇄Code editor a surreal target gets; the dialect is behind a seam; the wire shape
(`federation.query {source, sql}`) is unchanged. Open questions OQ #1–#6 all resolved as recommended
below. Session
[`sessions/frontend/query-builder-common-session.md`](../../sessions/frontend/query-builder-common-session.md);
public [`public/frontend/data-studio.md`](../../public/frontend/data-studio.md) ("Query builder —
common across dialects"). Original ask below.

A LOCAL TABLE source (SurrealDB, `store.query`) gets the interactive Builder⇄Code editor
(table/column/WHERE/GROUP/ORDER/LIMIT rows with a live SQL preview); an external DATASOURCE
(`federation.query` — postgres/timescale/sqlite, e.g. `demo-buildings`) gets only a raw-SQL
textarea. **The builder UI is already generic** — only the SQL *dialect emitter* is Surreal-specific.
Lift the deferral recorded in [`dashboard/viz/datasource-binding-scope.md`](dashboard/viz/datasource-binding-scope.md)
("Deferred (named, not silent): `federation.datasource.schema` … a federation target uses the
raw-SQL editor until it lands"): its prerequisite — a real discovery verb — has since shipped as
`federation.schema {source, table?}` ([`federation/schema.rs`](../../../../rust/crates/host/src/federation/schema.rs),
debugged in [`debugging/agent/federation-information-schema-probe-cryptic-plan-error.md`](../../debugging/agent/federation-information-schema-probe-cryptic-plan-error.md)).
Make the builder **common**: one structured-query state, N dialect emitters; the federation target
reuses the exact `SqlQueryEditor` the surreal target uses, fed by `federation.schema` for its
dropdowns and emitting dialect-correct standard SQL.

## Goals

- **One builder component, not a fork.** A federation target opens the same `SqlQueryEditor`
  (Builder ⇄ Code) the surreal target opens today — same rows, same Code→Builder confirm-on-clobber
  behaviour, same live preview. The datasource `kind` (`surreal` vs `sqlite`/`postgres`/`timescale`)
  selects the **dialect emitter** behind a seam; it never branches the component tree (rule 10 —
  `kind` is config data, the datasource *name* stays opaque).
- **An emitter seam, dialect per file.** Keep `panel-kit/sql/query.ts` as the shared
  `SqlBuilderQuery` state. Keep `toSurrealQL.ts` as ONE emitter impl. Add a standard-SQL emitter
  for federation, plus the dialect dispatch. The subset the builder can express
  (SELECT/FROM/WHERE/GROUP BY/ORDER BY/LIMIT) is ANSI; the deltas to mind are identifier quoting,
  aggregate spelling, and time-bucket ergonomics (a chart's `time-series` format hint).
- **Code stays the escape hatch both ways.** Builder → Code is free (regenerate the string); Code →
  Builder confirms (hand-edited SQL may not round-trip). Identical to today's surreal behaviour —
  verified against how `state.sql.rawSql` round-trips through `cellEditorState.ts`.
- **Federation dropdowns from `federation.schema`.** The builder's table/column dropdowns for a
  federation target are populated by the SHIPPED `federation.schema {source, table?}` verb — reusing
  the existing `discoverTables`/`describeTable` client ([`datasource.api.ts`](../../../../ui/src/lib/datasources/datasource.api.ts)),
  the same load pattern `SqlQueryEditor` already uses for local `store.schema` (`readSchema()`).
  **No second schema-fetch path.**
- **The wire shape does not change.** `federation.query { source, sql }` stays the args; the emitter
  just produces the `sql` string. `viz.query` keeps rendering it. No new verb, cap, table, or
  outbox target (the platform checklist below).

## Non-goals

- **No new host verb / cap / table.** `federation.schema` and `federation.query` are shipped and
  gated under `mcp:federation.query:call`; this scope adds NO MCP surface (the work is UI + a TS
  emitter module).
- **No rail Sources-tree drill into federation tables/columns.** The rail's `CatalogExplorer`
  ([`SourcesPane.tsx`](../../../../ui/src/features/data-studio/panes/SourcesPane.tsx)) shows a
  federation *datasource row* today; drilling into its tables/columns is a SEPARATE composition over
  the `@nube/source-picker` loader seam (a new `readFederationSchema` loader + a `fedSchema` section
  kind). It composes onto this slice cleanly but is NOT required for the headline (the builder loads
  its own dropdown schema, exactly as it does for local tables today). Filed as a named follow-up,
  not silently dropped.
- **No per-dialect emitter file explosion.** One standard-SQL emitter covers sqlite/postgres/
  timescale for the SELECT subset the builder can produce. Split into per-dialect files ONLY when a
  real delta forces it (FILE-LAYOUT rule 8 cuts both ways — a premature split is its own clutter).
- **No SurrealQL-for-federation.** SurrealDB stays the one datastore; a federation target emits
  standard SQL for the external engine, never SurrealQL (rule 2).
- **No builder features the surreal path doesn't have.** A join row, subqueries, CTEs, window
  functions, parametrized SQL, write statements — none are in scope for either dialect; both stay
  SELECT-only, leashed by the host's parse-allowlist (`store.query`) and SELECT-only sidecar
  validation (`federation.query`).
- **No deep-link / share-URL changes.** Routing is untouched.

## Intent / approach

The model is **one structured query, N dialect emitters** — Grafana's `grafana-sql` architecture,
which the shipped `panel-kit/sql/` already half-implements. Three pieces, one responsibility each:

1. **The shared state stays put.** `panel-kit/sql/query.ts` — `SqlBuilderQuery`
   (table/columns/filters/groupBy/orderBy/limit) + `SqlSourceState` (mode + rawSql + builder +
   format). UNCHANGED. This is the typed editor state both Builder and Code mutate; `cellEditorState.ts`
   already round-trips it through `cell.options.sql` for both ADD and EDIT.

2. **The emitter seam.** A new `panel-kit/sql/dialect.ts` owns a `SqlDialect` enum
   (`"surreal" | "standard"`) + `emitSql(dialect, query): string` dispatch. `toSurrealQL.ts` stays
   one impl ( SurrealQL specifics: `math::sum(col)` / `count()` / bare identifiers / `LIMIT n`).
   A new `panel-kit/sql/toStandardSql.ts` is the federation impl — ANSI SELECT over the same
   `SqlBuilderQuery`, double-quoted identifiers (the `ident()` helper in
   [`useDatasourceQuery.ts:21`](../../../../ui/src/features/datasources/useDatasourceQuery.ts) already
   shows the quoting rule), `SUM(col)`/`COUNT(*)`/`COUNT(col)`, `LIMIT n`. The dialect is picked
   from the target's datasource `kind`, never a hardcoded datasource name (rule 10).

3. **The component takes the dialect + a schema loader.** `SqlQueryEditor` and `VisualEditor` gain a
   `dialect: SqlDialect` prop and stop importing `toSurrealQL` directly (they call `emitSql(dialect, q)`).
   The schema dropdowns already take a `Schema` shape — `SqlQueryEditor` is changed to ACCEPT a
   schema (or a schema-source descriptor) rather than hardcoding the `readSchema()` call, so the host
   (`QueryTab.tsx`) decides which schema source feeds it: `readSchema()` for surreal,
   `discoverTables`+`describeTable`-projected-into-the-same-`Schema`-shape for federation. The host
   owns the composition; the editor stays transport-agnostic (it does not import `federation.*`).

The host change in `QueryTab.tsx` is small: the federation branch (`isFederation`) swaps its
`<Textarea>` for `<SqlQueryEditor dialect="standard" schema={federationSchema(source)} ...>`,
writing back to `target.args.sql` from `state.sql.rawSql` AND storing `state.sql` in the editor
state (so reopening returns to the builder — the round-trip surreal already has). The raw textarea
becomes the Code half of the same editor.

**Rejected: a per-datasource-kind builder component.** Forking `SqlQueryEditor` per dialect doubles
the surface, drifts from the surreal path under maintenance, and forces every future builder feature
(rows this scope defers) to be built twice. The whole point of the shipped `panel-kit` extraction
(viz panel-editor scope) was one headless builder model with N views; dialect behind a seam extends
that thesis to the emitter.

**Rejected: a server-side emitter.** Compiling the structured query to SQL server-side (mirroring
`lb-prql`'s `compile(prql, dialect) -> sql`) would let a headless client/AI emit builder state and
get SQL back — but no such caller exists today (the AI uses `federation.query {source, sql}`
directly with raw SQL, the shipped descriptor-led path). The emitter is pure UI→string; lifting it
server-side is a deferred follow-up the day a second client needs it (cross-link
[`query/prql-query-scope.md`](../query/prql-query-scope.md) is the precedent for that day).

**Also rejected: folding schema discovery into `datasource.list`/`datasource.test`.** Same call as
the 2026-06-29 binding scope: list is admin metadata, test is a connectivity probe — schema read is
a distinct, cacheable concern. `federation.schema` already exists, gated under the read cap; reuse
it, do not widen another verb.

## How it fits the core

- **Workspace is the hard wall (rule 6).** A federation target's dropdown schema and its emitted SQL
  resolve ONLY in the caller's workspace: `federation.schema` is workspace-pinned
  (`resolve(&node.store, ws, source)`, see [`federation/schema.rs:40`](../../../../rust/crates/host/src/federation/schema.rs));
  `federation.query` is workspace-pinned (same resolve). A ws-B builder cannot see ws-A's datasource
  schema or rows. The emitter runs client-side over the workspace-walled descriptor the host
  returned — it cannot name a cross-tenant table. **Mandatory workspace-isolation test applies.**
- **Capability-first (rule 5).** No new cap. Both `federation.schema` and `federation.query` are
  gated under `mcp:federation.query:call` (the discovery half is the same read privilege as a live
  query, by design — `federation/schema.rs:36`). A member without the cap sees the builder degrade
  honestly: the table dropdown is empty (a discovery deny collapses to no rows, per the
  system-catalog contract), the Code half still works, and a Run is denied at the host with an
  opaque error. **Mandatory capability-deny test applies** (both schema and query denies).
- **Symmetric nodes (rule 1).** Pure UI + a TS emitter; identical on edge and cloud. The federation
  sidecar is a Tier-2 native extension wherever it runs; the builder neither knows nor branches.
- **One datastore (rule 2).** SurrealDB stays the authority. A federation target READS an external
  DB through the gated verb; it does not become a second store. The emitter produces SQL FOR THE
  EXTERNAL ENGINE, never SurrealQL over federation.
- **State vs motion (rule 3).** N/A — pure UI request/response. The builder state is a `cell.options.sql`
  record (state, SurrealDB); the query is one `federation.query` call (request/response, not motion).
- **Stateless extensions (rule 4).** N/A — no extension change. The federation extension's schema
  verb is already stateless (reads the external catalog through the sidecar per call).
- **MCP is the contract (rule 7).** The builder emits the `sql` arg of the shipped
  `federation.query` MCP tool. Nothing downstream of the builder knows "this was authored visually."
- **Core knows no extension (rule 10).** The builder branches on the target's datasource `kind`
  (`"surreal" | "federation"` — config data, derived from `target.datasource.type`), NEVER on a
  datasource name. The emitter seam is keyed on `kind`, not on `"demo-buildings"` or any other id.
  Swapping sqlite for postgres changes the `kind` and (potentially) dialect details; it never forces
  a core mediation change. A swap of equivalent datasources never touches the builder component.
- **No mocks / no fake backend (rule 9 — testing §0).** The new emitter is exercised against the
  REAL sqlite engine in tests — same pattern as [`federation_sqlite_test.rs`](../../../../rust/crates/host/tests/federation_sqlite_test.rs):
  a real on-disk `.db` seeded with real rows, the real federation sidecar built with default
  features (sqlite is not feature-gated), the real gateway. No `*.fake.ts`, no in-memory
  re-implementation of `federation.query`. See the testing plan.
- **One responsibility per file (rule 8).** The plan respects FILE-LAYOUT: `query.ts` (state),
  `toSurrealQL.ts` (SurrealQL emitter), `toStandardSql.ts` (standard-SQL emitter), `dialect.ts`
  (dispatch). `SqlQueryEditor.tsx` / `VisualEditor.tsx` are the one builder component (shared). The
  host composition lives in `QueryTab.tsx` (already the one per-datasource-kind dispatch).
- **API shape (§6.1).** N/A — adds no MCP verb. The builder consumes `federation.schema`
  (`{source, table?}` → `{tables:[…]}`/`{columns:[…]}`) and emits the `sql` arg of `federation.query`
  (`{source, sql}`). Both are existing get/list-shape reads.
- **Durability.** N/A — no must-deliver effect; the query is a synchronous read.
- **SDK/WIT impact.** None. Pure TS frontend + a TS emitter module.
- **Skill doc.** N/A — adds no agent-/API-drivable surface (no new MCP verb, route, or automatable
  task). The existing `federation.query` / `federation.schema` skills are unchanged.

## Example flow

The headline path — author a federation panel through the builder, end to end:

1. In `/t/$ws/data-studio`, the user picks **New panel** (or clicks a datasource row in the rail's
   Sources tab → "open in builder"). A stacked builder tab opens; stage 1 is the Query surface.
2. The Datasource dropdown reflects the saved `target.datasource`. The user picks `demo-buildings`
   (a registered sqlite federation source). `QueryTab.selectDatasource` writes a federation target:
   `{ tool:"federation.query", args:{ source:"demo-buildings", sql:"" }, datasource:{ type:"federation", uid:"datasource:acme:demo-buildings" } }`.
3. The Query surface renders `<SqlQueryEditor dialect="standard" schema={...} value={state.sql ?? emptySqlSource()} .../>`
   instead of the legacy `<Textarea>`. The header shows the same Builder⇄Code toggle and Format
   (Table | Time series) the surreal path shows.
4. The editor mounts in Builder mode. It loads the schema: the host calls `discoverTables("demo-buildings")`
   → `federation.schema {source}` → `{tables:[{name:"site"},{name:"point_reading"},…]}`. (Lazily,
   when the table dropdown is first opened — same pattern as `useSqlSchema.ts`.)
5. The user picks `point_reading` in the Table dropdown. The editor calls
   `describeTable("demo-buildings","point_reading")` → `federation.schema {source, table}` →
   `{columns:[{name:"time",…},{name:"point_id",…},{name:"value",…}]}`. The Column/Filter/Group/Order
   dropdowns populate.
6. The user adds Column `value` with aggregation `avg`, Filter `point_id = "p1"`, Group by `time`,
   Order by `time desc`, Limit `100`. On every change the emitter regenerates the raw SQL via
   `emitSql("standard", query)`:
   `SELECT AVG("value") AS avg_value FROM "point_reading" WHERE "point_id" = 'p1' GROUP BY "time" ORDER BY "time" DESC LIMIT 100`.
   The live preview shows it.
7. The user hits **Run**. The host runs `federation.query {source:"demo-buildings", sql:"<emitted>"}`
   under `mcp:federation.query:call`, workspace-pinned. The seeded rows return; stage 2 reveals the
   preview + viz gallery.
8. The user switches to **Code**, hand-edits the SQL (e.g. adds a second filter). The editor warns
   nothing on Builder→Code (free); if the user switches back to **Builder**, the editor confirms
   ("Switch to Builder? Hand-edited SQL may be replaced…") because the typed query may not represent
   the hand-edit — same gate as today's surreal behaviour.
9. **Save** writes a normal v3 cell: `target = { tool:"federation.query", args:{ source, sql } }`,
   `options.sql = state.sql` (so reopening returns to the builder, exactly like a surreal cell).
   `panel.save` round-trips; the gallery proves the ONE-query invariant (preview + thumbnails share
   one `vizQueryKey`).
10. **Reopen**: `cellToEditorState` rehydrates `state.sql` from `cell.options.sql`; `QueryTab`'s
    `dsKindOf` recognizes the federation target; the builder reopens with the table/column/filter
    rows intact. ADD == EDIT.

## Testing plan

Per [`scope/testing/testing-scope.md`](../testing/testing-scope.md). The mandatory categories that
apply: **capability-deny** (§2.1), **workspace-isolation** (§2.2). No mocks (§0). The layers:

### Unit (pure, ms)

- **Dialect goldens** (`panel-kit/sql/toStandardSql.test.ts` + an extension to the existing
  `toSurrealQL.test.ts`): a fixed `SqlBuilderQuery` → the expected SQL string, one case per dialect.
  Cover the deltas explicitly:
  - identifier quoting (`"Weird Table Name"`, `"col"`);
  - aggregate spelling (`COUNT(*)`, `AVG("value")` for standard; `count()`, `math::avg(value)` for Surreal);
  - filter value escaping (the `'` doubling rule, applied to both dialects' string literals);
  - empty/table-less query → `""` (the "incomplete builder" contract);
  - LIMIT bound (the host caps regardless; the emitter just renders the int).
- **`emitSql` dispatch** (`dialect.test.ts`): for each `SqlDialect`, `emitSql(d, q) === <impl>(q)`.
- **Round-trip extension** (`cellEditorState.test.ts` — extend the existing `editorStateToCell ∘ cellToEditorState ≡ id`
  suite): add a federation-target cell that CARRIES `options.sql` (a `SqlSourceState` with
  `mode:"builder"`, a `builder`, and the `rawSql` the standard emitter produced). Assert byte-identical
  round-trip AND that `targets[0].args.sql === state.sql.rawSql`.
- **`QueryTab` dispatch** (`QueryTab.test.tsx` — extend): selecting a federation datasource opens
  `SqlQueryEditor` (not a `Textarea`); `state.sql` is set; switching to a surreal datasource keeps
  the editor (dialect prop swaps).

### Gateway (real, rule 9 — the headline)

A new `ui/src/features/panel-builder/tabs/queryBuilderCommon.gateway.test.tsx` driving the real
spawned gateway + the real sqlite demo datasource (the pattern from
[`federation_sqlite_test.rs`](../../../../rust/crates/host/tests/federation_sqlite_test.rs) and the
`DataStudioBuilderFlow.gateway.test.tsx` rect-stub discipline from the data-studio rail):

- **Headline: builder → SQL → real rows.** Seed `demo-buildings` via `make seed-demo-sqlite` (or the
  in-test equivalent — open a real `.db`, register via `datasource.add`). Mount `QueryTab` with a
  federation target. Pick table `point_reading`, add a Column `value` avg + Filter + Limit. Assert:
  the emitted SQL is the standard-SQL string; the preview returns the seeded rows through the real
  `federation.query` engine.
- **Code→Builder clobber guard.** Hand-edit SQL in Code mode, switch to Builder, accept the confirm;
  the builder query replaces the raw string. Cancel the confirm; the raw string is preserved.
- **Reopen round-trip.** Save the panel (`panel.save`), reload via `panel.get`/`dashboard.get`,
  remount the builder; the builder query + raw SQL rehydrate from `options.sql`; a re-run is the
  SAME SQL (no drift).
- **Capability-deny (mandatory §2.1).** A session without `mcp:federation.query:call`:
  - the table dropdown is empty / errors honestly (a `federation.schema` deny collapses to no rows,
    per the system-catalog deny contract);
  - a Run is denied at the host with an opaque error surfaced verbatim (never fabricated rows).
- **Workspace-isolation (mandatory §2.2).** A ws-B session cannot discover or query ws-A's
  `demo-buildings` (workspace-pinned resolution): the dropdown is empty AND a forged cross-tenant
  target is refused at the host. (The isolation is enforced at the host; the test asserts the UI
  surfaces the deny honestly rather than fabricating rows.)

### Backend regression (no new behaviour — pin the contract)

The host `federation_sqlite_test.rs` already covers `federation.schema` deny + ws-isolation
(`federation_end_to_end_sqlite`, lines 162-200 + 243-276). This scope adds NO backend change; the
regression is the EXISTING test staying green. If the implementing session touches
`federation/schema.rs` at all (it should not), it must extend that suite, not bypass it.

### What is NOT a test here

- No fake datasource, no `*.fake.ts`, no in-memory `federation.query` re-implementation (rule 9 / §0).
  The real sqlite engine + the real sidecar are exercised; the seeded `.db` is a SEED (feeds the
  real path), not a mock.
- No Surreal-side regression beyond the dialect golden + the round-trip extension — the surreal
  emitter and the existing `SqlQueryEditor` surreal path are UNCHANGED (the dialect dispatch is the
  only seam, and the dispatch's `"surreal"` arm returns byte-identical output to today's direct
  `toSurrealQL` call).

## Risks & hard problems

- **Identifier quoting across dialects.** Postgres folds unquoted identifiers to lowercase; sqlite
  is permissive; SurrealDB uses bare lowercase. The standard emitter MUST double-quote (so a
  mixed-case or reserved-word column can never break out of the identifier position), the SurrealQL
  emitter stays bare. A column with an embedded `"` is escaped by doubling (the `ident()` rule).
  Goldens pin every dialect's quoting.
- **The Code→Builder clobber gate carries over unchanged.** The current surreal behaviour (confirm
  before regenerating from a possibly-stale `builder`) is correct and dialect-agnostic; the standard
  path inherits it. The risk is a REGRESSION in the surreal path if the dialect-seam refactor is
  careless — pinned by extending the existing surreal builder tests, not just adding standard ones.
- **`state.sql` round-trip is new for federation.** Today federation stores raw SQL inline in
  `target.args.sql` and skips `state.sql` entirely. After this slice, federation ALSO stores
  `options.sql` (the builder state). The risk is a migration bump for federation cells authored
  before the slice: they have `target.args.sql` but no `options.sql`, so on reopen they fall to
  Code mode (the raw SQL is preserved; the builder query is empty). Documented as the honest
  behaviour — no silent fabrication of a builder query from a hand-edited string.
- **Dialect drift over time.** A future builder feature (joins, window functions) may need
  per-dialect emission that the ANSI subset cannot express. The seam (`emitSql(dialect, q)`)
  accommodates this without a component fork: add a dialect case, split the standard emitter per file
  when a real delta forces it. Filed as the open question.
- **Schema-cache staleness.** `federation.schema` dropdowns can drift from the live DB (columns
  added/removed mid-session). The existing `useSqlSchema` cache (per-source, lazy column fill)
  carries the same trade-off; this slice does not worsen it. Named follow-up: a "refresh schema"
  affordance — out of scope.
- **CodeMirror highlighting flavour.** The shipped `SqlEditor` (CodeMirror) is SurrealQL-flavoured;
  standard SQL renders correctly but the highlighter may colour a few keywords differently. Cosmetic
  only; a standard-SQL grammar is a deferred polish item (open question).

## Open questions (resolved 2026-07-06 — see session)

1. **Per-dialect emitter files vs one standard emitter?** ✅ **ONE `toStandardSql.ts` for v1.**
   The SELECT subset the builder can express is ANSI; sqlite/postgres/timescale all speak it. Split
   into per-dialect files ONLY when a real delta forces it (see OQ #6). Shipped: one file, 82 lines.
2. **Where does the standard emitter live?** ✅ **`ui/src/lib/panel-kit/sql/`** (beside
   `toSurrealQL.ts`). Keeps the "one builder model, N emitters" seam in one place (FILE-LAYOUT);
   the emitter is UI-only (no federation host code).
3. **How does `SqlQueryEditor` receive the schema?** ✅ **ACCEPT a `schema: Schema` prop** + STOP
   calling `readSchema()` itself. The host (`QueryTab.tsx`) provides the schema via the two new
   hooks (`useLocalSchema`, `useFederationSchema`); the editor stays transport-agnostic.
4. **Reuse `useSqlSchema` or add a parallel hook?** ✅ **NEW `useFederationSchema` hook** that
   projects `describeTable`'s `{name, dataType, nullable}` onto the `Schema` shape (`dataType` →
   `type`) so the editor consumes ONE shape regardless of dialect. Same lazy per-table column-fill
   pattern `useSqlSchema` proved; the shapes differ enough that a small adapter hook was cleaner
   than bending `useSqlSchema`'s `{tables: string[], columns: Record<>}` into `Schema`.
5. **Rail Sources-tree drill into federation tables/columns — same slice or follow-up?** ✅
   **FOLLOW-UP (named, not silent).** The builder-internal schema load is the headline; the rail
   drill composes onto `@nube/source-picker`'s `SourceLoaders` (a new `readFederationSchema` loader
   + a `fedSchema` section kind + the catalog's existing table→column tree) — clean composition,
   not required for the headline. Filed as a named follow-up in the session + STATUS.md.
6. **`format: "time-series"` for federation — does it need time-bucket emission?** ✅ **v1 PASSES
   `GROUP BY "time"` THROUGH; the chart view bins client-side.** Simpler, dialect-free; the natural
   trigger for OQ #1's per-dialect split is a real need for `date_trunc`/`strftime`/`time_bucket`.

## Related

- README §3 (rules 2, 5, 6, 7, 8, 9, 10 — the non-negotiables this scope holds).
- `docs/scope/frontend/dashboard/viz/datasource-binding-scope.md` — the v3 datasource binding that
  DEFERRED this; the table on line 62-67 names the federation row's "SQL Builder ⇄ Code (dropdowns
  from `datasource.schema`)" editor this scope delivers.
- `docs/scope/frontend/data-studio-10x-scope.md` — the current Data Studio shape (the surface this
  slice lands in).
- `docs/scope/frontend/system-catalog-scope.md` — the `@nube/source-picker` catalog this slice
  composes onto (open question #5).
- `docs/scope/query/prql-query-scope.md` — the prior art for per-dialect SQL emission
  (`compile(prql, dialect) -> sql`); the precedent for open question #1's per-dialect split and the
  "server-side emitter" rejected alternative.
- `docs/scope/datasources/sqlite-datasource-demo-scope.md` — the seeded SQLite demo datasource
  (`demo-buildings`) this scope tests against; `make seed-demo-sqlite` is the no-Docker fixture.
- `docs/debugging/agent/federation-information-schema-probe-cryptic-plan-error.md` — the bug that
  surfaced `federation.schema`'s descriptor (the prerequisite this scope lifts).
- `rust/crates/host/tests/federation_sqlite_test.rs` — the rule-9 test pattern (real on-disk `.db` +
  real sidecar, no Docker) the testing plan copies.
- `ui/src/lib/panel-kit/sql/query.ts` · `toSurrealQL.ts` — the shared state + the one Surreal emitter
  this scope keeps.
- `ui/src/features/panel-builder/tabs/QueryTab.tsx` — the per-`kind` dispatch this slice edits (the
  federation branch only).
- Public: [`public/frontend/data-studio.md`](../../public/frontend/data-studio.md) (the section this
  scope promotes into on ship).
