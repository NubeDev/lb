# Session — panel rows (collapsible section grouping)

Built Stage 2 of the Grafana-conversion umbrella from
[`scope/frontend/dashboard/panel-rows-scope.md`](../../scope/frontend/dashboard/panel-rows-scope.md):
full-width, titled, collapsible section headers that group the panels beneath them, as
**a row is a cell** (`view:"row"`) with **positional membership** — Grafana's expanded encoding.
No new host verb, no new table, no new cap.

## What shipped

- **`View` union gains `"row"`** ([`ui/src/lib/dashboard/dashboard.types.ts`](../../../ui/src/lib/dashboard/dashboard.types.ts))
  — additive/serde-default, so a pre-rows dashboard loads unchanged.
- **`lib/dashboard/rows.ts`** — the one place that knows positional membership:
  `rowMembers(cells, row)` = the non-row cells whose `y` is between this row's `y` and the next row's
  `y`; `ungroupedCells`, `rows`, `isRow`, `isCollapsed`, and `visibleCells` (the render-time transform
  that drops a collapsed row's members while keeping their record geometry). `ROW_W = 12` (our grid is
  12-col, not Grafana's 24), `ROW_H = 1`.
- **`views/RowHeader.tsx`** — a flat, full-bleed Grafana-style section bar (bottom border, no card):
  hover grip · chevron · title · "· N panels" count, with the chevron+title as one full-width toggle
  (click anywhere collapses; double-click renames) and a hover remove at the far right. The bar is
  `h-full` and full-bleed (no inset gutter) so its edges align edge-to-edge with the panels below —
  the initial "floating card + left gutter" version read as misaligned vs Grafana and was reworked.
- **`Grid.tsx`** — renders `visibleCells(cells)` and special-cases `isRow(c)` to draw the header bar
  (drag handle + remove only, no widget chrome; `isResizable:false`). The drag-carry rule lives in
  `apply`: a moved row shifts its (pre-move) members by the same Δy so a section stays contiguous —
  the one non-trivial interaction positional membership needs. Merged cleanly with the concurrent
  `WidgetCell`/`useDisplayOverride` extraction on the non-row branch.
- **`DashboardView.tsx`** — a "＋ Row" button (appends `Cell{view:"row"}` at the board bottom),
  `toggleRow` (flips `options.collapsed`), `renameRow`, all through the shipped `dash.saveCells` →
  `dashboard.save`. Remove of a row is **row-only** (least-destructive default per the scope's open Q).
- **Host touchpoint (the one flagged in the scope):** `"row"` added to
  [`widget_catalog.json`](../../../rust/crates/host/src/dashboard/widget_catalog.json) as
  `kind:"layout"`, `data:false` — so the shipped save-gate (`dashboard/views.rs`
  `check_view_cells`) accepts a row cell instead of rejecting it as an unknown view. `layout` kind is
  exempt from the host test's "viz views must carry options" rule. A matching `case "row"` in
  `WidgetView.tsx` keeps the catalog↔renderer bijection (`widgetCatalog.consistency.test`) — the Grid
  intercepts rows before the widget dispatcher, so that case is only a read-only fallback.

## Row presentation options (added)

Three per-row toggles, editable in a **popout modal** (`RowOptionsDialog.tsx`) opened from a gear on
the row bar (hover), and also registered in the panel-builder option registry so the wizard's Options
step edits the same defs:

- **Show panel count** (`options.showCount`, default `true`) — the "· N panels" count beside the title.
- **Show divider line** (`options.showLine`, default `true`) — the bottom rule under the bar.
- **Collapsed by default** (`options.collapsed`, default `false`) — the stored open/closed state on load
  (it IS the collapse flag; "default" = the value applied when the dashboard loads).

Both display flags default TRUE so a pre-options row is unchanged; only an explicit `false` hides them
(`rowOptions()` reader in `rows.ts`). Registry def `panel-builder/options/defs/row.ts` (group "Row",
`row` added to `NO_FIELDCONFIG_VIEWS`); catalog `row` entry gains the two new `options`-scope toggles.
`saveRowOptions` in `DashboardView` merges them into the cell's `options` via `dashboard.save`.

## Tests (all green)

- **Unit** [`rows.test.ts`](../../../ui/src/lib/dashboard/rows.test.ts) — 9 tests: positional
  membership (between/adjacent-rows/trailing/no-rows), collapse-independence, `visibleCells` render
  transform, non-mutation of stored geometry.
- **Gateway** [`rows.gateway.test.tsx`](../../../ui/src/features/dashboard/rows.gateway.test.tsx) — 6
  tests, real node + real store: row cell + members + `collapsed` round-trip byte-clean; toggle
  persists; row-only delete leaves members; **additivity** (pre-rows dashboard unchanged);
  **capability deny** (row save without `dashboard.save` refused server-side); **workspace isolation**
  (ws-A rows invisible to ws-B).
- **Consistency** — `widgetCatalog.consistency.test` still green with `"row"` in both catalog + switch.
- **Host** — `catalog_parses_ids_unique_viz_has_options` still green (`layout` kind exempt).
- No regression in the existing `DashboardView.gateway.test.tsx` (11 tests).

`pnpm exec tsc --noEmit` clean; eslint clean on new files (Grid keeps only its pre-existing raw-button
warnings).

## Not done / follow-ups (stated in scope)

- Row `repeat` — depends on multi-value variables; a named follow-up.
- Mapper normalization of Grafana's dual row encoding onto our stored form lives in
  [`viz/import-export-scope.md`](../../scope/frontend/dashboard/viz/import-export-scope.md); this
  session pins only *our* single stored (expanded) form.
- A "＋ delete N panels" affordance on row delete (we default to row-only).

## Notes

Committing is left to the user (working on master, tokens saved) — no commit made this session.
