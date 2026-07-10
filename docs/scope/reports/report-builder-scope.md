# Reports scope — the report builder + branded PDF exporter

Status: scope (the ask). Promotes to `doc-site/content/public/reports/reports.md` once shipped.

A **notebook-style report builder**: a workspace asset made of **ordered blocks** — markdown
text, images, and **the workspace's existing dashboard widgets/panels** — authored in a
true-to-print editor, branded with a reusable **brand profile** (logo, colors, fonts,
header/footer), and exported as a **branded PDF**. This ports the proven document-writer from
the previous `lazybones` project (`/home/user/code/rust/lazybones`, `docs/doc-writer/`) onto
this platform's shipped seams: the same authoring UX and Typst PDF pipeline, but the content
blocks now include **live panels** rendered through the one shipped widget path. The
lazybones code is the reference implementation — **copy what shipped there** (reviewed
against its code, not its docs — the two disagree; see "Lessons imported"), don't re-derive it.

> Read with: `../frontend/dashboard/library-panels-scope.md` (the shipped `panel:{id}` asset +
> `panel_ref` + standalone render — the embed primitive), `../widgets/widget-platform-scope.md`
> (the one widget envelope), `../document-store/document-store-scope.md` (the markdown/asset
> substrate — sibling, see "Boundary with document-store"), `../frontend/workspace-branding-scope.md`
> (workspace identity ≠ report brand profiles), README §6.5 (capabilities), §6.12 (assets), §7
> (workspace = tenant).

---

## Goals

- **A `report:{ws}:{id}` asset** — modeled on `dashboard`/`panel` (stable slug, `owner`,
  `visibility (private | team | workspace)`, share via the S4 `share` edge, soft-delete,
  `schema_version`) holding an **ordered `blocks[]` array** (the notebook). LWW whole-record
  save like `dashboard.save`; the save participates in the undo journal for free.
- **Three block kinds, one envelope each**:
  - `markdown` — a body string (headings, tables, lists, code — GFM), plus a `page_break`
    toggle (the lazybones page semantics).
  - `image` — an `asset_id` into the shipped `assets.*` store (README §6.12), with
    caption/width options.
  - `panel` — **exactly the shipped Cell duality**: either `panel_ref: "panel:{id}"` (a
    library panel — edit once, every report updates) or an inline `PanelSpec` ("save this
    chart into the report only"). Rendered through the **same** `WidgetHost` path as
    dashboards — no parallel renderer, and extension widgets (`view:"ext:<id>/…"`) work with
    zero report-side code (rule 10: the id stays opaque data).
- **Full CRUD over MCP** — `report.get|list|save|delete|share`, each its own file, its own
  capability, workspace-first — the exact `panel/` module pattern
  (`rust/crates/host/src/panel/tool.rs`). (`report.usage` deferred — decision 4.)
- **Brand profiles** — a `brand:{ws}:{id}` record (name, logo, colors
  `primary/accent/text/background`, fonts `heading/body`, header/footer text) with
  `brand.list|get|save|delete` and a **reusable `BrandPicker`** component. Many profiles per
  workspace; a report stores a `brand_id`. The lazybones lesson holds: branding is a
  **standalone, cross-cutting** resource — the report builder is merely its first consumer
  (the login/workspace `ui_branding` blob stays what it is: workspace *identity*, not a
  document brand; the default `brand` seed derives its initial logo/name from it).
- **Branded PDF export** — a pure `lb-render` crate ported from `lazybones-render` (Typst
  `=0.15.0` stack, custom `RenderWorld`, embedded `typst-assets` fonts — no external binary,
  works offline, symmetric on every node). Cover page, running header/footer, optional page
  numbers + table of contents, brand colors/fonts throughout — the exact `pdf.rs` template.
- **Panels export as client snapshots** — the browser (which is already rendering every
  widget live, under the *viewer's* caps) captures each panel block to a PNG at export time
  and sends the snapshots with the export request; the node composes markdown + images +
  snapshots into the PDF. The server **never fetches widget data for export** — the PDF can
  only ever contain what the exporting user could see on screen. (Alternative rejected below.)
- **The lazybones editing UX, kept** — this is the headline requirement:
  - a **true-A4 WYSIWYG markdown editor** (TipTap + `tiptap-markdown` round-trip) sized to
    the real print geometry (210×297 mm, the same margins the Typst template uses) with an
    overflow indicator — on-screen matches print;
  - **drag-to-reorder blocks** (the `page-organizer` pattern);
  - **live preview that IS the report** — blocks render live in the editor (widgets are real,
    through `WidgetHost`), on A4 "sheets on a grey desk" print-fidelity styling; switching
    the brand re-styles the preview immediately;
  - insert-image from a small **asset library** panel (upload → `assets.put` → insert).

## Non-goals (v1)

- **GitHub publishing** (lazybones had branch/commit/PR). No `lb-gh` consumer here yet;
  deferred until `git-sync/` lands its `git.*` verbs — then it's an outbox effect, not a
  report feature.
- **Sources / research uploads / RAG** (lazybones `source/` + `extracted_text`). Different
  concern; this platform's substrate for it is `document-store/` + `tags/`. Deferred.
- **Scheduled / emailed report runs** — the obvious follow-on (a `reminder` fires a job that
  exports and delivers via the outbox), but it needs a *server-side* widget snapshot answer
  (see Risks) — explicitly deferred, seam named.
- **Server-side HTML preview** (lazybones `render_html`). Not needed here: our live preview
  is the real React block renderer — strictly better than an iframe of server HTML. The Typst
  PDF is the only server render target, which halves the "two implementations of one design"
  maintenance burden styling.md warned about.
- **Live/self-updating PDFs, PDF forms, page-level collaborative editing** — a PDF is a
  snapshot; the report record is save-button + undo like every asset.
- **Cross-workspace reports** — the wall holds (rule 6).

## Intent / approach

**One new asset family + one pure render crate + one UI feature; everything else is reuse.**

The report is a first-class host asset exactly like `dashboard` and `panel` — not a
document-store `Doc`, and not an extension. The load-bearing observation from the reuse
survey: **the embed primitive already exists end-to-end.** `PanelPage.tsx` renders one
library panel outside any grid (`DashboardCacheProvider` → `getPanel` → `specToCell` →
`WidgetHost`); a report's panel block is that same composition repeated per block. The
builder therefore writes **no new data path**: panel data flows through `viz.query` under
the viewer's caps per call, library refs hydrate host-side at `report.get` exactly like
`panel_ref` cells do at `dashboard.get` (reuse `panel::hydrate` / `validate_and_strip_refs`).

The PDF pipeline is a **port, not a rewrite**, of `lazybones-render` — the crate is pure
(strings + bytes in, PDF bytes out, no store dep), which is why it ports cleanly:
`convert.rs` (the pulldown-cmark→Typst converter with its two robustness rules: every text
run as a `#"…"` string literal; structure via Typst function forms), `pdf.rs`
(`build_template()`: palette derivation, cover, running header/footer, dotted-leader TOC,
zebra tables, code panels), `world.rs` (custom `RenderWorld`, `typst-assets` fonts, virtual
image files), `model.rs` (`Assembled`/`Brand`/`ImageAsset` inputs). Assembly (resolve blocks,
brand, logo bytes, snapshots) lives host-side next to the export verb, keeping the crate pure
— the same split lazybones proved.

**Alternatives rejected:**

- *Server-side widget rendering for export* (headless browser, or re-implementing charts in
  Rust): a headless browser is a huge, asymmetric dependency (breaks rule 1's spirit on an
  edge node); a Rust chart re-implementation is a **parallel renderer** — the exact thing the
  library-panels scope forbade — and would drift from ECharts/extension widgets forever.
  Client snapshots cost one `getDataURL`/`html-to-image` call per block and inherit the
  viewer-caps story for free.
- *Browser print-to-PDF instead of Typst*: no cover/header/footer/TOC control, output varies
  by browser, no branding fidelity — it's the debugging fallback, not the product. (A
  print-fidelity CSS view is still worth shipping as the preview skin.)
- *Reports as document-store `Doc` records*: a report is not one markdown body — it's typed,
  ordered blocks referencing panels; forcing it through an opaque `content` string loses
  hydration, validation, and `usage`. The document-store stays the *prose* substrate.
- *Blocks as separate records with fractional positions* (lazybones `page` rows): lazybones
  needed per-page saves; here the shipped precedent is `dashboard.cells[]` — whole-record
  LWW + undo. An ordered array in one record is simpler and reorder is free. Per-block
  records return only if co-editing ever demands them.

**Boundary with document-store & the reference-extensions plan:**
`document-store/document-store-scope.md` owns reusable *markdown documents* (link graph,
doc→asset embeds); this scope owns the *composed, branded, panel-bearing, exportable report*.
The `extensions/reference-extensions-scope.md` "markdown doc-store + PDF" reference extension
stays valid as an extension-tier demo; once `lb-render` exists it should consume that crate
rather than grow a second Typst stack (noted there as a follow-up, not a dependency).

## Lessons imported from lazybones (code-reviewed, not doc-assumed)

The lazybones docs and code **disagree**; the port follows the code:

1. Content shipped as **ordered pages with `page_break` toggles**, not the single
   `Document.body` the docs describe → our `blocks[]` + per-markdown-block `page_break`.
2. The "no editor lib, plain textarea" doc decision was reversed in practice — **TipTap on a
   true-A4 sheet is what shipped and what made the UX good**. We adopt TipTap deliberately,
   up front.
3. `render_pdf` returns `Result<Vec<u8>, RenderError>`; `typst-as-lib` was **not** used — a
   custom `World` was; `typst-layout` is a direct dep (for `PagedDocument`). Pin the working
   set: `typst = =0.15.0`, `typst-pdf = =0.15.0`, `typst-assets = =0.15.0` (`fonts`),
   `typst-layout = =0.15.0`, `comemo = =0.5.1`, `pulldown-cmark = 0.13`.
4. The Typst **string-vs-content gotcha** (author text must be emitted as `#`-string literals
   in content position) and the **embedded-font constraint** (only `typst-assets` fonts —
   Libertinus Serif, DejaVu Sans Mono, New Computer Modern — render in the PDF; unknown brand
   fonts silently fall back) carry over verbatim. Surface the font list in the brand editor
   instead of a free-text font field.
5. `pdf-extract` panics on malformed PDFs (needed `catch_unwind`) — not imported (sources are
   out of scope), noted for whoever builds them later.
6. The de-risk ordering worked: **a Phase-3a spike proving `typst::compile` → PDF bytes with
   an embedded font gates the rest of the render work.** Keep it.

## How it fits the core

- **Tenancy / isolation:** `report` and `brand` records live in the workspace namespace like
  every asset; ws B can never read/list ws A's reports — the mandatory isolation test.
- **Capabilities:** `mcp:report.<verb>:call` and `mcp:brand.<verb>:call` per verb; the deny
  is opaque (`ToolError::Denied`) like `panel.*`. Sharing a report shares its **definition**;
  every embedded panel's data is re-checked against the *viewer's* caps at render (`viz.query`
  leash) — a report is a lens, never a grant path. The PDF is different by nature: it embeds
  data **as pixels** under the *exporter's* caps — exporting is the moment access is
  exercised, and the file is thereafter out-of-band (same as any screenshot). Gate export
  with its own `mcp:report.export:call` so an admin can grant view-but-not-export.
- **Symmetric nodes:** Typst compiles in-process on any node; no cloud branch, no external
  binary, works offline.
- **Data (SurrealDB):** two new tables, `report` and `brand`, typed nested objects (no
  app-side JSON parsing), serde-defaulted additive fields — the closed-struct discipline.
  Images ride the **shipped** `assets.*` store; logos small enough may inline as data-URIs
  (the branding-blob pattern, 256 KiB cap). No new persistence layer, no blob service.
- **State vs motion:** reports/brands are pure state; nothing here touches the bus. N/A.
- **Stateless extensions:** N/A — this is host-native (like `dashboard/`), not an extension;
  extension **widgets** appear inside reports only as opaque `view` ids through the shipped
  envelope (rule 10 holds: no report code branches on an extension id).
- **MCP surface (API shape, §6.1):**
  - CRUD + get/list: `report.get|list|save|delete|share` and `brand.get|list|save|delete`
    (`report.usage` deferred until something embeds a report — decision 4).
  - `report.export` — **bounded-synchronous**: one report, snapshots supplied in the call,
    one Typst compile (sub-second for tens of pages in lazybones). The gateway carries it as
    `POST /reports/{id}/export.pdf` (binary response + snapshot payload don't fit the JSON
    MCP envelope; the verb still authorizes through the one chokepoint). If scheduled/batch
    export lands later it becomes an `lb-jobs` job per §6.1 — seam named, not built.
  - Live feed: N/A — reports are save-button assets; the editor needs no watch verb.
- **Durability / outbox:** N/A in v1 (no must-deliver effect); the deferred email/schedule
  slice will stage delivery through the outbox.
- **One responsibility per file:** `rust/crates/host/src/report/` and `.../brand/` mirror
  `panel/` (model / verb-per-file / tool.rs / hydrate / validate); `lb-render` keeps the
  lazybones file split (`convert.rs`, `pdf.rs`, `world.rs`, `model.rs`, `error.rs`).
- **SDK/WIT impact:** none — no ABI change; extension widgets are reached through the
  existing render path only.
- **Skill doc:** yes — this is an agent-drivable surface (an agent can author a report over
  `report.save` from existing panels). The implementing session writes
  `docs/skills/reports/SKILL.md` grounded in a live run.

## The pieces (build map)

**Rust**
- `rust/crates/render/` (`lb-render`, new, pure) — port of `lazybones-render` minus
  `html.rs`; input `Assembled { title, sections, brand, logo, images, options }` where a
  section is markdown **or** a pre-rendered snapshot image.
- `rust/crates/host/src/report/` — `model.rs` (`Report`, `Block { kind, body?, asset_id?,
  panel_ref?, spec?, page_break, options }`), verb files, `tool.rs`, hydrate/validate reusing
  `panel::hydrate`; `export.rs` (assembly: blocks + brand + `assets.get` bytes + supplied
  snapshots → `lb_render::render_pdf`).
- `rust/crates/host/src/brand/` — `model.rs` (`Brand`, colors/fonts structs), verb files,
  `tool.rs`, `seed.rs` (one neutral default, initialised from `ui_branding` when present —
  pickers never empty).
- Catalog entries in `host/src/system/catalog.rs`, routing arms in `tool_call.rs`, gateway
  routes (`rust/role/gateway/src/routes/report.rs` + `brand.rs` + a new `assets.rs` binary
  arm) registered in `rust/role/gateway/src/server.rs`'s `router()`, and the `http.ts` IPC
  mirrors (the known 4-mirror gotcha for every new save field). Each handler runs
  `authenticate(&gw, &headers)` → host verb (`p.ws()` supplies the workspace, never the body).

**Two confirmed gaps a build session MUST close (verified against this repo, not assumed):**
- **Gateway body limit — none exists today.** `rust/role/gateway/src/server.rs` has no
  `DefaultBodyLimit`/`RequestBodyLimitLayer`, so axum's **default 2 MB** applies to every
  extractor and will **reject a multi-MB snapshot POST**. Add `DefaultBodyLimit::max(N)`
  (per the export route, e.g. 32 MB) in `server.rs`. This is on the critical path for export.
- **Binary-asset wiring is greenfield.** The store + host verb are done and gated:
  `lb_host::put_asset(store, principal, ws, id, mime, bytes, ts) -> Result<Asset, AssetError>`
  enforces `store:asset/{id}:write` + owner-forced + an **8 MiB** `MAX_ASSET_BYTES` cap (bytes
  are base64-transparent record values — `rust/crates/assets/src/asset/model.rs`). But there is
  **no `/assets` binary gateway route, no `assets_put_asset` http.ts case, and no image-upload
  UI client** — all three are CREATE. (The existing `assets.rs` route only serves docs/skills.)
  Image blocks and brand logos both ride this once wired; keep individual images ≤8 MiB.
- **Export returns bytes, not JSON** — `http.ts`'s `getJson`/`postJson` assume JSON; add one
  `postBytes`/blob helper. The raw-bytes response pattern to copy is
  `rust/role/gateway/src/routes/ext_ui.rs` (tuple `(StatusCode, [(CONTENT_TYPE, …)], Vec<u8>)`),
  but the export handler MUST add `authenticate` (ext_ui is unauthenticated).

**Sidebar / nav access (CREATE — the 7 edits, all verified):**
1. `ui/src/features/shell/NavRail.tsx` — add `| "reports"` to the `CoreSurface` union
   (and place it in `SURFACE_GROUPS`).
2. `ui/src/features/shell/surfaceDefs.ts` — add `reports: { key:"reports", icon:<Lucide>, label:"Reports" }` to `SURFACE_DEF` (import the icon, e.g. `FileText`).
3. `ui/src/features/routing/surface.ts` — add `reports: "/reports"` to `CORE_PATHS`.
4. `ui/src/features/routing/allowed.ts` — push `"reports"` when the reach cap holds; add the
   `CAP` constant in `@/lib/session`. Fallback nav (`reach:*:view`) shows it for everyone by
   default; a curated nav needs the server to mint `reach:reports:view`.
5. `ui/src/features/routing/createAppRouter.tsx` — import the page + add
   `coreRoute("/reports", "reports", () => <ReportsView/>)` to `routeTree` (wraps in `CoreGate`).
6. Create the page under `ui/src/features/reports/`.
7. Server: mint `reach:reports:view` in the nav/reach fold so curated-nav users reach it.

**UI** (`ui/src/features/reports/` + shared pieces broken out for reuse)
- `ReportsPage` (roster, the dashboard-list pattern), `ReportEditor` (block list: add
  markdown/panel/image, drag reorder, per-block controls), `ReportView` (read/print-fidelity
  A4 skin — also the preview), `ExportButton` (snapshot pass → POST → download).
- Panel block = `DashboardCacheProvider` + `specToCell` + `WidgetHost` (the `PanelPage`
  composition, extracted into a reusable `PanelEmbed` so `PanelPage`, reports, and future
  channel embeds share one file).
- **Shared components broken out (not report-private):** `components/markdown-editor/`
  (the TipTap A4 page editor — first consumer here, document-store's editor is the named
  second), `components/brand-picker/` (BrandPicker + swatch), the existing
  `branding-assets.ts` File→data-URI helper reused for logo upload. Snapshot capture as
  `lib/snapshot/` (ECharts `getDataURL` fast path, `html-to-image` fallback) — usable later
  by "copy widget as image" anywhere.
- New deps to flag: `@tiptap/*` + `tiptap-markdown`, `html-to-image` (key-stack rows added).

## Example flow

1. Maria opens **Reports → New report**, names it "Q3 Site Energy", picks the "Nube IO"
   brand profile from the BrandPicker (seeded default; she created the profile once under
   Settings → Brands with the company logo + colors).
2. She adds a **markdown block** and writes the executive summary in the A4 WYSIWYG editor —
   the sheet on screen has the exact margins the PDF will have.
3. She adds a **panel block**, picks "Site kWh (monthly)" from the library-panel list
   (`panel.list` — the same chart already on two dashboards), sets the report's time range;
   the live chart renders in place through `WidgetHost` under **her** caps.
4. She adds an **image block**, uploads a site photo (→ `assets.put`), drags the block above
   the chart, toggles `page_break` on the summary block.
5. **Save** → one `report.save` (undo-journaled). A teammate with `report.get` but without
   the panel's datasource caps opens it and sees the prose + a denied panel placeholder —
   the lens never widened access.
6. **Export PDF** → the browser snapshots the panel block to PNG, POSTs
   `/reports/q3-site-energy/export.pdf` with the snapshot; the node assembles blocks + brand
   + logo and Typst-compiles; the download has the cover page, running header/footer, page
   numbers, TOC, brand colors — and the chart exactly as she saw it.

## Testing plan

Per `../testing/testing-scope.md` — real store, real gateway, no fakes:

- **Capability deny (mandatory):** `report.save` / `report.export` / `brand.save` each denied
  for a principal without the cap (per-verb, opaque deny); view-without-export proven.
- **Workspace isolation (mandatory):** ws B cannot `report.get|list` ws A's reports or
  brands; export route 404/denies cross-ws.
- **Rust unit/integration:** report CRUD round-trip (typed blocks survive), `panel_ref`
  hydration + dangling-ref validation (mirror the `panel/` tests), brand seed idempotent,
  undo restores a prior `blocks[]`; `lb-render`: the **spike test** (trivial `.typ` → PDF
  bytes, embedded font — gates the phase), converter cases ported from lazybones
  (`*#_$` text can't break markup, fenced code multiline, tables, task lists), color
  fallback on a bad hex, unknown-font fallback.
- **Gateway/UI (`pnpm test:gateway`, real node):** create → add three block kinds → save →
  re-read → blocks intact; panel block renders real `viz.query` data inside
  `DashboardCacheProvider` (the known provider gotcha); export POST returns
  `application/pdf` magic bytes with a snapshot payload; deny + isolation from the UI path.
  (Validate via the touched files — the full gateway suite has known unrelated reds.)
- **E2E (built shell, real node):** author → reorder → export → non-empty PDF downloaded.

## Risks & hard problems

- **markdown→Typst conversion** is still the bulk of render effort — mitigated by porting
  lazybones `convert.rs` + its tests wholesale rather than re-deriving.
- **Typst version drift**: the pins are what compiled in lazybones (Rust ≥1.93, edition
  2024); re-verify against this workspace's toolchain in the spike **before** any other
  render work. This is the only hard-block phase.
- **Snapshot fidelity**: `html-to-image` on foreign-object/canvas mixes can miss styles;
  ECharts' native `getDataURL` covers the common charts — the fallback needs a per-view test
  matrix. Extension widgets in sandboxed tiers may not be capturable → export renders a
  titled placeholder, honestly, rather than failing the export.
- **Export is client-coupled**: no browser session ⇒ no widget snapshots ⇒ scheduled exports
  can't reuse this path. That's the named deferral, not an accident — don't let the schedule
  slice quietly grow a headless browser without its own scope.
- **Big reports**: one record holding many blocks with inline images-as-data-URIs would
  bloat — images MUST be `asset_id` refs (only brand logos may inline, capped); state the
  block-count bound (soft cap ~200 blocks) and test a large save.
- **Font expectations**: brand fonts that aren't embedded silently fall back in the PDF —
  the brand editor lists only the embeddable fonts (lesson 4) to make this impossible to
  trip over, but imported brands may still carry unknown names.

## Decisions (no open questions — resolved for the build session)

All four prior open questions are **decided**; a build session implements these, it does not
re-litigate them. Each is the long-term-best call, grounded in the shipped precedents above.

1. **Report-level time range + per-block override.** The report carries one range/variable
   toolbar (the shipped dashboard `Toolbar`/`Variable` model, reused verbatim); a panel block
   may pin its own range in per-block options, which wins for that block. Matches the dashboard
   precedent, so `WidgetHost`'s `range`/`scope` props flow with zero new machinery. (Rejected:
   per-block-only — it makes a whole-report range change an N-block edit.)
2. **Snapshots ride the export POST body; raise the gateway body limit.** Snapshots go in the
   `report.export` POST body (simplest, no lifecycle to manage), and the build adds
   `DefaultBodyLimit::max(32 MB)` scoped to the export route in `server.rs` (see the gap note
   above — there is **no** limit today, default 2 MB would reject them). (Rejected: pre-upload
   snapshots as short-lived assets — adds a GC/cleanup burden for data that lives one request.)
3. **Header/footer are plain text with `{page}`/`{title}`/`{date}` tokens** — data, not a
   template language (rule: keep author input as data). The Typst layer substitutes tokens.
   The brand editor's font control is a **select of the embeddable fonts only** (Libertinus
   Serif, DejaVu Sans Mono, New Computer Modern — lesson 4), never a free-text font field.
4. **`report.usage` is deferred (not v1).** `panel.usage` exists only because dashboards
   reference panels; **nothing references a report yet**, so there is no usage to compute.
   `report.delete` is a plain soft-delete in v1. When a nav/channel first embeds a report,
   add `report.usage` then (seam named, mirrors `panel.usage`).

**Also decided in-scope so the build has no ambiguity:**
- **Storage shape:** blocks are an ordered `blocks[]` array in the one `report` record
  (dashboard `cells[]` precedent — whole-record LWW + free undo + free reorder), **not**
  per-block records.
- **Panel blocks reuse the shipped host functions directly:** `lb_host::hydrate_cells(store,
  principal, ws, cells)` at report get/export (expands `panel_ref`, degrades missing refs to a
  placeholder, never fails the read) and `validate_and_strip_refs(store, principal, ws, cells)`
  at `report.save` (rejects dangling refs, strips echoed spec). A panel block IS a `Cell`
  (`lb_host::Cell`); no new hydration code.
- **Images are `asset_id` refs into the shipped `assets.*` store**, ≤8 MiB each (the host
  cap); only brand **logos** may inline as data-URIs (the branding-blob pattern, 256 KiB cap).
- **Export is bounded-synchronous** (`POST /reports/{id}/export.pdf`, binary response). If the
  deferred scheduled/emailed slice ever lands, export becomes an `lb-jobs` job — seam named.

## Related

- `../frontend/dashboard/library-panels-scope.md` — the shipped panel asset this embeds.
- `../widgets/widget-platform-scope.md` — the one widget envelope + `WidgetHost`.
- `../document-store/document-store-scope.md` — the prose/asset substrate (sibling boundary).
- `../frontend/workspace-branding-scope.md` — workspace identity vs report brand profiles.
- `../extensions/reference-extensions-scope.md` — the "doc-store + PDF" reference ext should
  consume `lb-render` once it exists.
- `../../key-stack.md` — Typst / TipTap / html-to-image rows.
- Lazybones reference implementation: `/home/user/code/rust/lazybones` —
  `crates/lazybones-render/` (port source), `docs/doc-writer/{README,styling,ui-scope}.md`
  (read with the code-vs-docs corrections in "Lessons imported").
- Skill (written on ship): `docs/skills/reports/SKILL.md`.
