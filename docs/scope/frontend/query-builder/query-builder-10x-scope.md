# Query-builder 10x scope — umbrella: a Tabularis-grade visual builder, a schema-aware editor, and a standalone Query workbench

Status: scope (the ask). Promotes to `public/frontend/query-builder.md` once the slices ship.

We already ship a **visual SQL builder** (shipped 2026-07-06, `query-builder-common-scope.md`): a typed
`SqlBuilderQuery` edited by a Builder⇄Code editor, rendered to N dialects behind an emitter seam
(`emitSql`), fed by `store.schema` / `federation.schema`, run by `store.query` / `federation.query`. It
is clean but **basic**: one table, one column list with five aggregations, ANDed equality filters, a
single ORDER BY, a LIMIT — no joins, no HAVING, no aliases, no OR, no multi-sort. The Code half is a
CodeMirror editor with the stock `@codemirror/lang-sql` grammar and **no schema-aware completion**.

We cloned **Tabularis** (`/tmp/tabularis`, a React + Tauri multi-DB SQL workspace) whose visual query
builder and editor are materially better. This umbrella turns "steal the good parts of Tabularis" into
three buildable slices that **extend our shipped seams** rather than replace them, plus a **standalone
Query workbench view** (like Flows / Rules) that later mounts as a Data Studio pane. A companion doc
[`tabularis-harvest.md`](tabularis-harvest.md) inventories everything else in Tabularis worth taking and
says, per item, take-now / take-later / skip.

## The one-line thesis

**Copy Tabularis's code file-by-file where the file is compatible; port only the data layer.** (Revised at
peer review 2026-07-06 — the user's experience is that rewrites silently drop features, and he's right.)
Pure-TS utilities (`sqlSplitter`, `sqlFormat`) are copied **verbatim** with attribution; component files
(`TableNode`, `JoinEdge`, the settings panel) are copied and *adapted* — interaction details kept intact,
only their store/Tauri/Monaco couplings rewired onto our seams — with a mandatory per-file **parity
checklist** (every prop/handler/behaviour: ported or deliberately-dropped-with-reason) as the anti-miss
gate (slice 1 §Copy discipline). What is never copied is the architecture: our `SqlBuilderQuery` + `emitSql` dialect seam
is the source of truth and stays it; Tabularis's React-Flow canvas, its schema-aware Monaco completion,
and its `sqlSplitter`/`sql-formatter` are the UX we graft onto that seam. We never adopt Tabularis's
string-concat SQL generator (it bypasses our dialect seam and has an injection surface), its Tauri
`invoke("get_columns")` schema path, or its Monaco dependency — each has a shipped equivalent here.

## The three slices

| Slice | Scope doc | One line | Backend? |
|---|---|---|---|
| **1. Visual canvas builder** | [`visual-canvas-builder-scope.md`](visual-canvas-builder-scope.md) | Extend `SqlBuilderQuery` with joins / HAVING / aliases / multi-sort / OR-groups; render it on a `@xyflow/react` canvas (drag tables, connect columns to make joins) as a *view* over the typed model; extend both dialect emitters. | **None.** |
| **2. Schema-aware SQL editor** | [`sql-editor-10x-scope.md`](sql-editor-10x-scope.md) | Feed the live `Schema` into `@codemirror/lang-sql`'s built-in completion (table + column IntelliSense); add a dialect-aware multi-statement splitter (ported `sqlSplitter`) and a Format button (`sql-formatter`). Stay on CodeMirror — no Monaco. | **None.** |
| **3. Query workbench** | [`query-workbench-view-scope.md`](query-workbench-view-scope.md) | **(Retargeted 2026-07-06, user decision.)** No new rail surface — upgrade the existing **Datasources page**: `DatasourceDetail`'s ad-hoc SQL area becomes the full workbench (builder+editor+run+results), extracted as a reusable `QueryWorkbench({ ws, sel, onSel })`. One `VIEW_PANES` line also opens it as a Data Studio pane. | **None.** |

All three are **UI + pure-TS only**. No new MCP verb, capability, table, or outbox target — they reuse
`store.schema`/`store.query` and `federation.schema`/`federation.query` over the generic `/mcp/call`
bridge (verified: `docs/prompts/data-studio/README.md` + the query-builder-common scope). **Saving named
queries is explicitly the user's separate work** (a `querydef.*` verb following the `layout.*` chain) and
is out of scope for all three slices — §"Saved queries" names the seam it plugs into so nothing here
blocks it.

## Goals

- **Join-capable visual authoring.** A non-SQL user can drag two tables onto a canvas, connect a column
  in one to a column in the other to make an INNER/LEFT/RIGHT/FULL/CROSS join, tick columns, add
  aggregations + aliases, add WHERE/HAVING rows (with AND/OR), sort by several columns, and see a live,
  dialect-correct SQL preview — then run it and see rows.
- **Schema-aware Code editing.** In Code mode, typing `table.` offers that table's columns; typing in a
  fresh statement offers the workspace's tables; a Format button pretty-prints; multi-statement text is
  split so a single statement can be run/previewed.
- **One builder, three homes, one model.** The exact same component tree serves (a) the dashboard/panel
  builder it already serves, (b) the new standalone `/t/$ws/query` view, and (c) a Data Studio pane —
  because the view takes the Flows/Rules `{ ws, sel, onSel }` prop shape. The persisted `SqlBuilderQuery`
  round-trips byte-for-byte as it does today (`cellEditorState.ts`).
- **Zero backend, zero new dependency.** `@xyflow/react` (canvas) and `@codemirror/lang-sql` (completion)
  are already in `ui/package.json`; `sql-formatter` is the one small add. No Rust change.

## Non-goals

- **No new MCP verb / cap / table / outbox target** in any slice. If a slice finds it needs one, that is
  a finding to surface, not a silent add (platform checklist §6).
- **No NEW saved-query persistence.** Saved queries **shipped 2026-07-06** on the existing `query.*`
  verbs (see §"Saved queries"); the slices consume that, never add a second store. Query history and the
  "New view" chooser listing saved queries remain follow-ups.
- **No Monaco.** Slice 2 stays on CodeMirror (rationale in that scope). Adopting Monaco is recorded as the
  considered-and-rejected alternative.
- **No write SQL, no DDL, no multi-statement *execution*.** Every path stays SELECT-only, parse-allowlisted
  + workspace-walled + row-capped at the host, exactly as today. The splitter lets you *pick* one statement
  to run; it never runs a batch.
- **No rich editable data grid (yet).** Slice 3 renders results with the shipped viz/table render path.
  Tabularis's tanstack-table+virtual grid with inline edit is a named harvest follow-up, not slice-3 scope.
- **No new query engine.** SurrealDB (`store.query`) and the DataFusion federation sidecar
  (`federation.query`) stay the only two engines (rule 2). Tabularis's 13-driver plugin protocol is a
  harvest note, not this work.

## Intent / approach — the cross-cutting decisions

The per-slice docs carry the detail. The decisions that bind all three, made here so they are not
re-litigated three times:

1. **`SqlBuilderQuery` stays the single source of truth; the canvas is a *projection*.** Tabularis stores
   the query AS React-Flow `{nodes, edges}` and string-concats SQL from it. We do the opposite: the typed
   `SqlBuilderQuery` (extended in slice 1) is what persists and what `emitSql` renders; the canvas node
   positions are **view state** (optionally persisted as an opaque `builderLayout` blob for nice reopening,
   never as the semantic truth). This keeps the dialect seam, the round-trip contract, and rule-10
   discipline intact, and means an AI or a headless caller can still author `SqlBuilderQuery` without a
   canvas. **Rejected: adopt Tabularis's node/edge-as-truth model** — it would fork our persistence, break
   `emitSql`, and reintroduce the string-concat injection surface.

2. **Extend the emitters, don't fork them.** Joins/HAVING/aliases/multi-sort are added to BOTH
   `toSurrealQL.ts` and `toStandardSql.ts` behind the same `emitSql(dialect, query)` dispatch. Where a
   dialect genuinely differs (Surreal has no ANSI `JOIN … ON` — see slice 1's risk section), that is the
   trigger to split a per-dialect emitter file (the OQ #1 the common scope already anticipated), not to
   branch inside the component.

3. **Stay on CodeMirror.** `@codemirror/lang-sql@6.9.1` already accepts a `schema` for completion; our
   `useCodeMirrorTheme` already themes it; it is used across the app and plays well inside the multi-pane
   Dockview (Monaco's theme is a global singleton — awkward with N panes). We port Tabularis's completion
   *behaviour* (schema-fed, dot-triggered) onto CodeMirror's native provider. **Rejected: swap to Monaco**
   — a ~2 MB dependency + a web worker + a global-theme model, to buy an IntelliSense feel CodeMirror can
   substantially match with the schema we already load. If, after slice 2 ships, the team still wants a
   full IDE editor, that is a clean future swap (open question, recorded — not blocking).

4. **One component, the Flows/Rules prop shape.** The view is `QueryWorkbenchView({ ws, sel, onSel })` so
   the *same* component is the routed page AND the Data Studio pane (`ViewDockPanel` mounts it with
   `EmbeddedPageContext`). This is exactly how Flows/Rules already double as pages and panes.

## How it fits the core

Addressed once here for the shared posture; each slice's doc re-checks the ones it touches.

- **Workspace is the hard wall (rule 6).** No slice takes a workspace argument. `store.schema`/`store.query`
  derive the workspace from the session token; `federation.schema`/`federation.query` are workspace-pinned
  at the host (`resolve(&store, ws, source)`). The canvas and editor render only the walled schema the host
  returned; the emitter is client-side and cannot name a cross-tenant table. **Mandatory workspace-isolation
  test applies** (a ws-B session sees no ws-A tables and a forged cross-tenant target is refused at the host).
- **Capability-first (rule 5).** No new cap. The builder reads under `mcp:store.schema:call` /
  `mcp:federation.query:call` (the latter gates both schema discovery and query, by design) and runs under
  `mcp:store.query:call` / `mcp:federation.query:call`. A member without a cap sees the surface **degrade
  honestly** — empty dropdowns/completion, a denied Run surfaced verbatim, never fabricated rows.
  **Mandatory capability-deny test applies.**
- **Symmetric nodes (rule 1).** Pure UI + TS; identical on edge and cloud. The federation sidecar is a
  Tier-2 extension wherever it runs; the builder neither knows nor branches.
- **One datastore (rule 2).** SurrealDB stays the authority; a federation target *reads* an external DB
  through the gated verb. No slice adds a persistence layer (saved queries are already `query:{ws}:{id}`
  SurrealDB records behind the shipped `query.*` verbs).
- **State vs motion (rule 3).** N/A — request/response reads. Builder state is a `cell.options.sql` record
  (state); a run is one `store.query`/`federation.query` call (not motion).
- **Stateless extensions (rule 4).** N/A — no extension change.
- **MCP is the contract (rule 7).** The builder emits the `sql`/query arg of existing MCP tools; nothing
  downstream knows a query was authored on a canvas.
- **Core knows no extension (rule 10).** The dialect is selected from the datasource `kind`
  (`surreal | standard`), never a datasource name. The canvas/editor never branch on an id. Swapping
  sqlite→postgres changes `kind`, never a core mediation.
- **No mocks / no fake backend (rule 9).** Slice 3's gateway test drives the **real** spawned gateway +
  the real SQLite demo datasource (`make seed-demo-sqlite`), the pattern from
  `federation_sqlite_test.rs`. Emitter extensions are pinned by pure goldens. No `*.fake.ts`.
- **One responsibility per file (rule 8).** File plans in each slice. The canvas node/edge components,
  the emitter extensions, the completion source, and the splitter are each their own file.
- **API shape (§6.1).** N/A for slices — they add no MCP verb. (The user's saved-query work adds CRUD +
  list; see §"Saved queries".)
- **Durability / secrets.** N/A.
- **SDK/WIT impact.** None. Pure TS frontend.
- **Skill doc.** N/A for the slices (no new agent-/API-drivable verb). Saved queries ride the existing
  `query.*` verbs (already the platform's drivable surface); no `querydef.*` and no new skill doc — verify
  the existing `query.*` skill coverage mentions the `datasource:<name>` target form.

## Saved queries — SHIPPED (2026-07-06) on the existing `query.*` verbs

> **Updated 2026-07-06:** saved queries landed on the Datasources page, and the hypothetical `querydef.*`
> chain this section previously sketched is **dead — do not build it**. The platform already had a full
> workspace-scoped `query.*` verb family (`query.list/get/save/delete/compile/run`,
> `ui/src/lib/queries/queries.api.ts`); the datasources surface now uses it:
>
> - `features/datasources/useDatasourceQueries.ts` — per-source hook: `query.list` (whole-workspace
>   roster) filtered **client-side** to `target === "datasource:<name>"` (a pure projection, no second
>   call), plus load/save/remove. Saves with `lang:"raw"` + the implicit datasource target (raw SQL
>   against the external engine; PRQL is the platform-target workbench's language).
> - `features/datasources/SaveQueryDialog.tsx` — id/name/description form; the author supplies the slug,
>   the current editor SQL + datasource target are baked in; errors surfaced verbatim.
> - `features/datasources/SavedQueriesDialog.tsx` — lists the filtered roster; click loads the text into
>   the editor (resolving the full record via `query.get` — the roster row omits `text`); per-row delete.
> - `DatasourceDetail.tsx` wires both dialogs into the SQL editor header, beside Run.
>
> A saved query is a `query:{ws}:{id}` record whose `target` (`datasource:<name>`) `query.run` resolves
> in the caller's workspace. **Slice 3 must build on this** — the workbench's `sel` is a saved-query id
> loaded via `query.get`, and its Save goes through the same hook; no new persistence, verb, or cap.

What remains open for slice 3 is only presentation: whether the Data Studio "+ Open view" chooser lists
saved queries directly, and the `?query=<id>` deep-link on the datasources route.

## Example flow (the headline, end to end)

1. In the rail, the user clicks **Query** (the new surface) → `/t/$ws/query`. `QueryWorkbenchView` opens a
   fresh builder over the default datasource; the canvas is empty with a "drag a table" affordance.
2. The user drags `site` and `point_reading` from the Sources rail onto the canvas. Each becomes a table
   node listing its columns (columns loaded lazily via `store.schema`/`federation.schema`).
3. The user drags from `site.id` to `point_reading.site_id` → a join edge appears; clicking it cycles
   INNER→LEFT→…. This writes a `SqlJoin` into `SqlBuilderQuery.joins`.
4. The user ticks `site.name`, and `point_reading.value` with aggregation `avg` and alias `avg_value`; adds
   a WHERE `point_reading.ts > '2026-01-01'` and a HAVING row on `avg(value) > 10` — emitted as
   `HAVING AVG("point_reading"."value") > 10`, never the alias (ANSI/Postgres forbid SELECT aliases in
   HAVING; the UI may *display* the alias, the emitter re-expands the aggregate); sorts by `site.name` asc,
   `avg_value` desc; limit 100. Every edit regenerates the SQL via `emitSql(dialect, query)`; the live
   preview + the Code editor show it.
5. The user switches to **Code**, and as they type `site.` completion offers `id, name, …` (slice 2). They
   hit **Format**; the SQL pretty-prints. They switch back to **Builder** (confirm-on-clobber, unchanged).
6. **Run** → `store.query`/`federation.query` under the read cap, workspace-walled. Rows render in the
   results grid (slice 3).
7. Later (user's saved-query work) they **Save** the query; it appears in the Data Studio "+ Open view"
   list and as a deep-linkable `/t/$ws/query/$id`.

## Testing plan (umbrella — slices carry the detail)

Per `scope/testing/testing-scope.md`. Mandatory categories that apply across the slices:

- **Capability-deny (§2.1)** — slice 3 gateway test: a session without `mcp:store.query:call` /
  `mcp:federation.query:call` sees empty schema/completion and a denied Run surfaced verbatim; no rows
  fabricated.
- **Workspace-isolation (§2.2)** — slice 3 gateway test: ws-B sees no ws-A tables; a forged cross-tenant
  target is refused at the host.
- **No mocks (§0)** — the real gateway + real SQLite demo datasource; emitter goldens are pure, not fakes.
- **Emitter goldens (unit)** — slice 1 extends `toSurrealQL.test.ts` / `toStandardSql.test.ts` with
  join/HAVING/alias/multi-sort/OR cases per dialect, plus the empty/incomplete-builder `""` contract.
- **Round-trip (unit)** — slice 1 extends `cellEditorState.test.ts`: an extended `SqlBuilderQuery`
  (with joins/having) round-trips byte-identical and a **pre-slice** query (no joins) still reopens.
- **Completion (unit)** — slice 2: given a `Schema`, the completion source yields the right table/column
  candidates for a dot-trigger and a bare position.

## Risks & hard problems

- **SurrealQL has no ANSI `JOIN … ON`.** The biggest gap between the two dialects. Slice 1 must decide the
  Surreal join strategy (record-link graph traversal vs. a `WHERE`-join vs. *disable the join affordance
  for the surreal dialect and only enable it for standard/federation targets*). Recommendation in slice 1:
  **enable joins only for `standard` dialect in v1**, gate the canvas join affordance on `dialect`, and file
  Surreal joins as a follow-up — never emit invalid SurrealQL. This is the natural trigger to split
  `toSurrealQL`/`toStandardSql` into join-aware per-dialect emitters.
- **Model migration.** Extending `SqlBuilderQuery` must keep every existing persisted cell readable. All new
  fields are optional; a pre-slice query is a valid extended query (empty `joins`, no `having`). Pinned by
  the round-trip test with a pre-slice fixture.
- **Canvas ↔ model sync.** Keeping the React-Flow view in lockstep with the typed model (and not letting
  node positions leak into the semantic query) is the fiddly part. Slice 1 makes the model the single
  writer; the canvas dispatches typed edits, never stores query semantics in `{nodes, edges}`.
- **CodeMirror completion quality vs. Monaco.** CodeMirror's schema completion is good but not Monaco's
  full IntelliSense (no signature help, weaker context inference). Accept for v1; the open question records
  the Monaco path if the gap proves unacceptable.
- **Injection.** Tabularis interpolates filter values verbatim. Our emitters already escape (`renderValue`
  doubles quotes) — slice 1 must keep every new value/identifier path going through `ident()`/`renderValue`,
  never raw concat.

## Open questions — RESOLVED (peer review 2026-07-06; user directive: decide for best long-term)

1. **CodeMirror vs. Monaco** — **CONFIRMED by the user: CodeMirror.** Closed.
2. **Surreal join strategy** — DECIDED: `standard`-dialect joins only in v1; Surreal record-link joins are a
   filed follow-up. Never emit invalid SurrealQL.
3. **Persist canvas positions** — DECIDED: yes, opaque `builderLayout` blob on `SqlSourceState`; never
   semantic truth.
4. **Results grid** — DECIDED: reuse the shipped render path (the datasources page's `QueryResults.tsx`);
   the rich Tabularis grid stays a harvest follow-up.

Per-slice open questions are likewise resolved in their docs (operator set, `groupBy` qualification,
orderBy array normalization, formatter gating/keybinding, splitter home, picker default). The
reconcile-saved-queries open item the peer review surfaced is **CLOSED (2026-07-06)**: saved queries
shipped on the existing `query.*` verbs via the Datasources page (see §"Saved queries") — `querydef.*` is
dead and no second store exists.

## Related

- README §3 (rules 1–10 — the non-negotiables).
- [`visual-canvas-builder-scope.md`](visual-canvas-builder-scope.md) · [`sql-editor-10x-scope.md`](sql-editor-10x-scope.md) · [`query-workbench-view-scope.md`](query-workbench-view-scope.md) — the slices.
- [`tabularis-harvest.md`](tabularis-harvest.md) — everything else in Tabularis worth taking.
- `docs/scope/frontend/query-builder-common-scope.md` — the SHIPPED builder this extends (the `SqlBuilderQuery` + `emitSql` seam).
- `docs/scope/frontend/data-studio-10x-scope.md` + `docs/prompts/data-studio/README.md` — the Data Studio surface + the standalone-view/pages-as-panes registration map.
- `docs/scope/frontend/system-catalog-scope.md` — the `@nube/source-picker` catalog the canvas drags from.
- `docs/scope/datasources/sqlite-datasource-demo-scope.md` — the seeded `demo-buildings` fixture the gateway test runs against.
- `docs/public/frontend/data-studio.md` · `docs/public/frontend/query-builder.md` (the stub this promotes into).
- Tabularis source (read-only reference, do NOT copy code wholesale): `/tmp/tabularis/src/components/ui/VisualQueryBuilder.tsx`, `/tmp/tabularis/src/utils/visualQuery.ts`, `/tmp/tabularis/src/utils/autocomplete.ts`, `/tmp/tabularis/src/utils/sqlSplitter/`.
</content>
</invoke>
