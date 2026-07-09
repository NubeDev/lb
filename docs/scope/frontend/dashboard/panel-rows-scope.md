# Dashboard scope — panel rows (collapsible section grouping)

Status: **scope (the ask)**. Stage 2 of [`grafana-conversion-scope.md`](grafana-conversion-scope.md).
Additive over the shipped v3 `Cell` ([`viz/panel-model-scope.md`](viz/panel-model-scope.md)). Promotes to
[`public/dashboard/dashboard.md`](../../../../doc-site/content/public/dashboard/dashboard.md) on ship.

One paragraph: give a dashboard **rows** — full-width, titled, collapsible section headers that group the
panels beneath them, exactly as Grafana does. We add them the way Grafana does: **a row is a panel** — a
`Cell` with `view:"row"` living in the same flat `cells[]` array, discriminated by its view. Membership is
**positional** (the cells between this row's `y` and the next row's `y` belong to it), which is Grafana's
*expanded* encoding, so no per-cell field is added and a drag-reorder never rewrites a membership list.
Collapse is a boolean option that hides the members and pulls their vertical space closed. This is a
**client-side layout feature over an additive record field** — zero new host verbs, zero new tables — and
it makes the Grafana→us mapper ([`viz/import-export-scope.md`](viz/import-export-scope.md)) able to carry a
Grafana row 1:1 instead of degrading it.

## Goals

- **A `row` cell** (`view:"row"`, `w:24`, `h` small) that renders a full-width collapsible header with a
  title and a chevron; additive/serde-default so a pre-rows dashboard loads unchanged.
- **Positional membership** — a row owns the cells between its `y` and the next row's `y`. No `rowId` on
  child cells; membership is derived from geometry, matching Grafana's expanded model.
- **Collapse/expand** — a row's `options.collapsed` boolean hides its member cells and collapses their
  vertical space; expanding restores them at their stored positions. Persisted via `dashboard.save`.
- **Author affordances** — add a row (from the panel palette / a "＋ Row" button), rename it inline, drag it
  (members move with it), delete it (offer "delete row only" vs "delete row + panels").
- **Mapper-ready** — the storage encoding the Grafana mapper normalizes *both* Grafana row encodings onto
  (collapsed-nested `row.panels[]` and expanded-siblings), stated here so the mapper is mechanical.

## Non-goals

- **No row `repeat`** (repeat a row per variable value) in this slice — it depends on multi-value variables
  ([`dashboard-variables-advanced-scope.md`](dashboard-variables-advanced-scope.md)) and is a **named
  follow-up** of both. Stated, not silently dropped.
- **No nested rows / no tabs.** Grafana's classic model has no nested rows; the v2 `TabsLayoutKind` /
  nested-rows redesign is a future export format we do not target.
- **No new grouping table or verb.** Rows are cells; they ride the shipped `dashboard.save`/`get`.
- **No change to non-row cells.** A member cell is an ordinary v1/v2/v3 cell; it gains nothing.

## Intent / approach

**A row is a `Cell` with `view:"row"`; membership is positional.** This is the smallest change that is also
1:1 with Grafana. The `View` union (`ui/src/lib/dashboard/dashboard.types.ts`) gains `"row"`; the grid
renderer (`features/dashboard/Grid.tsx`) special-cases the row view to draw a full-width header bar instead
of a widget frame, and to fold/unfold the cells geometrically beneath it. A tiny pure helper
(`lib/dashboard/rows.ts`) computes membership from the `cells[]` + their `y` — the one place that knows "the
cells under a row are the ones between its `y` and the next row's `y`". Collapse writes
`options.collapsed:true` and the layout hook shifts member cells' effective height to 0 (kept in the record
at their real positions, so expand restores them).

**Rejected: a separate `rows[]` structure on the dashboard.** Cleaner-looking, but it forks the flat
`cells[]` grid every renderer/import path already speaks, breaks the 1:1 Grafana map (Grafana rows *are*
panels), and needs a new record field + save/get handling. Row-as-cell rides everything shipped.

**Rejected: an explicit `rowId` on each child cell.** Explicit membership survives a reorder without
recomputation, but it adds a per-cell field, diverges from Grafana's positional model (forcing the mapper
to *invent* ids), and creates a consistency burden (an orphaned `rowId`, a stale membership on drag). We
store what Grafana's *expanded* dashboard stores — geometry — and derive membership. The one cost
(reordering must keep a row contiguous with its members) is a layout-hook concern, not a data one.

**Storage encoding (the mapper contract).** We store the **expanded** form always: a row cell + its members
as flat siblings ordered by `y`; `options.collapsed` is a *view* flag, not a different serialization. On
**import**, a Grafana *collapsed* row (children nested in `row.panels[]`) is flattened to siblings with
`collapsed:true`; a Grafana *expanded* row maps straight across. On **export**, a `collapsed:true` row
re-nests its positional members into `row.panels[]` (Grafana's collapsed encoding); an expanded row emits
siblings. The dual-encoding normalization lives in the mapper
([`viz/import-export-scope.md`](viz/import-export-scope.md)); this scope only pins *our* single stored form.

## How it fits the core

- **Tenancy / isolation (rule 6):** a row cell is ordinary workspace-scoped bytes on the `dashboard:{id}`
  record; nothing new. Isolation test: a ws-A dashboard with rows is invisible to ws-B (the shipped
  dashboard isolation, re-run with a row present).
- **Capabilities (rule 5/7):** authored under the shipped `mcp:dashboard.save:call`; read under
  `dashboard.get`. **No new cap.** Deny path unchanged — saving a dashboard with a row cell without the save
  cap is denied opaquely (the mandatory deny test, re-run with a row).
- **Placement (rule 1):** pure client-side layout over an additive field; `either`, no role branch.
- **MCP surface (§6.1):** **no new verb.** Rows ride `dashboard.save`/`dashboard.get`. If the
  [`widget-catalog-scope.md`](widget-catalog-scope.md) host save-validation lands (reject unknown `view`),
  `"row"` must be a catalog entry so a row cell is not rejected — **flagged** as the one host-side touchpoint
  (a catalog data-file entry, not a code branch).
- **Data (SurrealDB):** `cells[]` gains `view:"row"` cells; no new table, no new top-level field. State vs
  motion holds.
- **Extensions (rule 10):** `"row"` is an opaque view id; a row groups `ext:<id>/<widget>` cells like any
  other. No branch on an extension id.
- **SDK/WIT impact:** the `View` union gains `"row"` (additive). No `vars`-lib touch.

## Example flow

1. In the dashboard editor the author clicks **＋ Row**. A `Cell{ view:"row", w:24, h:2, y: <bottom>,
   title:"New row" }` is appended and `dashboard.save`d.
2. The author drags three existing panels below the row header. On save they are siblings with `y` greater
   than the row's and less than any next row's — so `rowMembers(cells, rowCell)` returns them.
3. The author clicks the row's chevron. The editor writes `options.collapsed:true`; the layout hook sets the
   members' effective height to 0 and pulls the rows below upward. `dashboard.save` persists
   `collapsed:true`; the members keep their real `x/y/w/h`.
4. Reopening the dashboard, the row renders collapsed with a "3 panels" count; expanding restores the three
   panels at their stored positions.
5. Later the author imports a Grafana dashboard with a collapsed row (children in `row.panels[]`). The mapper
   flattens them to siblings with `collapsed:true` — the imported row behaves identically to an authored one.

## Testing plan

Per [`../../testing/testing-scope.md`](../../testing/testing-scope.md) — real gateway, real store, seeded
real rows, no `*.fake.ts`.

- **Unit (`lib/dashboard/rows.test.ts`):** `rowMembers` returns exactly the cells between a row's `y` and
  the next row's `y`; a dashboard with no rows returns every cell as un-grouped; two adjacent rows partition
  cleanly; a collapsed row's members are still returned (membership is positional, independent of collapse).
- **Gateway (`features/dashboard/rows.gateway.test.tsx`):** save a dashboard with a row + members → `get`
  round-trips the row cell + `collapsed` flag byte-clean; toggling collapse persists; deleting "row only"
  leaves members, "row + panels" removes them.
- **Additivity (mandatory regression):** a pre-rows dashboard record (no `view:"row"` cell) loads, renders,
  and round-trips unchanged.
- **Capability deny (mandatory):** saving a dashboard containing a row cell without `mcp:dashboard.save:call`
  is denied opaquely.
- **Workspace isolation (mandatory):** a ws-A dashboard with rows is not readable by ws-B.
- **Mapper hand-off (in the import-export scope, referenced):** a Grafana collapsed row and an expanded row
  both normalize to our single stored form and re-export to their original encoding — the round-trip guard.

## Risks & hard problems

- **Positional membership vs drag-reorder.** Dragging a panel across a row boundary silently changes its
  membership; dragging a *row* must carry its members. The layout hook owns keeping a row contiguous with
  its members on drag — the one non-trivial interaction, and the reason positional (not `rowId`) needs a
  deliberate reorder rule. Test the drag-across-boundary case.
- **Collapse + react-grid-layout.** Hiding members by zeroing height must not corrupt their stored `y`
  (expand must restore exactly). Keep the real geometry in the record; collapse is a render-time transform.
- **Empty rows / trailing row.** A row with no members below it (last row, nothing under it) must render an
  honest empty section, not swallow the next content.

## Open questions

- **Storage encoding** (from the umbrella): confirmed **positional / expanded-form** here — the answer the
  umbrella leaned toward. Revisit only if the drag-contiguity rule proves too costly, in which case an
  explicit `rowId` is the fallback (documented, not built).
- **Collapse-height mechanism:** zero the members' `h` at render, or maintain a parallel "visible cells"
  layout the grid consumes? Lean render-time transform (keeps the record clean). Resolve in build.
- **Delete UX default:** does "delete row" default to row-only (safe) or row+panels? Lean **row-only**
  (least destructive) with an explicit "＋ delete N panels" affordance.

## Related

- [`grafana-conversion-scope.md`](grafana-conversion-scope.md) — the umbrella + the audit that ranks rows
  a "close now" gap · [`viz/import-export-scope.md`](viz/import-export-scope.md) — the mapper that
  normalizes Grafana's dual row encoding onto our stored form.
- [`viz/panel-model-scope.md`](viz/panel-model-scope.md) — the additive `Cell` + the `View` union `"row"`
  extends · [`widget-catalog-scope.md`](widget-catalog-scope.md) — the host save-validation `"row"` must be
  a catalog entry for (the one host touchpoint).
- Grafana reference: `/tmp/grafana/kinds/dashboard/dashboard_kind.cue` (`#RowPanel` `:833-857`),
  `public/app/features/dashboard/state/DashboardModel.ts:956-1035` (`toggleRow` — the dual-encoding source).
- Public: [`../../../../doc-site/content/public/dashboard/dashboard.md`](../../../../doc-site/content/public/dashboard/dashboard.md).
- README **§3** (rules 1/5/6/7/10), **§6.1** (API shape — why no new verb).
