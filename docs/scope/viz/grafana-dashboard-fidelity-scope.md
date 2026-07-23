# Viz scope — Grafana dashboard fidelity (lossless import + a richer native model + a render that never drops), measured against the pdnsw IAQ page

Status: scope (the ask). Promotes to `doc-site/content/public/frontend/dashboard.md` once shipped.

Converting a **real** Grafana dashboard into our record is easy to *claim* and hard to *land*: the import
verb reports "33 panels mapped" while **zero of 35 tiles actually draw**. This scope is the **fidelity**
half of the dashboard campaign — the sibling of the **speed** half
([`../caching/dashboard-query-acceleration-scope.md`](../caching/dashboard-query-acceleration-scope.md),
27 s → 1.2 s). It is grounded, not aspirational: every gap below was **measured** by importing the live
NubeIO **"Indoor Air Quality and Environment Monitoring"** board (`rd-pdnsw.nube-iiot.com/d/yrhXbfWIk`,
schemaVersion **27**, 35 panels: 18 `stat`, 12 `graph`, 3 `row`, 1 `text`, 1 `dashlist`) against a running
node with the `pdnsw` Timescale datasource, then fixing it tile-by-tile until **24/24 data cells drew**.
The through-line: make `dashboard.import` **lossless** (every mappable panel comes out wired and drawing),
make the **native model rich enough** that there is something faithful to map *onto* (10×-ing what a
dashboard can express), and make the **renderer robust** so a converted tile never silently drops to a
broken fallback. The exit gate is a real Grafana export in → a page that draws, aligns, and reads like the
original — no "no template" boxes, no blank tiles, no axis-less charts.

> Read with: [`frontend/dashboard/viz/import-export-scope.md`](../frontend/dashboard/viz/import-export-scope.md)
> (the `dashboard.import`/`export` verbs + the bidirectional mapper this hardens), [`grafana-parity-backend-scope.md`](grafana-parity-backend-scope.md)
> (the additive model fields + the import pin this consumes), the **downstream UI half** in rubix-ai
> `docs/scope/frontend/dashboard/viz/grafana-parity-ui-scope.md` (the typed option shapes + renderers), and
> the **caching sibling** above (a fast open and a faithful open are the two halves of "a dashboard that
> never looks broken"). The empirical write-up this scope distills: rubix-ai
> `docs/testing/dashboard/grafana-conversion-review.md`.

---

## Owning repos (cross-repo — WORKFLOW-LB §2)

Like the caching scope, this lands in **two repos, two PRs**, independently shippable and gracefully
degrading (lb additive first, rubix-ai bumps the pin and lights up):

1. **`NubeDev/lb` (this repo) — the platform + upstream-UI half, the bulk.** The `dashboard.import` mapper
   hardening (wire `graph` targets, translate `$__*` macros, grid remap, drop placeholders, honest report),
   the additive native-model fields (a real `text`/`markdown` view, y-axis min/max/soft-clamp, panel/row
   `repeat`), and the **renderer robustness** fix in the vendored-upstream UI (`ui/src/features/dashboard/`).
   Released as a `node-v0.x.y` + `ui-v0.x.y` tag pair.
2. **`NubeIO/rubix-ai` — the consumer half.** Bumps the lb pin; carries the one already-shipped renderer fix
   as a **vendored divergence** (`ui/VENDOR.md`) until (1) merges it upstream; owns the conversion review +
   the E2E acceptance walk. Its stale standalone-tool scope
   (`docs/scope/frontend/dashboard/grafana-conversion-scope.md`) is superseded by this doc and re-pointed.

## Goals

- **`dashboard.import` is lossless: every mappable panel comes out wired and drawing.** The measured bug is
  that a `graph` panel maps to a cell with **`tool:""`** and its `rawSql` stranded in `args.rawSql` — which
  `viz.query` **silently skips** (blank tile), while the report still counts it "mapped." A mapped panel MUST
  emit an executable source (`tool:"federation.query"`, `args.{source,sql}`) or a **reported** degrade —
  never a mapped-but-unrunnable tile.
- **Grafana SQL macros are translated, not copied.** `$__time(col)` → `col`, `$__timeFilter(col)` → the host
  `col >= to_timestamp($__from/1000) AND col < to_timestamp($__to/1000 + 86400)` idiom (dashboard-time-range
  tokens), `$__timeGroup`/`$__interval` → their equivalents. Load-bearing, not cosmetic: an untranslated
  `$__timeFilter` leaves the query **unbounded**, and on a real `histories` table an unbounded scan **hit the
  30 s query bound and was cancelled** — a "mapped" chart that can never render.
- **The grid maps to the grid we actually ship.** Resolve the **doc-vs-reality drift**: import-export-scope
  says "both 24-col (the spine pins our grid to 24)"; the shipped UI is **`GRID_COLS = 12`** (`gridGeometry.ts`).
  Copying Grafana's 24-col `gridPos` verbatim onto a 12-col board overflows every tile (a `w=8 x=16` third-
  width panel lands off-grid and stretches full-width with huge gaps — the "looks like shit" alignment bug).
  Pick one grid and make the mapper honor it.
- **Placeholders are dropped, not carried as empty "no template" tiles.** Grafana's default `metric_table`
  banner-stats, an empty logo `<div>`, and `dashlist` have no data and no host equivalent; mapping them to a
  `template`/unsupported cell renders an ugly empty box. Drop-with-notice; the report names each.
- **The native model is rich enough to be a faithful target (the 10×).** Add the fields a real page needs and
  we lack: a first-class **`text`/`markdown` view** (the #1 structural gap — `text`/`dashlist` had nowhere to
  land), **y-axis min/max + soft-clamp** (real sensor-glitch outliers squashed the temperature axis; Grafana
  had no max either, but we can offer one), and **panel/row `repeat`** over multi-value variables.
- **The renderer never drops to a broken fallback.** A timeseries panel whose value column is *renamed*
  (`value AS "Temperature"`) fell through to the axis-less legacy sparkline (no left axis, corner readout,
  bottom legend) — measured on 3 of 12 charts. The auto-plot suggester must type columns **by value, not
  name** (matching `num.ts::rowNumber`), so any time-plus-numeric frame gets real axes. **Already fixed** in
  the rubix-ai vendored UI; this goal is to **land it upstream** so the whole family gets it.
- **Honest by construction.** Every degrade/drop is a **report line**; a dropped feature with no line is a
  test failure (the tool's honesty contract). A converted board is either drawing or visibly, namedly
  degraded — never silently blank.

## Non-goals

- **Not the export half.** Our JSON → Grafana JSON stays in import-export-scope Phase 4; this cut is
  Grafana → us fidelity only.
- **Not "repair the source's SQL bugs."** The pdnsw page shipped queries with their own defects (`ORDER BY
  time` where the column is `timestamp` → silently empty). The mapper **reports** an unexecutable query; it
  does **not** silently rewrite arbitrary user SQL (macro translation is the bounded exception — a
  documented dialect map, not a bug-fixer). A repaired-behind-your-back query is a worse surprise than a
  named degrade.
- **Not the Rubix-OS plugin-query synthesizer (this cut).** 18 of the page's `stat` panels use the Rubix-OS
  datasource **plugin** query (a `selectedPoint`/`selectedDevice` object tree, **no SQL**) — the mapper
  correctly binds + reports "no SQL query, renders empty." Synthesizing `SELECT value … WHERE
  point_uuid=…` from that object is a **named follow-up** (`grafana-parity-backend`'s datasource remap
  family), plugin-specific and out of the generic Grafana mapper. Named here so it's a decision, not an
  omission.
- **Not caching / speed.** The 10× on *open time* is the sibling scope; this is 10× on *what a dashboard can
  faithfully express*. They compose (a bounded, macro-translated query is exactly what the cache keys on).
- **Not `schemaVersion` migration rework.** The shipped importer already migrates 27 → current
  (`migratedFrom: 27`); the migration pin stays `grafana-parity-backend`'s. This scope assumes a normalized
  post-migration shape.
- **Not the v2 (`dashboard.grafana.app/v2beta1`) kind-based layout.** Classic flat `panels[]` only (rejected-
  with-notice on a v2 input — unchanged).

## Intent / approach

**Three composable slices, each the long-term shape. Slices 1 & 3 are the measured must-fixes and land
first; slice 2 is the additive model growth that makes the mapper's job faithful rather than lossy.**

### Slice 1 — Lossless mapper: wire every panel, translate every macro, honor the real grid

The mapper already migrates schema, binds datasources, and preserves rows/timezone. The gaps are all in the
**panel → cell** step, and each is small and mechanical:

- **Wire `graph`/`rawSql` targets.** Set `tool:"federation.query"`, move `rawSql` → `args.sql` under the
  mapped `args.source`. A `tool:""` cell is the silent-wiring failure `viz.query` skips — the single reason
  12 charts were blank. (One function in the mapper; the contract is "a mapped data target is executable or
  reported.")
- **Translate the Grafana SQL dialect.** A bounded macro map: `$__time(x)→x`, `$__timeFilter(x)→` the host
  `$__from`/`$__to` window (end-day-exclusive), `$__timeGroup`/`$__interval`/`$__timeFrom`/`$__timeTo` →
  their equivalents. Anything unrecognized is **left verbatim and reported** (`unsupported macro $__foo`),
  never silently dropped. This is where "mapped but times out because the scan is unbounded" gets fixed.
- **Grid remap (decide the grid first — Open Q1).** If the shipped grid is 12-col (as `gridGeometry.ts` is
  today), the mapper halves x/w, scales row-height (Grafana 30 px → our 56 px), and repacks y by band so
  tiles align 3-across instead of overflowing. If the spine is re-pinned to 24-col (matching Grafana 1:1),
  `gridPos` copies verbatim and `gridGeometry.ts` is the bug to fix. **Either way the doc and the code must
  agree** — the drift is the bug.
- **Drop placeholders with a notice.** Detect Grafana's default `metric_table`/`time_column` placeholder
  query and empty-content `text` panels; drop them (don't emit a "no template" cell) and report each.
- **Honest report, completed.** Add the missing lines: mapped-but-unwired (now impossible by construction,
  but asserted), dropped annotation plane, dropped `refresh`/decorative panels. A degrade with no report
  line is a test failure.

**Rejected:** a standalone file-in/file-out converter tool (the old `grafana-conversion-scope` shape). The
`dashboard.import` host verb already shipped and is the real seam — a second mapper forks the contract and
re-solves tenancy/datasource-remap that the verb already owns. Harden the verb; retire the standalone tool.

### Slice 2 — A richer native model (the 10× on expressiveness)

The mapper can only be as faithful as the record is expressive. Add the fields a real page needs, all
additive/serde-default (absent ⇒ today's cell), each named in `grafana-parity-backend` and rendered by the
rubix-ai UI half:

- **A real `text`/`markdown` view.** The #1 structural gap: `text` and `dashlist` had no host home, so they
  degraded to a raw-JSON/"no template" box. A `markdown` view (sanitized) lets a converted note/section
  banner render as the original intended — and is broadly useful beyond import.
- **Y-axis min/max + soft-clamp + unit.** The temperature charts carried real sensor-glitch spikes (up to
  538 °C) that squashed the axis; Grafana set no y-max either, but our fieldConfig can offer `min`/`max` and
  a soft-clamp so an outlier doesn't destroy the read. The Grafana `unit` (`celsius`/`ppm`) is already carried
  and labels the axis — extend to decimals/thresholds.
- **Panel/row `repeat` over multi-value variables.** The mapper preserves a panel's `repeat` field; the model
  + renderer expand one tile per variable value (bounded "+N more"). Depends on the advanced-variables work
  already shipped as model fields (`grafana-parity-backend`).

### Slice 3 — A renderer that never drops (never-looks-broken)

Two robustness fixes so a faithfully-mapped cell actually draws:

- **Auto-plot types columns by value, not name (SHIPPED — upstream it).** `cellPlotOrSuggested` (`plot.ts`)
  drew real axes only for a column literally named `value`/`payload`; a renamed value column
  (`value AS "Temperature"`) dropped to the axis-less sparkline. The fix prefers `value`/`payload`, else
  defers to the general `suggestPlot` (value-typed, like `num.ts::rowNumber`). Landed in rubix-ai's vendored
  UI + unit-pinned; **this slice merges it into lb** so the fix isn't a divergence a resync clobbers.
- **Bounded-by-default reads.** A converted query that forgets its window scans forever (the 30 s cancel).
  The mapper's `$__timeFilter` translation is the primary guard; a defense-in-depth secondary is a
  resolver-level default window when a `viz.query` target names a time column but no bound (composes with the
  caching scope's quantiser — same windowed-args seam). Recommend mapper-first, resolver-guard as a follow-up.

**Why all three:** slice 1 makes a page *map* to drawing cells; slice 2 gives those cells something faithful
to *be*; slice 3 makes them *render* without dropping. Ship one and a converted board still looks broken in
some dimension (blank / empty-box / axis-less). Together: a real Grafana export in → a page that draws,
aligns, and reads like the original.

## How it fits the core

- **Tenancy / isolation:** unchanged and central — `dashboard.import` sets the target workspace from the
  caller's **token**, never from the imported JSON, and the datasource-remap step (import-export-scope's
  tenancy-critical phase) binds a Grafana `uid` onto one of **this workspace's** registered datasources. A
  ws-B import can never bind a ws-A datasource. **Mandatory isolation test:** import the same JSON in ws A/B;
  each binds only its own datasources; a cross-ws datasource name in `mappings` is rejected.
- **Capabilities & deny path:** no new capability. `dashboard.import` rides `mcp:dashboard.save:call` (it
  writes a dashboard); the mapper never grants extra reach — a mapped target is re-authorized under the
  caller's grants at `viz.query` time exactly as a hand-authored one. Deny test: a caller without a target's
  read cap sees that tile denied-opaque, not the mapper leaking it.
- **Placement:** either — import is a host verb on any node; the model + renderer are symmetric. No role
  branch (rule 2).
- **MCP surface (§6.1 — judged):**
  - **Changed verb:** `dashboard.import` gains the hardened mapper behavior (wire/translate/remap/drop/report)
    — behavior change, **no** contract change (same `{json, mappings?, id?, now}` in, same `{id, report}`
    out; the `report.degraded[]` gains lines). Additive.
  - **Changed record:** `dashboard.save` accepts the new additive cell/view fields (a `markdown` view, y-axis
    `min`/`max`/`softClamp`, `repeat`) — serde-default, backward-compatible.
  - **No new CRUD / live-feed / batch:** import is a one-shot write; read is the existing `dashboard.get`.
    N/A stated.
- **Data (SurrealDB):** none new — the mapper writes a `Dashboard` record through the existing
  `dashboard.save` path; the new fields are additive on that record. No new table.
- **Bus (Zenoh):** N/A — no motion; import is a synchronous write, render reads are state.
- **Sync / authority:** SurrealDB stays authority for the stored dashboard; the imported JSON is interchange
  mapped at the edge and **never stored raw** (the panel-model spine decision). Offline unchanged.
- **Secrets:** none — a Grafana JSON carries no secret; the datasource DSN stays mediated in `lb-secrets`,
  bound by name at remap, never in the record or a frame.
- **One responsibility per file (FILE-LAYOUT):** the macro map in its own `viz/import/macros.rs`; the grid
  remap in `viz/import/grid.rs`; the placeholder-drop + report in `viz/import/report.rs`; the renderer fix in
  `ui/src/features/dashboard/views/plot.ts` (already split). Each ≤400 lines; no `utils.rs`.
- **SDK/WIT impact:** **none.** `dashboard.import`/`viz.query` are host-native verbs; the model fields are
  serde on the record; the renderer is UI. No plugin-boundary change. (Flagged per the checklist.)
- **Rule 10 (no special-casing):** the macro map is a **dialect** translation, not a per-source branch; the
  mapper treats every `panel.type`/datasource/variable as opaque data and never branches on one of *our*
  extension ids — an unmappable panel type is a reported degrade, not an `if type == …`.

## Example flow

An ops user imports the live pdnsw IAQ export into ws `acme` (datasource `pdnsw` registered).

1. **Preview.** `dashboard.import {json}` (no `mappings`) → `{report}`: 33 panels mappable, 2 datasources to
   bind (`-- Grafana --`, `Rubix OS Data Source`), `migratedFrom: 27`, and — **new** — degrade lines for the
   dropped placeholders + the plugin-query stats.
2. **Commit.** `dashboard.import {json, mappings:[…→pdnsw], id, now}`. The mapper, **hardened**: wires each
   `graph` target (`tool:federation.query`, `rawSql`→`sql`), **translates** `$__time`/`$__timeFilter` to the
   bounded host window, **remaps** the 24-col grid to the shipped 12-col grid (tiles pack 3-across),
   **synthesizes** a bounded latest-value query for each plugin stat *(if slice-2 follow-up shipped; else
   reports it)*, and **drops** the 6 banner placeholders + logo + dashlist with a notice.
3. **Open.** The board draws: 3 rows, 12 timeseries charts (real axes — the renamed-value-column ones too,
   via slice 3), 12 stat tiles with live readings, aligned 3-across. **24/24 data cells draw** (measured).
4. **A tile's source SQL is genuinely wrong.** Its cell renders the honest empty frame and the report named
   it a degrade — the board never blanks on one bad tile.
5. **A colleague opens the same link.** The caching sibling serves it warm (composed, not this scope).

## Testing plan

Real embedded node (`mem://` store, real gateway, a **real spawned Timescale/Postgres** seeded with rows —
the sanctioned fake-boundary, `testing-scope.md` §0) plus the rubix-ai E2E acceptance walk. No mocks. A real
Grafana export `.json` is a **fixture**, not a fake. Mandatory categories:

- **Golden import (headline):** the pdnsw IAQ export fixture → assert the stored `Dashboard`: 12
  `timeseries` + 12 `stat` + 3 `row` cells, every data cell's source `tool:"federation.query"` (no `tool:""`),
  every `graph` query macro-free (`$__` absent), grid packed to 12-col (max `x+w ≤ 12`, no overlap). The
  output must deserialize as a `Dashboard` (the shape guard).
- **Macro translation:** a `$__timeFilter(ts)` target → the bounded host window; run it → returns rows within
  the 30 s bound (the un-translated form is the regression: assert it would be unbounded). `$__time`,
  `$__timeGroup` unit-mapped; an unknown `$__foo` left verbatim **and** reported.
- **Wired-and-drawing (the core assertion):** every mapped data cell run through `viz.query` returns a frame
  (0 `tool:""`); flip one cell's source to `tool:""` and the render test goes red (the blank-panel guard —
  proven in rubix-ai E2E: 12 plots, 0 sparkline fallbacks).
- **Grid alignment:** the converted board's stat tiles occupy ≥3 columns (3-across), no cell exceeds the grid
  width, no y-overlap (the alignment regression — proven in E2E by distinct-column count).
- **Placeholder drop:** a fixture with a `metric_table` banner + empty logo `text` + `dashlist` → those
  panels are **absent** from `cells[]` and **present** in `report.degraded[]` (zero "no template" cells).
- **Renderer robustness (slice 3):** a frame with `value AS "Temperature"` (renamed value column) →
  `cellPlotOrSuggested` returns a line spec, not null (unit-pinned in `plot.test.ts`); E2E asserts 0
  `timeseries latest` sparkline fallbacks across the board.
- **Report completeness (honesty contract):** every degrade/drop that appears in a fixture has a
  `report.degraded[]` line — a dropped feature with no line fails the test.
- **Capability-deny + workspace-isolation (mandatory §2.1/§2.2):** import in ws A/B binds only own
  datasources; a cross-ws datasource name in `mappings` is rejected; a caller lacking a target read cap sees
  that tile denied-opaque post-import.
- **Frontend (rubix-ai, real gateway):** the E2E acceptance walk — import the real export, open the board,
  assert 12 chart surfaces with y-axes + ≥6 stat tiles + 3 row titles + 0 "no template" + 0 sparkline
  fallbacks + 3-across alignment; two-theme, screenshots by eye. (Shipped:
  `ui/e2e/grafana-iaq-convert.spec.ts`.)

## Risks & hard problems

- **The grid decision is load-bearing and currently ambiguous.** The scope says 24-col; the code ships
  12-col. Picking wrong (or not picking) reintroduces the overflow. This must be **resolved first** (Open
  Q1) — every mapper grid line depends on it, and the fix is either in the mapper (remap to 12) or in
  `gridGeometry.ts` (re-pin to 24), not both.
- **Macro translation is a dialect, and dialects leak.** `$__timeFilter` is easy; `$__timeGroup(col,'5m')`,
  `$__unixEpochFilter`, nested macros, and Grafana's per-datasource macro variants are a long tail. The
  bounded map covers the measured page; the honest fallback (leave verbatim + report) keeps an unknown macro
  from silently corrupting a query. Do **not** grow this into a Grafana SQL emulator — cover the common set,
  report the rest.
- **"Wire everything" must not become "execute everything."** A mapped-but-wrong query (the source's own
  `ORDER BY time` bug) should render an honest empty frame + a report line, **not** be silently repaired.
  The line between *translate the dialect* (in scope) and *fix the user's SQL* (out) is the honesty boundary
  — cross it and imports become unpredictable.
- **Renderer robustness has a tail beyond the value-column case.** Types-by-value fixes the measured bug;
  other frame shapes (multi-numeric with no obvious value, all-null series, category-only) still need sane
  fallbacks. The general `suggestPlot` already handles most; enumerate the residue as the fix lands.
- **Model growth vs. round-trip.** Every new field (markdown, y-clamp, repeat) must survive export back to
  Grafana (Phase 4) or be explicitly Grafana-less — carry-don't-strip so a Grafana import still round-trips
  1:1 (the panel-model spine rule).

## Open questions

1. **The grid: 12 or 24?** The shipped UI is 12-col; the spine doc says 24. **Recommend keeping 12** (the
   real, shipped, denser grid) and fixing the mapper to remap + the doc to match — re-pinning the whole UI to
   24 is a larger, riskier change for a 1:1-with-Grafana benefit users don't see. Decide **before** any
   mapper grid work.
2. **Plugin-query synthesis — this campaign or a follow-up?** 18 of 35 tiles on *this class* of NubeIO board
   need it, but it's plugin-specific (not generic Grafana). **Recommend a named follow-up** in
   `grafana-parity-backend`'s datasource family; this scope reports it honestly meanwhile.
3. **Bounded-by-default: mapper-only or resolver-guard too?** Recommend **mapper-first** (translate
   `$__timeFilter`), with a resolver-level default window as a defense-in-depth follow-up composed with the
   caching quantiser — not a hard requirement of this cut.
4. **Markdown view sanitization boundary.** A Grafana `text` panel can carry arbitrary HTML (the pdnsw logo
   was a raw `<div>` with a background-image). Recommend a **sanitized markdown** view (strip scripts/styles,
   allow basic formatting + images) — decide the allowlist with the UI half.

## Related

- [`frontend/dashboard/viz/import-export-scope.md`](../frontend/dashboard/viz/import-export-scope.md) — the
  `dashboard.import`/`export` verbs + bidirectional mapper this hardens (and the 24-vs-12 grid line to
  reconcile at `:110`).
- [`grafana-parity-backend-scope.md`](grafana-parity-backend-scope.md) — the additive model fields (markdown
  view, y-clamp, repeat, advanced variables) + the `schemaVersion` import pin this consumes.
- [`../caching/dashboard-query-acceleration-scope.md`](../caching/dashboard-query-acceleration-scope.md) —
  the **speed** sibling (10× open time); a bounded, macro-translated query is exactly what its cache keys on.
- **rubix-ai** `docs/scope/frontend/dashboard/viz/grafana-parity-ui-scope.md` — the UI half (typed option
  shapes, renderers, the missing views); `docs/testing/dashboard/grafana-conversion-review.md` — the measured
  write-up this scope distills; `ui/e2e/grafana-iaq-convert.spec.ts` — the acceptance walk;
  `docs/scope/frontend/dashboard/grafana-conversion-scope.md` — the **stale standalone-tool** scope this
  supersedes (re-point it here).
- README `§3` (workspace wall, capability-first, one datastore, symmetric nodes, rule 10), `§6.1` (API shape).
- Skill: `skills/dashboard-import/SKILL.md` — the drivable surface (preview→commit an export, read the
  report) the implementing session writes, grounded in a live run against a real export.
