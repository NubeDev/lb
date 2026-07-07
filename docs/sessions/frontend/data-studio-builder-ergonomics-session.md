# Data Studio — stacked builder ergonomics (gallery rail + collapsible preview)

**Date:** 2026-07-06 · **Scope:** follow-up UX on
[data-studio-10x-scope.md](../../scope/frontend/data-studio-10x-scope.md) phase 3 (the
stacked query-first builder). User ask (screenshot-annotated): *move the panel selections*
into the unused right-hand area, and *let me minimise the panel preview* so the options
surface gets the vertical room.

## What changed

- `ui/src/features/panel-builder/VizGallery.tsx` — new `orientation?: "row" | "column"`
  prop. `"column"` renders the same cards as a **vertical rail** (`w-44`, `overflow-y-auto`,
  full-width cards); `"row"` is the unchanged horizontal strip (the default, so no other
  consumer moves).
- `ui/src/features/panel-builder/PreviewPane.tsx` — optional `open` / `onOpenChange`
  props. When provided, the existing "Preview" label becomes the disclosure toggle (one
  label, not a second header bar); collapsed, only a slim bar renders and the height goes
  to the flex sibling. Omitted → the always-open pane (split layout untouched).
- `ui/src/features/panel-builder/BuilderPane.tsx` (stacked stage 2/3) — the layout is now
  two columns: LEFT = collapsible preview + status bar + `OptionsDrawer` (which is `flex-1`
  when the preview folds, so options win the space); RIGHT = the `VizGallery` column rail.
  The `flowKind` path keeps its `VizPicker` pill row in the left column (no rail).
  Preview-open state is per-tab, in-session (`useState`), matching the drawer's precedent.

Rejected alternative: a separate disclosure bar above `PreviewPane` — it duplicated the
pane's own "Preview" label; folding the toggle into the pane keeps one affordance.

## Slice 2 — big-result freeze (display-only downsampling)

User report: the page freezes with a large result. Cause: every row became an SVG node, and
the stacked builder renders the dataset up to 7× at once (preview + 6 gallery thumbnails);
the table view rendered every row to the DOM. Fix — bound what is DRAWN, never what is
fetched/transformed/saved (rule 9 untouched):

- `ui/src/features/charts/downsample.ts` — `downsamplePoints` (min/max per bucket, spikes
  survive; rejected naive striding for that reason) + `downsampleRows` (shared-x
  representative rows for the multi-series plot, first/last kept). Unit tests in
  `downsample.test.ts`.
- `ui/src/features/charts/chartBudget.tsx` — `ChartBudgetProvider`/`useChartBudget`
  (default 1500 ≈ a full-width panel's pixels).
- Applied at the chart chokepoints: `SeriesLineChart` + `TimeseriesChart`
  (`dashboard/widgets/recharts.tsx`) and `PlotChart` (post-aggregation, so grouping still
  sees every row). Domains/legend/latest math still run on the raw series.
- `VizGallery` wraps thumbnails in a **120-point budget** — 6 mini-charts of a 50k-row
  result now draw ≤720 points total instead of 300k.
- `TablePanel` caps the DOM at 500 rows with an announced "showing first 500 of N rows"
  footer (sorting still sees all rows). Virtualization rejected for now — the cap is one
  line and honest; revisit with `@tanstack/react-virtual` if paging is asked for.

### Slice 2b — high-cardinality "split by" (never hide data)

Splitting by `point_id` (~600 distinct values) pivots into ~600 series: 600 SVG lines plus
a 600-entry Recharts legend that painted over the entire page. **Rejected:** a top-N series
cap (drafted as `capSeries.ts`, deleted) — the user's call, and correct: dropping series is
hiding data. Shipped instead, in `PlotChart`:

- **Every series is always drawn.** The per-series point budget shrinks as series count
  grows (total drawn points bounded at ~8× the chart budget, floor 50 points/series), so
  the page cost is capped without dropping a single series.
- **The legend is contained, not culled** — `maxHeight: 88px` + `overflowY: auto` on both
  the cartesian and pie legends: every entry still exists, it scrolls inside the pane
  instead of burying the page. `Wrap` gains `overflow-hidden` as the escape backstop.

## Slice 3 — a working render-template starter for the demo data

The shipped `DEFAULT_INLINE_CODE` (`dashboard/builder/editors/TemplateSourceField.tsx`)
bound a per-site aggregate the demo flow never runs. Replaced with a latest-readings table
bound to the exact rows of the last-N-per-meter window query
(`SELECT *, ROW_NUMBER() OVER (PARTITION BY point_id ORDER BY time DESC) AS rn FROM
point_reading … WHERE rn <= 100`): `{{rows.length}}`, `{{latest.point_id}}`/`{{latest.value}}`,
and `{{#each rows}}` over `{{point_id}}`/`{{time}}`/`{{value}}`/`{{rn}}`. The editor hint
paragraph now documents that shape. Regression: `templateInterpolate.test.ts` renders
`DEFAULT_INLINE_CODE` against those rows and asserts every binding resolves.

## Tests (green)

- `cd ui && pnpm test` — **114 files / 705 tests passed** (includes `VizGallery.test.tsx`,
  `OptionsDrawer.test.tsx`).
- `cd ui && pnpm test:gateway src/features/data-studio/DataStudioBuilderFlow.gateway.test.tsx`
  — **5/5 passed** against a real spawned gateway, including the capability-deny case
  (`viz.query` denied → honest degradation).
- `pnpm tsc --noEmit` — no new errors (pre-existing: `app-sidebar.tsx` SidebarMenuBadge/Sub,
  `SourcesPane.tsx` unused Suspense — on the branch before this session).

## Related

- Scope: [../../scope/frontend/data-studio-10x-scope.md](../../scope/frontend/data-studio-10x-scope.md)
- Sibling session: [data-studio-10x-session.md](data-studio-10x-session.md)
- Public: [../../public/frontend/data-studio.md](../../public/frontend/data-studio.md)
