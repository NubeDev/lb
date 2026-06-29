# Session — the rules editor UX (a guided, explorable authoring surface)

Topic: `frontend` · Scope: [rules-editor-ux-scope.md](../../scope/frontend/rules-editor-ux-scope.md) ·
Date: 2026-06-29 · State: **done** · Promotes to
[public/frontend/rules-workbench.md](../../public/frontend/rules-workbench.md) (the editor-UX additions).

## The ask

Turn the shipped rules Playground editor into a **guided, explorable authoring surface**: a user with no
idea how Rhai rules work should be able to discover the engine's functions, datasources, and example rules
and **one-click** them into the editor. Frontend-only — **no host changes, no new MCP verbs, no new caps**.
Reuse over duplication; accurate palette over invented signatures; honest states over fake data. Write the
scope first, then build per HOW-TO-CODE.

## Key decisions (taken before/while coding)

- **The function palette is STATIC typed data, not a host query.** The Rhai verbs the engine registers are
  compile-time known (`rust/crates/rules/src/verbs/*`); a "list registered functions" host verb would be a
  needless backend change (and would cross the no-host-changes line). So the palette is a typed UI catalog
  that **mirrors the crate verbs entry-for-entry** — signatures + `///`-doc summaries lifted from the crate.
  Accuracy is the contract (a lying signature is worse than none); the catalog lives in the same repo next
  to the verbs and is a code-review diff target.
- **The reuse boundary is the schema READER.** `readSchema`/`Schema`/`SchemaTable`/`SchemaColumn` lived in
  the **dashboard-named** `lib/dashboard/sql.api.ts` but is a generic store concern. Extracted it to a
  shared `lib/schema/` module that **both** the dashboard SQL builder **and** the rules data explorer
  consume. The dashboard VisualEditor keeps its typed-query **dropdowns** (a different affordance from the
  explorer's click-to-insert tree) — forcing one onto the other would regress the builder; the shared piece
  is the reader, which both already need. The new `SchemaBrowser` (click-to-pick tree) is a separate shared
  component the explorer consumes.
- **One reusable insert-at-cursor primitive.** A `components/codeeditor/CodeEditor` (`forwardRef`,
  ref handle `insertSnippet`) + `useEditorInsert` dispatch a **real CodeMirror transaction**
  (`replaceSelection`) so the controlled `onChange` fires and the buffer updates — not a string concat that
  fights the controlled value. Every palette/explorer panel inserts through this one primitive.
- **No new dependency, no Tabs primitive pulled in.** Reuses the shipped `@uiw/react-codemirror` +
  `lang-javascript`; the panel's three tabs are a small `PanelTabs` built from the shadcn `Button` (no
  shadcn Tabs primitive exists; Radix Tabs for three buttons is more than this needs).

## What shipped

**Reusable (shared) pieces:**
- `ui/src/lib/schema/{schema.api,index}.ts` — the extracted `store.schema` reader (`readSchema`/`Schema`/
  `SchemaTable`/`SchemaColumn`). `lib/dashboard/sql.api.ts` now keeps only `runQuery`/`QueryResult` and
  **re-exports** the reader for back-compat; the dashboard `SqlQueryEditor`/`VisualEditor` import it from
  `@/lib/schema` directly. **One module, two consumers.**
- `ui/src/components/codeeditor/{CodeEditor.tsx,useEditorInsert.ts,index.ts}` — the controlled code editor
  with the `insertSnippet` ref handle (the click-to-insert primitive).
- `ui/src/components/schema/{SchemaBrowser.tsx,index.ts}` — a reusable collapsible table→column tree with
  click-to-pick.

**Rules feature additions:**
- `ui/src/features/rules/catalog/{catalog.types,data,grid,timeseries,ai,output,index}.ts` — the typed
  function-palette catalog, one file per verb family, mirroring the crate verbs exactly (Data, Grid,
  Timeseries [first-class], AI, Output).
- `ui/src/features/rules/examples/examples.ts` — ready-to-run example rules (bodies reuse the proven
  gateway-test bodies so they run green).
- `ui/src/features/rules/panel/` — `AuthoringPanel` (the Functions | Examples | Data tabbed surface),
  `PanelTabs`, `FunctionPalette` + `FunctionEntry` (searchable, categorized, click-to-insert), `ExampleList`
  (click-to-load with dirty-confirm), `DataExplorer` + `useDataExplorer` (datasources + local schema +
  series, each honest loading/deny/empty/ready).

**Wiring (extended, not rewritten):**
- `RuleEditor.tsx` → `forwardRef<CodeEditorHandle>`, renders the shared `CodeEditor`.
- `RulesView.tsx` → a third region (the `AuthoringPanel`); holds the editor ref and an `insert(snippet)`
  that calls `insertSnippet`; passes `loadExample` to the panel.
- `useRules.ts` → `loadExample(body)` with the **dirty-confirm guard** (a window.confirm before clobbering
  unsaved edits), detaching from any open saved rule.

## Honest states + the no-host-change boundary

- A **denied** `datasource.list` (or schema/series) renders a **deny** in that section — never a fabricated
  roster (asserted in the test). Loading is a skeleton; empty teaches.
- **Honest gap surfaced, not hidden:** there is **no per-external-datasource table-introspection verb**
  (`datasource.list` is kind+endpoint only; `store.schema` is the local store). The explorer shows local
  tables + the source roster + the series roster; deep external-table browsing is a **named follow-up** (it
  would need a host verb — out of this frontend slice). Recorded in the scope's Open questions.
- The DSN never renders: the explorer surfaces only what `datasource.list` returns (a redacted ref); the
  test asserts no `secret` substring appears anywhere in the panel.

## The reuse proof (the headline)

The extracted `@/lib/schema` reader is consumed by **both** surfaces and **both** suites stay green:
- the **dashboard** `sqlSource.gateway.test.tsx` (8 tests) drives the SQL builder over the moved reader;
- the new **rules** `AuthoringPanel.gateway.test.tsx` reads the same module via the data explorer.

## Green test output

**ESLint** (`npx eslint` over the touched dirs — `components/codeeditor`, `components/schema`, `lib/schema`,
`features/rules`, `features/dashboard/builder/sql`, `lib/dashboard/sql.api.ts`): **0 errors**.

**Typecheck** (`npx tsc --noEmit`): **clean**.

**UI Vitest, real in-process gateway** (`npx vitest run --config vitest.gateway.config.ts
src/features/rules src/features/dashboard/builder/sql`):

```
 ✓ src/features/rules/RulesView.gateway.test.tsx (6 tests)          741ms
 ✓ src/features/rules/panel/AuthoringPanel.gateway.test.tsx (6 tests) 484ms
 ✓ src/features/dashboard/builder/sql/sqlSource.gateway.test.tsx (8 tests) 277ms
 Test Files  3 passed (3)   Tests  20 passed (20)
```

The new `AuthoringPanel.gateway.test.tsx` covers: the palette renders the real categories + click-to-insert
appends a snippet to the buffer; search filters by name; an example loads + runs green via real `rules.run`;
the dirty-confirm guard blocks a clobber; the data explorer lists a real datasource (seeded via the real
`datasource.add`) + local schema + real series (seeded via `seedIotDemo`) with NO DSN rendered; a denied
datasource section renders an honest deny.

**No regressions across the reader extraction** (`… src/features/dashboard`): `Test Files 4 passed (4) ·
Tests 35 passed (35)`.

**UI unit suite** (`pnpm test`): `Test Files 18 passed (18) · Tests 114 passed (114)`.

## Open questions / scope updates

All scope decisions were honored exactly (static catalog mirroring the crate; extract the schema reader,
both consume it; one insert-at-cursor primitive; no new dep/Tabs primitive; example bodies reuse proven
gateway-test bodies; no new caps/verbs/tables/`localStorage`/`if cloud`; honest deny/empty/loading). The
named follow-ups stand (per-external-datasource table introspection — needs a host verb; in-editor Rhai
autocomplete; placeholder-aware insert). Nothing in building contradicted the scope. No bugs hit → no
debugging entry this session.
