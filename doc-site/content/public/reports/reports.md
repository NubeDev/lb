# Reports

A **notebook-style report builder**: a workspace asset made of **ordered blocks** — markdown text,
images, and the workspace's existing dashboard widgets/panels — authored in a true-to-print editor,
branded with a reusable **brand profile**, and exported as a **branded PDF**. Built on the platform's
shipped seams: the same authoring UX and Typst PDF pipeline as the previous `lazybones` project, but
the content blocks now include **live panels** rendered through the one shipped widget path.

The ask + full design: `docs/scope/reports/report-builder-scope.md`. Build detail:
`HANDOVER-reports.md`.

## What it is

A **`report:{ws}:{id}` asset** — modeled on `dashboard`/`panel` (stable slug, `owner`,
`visibility (private | team | workspace)`, share via the S4 `share` edge, soft-delete,
`schema_version`) holding an **ordered `blocks[]` array** (the notebook). Whole-record last-writer-wins
save like `dashboard.save`; the save participates in the undo journal for free.

**Three block kinds, one envelope each:**

- **`markdown`** — a body string (headings, tables, lists, code — GFM) plus a `page_break` toggle.
- **`image`** — an `asset_id` into the shipped `assets.*` store, with caption/width options.
- **`panel`** — exactly the shipped Cell duality: either `panel_ref: "panel:{id}"` (a library panel —
  edit once, every report updates) or an inline `PanelSpec` ("save this chart into the report only").
  Rendered through the **same** `WidgetHost` path as dashboards — no parallel renderer, and extension
  widgets work with zero report-side code.

**The editor** is a toolbar + Write/Preview markdown editor (a plain textarea with formatting buttons
+ a live GFM preview), ported from lazybones' shipped editor — it reads correctly inside the report
block card. The A4 print geometry (ISO A4, 20 mm margins — the same dimensions the Typst template
uses) is preserved on the *preview* sheet (`ReportView`), so on-screen matches print. Move-up/down
block reorder (keyboard-accessible buttons), insert-image from the asset store, and a live preview
that IS the report — switching the brand re-styles it immediately.

**Brand profiles** — a `brand:{ws}:{id}` record (name, logo, colors `primary/accent/text/background`,
fonts `heading/body`, header/footer text) with `brand.list|get|save|delete` and a reusable
`BrandPicker`. Many profiles per workspace; a report stores a `brand_id`. The brand editor's font
control is a **select of the embeddable fonts only** (Libertinus Serif, DejaVu Sans Mono, New Computer
Modern) — unknown brand fonts silently fall back in the PDF, so the editor makes that impossible to
trip over.

## Branded PDF export

A pure `lb-render` crate (Typst `=0.15.0` stack, custom `RenderWorld`, embedded `typst-assets` fonts —
no external binary, works offline, symmetric on every node): cover page, running header/footer,
optional page numbers + table of contents, brand colors/fonts throughout. The markdown→Typst converter
emits every text run as a `#"…"` string literal (the gotcha that trips naive converters) and builds
structure via Typst function forms.

**Panels export as client snapshots.** The browser — which is already rendering every widget live under
the *viewer's* caps — captures each panel block to a PNG at export time and sends the snapshots with the
export request. The server **never fetches widget data for export** — the PDF can only ever contain what
the exporting user could see on screen. Export is bounded-synchronous
(`POST /reports/{id}/export.pdf`, binary response), gated by its own `mcp:report.export:call` cap so an
admin can grant view-but-not-export.

## How it fits the core

- **Tenancy / isolation:** `report` and `brand` records live in the workspace namespace like every
  asset; ws B can never read/list ws A's reports or brands.
- **Capabilities:** `mcp:report.<verb>:call` and `mcp:brand.<verb>:call` per verb; the deny is opaque.
  Sharing a report shares its **definition**; every embedded panel's data is re-checked against the
  *viewer's* caps at render — a report is a lens, never a grant path. The PDF embeds data **as pixels**
  under the *exporter's* caps.
- **Symmetric nodes:** Typst compiles in-process on any node; no cloud branch, no external binary.
- **State vs motion:** reports/brands are pure state; nothing here touches the bus.
- **Stateless extensions:** extension widgets appear inside reports only as opaque `view` ids through
  the shipped envelope — no report code branches on an extension id.

## MCP surface

- CRUD + get/list: `report.get|list|save|delete|share` and `brand.get|list|save|delete`.
- `report.export` — bounded-synchronous (snapshots supplied in the call). If scheduled/batch export
  lands later it becomes an `lb-jobs` job — seam named, not built.
- `report.usage` is deferred (nothing references a report yet — delete is a plain soft-delete in v1).

## Non-goals (v1)

GitHub publishing, sources/research/RAG, scheduled/emailed report runs (needs a server-side widget
snapshot answer — deferred), server-side HTML preview (the live React preview IS the report), live/
self-updating PDFs, PDF forms, page-level collaborative editing, and cross-workspace reports.

## Built-in role freshness (a load-bearing fix shipped with reports)

Adding the `report.*`/`brand.*` caps to the built-in `AUTHOR_CAPS` bundle surfaced a pre-existing
footgun: a workspace seeded *before* a new built-in cap was added keeps the stale `member`/
`workspace-admin` role rows forever (the seed is idempotent — writes only when absent), and
`resolve_caps` read that stored record. The durable fix (not a re-seed): the resolver now UNIONS the
live built-in cap bundle on top of the stored record for granted built-in roles, so a new built-in cap
takes effect the moment code ships. Scope: `docs/scope/auth-caps/builtin-role-freshness-scope.md`;
debug: `docs/debugging/auth/builtin-role-row-frozen-stale-on-new-caps.md`.
