# Viz scope — the field config (chart options done right, via user-prefs)

Status: **Phase 1 shipped (2026-06-29)** — `fieldConfig.defaults` (unit/decimals/min-max/thresholds/
mappings/color) + `byName`/`byType` overrides render through the ONE user-prefs bridge
(`features/dashboard/fieldconfig/format.ts`) with the documented fallback until `lb-prefs` ships
(`byRegex`/`byFrameRefID` deferred as named follow-ups). Part of the [`viz/`](README.md) slice; the
**option taxonomy** half of the panel model ([`panel-model-scope.md`](panel-model-scope.md)). Shipped
truth in [`public/frontend/dashboard.md`](../../../../public/frontend/dashboard.md).

One paragraph: this is the doc that fixes the user's core complaint — *"the options for a chart are really
bad."* Today a chart carries a single `unit` string, a stat a `unit`, a gauge a `min/max/unit`: no
decimals, no thresholds, no value mappings, no color modes, no `displayName`, no axis options. We replace
that with **Grafana's `fieldConfig` verbatim** — `defaults` (the per-field option set) plus per-field
`overrides[]` with matchers — so a Grafana dashboard imports 1:1 and a user's muscle memory transfers. The
headline: a field's `unit`/`decimals`/date formatting are **not** rendered by a second formatting stack —
they resolve through the **user-prefs** boundary ([`format.quantity`/`format.number`/`format.datetime`](../../../prefs/user-prefs-scope.md)),
so the **same panel** renders `43,2 km/h` for one viewer and `23.3 kn` for another. Canonical value in,
localized presentation out — we **never** store a formatted string.

## Goals

- **Adopt Grafana's `fieldConfig` shape verbatim.** `FieldConfig { defaults: FieldOptions; overrides: FieldOverride[] }`,
  where a `FieldOverride` is `{ matcher: Matcher; properties: DynamicConfigValue[] }`. Importing a Grafana
  panel's `fieldConfig` is a field-name copy, not a re-model. This doc **owns** `FieldOptions`,
  `FieldOverride`, `Matcher`, `ValueMapping`, `ThresholdsConfig`, `FieldColor` (the shapes
  [`panel-model-scope.md`](panel-model-scope.md) declares on the cell and delegates here).
- **The full option surface, not a `unit` string.** `displayName`, `description`, `unit`, `decimals`,
  `min`, `max`, `mappings[]`, `thresholds`, `color`, `noValue`, and a per-view `custom` bag (lineWidth,
  fillOpacity, axis placement — owned by [`chart-types-scope.md`](chart-types-scope.md)).
- **THE user-prefs bridge (the headline).** `unit` + `decimals` + date format become `format.quantity` /
  `format.number` / `format.datetime` calls resolved in the **viewer's** prefs. One formatter for the whole
  platform, not a chart-local copy of CLDR/unit math.
- **Phase the depth.** Phase 1 ships `defaults` only (covers the user's complaint completely). Phase 2 adds
  `overrides[]` with `byName`/`byType` matchers. `byRegex`/`byFrameRefID` are named follow-ups.
- **Additive over the v3 cell.** `fieldConfig` rides the existing `dashboard.save` UPSERT; no new datastore,
  no new verb, no new render-path capability.

## Non-goals

- **No alerting.** Thresholds *color* a value here; alert *rules* (fire on breach, notify, escalate) are the
  rules engine's job ([`../../../rules/rules-engine-scope.md`](../../../rules/rules-engine-scope.md)). A
  `ThresholdsConfig` is presentation, not a rule.
- **No second formatting stack.** We do **not** re-implement CLDR number/date formatting or unit conversion
  in a renderer. This scope *consumes* the prefs `format.*`/`convert.*` tools; it owns the **mapping table**
  from Grafana unit ids to our dimension/unit enum, nothing more.
- **No options *semantics* for a view's `custom` bag.** Per-panel-type field options (lineWidth, point size,
  axis side) are named here as the `custom` passthrough but defined by
  [`chart-types-scope.md`](chart-types-scope.md).
- **No stored formatted string, ever.** The store holds canonical values + a `fieldConfig`; the localized
  string is computed at render. (Core principle — README §3, prefs-scope.)

## Intent / approach

**Adopt Grafana's `fieldConfig` 1:1 and route its presentation half through user-prefs.** The `unit` field
in Grafana names a unit/format (e.g. `celsius`, `bytes`, `velocitykmh`, `percent`, `time:YYYY-MM-DD`). We
keep that field, but at render time map it to **our** dimension/unit enum and call the prefs formatter:

```
render(value, fieldConfig) =
  unit  ∈ canonical-dimension  → format.quantity(value, from_unit, dimension, { decimals })  // resolved in viewer prefs
  unit  ∈ passthrough (currency/custom) → format.number(value, { decimals }) + static suffix
  no unit, numeric              → format.number(value, { decimals })
  time field                    → format.datetime(instant, { pattern from unit })
```

**Rejected alternative — invent our own per-chart option shape (and a chart-local formatter).** That was
the shipped state (`unit` string + ad-hoc `toFixed`), and it is exactly the complaint: every chart formats
differently, nothing imports from Grafana, and a viewer's locale/units are ignored. Adopting Grafana's
shape makes import a copy; routing through prefs makes the *same* panel correct for every viewer. The cost
— a Grafana-unit→our-dimension mapping table — is one deliverable file, far cheaper than a forked taxonomy
plus a forked formatter that drifts from the platform's canonical-in/localized-out rule.

## How it fits the core

- **Tenancy / isolation:** `fieldConfig` is inert presentation data on the workspace-scoped `dashboard:{id}`
  record — no key, no wall of its own. The viewer's prefs (which decide units/locale) are resolved by
  `prefs.resolve` **for the calling principal in their workspace**; a ws-B viewer can never resolve ws-A
  prefs (prefs-scope owns that wall).
- **Capabilities:** **no new render-path cap.** Reading the value reuses the target tool's cap ∩ grant
  (panel-model). Formatting uses the prefs **low/no-grant utility tier** — `format.*`/`convert.*` touch no
  tenant data (only CLDR + unit math), so they need no fresh grant invented here. Saving a `fieldConfig`
  rides `mcp:dashboard.save:call`, already gated.
- **Placement:** `either` — the shape is pure data; the same render code runs on edge and cloud, Tauri
  `invoke` or gateway SSE+HTTP. No role branch.
- **MCP surface (§3.7, §6.5):** **none added** by this scope. Data arrives as **canonical** frames from
  `viz.query` ([`transformations-scope.md`](transformations-scope.md)); this scope's rendering then *calls*
  the prefs tools (`format.quantity` / `format.number` / `format.datetime` / `convert.unit`) to localize —
  data-shape (backend resolver) and presentation (prefs boundary) are distinct MCP verbs. `fieldConfig` is
  part of the `dashboard.save` UPSERT, not a new verb.
- **Data (SurrealDB):** one record, more defaulted fields. `overrides[]` is **bounded ≤64/panel** and
  `mappings[]`/`thresholds.steps[]` bounded (e.g. ≤64 each) so a `fieldConfig` cannot bloat the dashboard
  row (panel-model's caps). All fields `#[serde(default)]` — absent on a v1/v2 cell.
- **Bus (Zenoh):** unchanged. A `prefs.set` emits the "prefs changed" hint (prefs-scope) so an open panel
  re-renders with new units/locale **without** re-querying — state vs motion holds (the value is state, the
  re-render is a motion cue).
- **Sync / authority:** `fieldConfig` is additive on the shipped `(table,id)` dashboard UPSERT; it replays
  idempotently and an older node reads the cell via the `unit`-only fallback (forward-compatible by serde).
- **Secrets:** none — a `fieldConfig` holds no credentials; unit ids and colors are public.
- **SDK/WIT impact:** none beyond panel-model's v2→v3 additive bump. `fieldConfig` is part of that cell
  shape; this doc fixes its sub-shapes. No `[[widget]].scope`/bridge change.

### The shapes (owned here)

```
FieldOptions {                              // Grafana "FieldConfig" defaults; the per-field option set
  displayName?: string                      // override the field's rendered name (vars-interpolated)
  description?: string
  unit?: string                             // Grafana unit id ("celsius","bytes","percent","velocitykmh","time:...")
  decimals?: number                         // → format.*({ decimals })
  min?: number; max?: number                // axis / gauge bounds (canonical units)
  noValue?: string                          // text when null/empty
  mappings?: ValueMapping[]                  // value→{text,color,icon} (see below)
  thresholds?: ThresholdsConfig              // step coloring
  color?: FieldColor                         // color mode
  custom?: Record<string, unknown>           // per-view field options (lineWidth/fillOpacity/axis) — chart-types owns
  // filterable?/writeable?/links?/actions? — accepted on import, deferred render (named follow-ups)
}

FieldOverride { matcher: Matcher; properties: { id: string; value: unknown }[] }   // Grafana DynamicConfigValue[]
Matcher { id: "byName" | "byType" | "byRegex" | "byFrameRefID"; options?: unknown } // Phase 1: byName/byType

ValueMapping =
  | { type: "value";   options: Record<string, ValueMappingResult> }        // {"10":{text,color}}
  | { type: "range";   options: { from: number|null; to: number|null; result: ValueMappingResult } }
  | { type: "regex";   options: { pattern: string; result: ValueMappingResult } }   // deferred render with byRegex
  | { type: "special"; options: { match: "true"|"false"|"null"|"nan"|"empty"; result: ValueMappingResult } }
ValueMappingResult { text?: string; color?: string; icon?: string; index?: number }

ThresholdsConfig { mode: "absolute" | "percentage"; steps: { value: number | null; color: string }[] }
  // first step's value is always -Infinity (null), per Grafana

FieldColor { mode: FieldColorModeId; fixedColor?: string; seriesBy?: "last"|"min"|"max" }
  // modes: "thresholds" | "fixed" | "palette-classic" | "palette-classic-by-name"
  //      | "continuous-GrYlRd"/"continuous-viridis"/… | "shades"
```

### The unit picker / mapping table (a named deliverable)

A closed table maps Grafana unit ids → our `(dimension, from_unit)` (the prefs **closed dimension enum**:
`temperature`, `wind_speed`/`speed`, `distance`, `mass`, `pressure`, `data`/`bytes`, `percent`, `time`, …):

| Grafana unit id | dimension | from_unit | render |
|---|---|---|---|
| `celsius` | `temperature` | `degree_celsius` | `format.quantity` (viewer °C/°F) |
| `velocitykmh` / `velocityms` | `wind_speed` | `kilometer_per_hour` / `meter_per_second` | `format.quantity` |
| `bytes` / `decbytes` | `data` | `byte` | `format.quantity` (KiB/MB by prefs) |
| `percent` / `percentunit` | `percent` | `percent` / `ratio` | `format.number` + `%` |
| `short` / `none` | — | — | `format.number` (no dimension) |
| `currencyUSD`, `custom:<suffix>` | — (passthrough) | — | `format.number` + static suffix |

Grafana units with no canonical dimension (currency, arbitrary custom suffixes) are a **passthrough display
unit**: `format.number(value, {decimals})` + the literal suffix — locale-correct number, no unit
conversion. The table lives in `units.ts` and is the single source of truth for the unit dropdown.

## Example flow

1. A user edits a timeseries panel bound to a `wind_speed` series (canonical `meter_per_second`, provenance
   from the `unit:`/`dimension:` tags — [`../../../tags/tags-scope.md`](../../../tags/tags-scope.md)). In
   the **Field** tab they pick unit `velocityms`, `decimals: 1`, and a threshold step red ≥ 15.
2. `dashboard.save` UPSERTs the cell with `fieldConfig.defaults = { unit:"velocityms", decimals:1,
   thresholds:{ mode:"absolute", steps:[{value:null,color:"green"},{value:15,color:"red"}] } }` — same
   verb, more defaulted fields.
3. **Viewer A (metric, es-ES)** opens the dashboard. For each point the renderer calls
   `format.quantity(12.0, "meter_per_second", "wind_speed", { decimals:1 })`; the host resolves A's prefs →
   metric → `icu4x` renders **`43,2 km/h`** (comma decimal, localized unit). The line is colored via
   `thresholds` against the **canonical** 12.0 m/s.
4. **Viewer B (imperial/marine, en-US)** opens the *same* panel. `format.quantity(12.0, …)` resolves B's
   prefs → **`23.3 kn`**. No second save, no stored string — the canonical 12.0 produced both.
5. B sets a value mapping `{type:"special", match:"null", result:{text:"—"}}`; null samples render `—` for B
   only after they re-save it as an override, or for everyone if set on `defaults`.

### Sequencing fallback (prefs not shipped yet)

`lb-prefs` (the `format.*` tools) is **planned, not shipped**
([`../../../prefs/user-prefs-scope.md`](../../../prefs/user-prefs-scope.md)). Until it lands, the renderer
falls back to **today's behavior**: canonical value + a static unit label (the `unit` id's display string)
+ a local `decimals` `toFixed`. Crucially, **the `fieldConfig` shape is built now** — so when `format.*`
appears, `format.ts` swaps the fallback for the real call **with no schema change and no re-save**. This
is the explicit sequencing decision (see Risks); it lets Phase 1 ship the full option *surface* immediately
while the formatter arrives behind the same field.

## Testing plan

Per [`../../../testing/testing-scope.md`](../../../testing/testing-scope.md) — real gateway/store, seeded
real rows, **no `*.fake.ts`**.

- **Additive round-trip:** a seeded v3 cell with a full `fieldConfig` (unit/decimals/min/max/thresholds/
  mappings/color/displayName) round-trips through `dashboard.save`/`get` identically; a v1/v2 cell with no
  `fieldConfig` still loads (field absent/defaulted).
- **The user-prefs bridge (the headline):** seed **two real prefs records** (ws viewer A = metric/es,
  B = imperial/en) and one canonical series; assert the **same** panel renders `43,2 km/h` for A and
  `23.3 kn` for B via `format.quantity` — proving one formatter, not a chart-local copy. (Once prefs ships;
  until then, assert the fallback renders canonical + static unit and that the call site is `format.*`-ready.)
- **No stored string:** assert the saved record holds the canonical number + `fieldConfig`, **never** a
  formatted string (a lint/test on the example path — the regression that would fork canonical-in/out).
- **Thresholds color, not alert:** a value over a step colors red; assert **no** rule/notification fires
  (that's the rules engine) — boundary test.
- **Mapping table:** every Grafana unit id in `units.ts` maps to a valid dimension/unit or an explicit
  passthrough; an unmapped id degrades to `format.number` + raw suffix, never throws.
- **Bounds:** a `fieldConfig` exceeding the override/mapping/threshold caps is rejected (or truncated
  honestly), not stored unbounded.
- **Capability deny:** save/get still gated; `format.*` is callable from the low/no-grant tier and reading
  another user's prefs to format is denied (prefs-scope wall).
- **Workspace isolation:** a ws-B viewer formats with ws-B prefs only; the `fieldConfig` of a ws-A
  dashboard is never readable cross-workspace.

## Risks & hard problems

- **Prefs sequencing (the load-bearing one).** `lb-prefs` ships after this. The fallback (canonical +
  static unit) must be honest and the call site `format.*`-shaped so the swap is data-only. A renderer that
  hard-codes `toFixed` + a baked unit string *outside* `format.ts` would fork canonical-in/out — lint the
  example path and keep all formatting in the one bridge file.
- **The unit mapping is where lossy import hides.** Grafana has hundreds of unit ids; ours map onto a closed
  dimension enum. Unmapped ids must degrade visibly (passthrough number + suffix), never silently render
  wrong (e.g. treating `velocitymph` as `km/h`). `uom` typing prevents cross-dimension nonsense once mapped;
  the table is the risk surface.
- **Overrides + matchers are deep.** Per-field, regex, frame-ref matchers compound fast. The phasing
  (Phase 1 defaults; Phase 2 `byName`/`byType`; defer `byRegex`/`byFrameRefID`) is the guardrail; the
  `overrides[]` cap keeps the record bounded.
- **Threshold/mapping color must theme.** Grafana stores semantic color names (`green`/`red`) and hex; map
  these onto our `ui-standards` token palette so dark/light mode and accessibility hold — don't paint raw
  hex blind.
- **`min`/`max` are in canonical units.** Bounds and thresholds are evaluated against the **canonical**
  value (before prefs conversion); only the *axis labels* are localized. Conflating the two miscolors every
  imperial viewer — assert it in the bridge test.

## Resolved decisions

No blocking open questions — these are the long-term answers the build follows.

- **Map Grafana's semantic color names to our `ui-standards` tokens; pass explicit hex through verbatim.**
  `green`/`red`/etc. resolve to theme tokens (dark/light + accessibility correct) in `color.ts`; a literal
  `#rrggbb` is honored as-is. Never paint raw named colors blind.
- **Resolve prefs once per dashboard view, not per render call.** `prefs.resolve` is cached and re-fetched
  on the "prefs changed" bus hint; `format.ts` reads the cached resolution, avoiding an MCP call per point.
- **Accept Grafana's dotted override ids (`custom.lineWidth`) verbatim** so import is a copy; resolve them
  against `FieldOptions` + the view's `custom` schema. The supported id set per view is documented in
  [`chart-types-scope.md`](chart-types-scope.md).
- **Ship explicit `decimals` only; defer `decimals: auto` (significant-digits).** `format.number` can grow
  a sig-fig option in [`prefs-scope`](../../../prefs/user-prefs-scope.md) later — it's a formatter feature,
  not a field-config shape change.

## Known gaps (the Field-tab baseline, 2026-07-07)

A real-gateway audit ([`debugging/frontend/field-tab-options-that-do-nothing.md`](../../../../debugging/frontend/field-tab-options-that-do-nothing.md);
regression net `ui/src/features/dashboard/views/fieldTabBaseline.gateway.test.tsx`, 24 green) classified
every registered Field-tab option LIVE (observable render change) vs DEAD (stored + round-tripped, but
no renderer reads it). The editor-parity phase registered Grafana's full surface to complete the
**editor**; the matching **render** work only landed for the standard bridge + a few graph styles, and
`registryRoundTrip.test.ts` checks only the de/serializer, not the renderer — so the gap was invisible to
the green suite. These are the named follow-ups (the UX-simplify session decides per option: implement
the render path, or remove the option from the per-view registry so the editor never offers what the
renderer won't draw):

- **`timeseries` DEAD (9):** `mappings` (TS never calls `applyMappings`), `links` (no drilldown render),
  `custom.lineInterpolation` (recharts hardcodes `monotone`), `custom.gradientMode`, `custom.showPoints`,
  `custom.spanNulls`, `custom.axisPlacement` (the axis is `hide`n), `custom.stacking.mode`,
  `custom.thresholdsStyle.mode`.
- **`timeseries` PARTIAL (1):** `color` scheme — only `fixed` mode renders; `palette-*`/`continuous-*`
  silently fall back to the accent token (`fieldconfig/color.ts`). Either implement or scope the
  `ColorSchemeEditor` mode list so the picker never offers a mode the resolver degrades.
- **`table` DEAD (4):** `custom.width`, `custom.align`, `custom.cellOptions.type`, `custom.filterable`
  (the renderer introspects columns but applies none of them).
- **Registry render-guard (the prevention):** extend `registryRoundTrip.test.ts`'s exhaustiveness
  pattern to RENDER behavior — a new `OptionDef` whose `views` includes V must be observed by a test
  that renders V and asserts the option changes the output, or it fails. So the next option registered
  to "complete the editor" cannot go dead silently again.

## Related

- [`README.md`](README.md) — the viz umbrella + the reconciliation table (`fieldConfig` row).
- [`panel-model-scope.md`](panel-model-scope.md) — declares `fieldConfig` on the cell and delegates the
  `FieldOptions`/`FieldOverride`/`Matcher` shapes here; owns the `overrides[]`/`mappings[]` caps.
- [`chart-types-scope.md`](chart-types-scope.md) — the per-view `custom` field options (lineWidth, axis…)
  and which override ids each view supports.
- [`transformations-scope.md`](transformations-scope.md) — transforms produce the fields a `fieldConfig`
  targets (a `displayName` override matches a transformed field).
- [`datasource-binding-scope.md`](datasource-binding-scope.md) — `Target.refId` (A,B,…) an override can
  match by (`byFrameRefID`, deferred).
- [`import-export-scope.md`](import-export-scope.md) — maps Grafana `fieldConfig` ↔ ours using the unit table.
- [`panel-editor-scope.md`](panel-editor-scope.md) — the **Field** and **Overrides** editor tabs that drive
  these shapes (one field-code path so add and edit never drift).
- [`../../../prefs/user-prefs-scope.md`](../../../prefs/user-prefs-scope.md) — **the formatter.**
  `format.datetime`/`format.number`/`format.quantity`/`convert.unit` + the closed dimension enum; the
  low/no-grant utility tier. This scope consumes it, never re-implements it.
- [`../../../tags/tags-scope.md`](../../../tags/tags-scope.md) — the `unit:`/`dimension:` tags that supply a
  field's canonical *source* unit (the `from_unit` for `format.quantity`).
- [`../../../rules/rules-engine-scope.md`](../../../rules/rules-engine-scope.md) — where threshold *alerts*
  live (here thresholds only color).
- The Grafana reference clone at `/tmp/grafana` — `kinds/dashboard/dashboard_kind.cue` (~721–830:
  `FieldConfigSource`/`FieldConfig`/`ValueMapping`/`ThresholdsConfig`/`FieldColor`/`MatcherConfig`).
- README **§3** (rules 2/5/6/7), **§3.7**/**§6.5** (MCP as the contract — the `format.*`/`convert.*` tools),
  **§6.11** (tags = unit provenance).

### One-responsibility-per-file plan (FILE-LAYOUT)

The implementation lands one concern per file under `ui/src/features/dashboard/fieldconfig/`:

- `units.ts` — the Grafana-unit-id → `(dimension, from_unit)` / passthrough **mapping table** + the picker list.
- `format.ts` — **the user-prefs bridge.** The only place that calls `format.quantity`/`format.number`/
  `format.datetime`; holds the sequencing fallback (canonical + static unit until `lb-prefs` ships).
- `thresholds.ts` — resolve a canonical value against `ThresholdsConfig.steps` → a color token.
- `mappings.ts` — apply `ValueMapping[]` (value/range/regex/special) → `{text,color,icon}`.
- `color.ts` — `FieldColor` modes (thresholds/fixed/palette/continuous) → token; Grafana-name → token map.
- `matchers.ts` — `Matcher` evaluation (`byName`/`byType` Phase 1) selecting which fields an override hits.

The **Field** / **Overrides** editor tabs that author these shapes live in
[`panel-editor-scope.md`](panel-editor-scope.md) (one field-code path, shared by add and edit).
