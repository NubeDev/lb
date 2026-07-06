# Frontend — query-builder-common (session)

- Date: 2026-07-06
- Scope: ../../scope/frontend/query-builder-common-scope.md
- Stage: post-S10 (frontend slice; no stage exit gate crossed)
- Status: done

## Goal

Lift the deferral recorded in `scope/frontend/dashboard/viz/datasource-binding-scope.md:7-8`
("Deferred: `federation.datasource.schema` … a federation target uses the raw-SQL editor until it
lands"): a LOCAL TABLE source (SurrealDB, `store.query`) gets the interactive Builder⇄Code editor,
an external DATASOURCE (`federation.query`) gets only a raw textarea. Make the builder **common** —
one structured-query state, N dialect emitters — so a federation target gets the same builder
experience. The prerequisite (a real discovery verb) shipped as `federation.schema {source, table?}`,
so the deferral is liftable; this session is the frontend lift.

## What changed

The slice is **pure UI + one TS emitter module** — no new MCP verb / cap / table / outbox target /
host change. The wire shape (`federation.query {source, sql}`) is unchanged; the render path
(`viz.query`) is unchanged; rule 10 holds (the dialect is keyed on the target's datasource `kind`,
never on a datasource name).

**New files (one responsibility each, FILE-LAYOUT):**

- `ui/src/lib/panel-kit/sql/dialect.ts` — the emitter seam. `SqlDialect = "surreal" | "standard"` +
  `emitSql(dialect, query)`. The dispatch is the only place a dialect is selected.
- `ui/src/lib/panel-kit/sql/toStandardSql.ts` — the federation emitter (ANSI SELECT: double-quoted
  identifiers, `SUM("col")`/`COUNT(*)`/`AVG("col")`, single-quoted string literals with `'` doubled,
  `LIMIT n`). The safe superset for sqlite/postgres/timescale over the SELECT subset the builder can
  express.
- `ui/src/lib/panel-kit/sql/dialect.test.ts` + `toStandardSql.test.ts` — pure-logic goldens (15
  tests total: dispatch routing, dialects differ on the same query, identifier + literal escaping,
  aggregations, GROUP/ORDER/LIMIT, the empty/table-less guard).
- `ui/src/features/panel-builder/tabs/useLocalSchema.ts` — wraps `readSchema()` (the surreal/host
  side of the editor's schema contract).
- `ui/src/features/panel-builder/tabs/useFederationSchema.ts` — wraps the shipped
  `discoverTables`/`describeTable` (`federation.schema`) and projects `DbColumn.dataType →
  SchemaColumn.type` so the editor consumes ONE `Schema` shape regardless of dialect. Lazy per-table
  column fill (the `useSqlSchema` pattern).
- `ui/src/features/panel-builder/tabs/queryBuilderCommon.gateway.test.tsx` — the real-gateway proof
  (4 tests, including the mandatory cap-deny + ws-isolation).

**Modified files (minimal, surgical):**

- `ui/src/features/dashboard/builder/sql/SqlQueryEditor.tsx` — accepts `dialect: SqlDialect` +
  `schema: Schema` props, drops its hardcoded `readSchema()` effect (the host owns the load now),
  calls `emitSql(dialect, builder)` instead of `toSurrealQL(builder)`. The Code→Builder confirm
  dialog + the format toggle carry over unchanged. The `emptySqlSource` re-export stays (back-compat
  for `panelEditor.gateway.test.tsx` and `QueryTab`).
- `ui/src/features/dashboard/builder/sql/VisualEditor.tsx` — accepts `dialect`, calls `emitSql` for
  the live preview instead of `toSurrealQL` directly.
- `ui/src/features/panel-builder/tabs/QueryTab.tsx` — wires the two schema hooks; the federation
  branch SWAPS the `<Textarea>` + Run button for `<SqlQueryEditor dialect="standard"
  schema={federationSchema} ...>`. The surreal branch adds `dialect="surreal"` + `schema=
  {localSchema}`. Picking federation in the datasource dropdown now seeds `state.sql =
  emptySqlSource()` (so the editor reopens to Builder mode, the round-trip surreal already had).
  Migration: a federation cell authored BEFORE this slice (no `options.sql`) reopens to Code mode
  with the saved SQL preserved — we do NOT fabricate a builder query from hand-edited SQL (scope
  Risks). Removed the unused `Play`/`Button`/`Textarea` imports and the now-unused `onRun`
  destructure (the prop stays on `Props` so callers are unaffected; the canonical Run affordance is
  `BuilderToolbar`).
- `ui/src/lib/panel-kit/cellEditorState.test.ts` — two new round-trip cases: (1) a v3 federation
  cell carrying `options.sql` round-trips byte-identical AND `targets[0].args.sql === state.sql.rawSql`;
  (2) a pre-slice federation cell (args.sql but no options.sql) round-trips without fabricating a
  builder query (byte-clean).

The SurrealQL emitter (`toSurrealQL.ts`) and the structured-query state (`query.ts`) are UNCHANGED —
the dialect seam is the only addition. Surreal-path behaviour is preserved byte-for-byte (pinned by
the surreal-regression gateway test).

## Decisions & alternatives

- **One standard emitter vs per-dialect files (sqlite/postgres/timescale).** Chose ONE
  `toStandardSql.ts` for v1 (scope OQ #1). Rationale: the SELECT subset the builder can express
  (SELECT/FROM/WHERE/GROUP BY/ORDER BY/LIMIT) is ANSI; all three federation kinds speak it. Split
  into per-dialect files ONLY when a real delta forces it — the natural trigger is a time-bucket
  emit for the chart `time-series` format hint (postgres `date_trunc` / sqlite `strftime` /
  timescale `time_bucket`). Rejected a premature split: a per-kind file triple today would carry
  identical bodies and violate FILE-LAYOUT's "don't split until a real delta forces it" corollary.
- **Aliases double-quoted (`AS "avg_value"` not `AS avg_value`).** Surfaced when the first golden
  run failed: the emitter passes every identifier through `ident()`, including generated aliases.
  Chose to keep the quoting (consistent + safe — `count` is reserved in some dialects, and postgres
  returns lowercase keys for lowercase-quoted aliases so the result-column mapping is unaffected).
  Rejected an "alias special-case" — it would be the one identifier position that escapes `ident()`,
  and the safe superset is no harm.
- **The editor receives `schema: Schema` directly, not a loader thunk (scope OQ #3).** The host
  (`QueryTab`) decides the source + loads it through `useLocalSchema` / `useFederationSchema`; the
  editor stays transport-agnostic (imports neither `readSchema` nor `federation.*`). Rejected the
  alternative `schemaSource: () => Promise<Schema>` thunk — it would push host composition INTO the
  editor and re-introduce the very coupling the headless `panel-kit` extraction removed.
- **No second schema-fetch path through `@nube/source-picker` (scope OQ #5).** The rail's
  `CatalogExplorer` shows a federation *datasource row* today but does NOT drill into its tables/
  columns (no `readFederationSchema` loader on `SourceLoaders`). The builder loads its own dropdown
  schema directly via `discoverTables`/`describeTable` — the same pattern `SqlQueryEditor` already
  used for local `store.schema`. The rail drill is a clean composition onto the catalog's loader
  seam (a new `readFederationSchema` loader + a `fedSchema` section kind) but is a NAMED FOLLOW-UP,
  not this slice: the builder-internal load is the headline.
- **No server-side emitter (scope "Intent — rejected").** Could mirror `lb-prql`'s
  `compile(prql, dialect) -> sql` for headless/AI callers, but no such caller exists today (the AI
  uses `federation.query {source, sql}` directly with raw SQL, the shipped descriptor-led path).
  The emitter is UI→string; lifting it server-side is deferred to the day a second client needs it.
- **Removed the federation Run button.** The legacy federation branch had its own `<Textarea>` +
  Cmd+Enter Run button. After the swap, federation uses the same `SqlQueryEditor` surreal uses —
  which has NO in-editor Run button (the canonical Run is `BuilderToolbar`, shared by both kinds).
  This is a small UX consistency win, not a regression: the toolbar Run works for both.

## Tests

**Mandatory categories covered** (`scope/testing/testing-scope.md` §2):
- **Capability-deny (§2.1)** — `queryBuilderCommon.gateway.test.tsx` test 3: a session without
  `mcp:federation.query:call` (which gates BOTH `federation.query` AND `federation.schema`) still
  renders the editor; `federation.schema` is invoked and denied; the table dropdown is empty (the
  system-catalog deny contract — collapsed to no rows, never a fabricated roster).
- **Workspace-isolation (§2.2)** — `queryBuilderCommon.gateway.test.tsx` test 4: ws-B's
  `datasource.list` does not include ws-A's `demo-buildings` (workspace-pinned from the token; the
  wall is at the host).
- **No mocks (§0)** — every test drives the REAL spawned gateway + real `datasource.add` admin verb
  + real `federation.schema` MCP call. The federation SIDECAR is not spawned in the UI test env
  (a true external a UI test cannot cheaply run); `federation.schema` resolves to an honest typed
  error and the editor DEGRADES to an empty schema (the system-catalog contract). The real-row
  round-trip is `rust/crates/host/tests/federation_sqlite_test.rs`'s job — unchanged, stays green.

**Green output:**

```
$ pnpm vitest run src/lib/panel-kit/sql/ --reporter=dot
 ✓ src/lib/panel-kit/sql/toSurrealQL.test.ts (8 tests) 3ms
 ✓ src/lib/panel-kit/sql/toStandardSql.test.ts (11 tests) 2ms
 ✓ src/lib/panel-kit/sql/dialect.test.ts (4 tests) 2ms
 Test Files  3 passed (3)
      Tests  23 passed (23)

$ pnpm vitest run src/lib/panel-kit/cellEditorState.test.ts --reporter=dot
 ✓ src/lib/panel-kit/cellEditorState.test.ts (12 tests) 10ms
 Test Files  1 passed (1)
      Tests  12 passed (12)

$ pnpm vitest run src/features/panel-builder/tabs/QueryTab.test.tsx --reporter=dot
 ✓ src/features/panel-builder/tabs/QueryTab.test.tsx (11 tests) 110ms
 Test Files  1 passed (1)
      Tests  11 passed (11)

$ pnpm vitest run --reporter=dot   # full unit suite
 Test Files  119 passed (119)
      Tests  737 passed (737)

$ pnpm exec vitest run --config vitest.gateway.config.ts \
    src/features/panel-builder/tabs/queryBuilderCommon.gateway.test.tsx --reporter=dot
 ✓ src/features/panel-builder/tabs/queryBuilderCommon.gateway.test.tsx (4 tests) 307ms
 Test Files  1 passed (1)
      Tests  4 passed (4)

$ pnpm exec tsc --noEmit   # only the pre-existing transformDebug.gateway red remains
src/features/panel-builder/tabs/transforms/transformDebug.gateway.test.tsx(10,26):
  error TS6133: 'signInReal' is declared but its value is never read.

$ pnpm exec eslint <my files>   # clean
```

**Pre-existing reds (NOT this slice's — verified by `git stash` + rerun on clean master):**
`sqlSource.gateway`, `SystemView.gateway`, `WorkflowView.gateway`, `ProofPanel.gateway`,
`CommandPalette.reminders.gateway`, `App.gateway`, `PanelPage.gateway`, `AuthoringPanel.gateway`,
`McpServiceView.gateway`, `InboxView.gateway` all fail identically on the stashed baseline. All
QueryTab/SqlQueryEditor-touching gateway tests (`panelEditor.gateway`, `DataStudioBuilderFlow.gateway`,
`DataStudio.gateway`, `flowsPanelEditor.gateway`) are GREEN with my changes.

## Debugging

None — no debugging entries opened. The slice landed clean on the first gateway run; the only
iteration was the toStandardSql golden fix (aliases are identifiers → quoted), which the unit tests
caught before any integration run.

## Public / scope updates

- **Scope open questions resolved** (`scope/frontend/query-builder-common-scope.md` OQ #1, #3, #4,
  #5): all resolved as recommended in the scope. OQ #2 (where the standard emitter lives) resolved
  to `panel-kit/sql/` (beside `toSurrealQL.ts`). OQ #6 (time-bucket emit) deferred — v1 emits a
  plain `GROUP BY "time"` and lets the chart view bin client-side; the natural trigger for a
  per-dialect split remains a real time-bucket need.
- **Promoted to `public/frontend/data-studio.md`** — a new "Query builder — common across dialects"
  section describing the shipped behaviour (surreal ⇄ federation parity, the dialect seam, the
  schema-load hooks, the migration for pre-slice federation cells).

## Skill docs

n/a — no agent-/API-drivable surface. The slice adds NO new MCP verb, gateway route, or automatable
task. The existing `federation.query` / `federation.schema` skills (the query skill's "raw SQL for a
datasource" branch) are unchanged — the AI keeps authoring `federation.query {source, sql}` directly
with raw SQL; the builder is a UI affordance over the same wire shape.

## Dead ends / surprises

- The `git stash` baseline check surfaced a CONCURRENT session's in-flight edits in the tree
  (`BuilderPane.tsx`, `WorkbenchTab.tsx`, `NavRail.tsx`, `CopyTemplatePrompt.tsx`,
  `templatePrompt.*`, `RowsContext.tsx`, `surfaceDefs.ts`, `TemplateOptionsEditor.tsx`) — NOT MINE.
  I did not touch those; my changes are limited to the files listed in "What changed". The pre-existing
  reds those concurrent edits cause are theirs to close.
- The federation `Run` button removal (above) was a small surprise — the legacy branch had its own.
  Filed under Decisions as a consistency win, not a regression.

## Follow-ups

- **Rail Sources-tree drill into federation tables/columns** (scope OQ #5): compose onto
  `@nube/source-picker`'s `SourceLoaders` — a new `readFederationSchema(source)` loader + a
  `fedSchema` section kind + the catalog's existing table→column tree. Clean composition; not
  required for the headline.
- **Time-bucket emit for the chart `time-series` format hint** (scope OQ #6): postgres `date_trunc`
  / sqlite `strftime` / timescale `time_bucket`. The natural trigger for splitting
  `toStandardSql.ts` per dialect (OQ #1). v1 emits plain `GROUP BY "time"` and lets the chart view
  bin client-side.
- **CodeMirror standard-SQL grammar** (scope Risks): the shipped `SqlEditor` is SurrealQL-flavoured;
  standard SQL renders correctly but a few keywords may colour oddly. Cosmetic; deferred polish.
- **`onRun` prop on `QueryTab`** is now unused inside the component (callers still pass it). A future
  slice could wire Cmd+Enter inside `SqlQueryEditor` for both dialects (restoring the legacy
  federation affordance uniformly), at which point `onRun` becomes live again. Left as-is for now.

**STATUS.md updated?** yes — a new "Just shipped" entry at the top.

---

## Addendum — peer review of the 10x scope set (2026-07-06, session 2)

The user asked for a peer review of the `docs/scope/frontend/query-builder/` set and made three calls:
copy Tabularis's frontend code rather than rewrite ("rewrites miss things"), host the workbench on the
existing Datasources page (not a new rail surface), and resolve all open questions for best-long-term.

**Decisions recorded into the scopes:**
- **Copy discipline (hybrid, not full copy).** Pure TS copied verbatim (`sqlSplitter`, `sqlFormat`);
  component files (`TableNode`/`JoinEdge`/settings panel) copied and adapted — interaction kept, data
  layer rewired; architecture (nodes-as-truth, string-concat SQL, Monaco, Tauri invoke) never copied — it
  would break the `emitSql` seam, rule 9, and the round-trip contract. Anti-miss gate: a mandatory
  per-file parity checklist (ported / dropped-with-reason) in the build session doc.
- **Slice 3 retargeted.** `DatasourceDetail.tsx` already runs `federation.query` with `QueryResults` +
  `SavedQueriesDialog`; a new `/query` surface would double-build it. The workbench replaces that page's
  ad-hoc SQL area, extracted as `QueryWorkbench({ws,sel,onSel})` so one `VIEW_PANES` line also mounts it
  in Data Studio. CodeMirror confirmed by the user.
- **All OQs resolved** in place (operators LIKE/IS NULL now + IN deferred; `builderLayout` persisted;
  qualified `groupBy`; orderBy write-array/read-either; Format keybinding; splitter → `ui/src/lib/sql/split/`;
  surreal-local picker default with a future Prefs axis).

**Peer-review defects found and fixed in the model sketch (visual-canvas-builder-scope.md):**
1. HAVING must emit the aggregate expression, never the SELECT alias (ANSI/Postgres forbid it) —
   `SqlFilter.aggregation` added for `isAggregate` rows; umbrella example corrected.
2. `SqlJoinKey` lacked a `leftTable` — ambiguous once ≥2 joins exist; added (default = FROM table).
3. CROSS JOIN takes no ON — `SqlJoin.on` now optional/empty for `cross`.
4. `SqlFilter.value` made optional (IS NULL / IS NOT NULL carry none).
5. `orderBy: single | array` union pinned down: write array, read legacy single; round-trip contract
   split into byte-identical (new shape) vs semantic (legacy fixture).
6. Format button gated to `standard` dialect — `sql-formatter` can corrupt SurrealQL (`table:id`,
   `type::`, `->`).
7. New open item flagged: reconcile the future `querydef.*` seam with the datasources page's existing
   `useDatasourceQueries` persistence before building, or we get two saved-query stores.

No code changed this session — scope-doc revisions only; tests/debug-history N/A.

## Addendum 2 — saved queries shipped on the Datasources page (2026-07-06)

Shipped (3 new files, 1 edited, all `ui/src/features/datasources/`): `useDatasourceQueries.ts`
(per-source hook — `query.list` filtered client-side to `target === "datasource:<name>"`, load/save/
remove, saves `lang:"raw"` with the implicit datasource target), `SaveQueryDialog.tsx` (id/name/
description; editor SQL + target baked in; verbatim errors), `SavedQueriesDialog.tsx` (filtered roster;
click resolves the full record via `query.get` and loads the editor; per-row delete), and
`DatasourceDetail.tsx` wiring both into the SQL editor header beside Run.

Docs updated to match: `public/datasources/datasources.md` gained §"Saved queries"; STATUS.md got a
"Just shipped" entry; the query-builder scope set was reconciled — the hypothetical `querydef.*` chain
is dead (umbrella §"Saved queries" rewritten around `query.*`; slice-3 non-goal, registration note, and
`sel` risk updated: `sel` = saved-query id via `query.get`). The peer-review "two saved-query stores"
open item is closed.

## Addendum 3 — the "two SQL pages" decision (2026-07-06)

User asked: Data page (SurrealDB) vs Datasources page (federation) — shared library, merge, or a new
page? Decided: **shared component, keep both pages** (recorded in query-workbench-view-scope.md
§Registration item 5). Slice 3's extracted `QueryWorkbench({source,...})` mounts in DatasourceDetail
(standard dialect, already planned) AND in the Data page pinned to surreal-local (`store.schema`/
`store.query`) — the SQL box the Data page never had. Deleting the Data page was rejected (its
admin-only browser/graph has no home on Datasources; caps split would blur); a third page was already
rejected at peer review.
