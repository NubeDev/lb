# Frontend scope — the rules editor UX (a guided, explorable authoring surface)

Status: **scope** (the ask). Promotes to `public/frontend/rules-workbench.md` (the editor-UX additions) once
shipped. Target stage: **S9+ collaboration UI** — extends the **shipped** rules workbench Playground
(`public/frontend/rules-workbench.md`, `scope/frontend/rules-workbench-scope.md`). This is a **frontend-only**
slice: **no host changes, no new MCP verbs, no new caps**. It is reusable UI components + the rules editor
surface, over verbs that **already exist**.

A user who has **no idea how Rhai rules work** should be able to open the Playground and *discover* the
engine's vocabulary: a searchable, categorized **function palette** (the real registered Rhai verbs, with
signatures + one-line descriptions, one-click to insert at the cursor), a set of ready-to-run **examples**
(one click loads a working rule into the buffer), and a **data explorer** (the registered external
datasources, the local store schema, and the discoverable series — each click inserts the matching
snippet). The editor goes from "a blank CodeMirror box" to a guided surface that teaches itself.

## Goals

- **A function / helper palette** — a browsable, searchable, categorized list of the Rhai functions the
  engine actually registers (mirrored 1:1 from the `lb-rules` crate, never invented): **Data** (`source`,
  `query`, `history`, `span`, `last`, `param`), **Grid** (`filter`, `select`, `add_col`, `rename`,
  `group_by`, `join`, `col`, `head`, `size`, `columns`, `records`, `agg`, and the `Col` reductions
  `max`/`min`/`avg`/`mean`/`sum`/`count`/`std`/`first`/`last`/`p`), **Timeseries** (`rollup`, `lag`, `delta`,
  `rate`, `interpolate`, `gapfill`, `resample` — surfaced as a first-class category), **AI** (`ask`,
  `complete`, `classify`, `embed`), **Output** (`emit`, `alert`, `log`). Each entry shows its **signature**
  and a **one-line description** (lifted from the crate `///` docs), filterable by name, **click-to-insert**
  a snippet at the cursor. A typed catalog under `features/rules/catalog/<category>.ts` (one named data file
  per family — never a `utils` dump). **An entry that lies about a signature is worse than none**, so the
  catalog mirrors the crate verbs exactly.
- **Examples / recipes** — a small set of ready-to-run example rules a newcomer clicks to learn: a
  temperature-threshold alert, a rollup+aggregate, a findings/emit example, a federated-query example, and a
  trivial scalar. **One click loads the example into the buffer**, respecting the dirty indicator (confirm
  before clobbering unsaved edits). Bodies reuse the ones **already proven green** in the gateway tests so
  at least one runs.
- **A datasource / data explorer** — a panel of what the user can query, each click-to-insert:
  - **Registered external datasources** via `datasource.list` (kind + endpoint + redacted ref, never a DSN);
    click → inserts a `source("<name>")` / `query("<name>", "SELECT …")` snippet.
  - **The local SurrealDB schema** (tables + columns) via `store.schema`, through the **already-shipped**
    reader (`readSchema`/`Schema`/`SchemaTable`); click a table/column → inserts the matching snippet.
  - **The discoverable series** via `series.list` (the shipped `listRealSeries`); click → inserts a
    `history("series", "<name>", "24h")` snippet for the timeseries helpers.
- **Reusability (a hard requirement).** The schema reader currently lives in a **dashboard-named** module
  (`lib/dashboard/sql.api.ts`); it is not dashboard-specific. **Extract** it to a shared, named module
  `lib/schema/` and have **both** the dashboard SQL builder **and** the rules data explorer consume it. A
  single reusable **insert-at-cursor** primitive (a `CodeEditor` ref handle + a hook) is used by every
  palette/explorer panel. A reusable **`SchemaBrowser`** component (a click-to-pick table/column tree) lives
  in shared `components/`.
- **Design / UX (the headline).** A clean three-region layout — saved-rule rail · editor + run result ·
  a tabbed authoring panel (**Functions | Examples | Data**) — with search on the palette, hover/inline
  descriptions, keyboard-friendly controls, the project's shadcn primitives (`@/components/ui/*`) and the
  Lazybones design tokens (`border-border`, `bg-bg`/`bg-muted`/`bg-card`, `text-muted`/`text-fg`,
  `bg-accent`) — never raw Tailwind palette colors. Empty/denied/loading states are **honest** (a denied
  `datasource.list` renders a deny, never fake data).

## Non-goals

- **No host/backend changes, no new MCP verbs, no new caps.** Everything is built over the shipped
  `rules.*` / `datasource.*` / `store.schema` / `series.*` verbs the Playground already calls. (If building
  reveals a genuine need for a read-only host introspection verb — e.g. per-external-datasource table
  introspection — that crosses the line and is a **named follow-up**, not a silent gap; see below.)
- **No per-external-datasource table introspection.** `datasource.list` gives kind + endpoint only;
  `store.schema` is the **local** store. So the explorer shows local-store tables/columns + the
  registered-source roster + the series roster. **Deep per-external-table browsing is a NAMED FOLLOW-UP**
  (it needs a host verb that doesn't exist), not an implied capability. The explorer is honest about this.
- **No new editor dependency.** Reuses the shipped `@uiw/react-codemirror` + `@codemirror/lang-javascript`
  (Rhai is JS-like). No Monaco, no language server.
- **No autocomplete / LSP.** The palette is click-to-insert, not an in-editor IntelliSense provider. A
  CodeMirror Rhai completion source is a named follow-up.
- **No rewrite of the dashboard SQL VisualEditor.** The genuinely shared dependency is the schema *reader*
  (extracted, both consume it). The VisualEditor's typed-query **dropdowns** are a different affordance from
  the explorer's click-to-insert **tree**; forcing one onto the other would regress the dashboard builder.
  Both consume the shared reader; the dashboard keeps its dropdowns. (Decision recorded below.)
- **No `*.fake.ts`, no mock data.** Tests drive a real in-process gateway seeded via the real write path.

## Intent / approach

**Extend, don't rewrite.** The Playground stays a two-pane page; we add a **third region**: a tabbed
authoring panel. The catalog is **static typed data** (the engine's registered verbs don't change at
runtime — they're compiled into `lb-rules`), so the palette is a local catalog, **not** a host query — the
honest call (there is no "list the registered Rhai functions" verb, and inventing one would be a host
change). The data explorer **is** dynamic (workspace-specific), so it reads the shipped `datasource.list` /
`store.schema` / `series.list` verbs.

```
  features/rules/RulesView
    ├─ RuleRail (rules.list/get/delete)        — saved-rule roster (unchanged)
    ├─ editor column
    │    ├─ RuleEditor → CodeEditor (ref: insertSnippet)   ◄── click-to-insert from any panel
    │    └─ RunResult (rules.run)                            (unchanged)
    └─ AuthoringPanel  [ Functions | Examples | Data ]      ◄── NEW
         ├─ FunctionPalette  ← catalog/* (static, mirrors lb-rules verbs)  → insertSnippet
         ├─ ExampleList      ← examples/examples.ts (proven bodies)        → loadExample (dirty-confirm)
         └─ DataExplorer     ← datasource.list + store.schema + series.list → insertSnippet
                                          (via the SHARED lib/schema reader + SchemaBrowser)
```

**The reuse extraction (the load-bearing refactor):**

- `lib/dashboard/sql.api.ts` today exports both `runQuery`/`QueryResult` (SQL-specific — `store.query`) **and**
  `readSchema`/`Schema`/`SchemaTable`/`SchemaColumn` (a generic store-schema reader). **Split:** the schema
  reader moves to `lib/schema/schema.api.ts`; `runQuery` stays. The dashboard `SqlQueryEditor`/`VisualEditor`
  and the rules `DataExplorer` both import `readSchema`/`Schema` from `@/lib/schema`. The existing dashboard
  `sqlSource.gateway.test.tsx` proves the dashboard side stays green over the extracted module; a new rules
  test proves the rules side — **one module, two consumers, both tested.**
- The **insert-at-cursor** primitive: a `components/codeeditor/CodeEditor.tsx` (a `forwardRef` wrapper over
  `@uiw/react-codemirror`, ref handle = `{ insertSnippet(text) }` via a CodeMirror transaction at the
  selection) + `useEditorInsert.ts`. `RuleEditor` consumes it; the panels call `insertSnippet`. Reusable by
  a future chain/editor surface.
- A reusable **`components/schema/SchemaBrowser.tsx`** — given a `Schema` + `onPick(table, column?)`, renders
  a collapsible, keyboard-navigable table/column tree. The `DataExplorer` consumes it.

**Rejected alternatives:**

- *A host verb that lists the registered Rhai functions.* Rejected — it would be a host change for data that
  is **static and compile-time known**; a typed UI catalog mirroring the crate is honest and zero-cost.
  Keeping the catalog accurate is a code-review discipline (it sits next to the crate verbs in the repo).
- *Force the dashboard VisualEditor onto a shared click-to-insert browser.* Rejected — the VisualEditor binds
  selections into a typed `SqlBuilderQuery` (a different interaction); the shared piece is the **reader**,
  which both already need. (Recorded.)
- *An in-editor LSP/autocomplete.* Rejected for v1 — click-to-insert teaches discovery without a Rhai grammar
  + completion source (the named follow-up). Over-building the language tooling before the palette is proven.
- *Fetch the explorer data eagerly on mount of a hidden tab.* Rejected — load per-panel on first reveal with
  honest loading/deny/empty states (the product register's skeleton/empty discipline).

## How it fits the core

- **Tenancy / isolation (rule 6):** the explorer reads only the **shipped** workspace-walled verbs
  (`datasource.list` / `store.schema` / `series.list`) — a ws-B session sees only ws-B sources/tables/series.
  The catalog is static (no tenancy surface). The workspace-isolation guarantee is the **already-tested**
  one on those verbs; this slice adds no new data path to wall. **No new isolation test needed beyond the
  shipped per-verb ones** (stated, not skipped) — the explorer is a *caller* of already-isolated reads.
- **Capabilities (rule 5/7):** the page is a **caller** of existing caps — **no new caps**. The explorer's
  reads gate on the **shipped** `mcp:datasource.list:call`, `mcp:store.schema:call`, `mcp:series.list:call`;
  a run uses the shipped `mcp:rules.run:call`. The UI renders a **deny honestly** (a denied source list shows
  a deny, never fake data); the gateway is the real wall (already re-checks every verb server-side). **A
  denied-explorer-section test** (a section without its read cap renders a deny, not a fabricated list) is
  this slice's capability test — there is no new *verb* to deny-test (HOW-TO-CODE §3 step 4a applies to new
  verbs; this slice ships none).
- **Symmetric nodes (rule 1):** no `if cloud`. The panel is the same app on Tauri and the browser; the verbs
  it calls are role-mounted already.
- **MCP surface — consumed, not added (§6.1):** **no MCP tools added.** The slice consumes the shipped
  read verbs (`datasource.list`, `store.schema`, `series.list`) and the shipped `rules.run`. No CRUD, no
  live feed, no batch is introduced. The function catalog is **static client data**, not a verb.
- **Data (SurrealDB):** no new tables, no new records. The catalog + examples are static code constants. The
  explorer reads existing records via existing verbs. No `localStorage` durable state (rule 4); the active
  tab + search box are transient component state.
- **Bus (Zenoh):** none — the explorer is record/schema reads, not motion.
- **Secrets:** none new. `datasource.list` already returns a **redacted** ref, never a DSN; the explorer
  surfaces only what the verb returns (a redaction assertion is inherited from the shipped datasources slice;
  re-asserted here that the explorer never renders a `dsn`).
- **One responsibility per file (FILE-LAYOUT):** one catalog data file per verb family; one component per
  `.tsx`; one hook per `use<Concept>.ts`; the reusable pieces named by concept (`CodeEditor`,
  `SchemaBrowser`, `useEditorInsert`, `useDataExplorer`) — never `utils`/`helpers`/`common`.

## Example flow

1. **Discover.** A new user opens **Rules**. The right panel defaults to **Functions**. They type "roll" in
   the search box → the palette filters to `rollup(every, agg)` under **Timeseries**, showing the signature
   and "Time-bucket + aggregate." They hover for the full description.
2. **Insert.** They click `history(source, point, span)` → `history("series", "<point>", "24h")` is inserted
   at the cursor. They switch to **Data**, expand the local schema, click `cooler.temp` under a table → the
   point name is inserted. They click `rollup` → `.rollup("1h", "avg")` appends.
3. **Learn from an example.** Instead, they open **Examples**, click "Temperature threshold alert" → with no
   unsaved edits the buffer loads the proven body; **Run** renders a `critical` finding. (With unsaved edits,
   a confirm guards the clobber.)
4. **Explore data.** Under **Data → Datasources**, `datasource.list` shows `timescale (postgres ·
   tsdb.acme:5432)`; clicking it inserts `query("timescale", "SELECT … FROM … LIMIT 100")`. A workspace whose
   caller lacks `mcp:datasource.list:call` sees an **honest deny** in that section — never a fake roster.

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md` (real infra, seeded via the real write path —
**no mock data, no `*.fake.ts`**; the frontend tests drive a **real in-process gateway** seeded with real
rows via `seedIotDemo` + a real `datasource.add` through the shipped client + `signInWithCaps`).

- **Capability deny (honest state).** A `DataExplorer` whose session lacks `mcp:datasource.list:call`
  (or `store.schema`/`series.list`) renders a **deny** for that section, not a fabricated list. (No new verb
  to deny-test — this slice adds none; the per-verb gateway deny-tests are already green from the workbench
  slice.)
- **Workspace isolation.** Inherited from the shipped `datasource.list`/`store.schema`/`series.list` verbs
  (already gateway-tested two-session isolated); the explorer is a caller. A UI assertion confirms a fresh
  workspace's explorer shows no other workspace's sources/series.
- **The reuse proof (this slice's headline).** The extracted `lib/schema` reader is consumed by **both**: the
  existing dashboard `sqlSource.gateway.test.tsx` stays green (dashboard SQL builder over the moved reader),
  and the new rules explorer test reads the same module. Asserted by both suites passing.
- **No mocks / real seed.** Every explorer datum is a real read (real seeded series, a real registered
  datasource); every example run is a real `rules.run`.

Plus this slice's specific cases (Vitest, real in-process gateway):

- **Palette renders the real categories + click-to-insert.** The palette shows Data/Grid/Timeseries/AI/Output
  with the real verb names; clicking an entry inserts its snippet into the editor buffer (assert the buffer
  contains the snippet). Search filters by name.
- **Example loads + runs green.** Clicking an example loads its body into the buffer; running it (a proven
  body, e.g. the scalar or the seeded-series history) returns a real result. The dirty-confirm guard blocks a
  clobber of unsaved edits.
- **Data explorer lists real data + click-to-insert.** With a seeded datasource + seeded series, the explorer
  lists the real datasource (kind + endpoint, **no DSN**), the local schema tables, and the series; clicking
  each inserts the matching snippet.
- **Honest empty/deny/loading.** A section with no data renders an empty state that teaches; a denied section
  renders a deny.

## Risks & hard problems

- **Catalog accuracy is the load-bearing promise.** A palette entry with a wrong signature is worse than
  none. Mitigation: the catalog mirrors the crate verbs (`rust/crates/rules/src/verbs/*`) entry-for-entry,
  lives in the same repo next to them, and the descriptions are lifted from the `///` docs. A reviewer
  diffs the catalog against the crate. (No runtime check is possible without a host verb — out of scope.)
- **Insert-at-cursor under a controlled CodeMirror.** `@uiw/react-codemirror` is controlled; the insert must
  dispatch a real CM transaction (`replaceSelection`) so `onChange` fires and the buffer updates — not a
  string concat that fights the controlled value. Mitigation: hold the `EditorView` via `onCreateEditor`,
  dispatch at the current selection, then refocus; jsdom has no layout engine, so the test pastes/inserts the
  real path (mirroring the existing `typeBody` helper).
- **The extraction must not break the dashboard.** Moving the schema reader is a cross-feature refactor.
  Mitigation: keep the type/shape identical, update both import sites, run the dashboard SQL gateway test as a
  regression gate (it must stay green).
- **Panel density vs. the editor.** The third region competes for width. Mitigation: a fixed-width side panel
  with internal tabs + scroll, the editor flexes; honest skeleton/empty states (product register), accent
  only for the active tab + selection.

## Open questions

Decisions are **made** so the slice codes with no open question:

**Resolved (decisions taken):**

- **Catalog is static typed data, not a host query.** The registered verbs are compile-time known; a UI
  catalog mirroring the crate is honest and avoids a host change. Decided.
- **Reuse boundary:** extract the schema **reader** to `lib/schema/`; both the dashboard SQL builder and the
  rules explorer consume it. The dashboard VisualEditor keeps its typed-query **dropdowns** (a different
  affordance); the new `SchemaBrowser` (click-to-insert tree) is the explorer's. Decided.
- **Insert primitive:** a `CodeEditor` `forwardRef` handle (`insertSnippet`) + `useEditorInsert`, used by
  every panel. Decided.
- **Panel shape:** one side panel, three tabs (Functions | Examples | Data); a lightweight tab control built
  from the shadcn `Button` (no shadcn `Tabs` primitive exists; adding Radix Tabs is out of scope — a small
  named segmented control is enough and on-token). Decided.
- **Example bodies reuse the proven gateway-test bodies** so at least one runs green. Decided.
- **No new caps / verbs / tables / `localStorage` / `if cloud`.** Decided.

**Named follow-ups (not silent gaps):**

- **Per-external-datasource table introspection** — needs a read-only host verb that doesn't exist today
  (`datasource.list` is kind+endpoint only). When warranted, that is a host slice (would require sign-off on
  the "no host changes" boundary), not part of this frontend slice.
- **In-editor Rhai autocomplete (LSP/completion source)** — a CodeMirror completion provider over the same
  catalog; additive after the palette is proven.
- **A "copy snippet" / parameter-aware insert** — inserting with the cursor placed inside the first
  placeholder; v1 inserts the snippet text.

## Related

- `scope/frontend/rules-workbench-scope.md` + `public/frontend/rules-workbench.md` — the shipped Playground
  this extends (the editor, the api clients, the honest cage/deny rule).
- `sessions/frontend/rules-workbench-session.md` — what shipped + the decisions taken.
- `scope/frontend/dashboard-scope.md` — the SQL builder + the schema reader (`store.schema`) this slice
  extracts and shares; the "render the deny not a blank" honesty rule.
- `scope/rules/rules-engine-scope.md` — the shipped `rules.*` engine + the registered verbs the catalog
  mirrors (`rust/crates/rules/src/verbs/*`).
- `scope/datasources/datasources-scope.md` — the shipped `datasource.list` the explorer reads (redacted, no
  DSN).
- `scope/frontend/ui-standards-scope.md` — the design-token + shadcn-primitive rules the panel honors.
- `scope/testing/testing-scope.md` — the real-gateway, real-seed, no-`fake.ts` discipline.
