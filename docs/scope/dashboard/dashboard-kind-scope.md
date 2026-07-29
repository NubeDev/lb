# Dashboard `kind` + `report.export` re-addressed at report-kind dashboards

Status: built (2026-07-29). Downstream consumer: `NubeIO/rubix-ai`
(`docs/scope/frontend/reports/report-as-dashboard-scope.md` and its server-render sibling).

One paragraph: lb carries **two** record types that are the same thing wearing different clothes — a
`dashboard` (a react-grid board of cells) and a `report` (a linear `blocks[]` notebook whose `panel`
blocks each *contain a `Cell`*). Everything the dashboard plane grew — variables, undo, version
history, the panel wizard, import/export — had to be wired a second time into `report.*` or simply
not exist there, and it mostly did not exist there. This scope collapses the fork from lb's side: a
report becomes **a dashboard whose record says so**, via a typed `kind` field, and `report.export`
is re-addressed at that record — composing its A4 pages from the cell grid instead of from
`blocks[]`. The wire contract of the export route is unchanged; only the meaning of its `id` moves.

## Goals

- **A typed `kind` on `Dashboard`**, `"dashboard" | "report"`, absent ⇒ `"dashboard"`, with the exact
  preserve-on-omit discipline `width`/`timezone` use (`page-settings-scope.md` is the precedent) —
  **plus** a place on `DashboardSummary`, which `width` does not have and this needs, because the
  roster is exactly where the two kinds get partitioned.
- **`report.export` accepts a report-kind dashboard id.** Same route (`POST /reports/{id}/export.pdf`),
  same capability (`mcp:report.export:call`), same `ExportBody { snapshots: [{cellId, png}] }` — the
  snapshot key is already `cell.i`, which a dashboard cell already has. It reads through
  `dashboard_get`, so the same three gates re-run under the exporter.
- **Pages laid from the grid.** `lb-render` gains an additive *placed page* path: a page may carry
  positioned rectangles instead of flowed markdown. Screen and print share one geometry source, with
  a round-trip test, because a panel that sits in a different place on paper than on screen is the
  failure mode this whole layout change invites.
- **Additive, always.** A document with no placements renders byte-identically to what shipped; a
  record with no `kind` is a dashboard; no migration, anywhere.

## Non-goals

- **Retiring the `report.*` notebook verbs.** `report.save`/`get`/`list`/`share` and the `Report`
  record stay in-tree, unused by the shipped consumer. Deleting them is separate housekeeping and
  would make this a breaking release for no gain.
- **Server-side chart rendering.** The browser is the chart renderer; lb composes pages and places
  the captures it is given. Unchanged.
- **A brand per report.** A `Dashboard` has no `brand_id`; a report uses the workspace's default
  brand. Adding a per-report brand is a page-setting like any other, additively, when wanted.

## Intent / approach

`kind` mirrors `width` at all fourteen of its call sites, plus the summary row and its projection.
It is **validated** at save where `width` is not: an unknown `width` degrades to the default layout
and is visible on screen, whereas a mistyped `kind` drops the record out of *both* rosters — a
record that saved "successfully" and can then be found nowhere.

`report/export.rs` swaps `report_get` for `dashboard_get` and refuses a non-report dashboard. Page
composition moves to `report/compose.rs`: sort cells into reading order, band them onto pages by grid
row, and pair each with its capture. A cell with **no** capture is still placed, as an error tile —
so an incomplete render produces a PDF with a visible hole rather than one that is quietly shorter.

In `lb-render`, `geometry.rs` holds the A4 numbers and the react-grid arithmetic evaluated in
millimetres, and `place.rs` emits `#place`d boxes. `Assembled.placements` is positionally aligned
with `pages`; empty ⇒ the markdown path.

**Rejected alternatives.** (i) *`kind` in `options`/carry* — lb's typed struct drops unknown
top-level keys, and the roster must partition cheaply; carry-blobs are for page-local opacity.
(ii) *A separate `report.export_dashboard` verb* — two verbs doing one job, and the caps story
splits. (iii) *Keep one-cell-per-page* — trivially compatible with the existing renderer and
completely wrong: a report page holds several panels side by side, which is the point of authoring it
on a grid. (iv) *Emit the grid as a markdown table and let Typst flow it* — loses absolute position,
so screen and print diverge exactly where the author cares most.

## How it fits

- **Isolation/tenancy.** Unchanged. A report-kind dashboard is a workspace asset under the same wall.
- **Capabilities.** Authoring gates on `dashboard.save`. Export keeps its own `report.export` cap
  **and** now requires `dashboard.get` — export never became a side door onto a board the caller
  cannot read, and there is a test in both directions proving neither deny is a tautology.
- **API/MCP surface.** No new verbs. `dashboard.save`/`get`/`list` carry `kind`; `report.export`
  re-addressed.
- **Rule 10.** Nothing added here names a consumer. `kind` is a generic record field; the placed-page
  path is a generic renderer capability; the export refuses on a property of the record.

## Testing plan

Mandatory categories covered: **capability-deny** (export without `report.export`; export without
`dashboard.get`; both with a passing negative control) and **workspace-isolation** (a report id from
another workspace does not export). Plus: `kind` round-trip and preserve-on-omit across a layout save
and a partial meta save; explicit demotion still possible; unknown kind refused at save and over
REST; the geometry round-trip contract; reading order; empty pages preserved; an uncaptured panel
still placed; and a byte-equality proof that a placement-free document renders unchanged.

## Related

- Precedent for the typed page-settings field: `../frontend/dashboard/page-settings-scope.md`.
- The renderer: `crates/render/` (the lazybones Typst port).
- Downstream: `NubeIO/rubix-ai` `docs/scope/frontend/reports/report-as-dashboard-scope.md`.
- Sibling seams released alongside: `./embedder-outbox-and-service-token-scope.md`.
