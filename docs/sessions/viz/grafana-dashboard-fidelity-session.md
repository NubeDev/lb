# Viz — Grafana dashboard fidelity (session)

- Date: 2026-07-23
- Scope: ../../scope/viz/grafana-dashboard-fidelity-scope.md
- Stage: build — slices 1 & 2 (the lb platform + mapper half) + the slice-3 resolution
- Status: green (lb half). Consumer half (VENDOR.md divergence drop + pin bump + E2E) lands in rubix-ai.
- Tracking: #91 wire graph targets, #92 translate `$__*` SQL macros, #93 grid remap 24→12,
  #94 drop "no template" placeholders, #95 complete the honest report, #96 upstream the auto-plot fix.

## Goal / exit gate

Make `dashboard.import` **lossless** against a real Grafana export: every mappable panel comes out
**wired and drawing**, the 24-col grid **remaps** to our shipped 12-col grid, decorative placeholders
**drop with a notice**, and **every** drop/degrade earns a `report.degraded[]` line. Grounded in the
measured pdnsw IAQ conversion (`rubix-ai/docs/testing/dashboard/grafana-conversion-review.md`).

## What shipped (all in `crates/host/src/dashboard/grafana/`, one responsibility per file)

- **#92 — SQL macro translation (the load-bearing fix). `macros.rs` (new).** A bounded DIALECT map:
  `$__time(x)→x AS "time"`, `$__timeEpoch`, `$__timeFilter(x)→` the host end-day-exclusive
  `$__from`/`$__to` window, `$__timeFrom`/`$__timeTo`, `$__timeGroup(x,'5m')`. Anything unrecognized is
  left **verbatim + REPORTED** (`kind:"macro"`) — never silently rewritten. `$__from`/`$__to` are host
  tokens (and Grafana's own epoch-ms vars) → pass through, never reported. Wired at `bind.rs` when
  `rawSql`→`sql` is moved, so the stored target is the bounded, executable form. This closes the
  measured "unbounded scan hit the 30 s cancel" — the reason 12 charts could never draw.
- **#91 — wire graph targets.** `bind.rs` already sets `tool:"federation.query"` + `args.{source,sql}`
  for a bound target (it was added to fix the `tool:""` skip); this campaign proves it by construction:
  the golden real-node test asserts **0 `tool:""`** across every data cell.
- **#93 — grid remap 24→12. `grid.rs` (new).** DECISION LOCKED: keep 12-col; `gridGeometry.ts`
  untouched. The mapper halves x/w, scales height (30 px→56 px rows), and **repacks y by band** so tiles
  pack 3-across with no overlap. Runs once over `cells[]` after the panel map.
- **#94 — drop placeholders. `report.rs` (new).** `drop_reason(panel)` removes a `dashlist`, an
  empty/logo `text` panel, and a Grafana default `metric_table` banner — **no cell emitted**, a report
  line each. No more "no template" eyesores.
- **#95 — honest report, completed. `report.rs`.** `dashboard_drops(json)` names the annotation plane,
  the dropped `refresh` interval, and `graphTooltip`; the placeholder + macro drops all report. A drop
  with no line is a test failure — asserted in the golden test.
- **Slice 2 — richer model (additive, serde-default).**
  - **Real `text` view.** Grafana `text` → the shipped sanitized `text` view (`view_alias.rs`), and
    `text` is registered in `widget_catalog.json` so `dashboard.save` accepts it. The renderer already
    ships downstream (`rubix-ai ui/.../views/text/`, markdown/html/code + DOMPurify).
  - **Y-axis min/max + soft-clamp** ride the opaque `fieldConfig` unchanged (carried verbatim; the UI
    owns the typed shape) — round-trip pinned.
  - **Panel/row `repeat`** — new `Cell.repeat`/`repeatDirection`/`maxPerRow` (serde-default,
    skip-if-empty), carried by `to_cell`/`to_grafana`, round-trip pinned.
- **Toolbar chrome the import implies (`import.rs`).** A wired `$__from`/`$__to` window turns the date
  picker on (`toolbar.dateSelect`); a source `refresh` turns the refresh control on.
- **Doc drift fixed.** `import-export-scope.md` grid row corrected from "both 24-col" to
  "Grafana 24-col → our 12-col (remapped)".

## Slice 3 / #96 — RESOLVED (the plot.ts upstream half is obsolete)

The scope's slice 3 says to land the auto-plot renamed-value-column fix in lb at
`ui/src/features/dashboard/views/plot.ts`. **That home no longer exists:** lb's `ui/` tree was deleted
2026-07-15 (STATUS: "never recreate `ui/`") and the plot/charts logic (`cellPlotOrSuggested`,
`suggestPlot`, `fieldKind`) was **not** carried into `packages/*` — it lives solely in the product UI
(rubix-ai). Recreating it in lb would either resurrect `ui/` (forbidden) or plant unconsumed duplicate
code in `packages/*` (dead code). So:

- **The fix stays product-owned in rubix-ai** (already shipped + unit-pinned in `plot.test.ts`), and
  rubix-ai's `VENDOR.md` divergence entry is retired there — its "push upstream so a resync brings it
  back" premise is moot because there is no live lb `ui/` to resync from.
- **The load-bearing half of slice-3 robustness — bounded-by-default reads — DID land here** as #92:
  translating `$__timeFilter` is exactly what stops a converted chart from scanning forever.

This is recorded so #96 is a **decision, not an omission**.

## Tests (all green; each proves its own regression)

- `macros.rs` — every macro translates; unknown left verbatim + reported once; `$__from`/`$__to` pass
  through; nested parens balanced. (Break the map → red.)
- `grid.rs` — 3 Grafana thirds pack to 3 distinct columns; full-width/rows stay full; bands never
  overlap; height scales 30→56; odd geometry never exceeds 12 cols.
- `report.rs` — dashlist/empty-text/metric_table drop, a real note/query does not; dashboard drops name
  annotations/refresh/tooltip.
- `bind.rs` — macros translate on wire; unknown macro reported at bind.
- `model.rs` — `repeat` + y-axis fields round-trip and default byte-stable.
- **Golden real-node headline** (`role/gateway/tests/dashboard_grafana_test.rs`,
  `iaq_import_is_wired_macro_free_grid_aligned_and_honest`): a pdnsw-shaped export → commit on a real
  gateway/store → **6 cells (3 dropped), 0 `tool:""`, 0 `$__time(`/`$__timeFilter(`, 0 `json`
  placeholders, all x+w ≤ 12, 3 charts 3-across, text note carried, every drop reported,
  `toolbar.dateSelect` on, timezone preserved**. Plus the existing round-trip / ws-isolation /
  cap-deny / v2-reject tests stay green.

## Release

Cut a `node-v*` tag; rubix-ai bumps its `lb-node` pin, drops the `VENDOR.md` plot.ts divergence, and
keeps `ui/e2e/grafana-iaq-convert.spec.ts` green (#27, #28).
