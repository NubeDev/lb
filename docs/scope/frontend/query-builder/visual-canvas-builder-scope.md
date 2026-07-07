# Visual canvas builder scope — joins on a React-Flow canvas over the typed `SqlBuilderQuery`

Status: scope (the ask). Slice 1 of [`query-builder-10x-scope.md`](query-builder-10x-scope.md). Promotes
to `public/frontend/query-builder.md` on ship.

Replace the shipped row-list `VisualEditor` with a **drag-and-connect canvas** (Tabularis's UX): tables
are boxes with checkable columns; dragging column-to-column makes a visual join whose type cycles
INNER→LEFT→RIGHT→FULL→CROSS; a "Query settings" side panel holds WHERE/HAVING/GROUP BY/ORDER BY/LIMIT.
The canvas is a **view over the extended typed `SqlBuilderQuery`** — the model stays the source of truth,
`emitSql` stays the renderer. This slice adds **joins, HAVING, column aliases, multi-column ORDER BY, and
OR-grouped filters** to the model and both emitters.

## Goals

- **Visual joins.** Drag `a.col`→`b.col` to add a `SqlJoin`; click the edge to cycle join type. The join's
  ON keys are the connected column handles (Tabularis's model — `visualQuery.ts:166` traverses edges to
  emit `<type> JOIN <table> ON tA.<h> = tB.<h>`; we do the same, but into typed `SqlJoin[]`, not a string).
- **Per-column aggregation + alias + select-order.** A column popover picks aggregation
  (incl. `count_distinct`) and an alias; column order in SELECT is stable.
- **WHERE + HAVING with AND/OR.** A filter row carries a logical operator and an `isAggregate` flag; an
  aggregate filter emits into HAVING, a plain one into WHERE (Tabularis's split — `visualQuery.ts:245/281`).
- **Multi-column ORDER BY.** Several `{column, direction}` clauses, ordered.
- **Live dialect-correct preview**, unchanged in spirit: `emitSql(dialect, query)` on every edit.
- **Backward-compatible model.** Every existing persisted `SqlBuilderQuery` remains valid and reopens
  correctly; all new fields are optional.
- **Canvas is a projection.** Node positions are view state (persisted as an opaque `builderLayout` blob,
  never as query semantics). An AI/headless caller can still build `SqlBuilderQuery` with no canvas.

## Non-goals

- **No Surreal joins in v1.** SurrealQL has no ANSI `JOIN … ON`. Gate the join affordance on
  `dialect === "standard"`; a surreal target keeps the single-table builder. Surreal record-link joins are a
  named follow-up (open question). Never emit invalid SurrealQL.
- **No subqueries, CTEs, window functions, UNION, or write/DDL.** Still SELECT-only, host-leashed.
- **No new backend / verb / cap.** Pure model + emitter + UI.
- **No results grid or run wiring** — that's slice 3. This slice ends at "the model emits correct SQL and
  round-trips"; the existing panel-builder Run path already consumes `rawSql`.
- **No cross-datasource joins.** All join tables come from the one selected datasource's schema.

## Intent / approach

### The model extension (`panel-kit/sql/query.ts`)

Extend the existing interfaces; every addition optional so old data is still valid.

```ts
// UNCHANGED core; NEW optional fields marked.
export interface SqlColumn {
  name: string;
  aggregation?: SqlAggregation;   // + "count_distinct"
  alias?: string;                 // NEW — result column name
  table?: string;                 // NEW — qualifies the column when joins exist (else the FROM table)
  order?: number;                 // NEW — position in SELECT (Tabularis ColumnAggregation.order)
}

export type SqlLogical = "AND" | "OR";              // NEW
export interface SqlFilter {
  column: string;
  table?: string;                 // NEW — qualify under joins
  operator: SqlOperator;          // + "LIKE" | "IS NULL" | "IS NOT NULL" (decided — see OQ #2); "IN" deferred
  value?: string | number | boolean;  // now OPTIONAL — IS NULL / IS NOT NULL carry no value
  logical?: SqlLogical;           // NEW — how it joins the PREVIOUS filter (default AND)
  isAggregate?: boolean;          // NEW — true ⇒ emitted into HAVING, not WHERE
  aggregation?: SqlAggregation;   // NEW — REQUIRED when isAggregate: HAVING must emit the aggregate
                                  // expression (`AVG("value") > 10`), NEVER the SELECT alias —
                                  // ANSI/Postgres forbid aliases in HAVING (peer-review fix)
}

export type SqlJoinType = "inner" | "left" | "right" | "full" | "cross";  // NEW
export interface SqlJoinKey {
  leftTable?: string;             // NEW — which prior table owns leftColumn; default = FROM table.
                                  // Required for correctness with ≥2 joins (the left side of join N
                                  // may be ANY previously-joined table, not the FROM table) (peer-review fix)
  leftColumn: string;
  rightColumn: string;            // always a column of `SqlJoin.table`
}
export interface SqlJoin {                                                 // NEW
  table: string;                  // the joined (right) table
  type: SqlJoinType;
  on?: SqlJoinKey[];              // usually one key; array allows composite joins.
                                  // OPTIONAL/empty for "cross" — CROSS JOIN has no ON clause; the
                                  // emitter must omit ON when type === "cross" (peer-review fix)
}

export interface SqlBuilderQuery {
  table: string;                  // FROM (primary) table — UNCHANGED
  joins?: SqlJoin[];              // NEW
  columns: SqlColumn[];           // UNCHANGED shape, richer SqlColumn
  filters: SqlFilter[];           // now AND/OR + WHERE; aggregate ones split to HAVING by isAggregate
  groupBy: string[];              // UNCHANGED (may become {table,column}[] under joins — open question)
  orderBy?: SqlOrderBy | SqlOrderBy[];   // NEW — WRITE the array shape always; READ accepts the legacy
                                         // single object. Round-trip contract: new-shape cells are
                                         // byte-identical; a legacy single-orderBy cell round-trips
                                         // SEMANTICALLY (normalized to a 1-element array on first
                                         // save) — pinned by a legacy fixture (peer-review decision)
  limit?: number;                 // UNCHANGED
}
```

`emptyQuery()` and `SqlSourceState`/`emptySqlSource()` are unchanged (new fields default to
absent/empty). **`builderLayout`** (canvas node positions) is a SEPARATE optional blob on `SqlSourceState`,
opaque to the model and the emitter — recommendation, confirm in OQ #3:

```ts
export interface SqlSourceState {
  mode: SqlEditorMode; rawSql: string; builder?: SqlBuilderQuery; format: SqlFormat;
  builderLayout?: unknown;   // NEW — opaque React-Flow node positions; never read by emitSql
}
```

### The emitter extension (`toSurrealQL.ts` / `toStandardSql.ts` behind `emitSql`)

`toStandardSql.ts` gains, in order: `FROM <t0> <join clauses> WHERE <and/or> GROUP BY … HAVING …
ORDER BY <multi> LIMIT`. Reuse the existing `ident()`/`renderValue()` (never raw concat — the injection
guard). Column qualification: when `joins` is non-empty, qualify every identifier as `"table"."column"`;
alias each SELECT expr (`AS "alias"`); the ON clause is `"<t>"."<l>" = "<t2>"."<r>"`. HAVING is the
`isAggregate` filters; WHERE is the rest; AND/OR chains by each filter's `logical`.

`toSurrealQL.ts` gains HAVING/aliases/multi-sort/OR **but not joins** (v1). When `dialect === "surreal"`
and `joins` is non-empty (should not happen — the UI gates it), the emitter drops joins and the preview
shows a single-table query (defensive; the affordance is gated upstream). **If the join emission diverges
enough, split `toStandardSql.ts` into a join-aware file** (the common-scope OQ #1 trigger) — decide during
build; one file is fine if the ANSI join clause stays readable.

### The canvas UI (new files, `features/query-builder/canvas/` — shared, not dashboard-local)

Port Tabularis's *component design* (`/tmp/tabularis/src/components/ui/{VisualQueryBuilder,TableNode,JoinEdge}.tsx`)
onto our primitives + `@xyflow/react` (already a dep — the Flows canvas uses it). One responsibility per file:

| File | Purpose | Tabularis analog |
|---|---|---|
| `QueryCanvas.tsx` | The React-Flow canvas: renders a node per `query.table`+`query.joins`, an edge per join key; drop-to-add-table; drag-to-connect→`SqlJoin`; dispatches typed edits to the model. **Reads/writes `SqlBuilderQuery` only.** | `VisualQueryBuilder.tsx` (minus its string-concat + Tauri invoke) |
| `TableNode.tsx` | A table box: checkable columns, per-column popover (aggregation/alias). Emits column ticks/agg/alias as model edits. | `TableNode.tsx` |
| `JoinEdge.tsx` | The join line; click cycles the `SqlJoinType`. | `JoinEdge.tsx` |
| `QuerySettingsPanel.tsx` | Side panel: WHERE/HAVING rows (with AND/OR + isAggregate), GROUP BY, multi ORDER BY, LIMIT + the live SQL preview. | the sidebar half of `VisualQueryBuilder.tsx` |
| `canvasModel.ts` | Pure `SqlBuilderQuery` ⇄ `{nodes, edges}` projection (derive nodes/edges from the model + `builderLayout`; map a connect/disconnect back to a `SqlJoin` edit). No React. | (Tabularis has no equivalent — it stores nodes as truth; this file is our seam) |

`VisualEditor.tsx` becomes a thin host that renders `QueryCanvas` + `QuerySettingsPanel` (or, behind a flag,
the old row list — see migration). `SqlQueryEditor.tsx`'s Builder⇄Code contract, `emitSql` call, and
schema prop are **unchanged** — the canvas is just a richer Builder body.

**Schema feed unchanged.** The canvas consumes the same `schema: Schema` prop (`{tables:[{name,columns:[{name,type}]}]}`)
the row builder does — the host supplies `store.schema` (local) or the `useFederationSchema` projection
(federation). Dragging a table onto the canvas references a table already in `schema.tables`; lazy column
fill is the host's job (already handled for federation). **No new schema path, no Tauri `get_columns`.**

**Copy discipline (decided, peer review 2026-07-06).** The user's concern — *rewrites miss things* — is
addressed by **copying at the file level wherever the file is compatible**, not by adopting Tabularis's
architecture. Concretely: `TableNode.tsx` / `JoinEdge.tsx` / the settings-panel JSX are **copied from
Tabularis and adapted** (swap their store hooks for our typed-edit dispatch, their Tailwind classes for our
tokens) rather than re-imagined — keep their interaction details (hover states, type-cycling, column
popover, drag affordances) verbatim. What is NOT copied is the data layer (nodes-as-truth, string-concat
`visualQuery.ts`, Tauri `invoke`) — see the rejection below. **Anti-miss gate:** before the slice is called
done, produce a *parity checklist* in the session doc — every prop, handler, and visible behaviour of
Tabularis's `VisualQueryBuilder.tsx`/`TableNode.tsx`/`JoinEdge.tsx` (558 + companions lines — enumerable in
one pass), each marked **ported / deliberately dropped (reason)**. A behaviour missing from the checklist
is a review failure. Copied files carry the Apache-2.0 attribution header (see slice 2 §Licensing).

**Rejected: Tabularis's node/edge-as-truth + `visualQuery.ts` generator.** It would (a) fork our
persistence (canvas blob becomes the query), (b) bypass `emitSql` and the dialect seam, (c) reintroduce
verbatim value interpolation. We keep the model as truth and treat the canvas as a controlled view — more
plumbing (`canvasModel.ts`) but it preserves every core invariant.

## How it fits the core

- **Rule 10.** The canvas/emitter select behaviour by `dialect` (config), never a datasource name. The join
  affordance is gated on `dialect === "standard"` — a `kind`-derived value, not an id.
- **Rule 8.** File plan above — canvas node/edge/settings/model each their own file, ≤400 lines. If
  `QueryCanvas.tsx` approaches the limit, extract the drop/connect handlers to `canvasHandlers.ts`.
- **Rule 9.** Emitter extensions pinned by pure goldens (no mocks). The canvas's model round-trip is a pure
  unit test. The real-rows proof is slice 3's gateway test.
- **Rules 2, 5, 6.** Inherited from the umbrella — no engine, cap, or wall change.
- **Tenancy / caps / secrets / durability / SDK.** N/A here (pure UI + TS); the umbrella covers the read
  caps the eventual Run uses.
- **Skill doc.** N/A — no new drivable verb.

## Example flow

1. Host renders `<SqlQueryEditor dialect="standard" schema={fedSchema} value={state} onChange=…>`; Builder
   mode now shows `QueryCanvas` + `QuerySettingsPanel`.
2. `canvasModel.toFlow(query, builderLayout)` yields one node (`point_reading`) — the empty query's FROM
   table, or none if unset. The user drops `site` from the rail → `QueryCanvas` sets `query.table` (if
   first) or adds a node awaiting a join.
3. The user drags `site.id`→`point_reading.site_id`. `onConnect` maps the two handles to
   `{ table:"point_reading", type:"inner", on:[{leftColumn:"id", rightColumn:"site_id"}] }` and appends it
   to `query.joins`. `emitSql` now renders `… FROM "site" INNER JOIN "point_reading" ON "site"."id" = "point_reading"."site_id"`.
4. Tick columns, set `avg` + alias on `value`, add a HAVING row (`isAggregate`), sort by two columns. Each
   edit flows through `onChange({ …value, builder, rawSql: emitSql(dialect, builder) })` — the existing
   contract.
5. Builder→Code shows the emitted SQL; Code→Builder confirms (unchanged). Save persists `SqlSourceState`
   (+ `builderLayout`); reopen restores the diagram.

## Testing plan

Per `scope/testing/testing-scope.md`. This slice is pure UI + TS — its mandatory contribution is **goldens
+ round-trip**; the capability-deny / workspace-isolation mandatories are exercised in slice 3's gateway
test (the real run path).

### Unit (pure)

- **Emitter goldens** — extend `toStandardSql.test.ts` (and `toSurrealQL.test.ts` for the non-join
  additions):
  - single INNER/LEFT/RIGHT/FULL/CROSS join → the exact `… JOIN … ON …` string (standard only);
  - composite join key (`on` length 2);
  - qualified identifiers under joins (`"t"."c"`), aliases (`AS "x"`), `count_distinct` → `COUNT(DISTINCT "c")`;
  - WHERE vs HAVING split by `isAggregate`; AND/OR chaining by `logical`;
  - multi-column ORDER BY;
  - **back-compat**: a pre-slice query (no joins/having, single orderBy) emits byte-identical to today;
  - empty/incomplete (`table === ""`) → `""`.
- **`emitSql` dispatch** — `dialect.test.ts`: surreal with `joins` present drops joins (defensive), standard
  emits them.
- **`canvasModel` round-trip** — `canvasModel.test.ts`: `toFlow` then a connect/disconnect then back to
  `SqlBuilderQuery` yields the expected `joins`; node positions never appear in the query.
- **`cellEditorState` round-trip** — extend the existing suite: an extended query (joins + having +
  aliases + `builderLayout`) round-trips byte-identical; a pre-slice cell reopens (empty joins, single
  orderBy read back).

### What is NOT tested here

No mocks/fakes; no run wiring (slice 3 owns the real gateway test). The canvas's DOM measurement in jsdom
follows the Dockview rect-stub pattern only in slice 3 where it's mounted for real.

## Risks & hard problems

- **SurrealQL join gap** (the headline risk) — resolved v1 by gating joins to `standard`. If a future need
  for Surreal joins arrives, it splits the emitter and adds record-link traversal — do not shoehorn ANSI
  `JOIN` into SurrealQL.
- **Canvas/model drift** — the single-writer discipline (`canvasModel.ts` maps view events → typed edits;
  the model re-derives the view) is the guard. A bug here shows as a diagram that disagrees with the
  preview; the round-trip test catches the common cases.
- **Group-by/order-by under joins need qualification** — `groupBy: string[]` may need `{table,column}`
  when a column name is ambiguous across joined tables (OQ). v1: qualify from the column's owning node;
  keep `string[]` if unambiguous, promote to `{table,column}[]` only if a golden forces it.
- **Model migration** — all-optional fields + a pre-slice round-trip fixture keep old cells valid.
- **`@xyflow/react` in jsdom** — measures layout; slice 3's mount uses the rect-stub. This slice's tests
  are pure (no canvas mount), sidestepping it.

## Open questions — RESOLVED (peer review 2026-07-06, "best long-term" directive)

1. **Surreal joins** — DECIDED: standard-only in v1, affordance gated on dialect; Surreal record-link joins
   filed as a follow-up. Never emit invalid SurrealQL.
2. **Operator set** — DECIDED: add `LIKE` + `IS NULL`/`IS NOT NULL` now (`value` is optional in the model —
   see the interface); defer `IN` until a value-list UI exists.
3. **`builderLayout` persistence** — DECIDED: persist as an opaque blob on `SqlSourceState`. Long-term it
   avoids the "diagram scrambles on reopen" complaint; it is never read by `emitSql`.
4. **`groupBy` shape under joins** — DECIDED: promote to accepting `(string | {table, column})[]` — a plain
   string means the FROM table (back-compat), the object form qualifies. Long-term correct (ambiguous
   column names across joined tables are inevitable) and cheap now; retrofitting later touches persisted data.
5. **Split `toStandardSql.ts` for joins?** — DECIDED: split when either emitter file crosses ~250 lines or
   the join clause needs dialect-internal branching, whichever first; don't pre-split a readable file.

## Related

- [`query-builder-10x-scope.md`](query-builder-10x-scope.md) — the umbrella (cross-cutting decisions).
- `docs/scope/frontend/query-builder-common-scope.md` — the shipped `SqlBuilderQuery` + `emitSql` seam this extends.
- `ui/src/lib/panel-kit/sql/query.ts` · `toSurrealQL.ts` · `toStandardSql.ts` · `dialect.ts` — the files edited.
- `ui/src/features/dashboard/builder/sql/{VisualEditor,SqlQueryEditor}.tsx` — the Builder host (VisualEditor's body is replaced; SqlQueryEditor's contract is kept).
- `ui/src/features/flows/FlowCanvas.tsx` — the in-repo `@xyflow/react` reference (theme, controls, node registration).
- Tabularis reference (design only): `/tmp/tabularis/src/components/ui/{VisualQueryBuilder,TableNode,JoinEdge}.tsx`, `/tmp/tabularis/src/utils/visualQuery.ts` (the ON-key traversal at `:166`, the WHERE/HAVING split at `:245/:281`).
</content>
