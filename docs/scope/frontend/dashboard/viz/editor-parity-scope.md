# Viz scope — panel-editor parity (close the gap between the shipped spine and a usable editor)

Status: **SHIPPED (all 7 steps)** — built 2026-07-03; see
[`../../../../sessions/frontend/dashboard-editor-parity-build-session.md`](../../../../sessions/frontend/dashboard-editor-parity-build-session.md).
Originally written 2026-07-03 after a hands-on review of the shipped editor.
Part of the [`viz/`](README.md) slice; this is the **Phase 3.5** the umbrella phasing is missing:
Phases 1–3 shipped the *spine* (panel model, renderers, `viz.query` + `lb-viz`, the one
`cell ↔ editorState` round-trip), but the **editing surface is a stub** — a real person setting up a
dashboard hits raw-JSON textareas, free-typed property ids, and option tabs with a tenth of Grafana's
controls. The goal stated in the umbrella ("the full Grafana option surface", "a user's Grafana
muscle-memory transfers") is **not met** by what renders today.

## The honest review (what a user actually sees, 2026-07-03)

The architecture is right and largely built. The editor UI on top of it was shipped at
"round-trips correctly" quality, not "a person can use this" quality. Concretely, per tab:

### Query tab — `tabs/QueryTab.tsx` (314 lines)

| Have | Missing vs Grafana |
|---|---|
| Datasource dropdown (surreal / series / federation / flows / ext widgets); SQL Builder⇄Code for surreal; raw SQL + Run for federation | **Single target only** (`targets[0]` everywhere) — no A/B/C queries, no add/duplicate/delete/reorder, no per-query hide toggle, despite the model being `targets[]`. No query options row (max data points / min interval / relative time). No query inspector. Native `<select>` for datasource + source (no search). |

### Transform tab — `tabs/TransformTab.tsx` (225 lines)

| Have | Missing vs Grafana |
|---|---|
| Add/remove/reorder(↑↓)/disable list over 11 backend-real transform ids | **Only 4 ids have typed editors** (limit, reduce single-calc, sortBy single-key, filterByValue action/match only — no condition rows!). The other 7 — **including `organize`, the most-used transform** — fall to a raw `Options (JSON)` textarea (the exact thing the user called out). No "add transformation" picker with descriptions (a bare `<select>`). No field-name suggestions from the actual query result. No per-transform input→output debug view. `filterByValue` can't even edit its conditions. |

### Panel options tab — `tabs/PanelOptionsTab.tsx` (53 lines) + `tabs/options/*` (~350 lines total for 7 views)

| Have | Missing vs Grafana |
|---|---|
| Per-view dispatch; timeseries legend (show/mode/placement/calcs) + tooltip mode; stat/gauge/bargauge/barchart/pie/table minimal editors | **No panel title/description/transparent-background editing in the editor** (Grafana's "Panel options" group). Timeseries: no graph styles here (line width, fill opacity, gradient, line style, interpolation, point size, stacking, connect nulls), no axis group (placement, label, soft min/max, log scale), no threshold display style. Table (25 lines!): no column width/alignment, no cell type (colored bg/gauge/sparkline), no column filters, no footer. Similar 80% gaps on every other view. |

### Field tab — `tabs/FieldTab.tsx` (138) + `ThresholdsEditor.tsx` (101)

| Have | Missing vs Grafana |
|---|---|
| displayName, unit (flat dropdown), decimals, min/max, noValue, thresholds steps, timeseries drawStyle | **No value-mappings editor** — even though the render path (`fieldconfig/mappings.ts`) already applies them; authored mappings are invisible-to-edit. **No color-scheme picker** (`fieldconfig/color.ts` exists too). No absolute⇄percentage thresholds mode toggle in the UI (the config field exists; the editor never exposes it), and step colors are a fixed 6-name dropdown. Unit dropdown is flat + unsearchable. No data links. |

### Overrides tab — `tabs/OverridesTab.tsx` (103 lines)

| Have | Missing vs Grafana |
|---|---|
| byName/byType matcher, one property per override | Matcher value is a **free-typed text input** — no dropdown of the actual result fields. Property is a **free-typed dotted id** (`"unit \| decimals \| custom.lineWidth"` as a placeholder!) + a raw string value — no property picker, no typed control per property, no multiple properties per override, no regex/byQuery matchers. This is JSON-editing with extra steps. |

### Cross-cutting

- **No options search implementation visible to the user doing real work** (OptionsSearch exists but
  filters little because most options don't exist).
- **No table-view toggle** on the preview to inspect the transformed frames.
- Native `<select>`/`<textarea>` everywhere ("no shadcn Select primitive yet" is noted in-code 10+
  times) — the missing primitive is now the single biggest UI-quality blocker and must be built first.

### Root cause (why it feels vibe-coded)

Each phase shipped its *contract* (model, round-trip, resolver) with the **minimum UI to prove the
contract**, and the "deeper per-property UI is a Phase-2 follow-up" notes (see `OverridesTab.tsx`
header) were never scheduled. The tests pin round-trip fidelity, not usability — so everything is
green while the experience is a config-file editor. The fix is not architectural; it is a dedicated
UI-completion phase with **"a person can do X without typing JSON"** as the exit gate.

## Goals (Phase 3.5 — editor parity)

Exit gate for the whole phase: **a user can build every editor-supported panel end to end without
ever seeing JSON, a free-typed property id, or a field name they must remember and retype.**

1. **Primitives first.** shadcn `Select` (searchable), `Textarea`, `Checkbox`/`Switch`, `Slider`,
   color swatch picker, and a `FieldNamePicker` fed by the live preview's resolved frame
   (`viz.query` already returns the fields — the editor must *offer* them, never make the user type
   them). Every in-code "no shadcn Select primitive yet" suppression is then burned down.
2. **Transform tab: a typed editor for all 11 shipped ids.** Headline: **Organize fields** = the
   Grafana row list (drag to reorder, eye to hide, inline rename) over the *actual* result fields.
   `filterByValue` gets real condition rows (field picker + operator + value). `groupBy` gets the
   per-field Group-by/Calculate/Ignore rows. `calculateField` gets mode + operand pickers.
   `joinByField`/`filterFieldsByName`/`convert`-style ids get their small typed forms. The JSON
   textarea remains **only** as the escape hatch for an *imported unsupported* id, labeled as such.
   Add-transform becomes a searchable picker with one-line descriptions. Per-step "show result"
   debug table (re-uses the table renderer over the step's frames — needs `viz.query` to optionally
   return per-step frames or a `stopAt` param; small additive verb change, flag it).
3. **Field tab completion**: value-mappings editor (value/range/special rows → text + color; the
   apply-side already ships), color-scheme picker, thresholds absolute⇄percentage + swatches,
   searchable/grouped unit picker, data links list.
4. **Overrides tab done right**: matcher dropdowns fed by real fields; regex + byQuery matchers;
   "add override property" = searchable picker of the same option registry the Field/Panel-options
   tabs use, each property rendering its **normal typed control** inline; multiple properties per
   override. (This requires the option registry below — it's what makes it cheap.)
5. **An option registry, one property per file.** The reason Grafana can render any property inside
   an override is that options are *registered* (id, label, group, editor control, default), not
   hand-placed in tabs. Adopt that: `editor/options/registry/` with one option definition per file;
   the Field tab, per-viz tabs, overrides property picker, and options search all render **from the
   registry**. This is also what makes options search actually useful.
6. **Per-viz options to everyday-parity** (from the registry): timeseries graph styles + axis +
   stacking + threshold display; table column width/alignment/cell-type/filters/footer; stat/gauge/
   bargauge/pie value-options + orientation + display modes per the Grafana checklist.
7. **Query tab: multiple targets.** A/B/C query rows (add/duplicate/delete/hide/reorder) over the
   existing `targets[]` model (the backend resolver already dispatches all targets); query-options
   row (max data points / min interval / relative time) wired to the resolver's existing bounds;
   table-view toggle on the preview.
8. **Panel options group**: title, description, transparent background — edited in the editor, not
   only on the grid.

## Non-goals

- No new transform ids in `lb-viz`, no import/export (Phase 4), no repeat-by-variable, no
  expressions, no mixed-datasource per-query, no viz suggestions engine. Named, not silent.
- No change to the save path, caps, or tenancy — this is frontend + one small additive `viz.query`
  option (per-step frames for the debug view).

## Sequencing (each step ships green + docs, per HOW-TO-CODE) — ALL SHIPPED

1. ✅ **Primitives + FieldNamePicker** — `components/ui/{combobox,checkbox,color-swatch}.tsx` +
   `editor/fields/*`; every "no shadcn Select/Textarea/Checkbox primitive yet" suppression burned down.
2. ✅ **Option registry** — `editor/options/` (types/registry/binding/Control + `defs/*` one group per
   file); Field tab refit onto it; value-mappings + color-scheme + data-links editors; thresholds mode
   toggle + swatches; searchable grouped unit picker.
3. ✅ **Transform editors for all 11 ids** — `editor/tabs/transforms/*`; organize (row list: reorder/
   hide/rename over real fields), filterByValue condition rows, groupBy per-field rows, calculateField
   mode+operands, small forms; searchable add picker; JSON only for imported unsupported ids.
4. ✅ **Overrides on the registry** — matcher value controls (byName picker / byType / byRegexp /
   byFrameRefID), "add property" picker over the registry, each property its typed control, multi-property.
   Wired `byRegexp` render (spelling aligned to the backend).
5. ✅ **Per-viz options to parity** — `options/defs/{table,timeseriesViz,singleStat}.ts` rendered via
   `VizOptions`; table column width/align/cell-type/filter/footer, timeseries stacking/threshold-display,
   stat/gauge/bargauge/pie value options.
6. ✅ **Query tab multi-target** — `QueryTargets` (A/B/C add/duplicate/delete/hide/reorder) + query
   options row (`Cell.queryOptions`) + table-view toggle on the preview.
7. ✅ **Per-step transform debug view** — the one additive backend flag: `lb-viz::transform_stepwise` +
   `viz.query` `panel.debug`/`stopAt` → `steps[]`; `useVizSteps` + `TransformDebug` in the Transform tab.

## Testing plan (per testing-scope; real gateway, no fakes)

- Round-trip tests extend to every new option (registry-driven: iterate the registry and assert
  `editorStateToCell(cellToEditorState(c)) ≡ c` for a cell exercising **every registered option** —
  new options can't dodge the test).
- **Usability gates as tests**: build an organize-fields config through the UI (rename+hide+reorder)
  against real seeded rows and assert the saved `transformations[]`; author a value mapping and
  assert the rendered stat text/color; author an override via the pickers and assert the resolved
  field config. No JSON typed anywhere in these tests.
- Field pickers are fed from a **real** `viz.query` result (seeded rows), never a hardcoded list.
- Mandatory capability-deny + workspace-isolation tests unchanged (no new verbs beyond the
  `viz.query` per-step flag, which inherits its existing cap).

## Risks

- **The registry refit is the big rock** — it touches every tab. Do it second (after primitives) so
  every subsequent step lands on it instead of adding more hand-placed fields to migrate later.
- File-size discipline: one option definition / one transform editor per file keeps rule 8.
- Field pickers depend on a preview having run; must degrade to a free-text input (clearly labeled)
  when no frames exist yet — never block authoring.

## Related

- [`README.md`](README.md) — umbrella (insert this as Phase 3.5 before import/export).
- [`panel-editor-scope.md`](panel-editor-scope.md) — the Phase-1 editor this completes.
- [`field-config-scope.md`](field-config-scope.md) · [`transformations-scope.md`](transformations-scope.md)
  · [`chart-types-scope.md`](chart-types-scope.md) — the semantics owners.
- The Grafana reference clone at `/tmp/grafana` — `public/app/features/dashboard-scene/panel-edit`,
  `packages/grafana-data/src/field` (option registry pattern),
  `public/app/features/transformers` (per-transform editor UIs).
