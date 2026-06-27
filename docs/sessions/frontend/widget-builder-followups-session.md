# Session — widget-builder follow-ups (Slices A, B, C)

Status: **done** (2026-06-28)

The three additive follow-up slices the shipped widget-builder scope named under "Follow-up slices
(post-ship, additive)". All three are **additive over the shipped v2 surface** — no change to the frozen
v2 widget/bridge/cell contract. A SQL source is just another `{tool, args}`; a code editor is just the
authoring UI for the already-shipped `plot`/`d3`/`template` views.

- **Scope:** [widget-builder-scope.md](../../scope/frontend/dashboard/widget-builder-scope.md) →
  "Follow-up slices".
- **Public:** [public/frontend/dashboard.md](../../public/frontend/dashboard.md) (the SQL source +
  editors promoted).
- **Builds on:** the shipped [widget-builder-session.md](widget-builder-session.md) (the v2 builder,
  bridge, cell, `render_templates` CRUD).

---

## Slice A — `store.query` + `store.schema` (read-only SurrealDB), the "direct SurrealDB" source

A host MCP verb pair over the embedded store, wired end to end (store → cap → MCP → gateway/bridge →
http client → picker). It needs **zero** new widget binding — the source picker gains a "Direct
SurrealDB" entry that produces `{ tool: "store.query", args: { sql, vars? } }`, and every existing view
renders its rows unchanged.

**Backend** (`rust/crates/host/src/store_query/`, one verb per file, FILE-LAYOUT — mirrors the
`dbview`/`render_templates` host services):

- `store.query(sql, vars?) -> { columns, rows }`, gated `mcp:store.query:call`.
- **READ-ONLY is load-bearing** (`parse.rs`): we **PARSE** the statement with SurrealDB's own parser
  (`surrealdb::syn::parse` → `surrealdb::sql::Statement`) and allowlist by **statement kind** — a single
  `SELECT` (plus `INFO`/`SHOW` introspection). `CREATE`/`UPDATE`/`UPSERT`/`DELETE`/`INSERT`/`RELATE`/
  `DEFINE`/`REMOVE`/`ALTER`/`REBUILD`, multi-statement, transaction control, and `USE` (namespace
  naming) are each refused **before** the SQL reaches the store. **Never a substring/regex check** — the
  parser decides the kind (a `LIKE '%delete%'` test both over- and under-matches; explicitly banned by
  the scope).
- **Workspace wall** (`run.rs`/`schema.rs`): runs inside the caller's namespace via
  `Store::query_ws(ws, …)`, the workspace set host-side from the session token — never a `USE`/namespace
  in the SQL. A ws-B caller reaches only ws-B rows, structurally.
- **Bounded** (`model.rs`): a `SELECT` is wrapped `SELECT * FROM (<sql>) LIMIT 10_000 TIMEOUT 5s` so the
  row cap + timeout apply regardless of the author's clauses. `INFO`/`SHOW` are inherently single-row, so
  they run as-is (they can't be subqueried). An unbounded analytical scan is a **job**, not this verb.
- `store.schema() -> { tables:[{name, columns:[{name,type}]}] }`, gated `mcp:store.schema:call`,
  workspace-walled, from `INFO FOR DB` / `INFO FOR TABLE` (with a 1-row sample fallback for our
  schemaless `{ data: … }` records). Feeds Slice C's visual builder.
- Wired into `call_tool` (`is_host_native` matches the two exact verbs — **not** all `store.*`, since
  `store.tables/scan/graph` are the gateway-only `dbview` lens). Gateway routes `POST /store/query` +
  `GET /store/schema` (mirror `/store/*`); caps granted in dev claims.

**Frontend:** `ui/src/lib/dashboard/sql.api.ts` (`runQuery`/`readSchema` over the `mcp_call` bridge —
no http.ts change needed, `mcp_call` forwards any tool); a "Direct SurrealDB" `SourceEntry` in
`sourcePicker.ts`. The existing `useSource` already normalizes `{rows}` (it lists `"rows"` among the
result keys), so a `store.query` result flows into table/chart/stat/plot/template unchanged.

## Slice B — the in-app CodeMirror editor (ported from rubix-cube, data layer → bridge)

`ui/src/features/dashboard/builder/editors/` (one component per file):

- deps: `@uiw/react-codemirror ^4.25` + `@codemirror/{lang-javascript,lang-sql,state,view} ^6.x`
  (matching rubix-cube's versions).
- `theme.ts` — a token-bound `EditorView.theme` + `lineWrapping`, the lazybones analog of rubix-cube's
  shared `theme` the editors import.
- `CodeEditor.tsx` — the JSX editor (`javascript({ jsx: true })`); ported from rubix-cube's
  `manage-template-dialog/code-editor.tsx`, the react-hook-form `Controller` removed → a pure
  `value`/`onChange` controlled component.
- `PlotCodeField.tsx` — Plot/D3 editor; carries `DEFAULT_PLOT_CODE`/`DEFAULT_D3_CODE` + the bindings
  hint. **The snippet convention is lazybones's shipped iframe runtime** (`async (bridge, el, engine) =>
  { … }` calling `spec(bridge, root, engineMod)`), **not** rubix-cube's `({data, Plot, d3}) => element`
  — the runtime hands the snippet the bridge (the token-less, grant-leashed path), so it fetches its own
  rows. (Decision recorded below.)
- `TemplateSourceField.tsx` — inline-code tab OR a saved `render_templates` pick; the saved list reads
  **`template.list` over the bridge**, NOT REST (rubix-cube's SWR/`next`/REST removed).
- `SqlEditor.tsx` — the raw SurrealQL editor (`@codemirror/lang-sql`); ported from rubix-cube's
  `sql/sql-editor.tsx` with the `/api/.../sql/generate` AI button **dropped** (re-pointing it at an MCP
  `sql.generate` tool is a named follow-up).

`WidgetBuilder.tsx` swaps its raw `<textarea>` for these editors. The editor edits only the snippet
**string** into `cell.options.code` / a `render_template` reference — it holds no data and no token; the
string still runs **only** in the sandboxed iframe (trust contract unchanged: edit in the trusted shell,
run sandboxed).

## Slice C — the Grafana-style Builder⇄Code SQL editor

`ui/src/features/dashboard/builder/sql/` — the visual **query** builder (Grafana's `grafana-sql` model),
distinct from the shipped **chart** builder (which maps x/y over already-fetched rows):

- `query.ts` — the typed `SqlBuilderQuery` `{ table, columns:[{name, aggregation?}], filters[],
  groupBy[], orderBy?, limit? }` + the `SqlSourceState` (mode + raw SQL + builder query + format).
- `toSurrealQL.ts` — one file, the analog of Grafana's SQL `expressionBuilder`: emits
  `SELECT … FROM … WHERE … GROUP BY … ORDER BY … LIMIT …`. String values are single-quote-escaped;
  aggregations render `count()`/`math::sum|avg|min|max(col)` with stable aliases.
- `SqlQueryEditor.tsx` (← Grafana `QueryEditor.tsx`) — switches Builder vs Code by `editorMode`, keeps
  the two in sync (Builder regenerates the raw string on every edit; Code edits the string directly).
- `SqlQueryHeader.tsx` (← `QueryHeader.tsx`) — the Builder/Code toggle + a Format (Table | Time series)
  toggle + **confirm-on-switch-back** (Code→Builder asks first, since hand-edited SQL may not
  round-trip and would be clobbered).
- `VisualEditor.tsx` (← `visual-query-builder/VisualEditor.tsx`) — the rows (Table, Column/Aggregation,
  Filter, Group by, Order by, Limit) + a live SurrealQL preview, rendered with our shadcn
  Button/Input primitives (no `@grafana/ui`). Table/Column dropdowns are populated by `store.schema`.
- `RawEditor.tsx` (← `query-editor-raw/RawEditor.tsx` + `QueryEditorRaw.tsx` folded in) — wraps Slice
  B's `SqlEditor`.

The SQL source cell stores **both** the raw string (what `store.query` runs) **and** the
`SqlBuilderQuery` (when in Builder mode), so reopening returns to the builder. Builder mode can only
generate a `SELECT`; Code mode is still parse-allowlisted to `SELECT` by `store.query` (the host
boundary). The Loki builder files were **not** ported (kept as the reference for a future LogQL source).

---

## Tests (all green, real infra — no mocks, no `*.fake.ts`)

**Backend** (`rust/crates/host/tests/store_query_test.rs`, real store, real seed via `ingest.write`):

```
running 6 tests
test write_statements_rejected_at_parse_per_kind ... ok   # CREATE/UPDATE/DELETE/DEFINE/REMOVE/RELATE/
test query_denied_without_cap ... ok                       #   INSERT/UPSERT/multi/USE each rejected at
test schema_reports_tables_and_denies_and_isolates ... ok  #   PARSE per kind; store unmutated
test select_round_trips_seeded_rows ... ok
test two_session_isolation ... ok                          # ws-B SELECT can't read ws-A; USE refused
test row_cap_enforced ... ok
test result: ok. 6 passed; 0 failed
```

**Frontend unit** (`pnpm test` — `toSurrealQL.test.ts`, 8 cases: columns, aggregation, filter,
quote-escape, group-by, order, limit, Builder→Code→Builder round-trip):

```
✓ src/features/dashboard/builder/sql/toSurrealQL.test.ts (8 tests)
Test Files  9 passed (9)   Tests  44 passed (44)
```

**Frontend real-gateway** (`pnpm test:gateway` — `sql/sqlSource.gateway.test.tsx`, 8 cases: store.query
deny + SELECT round-trip + Code-mode write rejected + isolation; store.schema deny + isolation; an
e2e visual-editor → `toSurrealQL` → `store.query` → rows render in a `table` AND a `chart` widget on
real seeded rows):

```
✓ src/features/dashboard/builder/sql/sqlSource.gateway.test.tsx (8 tests)
Test Files  25 passed (25)   Tests  104 passed (104)
```

Plus the existing widget-builder gateway suite stays green (the SQL picker entry is additive — the
uninstall-eviction assertion `entries.some(e => e.label.includes("mqtt"))` is unaffected).

The scripted-template render path is the **shipped** one (the editors only author the string that the
shipped `ScriptedView`/iframe runtime renders), so the shipped scripted-template e2e covers it — Slice
B changes the authoring surface, not the runtime.

---

## Decisions & rejected alternatives

- **Parse-allowlist by statement KIND, never a substring check.** `surrealdb` re-exports
  `surrealdb_core::*`, so `surrealdb::syn::parse` + `surrealdb::sql::Statement` are available with
  `default-features = false`. We match on the `Statement` variant — the only correct way to know a
  statement is a read. A `USE` is refused outright (the wall is host-side); a hyphenated namespace like
  `ws-a` doesn't even parse (rejected as a `Parse` error, not a `Use`) — both are refusals, the test
  uses a valid identifier to exercise the by-kind `Rejected` path.
- **Bound via a subquery wrapper, only for `SELECT`.** `SELECT * FROM (<sql>) LIMIT … TIMEOUT …` caps any
  read regardless of the author's clauses; `INFO`/`SHOW` (single-row, non-subqueryable) run raw. The
  parse step returns the `ReadKind` so the runner knows which to do.
- **`store.query`/`store.schema` are NOT all of `store.*` in `is_host_native`.** `store.tables/scan/
  graph` are the gateway-only admin `dbview` lens, not bridge verbs — so `is_host_native` matches the
  two exact verb strings, not the `store.` prefix (which would wrongly claim the dbview verbs).
- **Plot/D3 default snippet matches the SHIPPED runtime, not rubix-cube's signature.** rubix-cube hands
  `data` straight to `({data, Plot, d3}) => element`; lazybones's iframe runtime calls
  `spec(bridge, root, engineMod)`, so the snippet is `async (bridge, el, engine) =>` and pulls its rows
  through the bridge (the v2 token-less path). Carrying rubix-cube's literal default would have produced
  a snippet that doesn't render here.
- **The SQL source reuses `useSource` unchanged.** `useSource.toRows` already lists `"rows"` among the
  result keys it unwraps, so a `{columns, rows}` result flows into every view with zero renderer change —
  the scope's "every existing view renders its rows unchanged" holds literally.
- **Used the shadcn `Button`/`Input` primitives** in the new SQL/editor components (the lint rule errors
  on raw `<button>`/`<input>`); native `<select>` stays with a justified per-element `eslint-disable`
  (no shadcn Select primitive exists yet, same as the shipped builder).

## Debugging

No non-trivial bug needed a `debugging/` entry. Two test-author fixes (caught by the real gateway/store,
not mocks): `INFO FOR TABLE` takes a literal identifier, not a `type::table($tb)` bind (switched to a
backtick-quoted name from trusted `INFO FOR DB` output); and the isolation test's `USE NS ws-a` failed
to *parse* (hyphen) rather than being *rejected by kind* — switched to a valid identifier so the by-kind
`Rejected` path is what's asserted. Both are test correctness, not product bugs.

## Cross-links

- Scope: [widget-builder-scope.md](../../scope/frontend/dashboard/widget-builder-scope.md) — the three
  follow-up slices ticked there.
- Public: [public/frontend/dashboard.md](../../public/frontend/dashboard.md) — the SQL source + editors.
- Builds on: [widget-builder-session.md](widget-builder-session.md) — the v2 builder/bridge/cell.
