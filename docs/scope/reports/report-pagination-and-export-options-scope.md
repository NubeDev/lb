# Reports scope — authored pagination + a real export-options contract

Status: scope (the ask). Promotes to the public docs once shipped.

Owning repo: **this one (`lb`)** for everything below; released as a `node-v*` tag that
**rubix-ai** then pin-bumps (`rubix-ai docs/WORKFLOW-LB.md` §4). The consumer half — the
builder affordances, the export dialog, the PDF preview — is the paired rubix-ai scope
[`docs/scope/frontend/reports/report-builder-ux-scope.md`](../../../../rubix-ai/docs/scope/frontend/reports/report-builder-ux-scope.md)
(in the sibling checkout). **This scope ships first and stands alone**: every change here is
additive and backward-compatible, so bumping the pin with no rubix-ai UI work changes nothing
a user can see.

Today a report author cannot say what goes on a page. Page assignment is *derived* from a
cell's grid row — `paginate` bands cells by `y`, flows a band that would not fit onto the next
page, and that is the whole vocabulary. It works, and it is genuinely fit-aware, but it gives
the author no way to express "these four tiles are page 1, the trend chart starts page 2", so
a report that is nearly right is fixed by dragging panels until the derived breaks happen to
land where they were wanted. And `report.export` accepts **only** `{ snapshots }` — there is
no page size, no orientation, no margins, no page-number or table-of-contents toggle on the
wire, even though the renderer already implements the last two (`RenderOptions`) and nothing
sets them. This scope adds two things and nothing else: an **explicit page-break marker on a
cell**, honoured by `paginate`, and a typed **`ExportOptions`** on the export contract that
carries the page geometry and the render toggles a caller wants. Both are additive and
serde-defaulted, so every shipped caller keeps its exact current behaviour.

## Goals

- **An author can force a page break.** `Cell` gains `pageBreakBefore: bool` — "this band
  starts a new page". `paginate` honours it ahead of its fit rule. It is a *marker on a cell*,
  not a page number on a cell, so it survives a drag (see Intent).
- **Export options exist on the wire, typed and defaulted.** A new `ExportOptions` carries
  `paper`, `orientation`, `marginXMm`/`marginTopMm`/`marginBottomMm`, `scale`, `pageNumbers`,
  `index`. Absent ⇒ exactly today's A4 portrait / 2.2·2.4·2.2 cm / scale 2 / no page numbers /
  no index. Both doors (`POST /reports/{id}/export.pdf` and the `report.export` MCP arm) take
  it, and a test asserts they still compose byte-identical PDFs.
- **`RenderOptions` gets plumbed.** `page_numbers` and `index` already render; `report_export`
  simply never set them. It sets them from `ExportOptions` now — a two-line fix that turns two
  dead renderer features on.
- **Page geometry becomes a parameter, not six constants.** `A4_*` in `geometry.rs` and the
  hardcoded `#set page(paper: "a4", margin: (..))` in `pdf.rs` become one `PageGeometry` value
  threaded through placement and the template. A4 portrait stays the default, to the
  millimetre.
- **The geometry contract stays enforceable across repos.** `geometry.rs`'s round-trip test is
  what stops the screen and the PDF drifting. It keeps that job: the test now pins the
  *derived* values for every supported paper/orientation, and the rubix-ai mirror asserts the
  same table.
- **Nothing shipped changes.** A caller that sends no options and a record with no
  `pageBreakBefore` produces the same bytes as today. That is a test, not an intention.

## Non-goals

- **No per-cell page NUMBERS.** Rejected in Intent — absolute numbers rot on the first drag.
- **No new report record type, no new verb.** A report is still a report-kind dashboard;
  export is still `report.export`. This adds fields to both, nothing more.
- **No server-side data fetching for export.** The lens holds: the PDF still contains only
  what the exporter's browser could see and send. Unchanged.
- ~~**No stored export profiles here.**~~ **REVERSED — see below.** The original wording:
  *"Where an admin's chosen options are saved is the rubix-ai scope's problem (it stores a
  profile on the dashboard record and sends it at export time). lb only needs to accept options
  on the call — storing a blob it never interprets would be state without a reader."*
- **No preview endpoint.** The preview is real `report.export` bytes rendered client-side;
  it needs no new surface.
- **No reflow of panel CONTENT to fit a page.** A panel is a photograph; the page places it.
  Making a chart re-render at a different aspect for print is a different, much larger ask.

### Reversal — stored export profiles ARE an lb field

Recorded rather than deleted, because the reasoning that produced the non-goal was sound and
still wrong: it assumed *"the client stores it"* was an available option, and on this record it
is not.

`Dashboard` has no `#[serde(flatten)]` catch-all, so it **drops unknown top-level keys on save**.
A client-authored `exportProfiles` survives exactly until the next layout save and then silently
vanishes — a control that appears to work and forgets. There is no other place on the record for
a profile to live, so "lb stores nothing" and "an admin can save a profile" cannot both hold.

That is the same mechanical reason `heading`, `headingSize`, `icon`, `color`, `varsDisplay`,
`reportIds`, `width` and `compact` are all typed lb fields whose ONLY reader is the client. The
"state without a reader" worry does not survive that precedent: the reader is the client, and
the fourth layer of the shipped preserve-on-omit `meta` pattern is a typed lb field.

So lb gains:

- `ExportProfile { id, name, options }` (`rust/crates/host/src/report/profile.rs`), reusing
  `ExportOptions` verbatim — a profile IS a named set of the options the export already takes, so
  there stays exactly ONE option vocabulary.
- `Dashboard.export_profiles: Vec<ExportProfile>` (`exportProfiles` on the wire), riding the
  `reportIds` contract exactly: **absent preserves, `[]` clears**, empty stays off the wire.

What did NOT change: **lb still never reads a profile at export time.** `report.export` takes no
profile id; the client picks a profile and sends that profile's `options` on the call. The field
is storage and serde — the export path is untouched.

## Intent / approach

**A break marker, not a page number.** The obvious model — `Cell.page: u32` — is wrong, and
worth saying why in one place. Page numbers are absolute and the board is relative: insert a
panel on page 1 and every later cell's `page` is stale, so the record needs a renumbering pass
on every edit, and two cells claiming the same page with incompatible rows have no defined
meaning. A boolean `pageBreakBefore` on the band that starts a page is *local* — it says
"break here", survives dragging, reordering and insertion untouched, and composes with the
existing fit rule instead of replacing it. It is also exactly the model the retired notebook
`Block` used (`pageBreak`), so it is the shape this codebase already proved.

**The rule, extended by one clause.** `paginate` keeps its current sentence — *a band lands on
the page the author put it on, unless it would not fit, in which case it flows whole onto the
next* — and gains: *…and a band marked `pageBreakBefore` always starts a new page.* The marker
is checked before the fit test, and it moves a band **forward only**, never backward, which
preserves the existing "deliberate blank pages are kept" property by construction.

**Geometry as a value.** The six `A4_*` consts become `PageGeometry { w_mm, h_mm, margin_x_mm,
margin_top_mm, margin_bottom_mm, scale }` with `PageGeometry::a4_portrait()` as the default.
`cell_rect_mm_on_page` and `a4_rows_per_page` take it as an argument; `pdf.rs` emits its
`#set page` from it. This is mechanical but touches the most contract-heavy file in the crate,
so it lands as its own commit with the round-trip test green before and after.

**Why not leave paper size out.** It was tempting to ship only the break marker and keep A4
hardcoded. But `ExportOptions` has to exist anyway for the render toggles, and adding `paper`
later would mean re-opening the same six files and the same cross-repo contract test a second
time. One geometry change, done once, with the test extended to cover the new axis.

## The one non-obvious implementation trap

`paginate` does not see `Cell`. `compose_pages` lays out over `Placed` (from
`report/rendered.rs`), which is built from the **client's rendered panels** when they carry
geometry — and a `RenderedPanel` has no `pageBreakBefore`, because the marker is a property of
the *record*, not of the photograph. So `Placed` gains the flag and `panels_to_place` resolves
it from the cells **by id**, exactly as `title_for` already resolves a title:

- an exact `cell.i` match carries its marker;
- a **repeat clone** (`{source}-clone-{n}`) is in no record, so it inherits the marker of the
  cell it was cloned from — the same fallback `title_for` uses, and for the same reason;
- the record-driven fallback path (an older client with no rendered geometry) reads the marker
  straight off the cell.

Missing this yields a feature that works in unit tests over `cells` and silently does nothing
in a real export, which is the failure mode worth spending four lines to prevent.

## How it fits the core

- **Tenancy / isolation:** unchanged. `report_export` reads through `dashboard_get` under the
  caller's principal; the workspace wall and the report-kind refusal are untouched. Options
  are per-call input, never stored, so they cross no tenancy boundary.
- **Capabilities:** unchanged, deliberately. `mcp:report.export:call` still gates export, and
  `pageBreakBefore` rides `dashboard.save`'s existing gate — authoring a page break is
  authoring a dashboard. **No new cap.** Adding one would be the wrong shape: a principal who
  may save a report may lay it out. The deny path is the shipped one — no cap ⇒ opaque
  `ReportError::Denied` before the record is read.
- **Placement:** either. Pure computation over values already in hand.
- **MCP surface:** shape unchanged (CRUD-adjacent single call). `report.export`'s input schema
  gains an optional `options` object; `dashboard.save`'s `cells[]` gains an optional boolean.
  Both are additive schema fields — an older client omits them and gets today's behaviour.
- **Data (SurrealDB):** one new typed field on the `Cell` struct. It **must** be typed: the
  `Dashboard` model drops unknown top-level keys, so an untyped field would round-trip to
  nothing on the first save. It follows the shipped preserve-on-omit pattern
  (`serde(default, deserialize_with = "null_default", rename = "pageBreakBefore")`) exactly as
  `min_w`/`v`/`title` do. No migration: absent ⇒ `false` ⇒ today's pagination.
- **Bus (Zenoh):** none.
- **Sync / authority:** none — no new state.
- **Secrets:** none.

## Example flow

1. An admin lays out a report: a markdown cover and three KPI tiles on rows 0–8, a trend chart
   at row 9, a water chart at row 16, a table at row 24.
2. They want the charts to start a fresh page. In the builder they mark the trend chart's band
   "start a new page"; the client sets `pageBreakBefore: true` on that cell and calls
   `dashboard.save`. lb stores it on the `Cell` and preserves it on every later save.
3. A member opens the report and clicks Export. The client captures the panels and POSTs
   `{ snapshots: [...], options: { paper: "a4", orientation: "portrait", pageNumbers: true,
   index: false } }`.
4. `report_export` authorizes `report.export`, reads the report, resolves the workspace brand,
   and calls `compose_pages(&cells, &panels, &geometry)`.
5. `paginate` walks the bands. Row 9 carries the marker, so it starts page 1 even though it
   would have fitted on page 0. Rows 16 and 24 follow the ordinary fit rule from there.
6. `cell_rect_mm_on_page` places each cell against its own page's origin, using the geometry
   from the options.
7. `render_pdf` emits `#set page` from that same geometry and, because `options.page_numbers`
   is now actually set, a numbered footer.
8. The member gets a PDF whose page 1 holds the cover and the tiles and whose page 2 begins
   with the trend chart — the layout the admin authored, not the one the row arithmetic
   happened to derive.

## Testing plan

Mandatory categories: **capability-deny** applies (the shipped `report.export` deny test must
still pass unchanged, proving no new gate slipped in). Workspace-isolation applies via the
existing export tests. Offline/sync and hot-reload are N/A — no new state, no new surface.

Real infra: these are pure-computation crates; `lb-render` tests compile actual Typst and
assert on real PDF bytes, and the host tests run the real store. No mocks (rule 9).

- **`paginate`**
  - A marked band starts a new page even when it would have fitted.
  - A marked band already at the top of a page does **not** emit a gratuitous blank page.
  - The marker moves a band forward only — an existing deliberate blank page is preserved.
  - Marker + a band too tall for any page: starts its page and is clamped, as today.
  - Every shipped `paginate` test passes **unchanged** with the marker absent.
- **`geometry`**
  - The round-trip test is extended to a table of (paper, orientation) → derived values, and
    the A4-portrait row is asserted **byte-equal to today's numbers** (166/251 mm, 627/949 px,
    28 rows/page at scale 2).
  - Aspect-ratio and uniform-reduction properties hold for every supported geometry, not just
    A4 — the existing property tests parameterised.
- **`compose_pages`** — a marked cell lands on its own page with its rect measured from that
  page's origin (the regression that made a flowed panel draw at the old page's offset).
- **`panels_to_place`** (the trap above) — a marker set on a cell is honoured when the layout
  comes from **client-rendered panels**, not only from the record; and a repeat clone inherits
  its source cell's marker. Without these two the feature passes its own unit tests and does
  nothing in a real export.
- **`report_export`**
  - Options absent ⇒ **byte-identical PDF** to the current implementation. This is the
    backward-compatibility guarantee and it is a byte assertion, not a smoke test.
  - `pageNumbers: true` ⇒ the rendered PDF differs from the same export without it (the
    `RenderOptions` plumbing is actually connected).
  - A non-A4 paper produces a PDF whose page box is that paper.
  - An invalid `paper`/`orientation` is a 400 with a named field, not a silent fallback.
- **Both doors** — the existing "route and MCP arm compose byte-identical PDFs" test is
  extended to pass the same `ExportOptions` through both.
- **E2E** — covered by the rubix-ai scope's export runbook against a live node.

## Risks & hard problems

- **The geometry refactor is the risky part, not the feature.** Six constants become a struct
  threaded through the two files that define the cross-repo contract. The mitigation is
  ordering: land the parameterisation with A4 as the only value and the round-trip test green
  (a pure refactor, zero behaviour change), *then* add the second paper size.
- **`rows_per_page` stops being a constant.** It currently derives from A4 + scale and is
  mirrored as a literal `28` on the client. Once geometry is a parameter it is a *function of
  the export options*, which means the client cannot know it from a constant — it has to
  compute from the same profile. Named here because it is the seam where screen and print can
  silently diverge; the rubix-ai scope carries the matching obligation.
- **Scale interacts with paper.** `REPORT_PRINT_SCALE = 2` exists because a 627 px column
  makes panels re-flow. On a wider paper the same argument gives a different sensible scale.
  Keeping `scale` an explicit field rather than deriving it avoids guessing, at the cost of
  letting a caller pick a bad combination — an honest trade, and the preview makes a bad one
  visible immediately.
- **`fit: "contain"` letterboxes.** A capture whose aspect does not match its slot gets
  whitespace — the "a stat panel looks bad on its own page" complaint. This scope does not fix
  it (the fix is authoring: a shorter band), but forcing a break makes it *easier* to hit, so
  it is worth naming. The rubix-ai scope addresses it in the builder.

## Open questions

None — the two that mattered are decided above and stated as decisions:
**break markers, not page numbers** (Intent, first paragraph), and **paper size is in scope**
(Intent, "Why not leave paper size out"). Everything else is mechanical.

## Related

- [`report-builder-scope.md`](./report-builder-scope.md) — the shipped builder + Typst
  exporter this extends; its "SECOND DOOR" box is the MCP arm `ExportOptions` must also reach.
- rubix-ai `docs/scope/frontend/reports/report-builder-ux-scope.md` — the paired consumer
  scope (builder affordances, export dialog, PDF preview). Bumps the pin this scope tags.
- rubix-ai `docs/scope/frontend/reports/report-as-dashboard-scope.md` — why a report is a
  dashboard, and the `kind`/`reportIds` preserve-on-omit pattern `pageBreakBefore` copies.
- `rust/crates/render/src/paginate.rs`, `geometry.rs`, `place.rs`, `pdf.rs` — the four files
  this touches in the render crate.
- `rust/crates/host/src/report/export.rs`, `compose.rs` — the host half.
