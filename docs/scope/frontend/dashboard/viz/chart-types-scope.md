# Viz scope — the standard visualization set (chart types)

Status: **Phase 1 shipped (2026-06-29)** — `timeseries` ships end to end (renderer + the full per-viz
`options` surface: legend/tooltip; the `chart` alias resolves to it). Phase 2+ panel types remain scoped.
Part of the [`viz/`](README.md) slice — the **standard visualization set** that fills in the `view`
vocabulary the spine ([`panel-model-scope.md`](panel-model-scope.md)) defines. Shipped truth in
[`public/frontend/dashboard.md`](../../../../public/frontend/dashboard.md).

One paragraph: this doc owns **which Grafana panel types we render, in what order, with which renderer, and
with what per-viz `options` shape**. The user's complaint is real — our shipped renderers expose almost no
options (chart: `unit`; stat: `unit`; gauge: `min/max/unit`; table: none) and look poor. The fix is not to
invent better options; it is to **adopt Grafana's panel types as our `view` vocabulary** and **adopt
Grafana's per-viz `options` shapes verbatim** so a `timeseries` panel here has the same legend/tooltip/axis
surface as in Grafana and import maps 1:1. The shipped `chart`/`stat`/`gauge`/`table` views become
**aliases** of `timeseries`/`stat`/`gauge`/`table`. This is additive over the v3 cell; it adds no datastore
and no new render-path capability (a panel's data is still its target tool's cap, host-re-checked).

## Goals

- **Adopt Grafana's panel-type ids as the `view` vocabulary** (`timeseries`, `barchart`, `stat`, `gauge`,
  `bargauge`, `table`, `piechart`, `histogram`, `state-timeline`, `status-history`, `heatmap`, `text`, …)
  so a panel's identity is Grafana's identity and import/export is a name match, not a translation table.
- **Alias the shipped views** so no v2 cell breaks: `chart`→`timeseries`, `stat`→`stat`, `gauge`→`gauge`,
  `table`→`table`. A v2 `chart` cell **is** a `timeseries` panel.
- **A typed, structured `options` shape per panel type**, with Grafana's option names verbatim
  (legend/tooltip/orientation/stacking/reduceOptions/…) — not a loose `Record`. The per-viz `options` is
  the whole of what this doc owns.
- **One renderer file per panel type** (FILE-LAYOUT), choosing recharts where it fits and naming a
  dedicated lib only where it doesn't.
- **Result-shape ↔ panel-type validation**: the editor offers only the views a target's data shape can
  honestly fill, and `reduceOptions` is the explicit bridge from frames to single values.
- **Phase it**: prove `timeseries` end to end first, then fan out; defer the exotic panels as named
  follow-ups that degrade honestly on import.

## Non-goals

- **No per-field options here.** Unit, decimals, min/max, thresholds, value mappings, color, displayName —
  all of that is **`fieldConfig`** and lives in [`field-config-scope.md`](field-config-scope.md). This doc
  owns the per-**viz** `options` (how the chart draws), not the per-**field** options (how a value reads).
  The line is Grafana's own line and we keep it.
- **No Grafana plugin runtime / Angular panels.** We render natively; we adopt the *option taxonomy*, not
  Grafana's panel SDK.
- **No exotic panels in the first cut.** `geomap`, `nodeGraph`, `flamegraph`, `traces`, `candlestick`,
  `xychart`, `trend`, `canvas`, `dashlist`/`alertlist`/`annolist`/`news` are **deferred** and degrade
  honestly on import (named, not faked — README's "degrade the rest honestly" rule).
- **No new datastore, no new render capability, no `if cloud`, no `*.fake.ts`.**

## Intent / approach

**Altitude:** a `view`→renderer **registry** plus a `view`→`options`-schema map. A cell carries
`view:"timeseries"` and a typed `options` blob; a small registry resolves the renderer component and the
default+validated options for that view. Adding a panel type = adding one file under
`ui/src/features/dashboard/views/<type>/` and one registry entry. This keeps the contract (the `view` id +
`options` shape) decoupled from the drawing library, so swapping recharts for visx on one panel type later
touches one file.

**Renderer choice — recharts where it fits, a dedicated lib only where it can't.** Recharts already ships
(`ui/src/features/dashboard/widgets/recharts.tsx`) and does line/area/bar/pie/radial well. Keep it for
`timeseries`, `barchart`, `piechart`, `stat` (sparkline), `gauge`/`bargauge` (radial/linear). Recharts is
weak at matrix/categorical-time visuals; name **visx** (or `@visx/heatmap` + `@visx/scale`) for `heatmap`,
`state-timeline`, `status-history`, `histogram` only when those phases land. `text` needs no chart lib
(markdown via the shipped renderer). **Don't over-engineer**: one lib per need, chosen per panel type, not
a grand abstraction over both.

**Rejected alternative — invent our own richer option set on the existing `chart`/`stat`/`gauge` views.**
That is exactly what produced the current bad, minimal options: a bespoke taxonomy that nobody can import
into and that drifts from Grafana's mental model. It would also fork the import/export mapper (no 1:1 name
map) and re-litigate decisions Grafana already made well (legend modes, reduce calcs, stacking semantics).
Adopting Grafana's panel ids and option names verbatim makes the mapper a near-identity and gives users a
surface they already know.

### The alias map (shipped view → Grafana panel id)

| Shipped v2 `view` | Grafana `view` (canonical) | Note |
|---|---|---|
| `chart` | `timeseries` | A v2 `chart` cell **is** a `timeseries` panel; the alias resolves to the timeseries renderer + options defaults. |
| `stat` | `stat` | Same id; gains the full stat `options`. |
| `gauge` | `gauge` | Same id; gains `showThresholdMarkers`/`orientation`/`reduceOptions`. |
| `table` | `table` | Same id; gains `showHeader`/`cellHeight`/`footer`/pagination. |
| `plot` / `d3` | (kept as-is) | Escape-hatch custom views — not Grafana panels; unchanged, not aliased. |
| `template`/`switch`/`slider`/`button` | (kept as-is) | Control views — out of Grafana's chart taxonomy; unchanged. |
| `ext:<id>/<widget>` | (kept as-is) | Extension-provided panels — federated, unchanged. |

The registry maps the alias to the canonical id at read time, so `view:"chart"` and `view:"timeseries"`
render identically; new cells write the canonical id.

### Phasing

- **Phase 1 — `timeseries` end to end** (the "start with one chart"). One renderer, the full
  `timeseries` `options` surface (legend/tooltip + the timeseries custom draw fields), the alias from
  `chart`, with `fieldConfig` ([`field-config-scope.md`](field-config-scope.md)) doing units/thresholds.
- **Phase 2 — the rest of the everyday set**: `stat`, `gauge`, `bargauge`, `table`, `barchart`,
  `piechart`. All recharts-able; all single-frame or reduceable. This is where the user's "options are bad"
  complaint is fully answered across the common panels.
- **Phase 3 — `histogram`, `state-timeline`, `status-history`, `heatmap`, `text`**. Introduces the visx
  dependency (heatmap/state-timeline) and the markdown `text` panel.
- **Deferred (named follow-ups)** — `geomap`, `nodeGraph`, `flamegraph`, `traces`, `candlestick`,
  `xychart`, `trend`, `canvas`, list panels, `news`. On import these map to a placeholder panel that names
  the unsupported type and shows the raw target/data — honest degradation, never a fabricated viz.

### The per-viz `options` shapes (Grafana names, verbatim)

Each is a **typed** shape (a discriminated union keyed by `view`), not a loose record. Names match Grafana's
`panelcfg.cue` so the mapper is 1:1. (Custom-draw fields like `drawStyle`/`lineWidth` are Grafana
`fieldConfig.custom`, so they live in [`field-config-scope.md`](field-config-scope.md), not here.)

- **`timeseries`** — `legend {displayMode: list|table|hidden, placement: bottom|right, showLegend,
  calcs[]}`, `tooltip {mode: single|multi|none, sort}`.
- **`barchart`** — `legend`, `tooltip`, `orientation: auto|horizontal|vertical`, `stacking:
  none|normal|percent`, `showValue: auto|always|never`, `barWidth`, `groupWidth`, `xField`, `colorByField`,
  `xTickLabelRotation`.
- **`stat`** — `graphMode: none|line|area`, `colorMode: value|background|background_solid|none`,
  `justifyMode`, `textMode`, `reduceOptions`, `showPercentChange`, `orientation`.
- **`gauge`** — `reduceOptions`, `showThresholdLabels`, `showThresholdMarkers`, `orientation`, `sizing`,
  `minVizWidth`, `minVizHeight`.
- **`bargauge`** — `displayMode: basic|lcd|gradient`, `orientation`, `reduceOptions`, `showUnfilled`,
  `valueMode`.
- **`table`** — `showHeader`, `cellHeight: sm|md|lg`, `footer`, `sortBy[]`, `enablePagination`. (Per-column
  cell display — color background, gauge cell — is `fieldConfig.custom`, owned by field-config.)
- **`piechart`** — `pieType: pie|donut`, `displayLabels: (name|value|percent)[]`, `legend`, `tooltip`,
  `reduceOptions`.
- **`histogram`** — `bucketSize`, `bucketOffset`, `combine`, `legend`, `tooltip`.
- **`state-timeline` / `status-history`** — `rowHeight`, `showValue`, `mergeValues`, `alignValue`.
- **`text`** — `mode: markdown|html|code`, `content`.

`reduceOptions {calcs:[…], fields, values, limit}` is the shared **frame→value** reducer for the
single-stat-style panels (`stat`, `gauge`, `bargauge`, `piechart`): it collapses series frames to the
value(s) those panels draw.

## How it fits the core

- **Tenancy / isolation:** none added — a panel type is pure presentation over a target's already
  workspace-scoped data. Choosing a `view` never crosses the wall; the data still arrives via the target's
  `bridge.call` ([`panel-model-scope.md`](panel-model-scope.md)).
- **Capabilities:** **no new render-path cap.** Every panel type renders the same target rows the v2 bridge
  already leashes by `cell.tools ∩ grant`, host-re-checked per call. Adding views does not widen reach.
  (Import/export verbs are [`import-export-scope.md`](import-export-scope.md)'s, not here.)
- **Placement (rule 1):** the registry and renderers are pure UI, identical on Tauri and gateway; no role
  branch. Same code, two transports.
- **MCP surface (§6.1):** none added. A panel is a generic front-end over the target tool; the `view` is a
  client-side render choice, not a server contract.
- **Data (SurrealDB):** the `options` blob is part of the v3 cell record (defaulted per `view`); no new
  record, no new query. Bounded: `options` is a small typed shape; nothing here grows the record beyond the
  spine's caps.
- **Bus / Sync:** unchanged — live panels stream over the shipped SSE; `options` rides the cell UPSERT and
  replays idempotently (state vs motion holds).
- **Secrets:** N/A — a panel type holds no credential; a federation target's DSN stays server-side.
- **SDK/WIT:** no WIT change. Extension-provided panels keep arriving as `ext:<id>/<widget>` (the shipped
  federated-widget path); we do not add a panel-plugin ABI here.

## Example flow

1. A user adds a panel; the editor's viz picker lists the **supported** views, filtered by the target's
   result shape (a scalar result shows `stat`/`gauge`/`bargauge`; rows-of-series shows
   `timeseries`/`barchart`; tabular shows `table`).
2. They pick **Timeseries** (Phase 1). The registry resolves the `timeseries` renderer and seeds default
   `options {legend:{showLegend:true, displayMode:"list", placement:"bottom"}, tooltip:{mode:"single"}}`.
3. They set legend to the `table` display mode with `calcs:["mean","max"]`; the renderer shows the legend
   table. Units/decimals/thresholds come from `fieldConfig` (a different tab), rendered via user-prefs.
4. They add a companion **Stat** panel over target A with `reduceOptions {calcs:["lastNotNull"]}`; the
   reducer collapses the series to one value, drawn with `graphMode:"area"`, `colorMode:"value"`.
5. They try to set the Stat to a `table` view — the picker doesn't offer it (a single reduced value can't
   honestly be a table). The choice degrades to what the shape supports; no fake value is invented.
6. `dashboard.save` UPSERTs the cells with their typed `options`; reload re-reads them; an older client
   falls back via the alias (`view:"chart"`/`widget_type`) and the v2 renderer.

## Testing plan

Per [`../../../testing/testing-scope.md`](../../../testing/testing-scope.md) — real gateway/store, seeded
real rows, **no `*.fake.ts`**.

- **Alias fidelity (headline):** a seeded **v2 `chart`** cell renders through the `timeseries` renderer and
  re-saves as a v3 cell whose data is identical; `view:"chart"` and `view:"timeseries"` produce the same
  output for the same target. Same for `stat`/`gauge`/`table`.
- **Options round-trip:** for each Phase-1/2 panel type, a cell with a fully-populated typed `options`
  round-trips through `dashboard.save`/`get` and re-renders identically; unknown/extra option keys are
  dropped (typed shape, not loose record).
- **Result-shape ↔ type validation:** seed a scalar source and assert the picker offers stat/gauge but not
  table; seed tabular rows and assert it offers table but not gauge; assert `reduceOptions` turns a
  multi-series frame into the single value a `stat`/`gauge` draws.
- **Honest degradation:** importing a `geomap` (or other deferred) panel yields the placeholder-with-name
  panel and the raw data, never a fabricated viz (the import test lives in
  [`import-export-scope.md`](import-export-scope.md); the *render* of the placeholder is asserted here).
- **Capability deny:** a panel whose target tool is denied renders the opaque deny state — proving the
  render path adds no bypass and reuses the target cap.
- **Workspace isolation:** a panel in ws-B renders only ws-B target data (the data path is the spine's;
  asserted here that no panel type leaks across the wall).

## Risks & hard problems

- **Scope sprawl across panel types.** Grafana has ~25 panels each with deep options. The phasing + the
  "supported subset, degrade the rest honestly" rule is the guardrail; resist building Phase-3/deferred
  panels early.
- **Recharts limits.** Recharts cannot do heatmap/state-timeline well; forcing it produces the same poor
  result we're fixing. Commit to visx for those (Phase 3) rather than a recharts hack — but don't pull visx
  in before Phase 3 (no premature dependency).
- **Options vs fieldConfig boundary drift.** It is tempting to put `lineWidth`/`unit` in panel `options`;
  Grafana puts draw-style in `fieldConfig.custom` and units in `fieldConfig.defaults`. Hold Grafana's line
  exactly or the import map stops being 1:1. Cross-check every option against `panelcfg.cue`.
- **Result-shape detection.** Mis-detecting a frame's shape mis-offers views. The detector must be
  conservative (offer a view only when the shape clearly supports it) and `reduceOptions` must be the
  explicit, visible bridge — never an implicit "guess a number" that fabricates a value.

## Resolved decisions

No blocking open questions — these are the long-term answers the build follows.

- **Use `visx` for the non-recharts panels** (heatmap, state-timeline/status-history, histogram
  primitives) — one extra dep covers the family; recharts stays for timeseries/barchart/piechart/stat/
  gauge. Revisit only if a panel needs something visx lacks.
- **`status-history` is the same renderer family as `state-timeline`** — two `view` ids, two thin files
  sharing `rowHeight`/`showValue`/`mergeValues`/`alignValue`. Both land in Phase 3.
- **Keep `plot`/`d3` custom scripted views out of the standard viz picker.** They aren't Grafana panels
  and don't import/export; they stay reachable as a power-user escape hatch (the shipped scripted views),
  not part of the Grafana-parity set.
- **Copy Grafana's `panelcfg` per-panel option defaults verbatim** so a freshly-added panel looks like
  Grafana's out of the box, then let `fieldConfig` + user-prefs localize presentation.

## Related

- [`README.md`](README.md) — the viz umbrella + the reconciliation table (view↔panel-type).
- [`panel-model-scope.md`](panel-model-scope.md) — the spine: `view`, `sources[]`/`Target`, `options`,
  `v:3`, `schemaVersion` (this doc fills in the `view` set + `options` shapes).
- [`field-config-scope.md`](field-config-scope.md) — the **per-field** options (unit/decimals/thresholds/
  color, `fieldConfig.custom` draw-style) that pair with these per-viz `options`.
- [`transformations-scope.md`](transformations-scope.md) — the pipeline that reshapes target rows before a
  panel draws them (e.g. `reduce` feeding a `stat`).
- [`datasource-binding-scope.md`](datasource-binding-scope.md) — where a panel's target data comes from.
- [`import-export-scope.md`](import-export-scope.md) — the Grafana-JSON mapper that relies on the verbatim
  panel ids + option names defined here; owns the honest-degradation import path.
- [`panel-editor-scope.md`](panel-editor-scope.md) — the editor that surfaces the viz picker + per-viz
  `options` tab (one field-code path so add==edit).
- [`../widget-builder-scope.md`](../widget-builder-scope.md) — the shipped v2 views these extend/alias.
- [`../../../prefs/user-prefs-scope.md`](../../../prefs/user-prefs-scope.md) — the formatting boundary the
  rendered values resolve through.
- `ui/src/features/dashboard/widgets/recharts.tsx` (existing renderers to extend) and
  `ui/src/features/dashboard/views/` (where per-type renderers land, one file per `view`).
- The Grafana reference clone at `/tmp/grafana/public/app/plugins/panel/<type>/panelcfg.cue` — the
  authoritative `options` + custom `fieldConfig` shape per panel type.
- README **§6.1** (timeseries + API shape), **§6.13** (UI delivery), **§3** (rules 1/5/6/7).
