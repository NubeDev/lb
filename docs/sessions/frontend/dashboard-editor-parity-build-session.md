# Session — panel-editor parity build (Phase 3.5, 2026-07-03)

**Status:** shipped — all 7 steps of
[`editor-parity-scope.md`](../../scope/frontend/dashboard/viz/editor-parity-scope.md) built, each green
before the next, one commit per step.

**Ask.** Execute Phase 3.5 (panel-editor parity): close the gap between the shipped viz spine and a
usable editor. The exit gate: *a user can build every editor-supported panel end to end without ever
seeing JSON, a free-typed property id, or a field name they must remember and retype.*

## What shipped (per step, per commit)

1. **Primitives + FieldNamePicker.** New shadcn primitives — `Combobox` (searchable Select, grouped, with
   descriptions + `allowCustom`), `Checkbox`, `ColorSwatchPicker`. A `FieldNamePicker` fed by the live
   preview's REAL `viz.query` result fields (`editor/fields/*`, one `usePanelData` read in PanelEditor
   provides them via context), degrading to labeled free-text when no frames yet. Burned down **every**
   "no shadcn Select/Textarea/Checkbox primitive yet" suppression across the editor tabs +
   `reminders/ActionEditor`.
2. **Option registry.** `editor/options/` — each option is one `OptionDef`
   (id/label/group/control/default/scope/path), aggregated in `registry.ts`, one group per `defs/*` file.
   The Field tab renders ENTIRELY from the registry via `OptionGroups` + one `Control` renderer. New
   authoring surfaces that had **no editor** despite the render path already applying them: value
   mappings, color scheme, data links. Plus searchable+grouped unit picker, thresholds absolute⇄
   percentage toggle + swatches.
3. **Typed transform editors for all 11 ids.** `editor/tabs/transforms/*`. Headline: **OrganizeEditor** —
   the Grafana row list over the ACTUAL result fields (reorder/hide/inline-rename), closing the user's
   "Organize fields is a raw JSON textarea" complaint. filterByValue condition rows, groupBy per-field
   rows, calculateField mode+operand pickers, small forms for the rest; searchable add picker; JSON only
   for imported unsupported ids.
4. **Overrides on the registry.** Matcher value controls (byName real-field picker / byType / byRegexp /
   byFrameRefID query picker), "add override property" = searchable picker over the SAME registry, each
   property its normal typed `Control`, multiple properties per override. Aligned the matcher id to the
   backend (`byRegex`→`byRegexp`) and wired `byRegexp` render.
5. **Per-viz options to parity.** `options/defs/{table,timeseriesViz,singleStat}.ts` rendered via a
   `VizOptions` body composed with the bespoke chip editors. Table column width/align/cell-type/filter/
   footer; timeseries stacking + threshold display; stat/gauge/bargauge/pie value options.
6. **Query tab multi-target.** `QueryTargets` wraps the single-target QueryTab with A/B/C rows
   (add/duplicate/delete/hide/reorder) via a scoped-state adapter (QueryTab stays single-target). New
   `Cell.queryOptions` (max data points / min interval / relative time). Table-view toggle on the preview.
7. **Per-step transform debug view** — the one additive backend change. `lb-viz::transform_stepwise` +
   `Frame::truncate`; `viz.query` reads `panel.debug` (`true | {stopAt}`) → `steps[]` (input + one per
   applied step), same cap, no new verb. `useVizSteps` + `TransformDebug` render it behind a Transform-tab
   toggle.

## Key decisions (the "why")

- **Registry second (the big rock), by design.** Every later step (overrides property picker, per-viz
  options, options search) lands on the registry instead of adding more hand-placed fields to migrate.
  The registry-driven round-trip test iterates the WHOLE registry so a new option can't dodge coverage —
  it caught a real gap (`options.footer` missing from `cellEditorState` OWNED_OPTION_KEYS).
- **Scoped-state adapter for multi-target** (step 6): rather than rewrite QueryTab's ~300 lines to splice
  at an index, `QueryTargets` narrows `state.targets` to `[active]` and maps writes back — QueryTab and
  the SQL/Flows sections stay single-target and untouched.
- **`byRegex`→`byRegexp`**: the TS `Matcher.id` spelling didn't match the backend's (`rust/crates/viz`
  config.rs), so TS-authored regex overrides silently never matched server-side. Fixed the type + wired
  the client render fallback. Logged separately? No bug entry — caught before shipping, not a regression.
- **Pie `displayLabels` is NOT a registry option** — it's a `PieChartLabel[]` array, not a scalar; a
  scalar select would round-trip wrong. Kept bespoke; named as a registry non-goal, not a silent gap.
- **Debug is opt-in + inherits the viz.query cap** — no new verb, no new cap surface; the frame budget
  applies to snapshots too (defense in depth).

## Usability gates as tests (no JSON typed anywhere in these)

- Author a **value mapping** through the UI → the stat renders the MAPPED text (real gateway).
- Build an **organize** config (rename+hide+reorder) over REAL result fields → assert `transformations[]`.
- Author an **override** via the pickers over REAL fields → assert the resolved fieldConfig applies to
  that field only.
- **Multi-target** rows add/duplicate/hide/reorder/delete over a real source picker; query options
  round-trip.
- **Per-step debug** runs a real sortBy+limit pipeline stepwise through the real host.

## Green output

Frontend unit suite (`cd ui && pnpm test`):

```
Test Files  65 passed (65)
     Tests  422 passed (422)
```

Backend (`cd rust && cargo build --workspace` clean; `cargo test -p lb-viz`):

```
test result: ok. 53 passed; 0 failed; 0 ignored   (incl. 4 new transform_stepwise tests)
```

`lb-host` viz tests: `test result: ok. 1 passed; 0 filtered viz suites green`.

Gateway suite (`cd ui && pnpm test:gateway`, real spawned node): the editor-parity additions are green —
`fieldNamePicker` (2), `valueMappingUsability` (1), `organizeUsability` (1), `overridesUsability` (1),
`queryTargets` (2), `transformDebug` (3), and the full `DashboardView` file (8/8, incl. the updated
full-option-surface + the rename test whose assertion I fixed).

**Pre-existing red (NOT this slice), confirmed by reproducing against an earlier commit:**
`Telemetry`/`Studio` `AbortSignal` env issue and the `sqlSource` table-render assertion — neither
touches the editor/options/transform files this slice changed. (The two stale `DashboardView`
assertions in the editor's own area — the NavMenu `"tab Field"` label and the rename
`findByText`-on-multiples — were fixed here; see the debug entry.)

## Debugging

One entry: [`../../debugging/frontend/panel-editor-tab-label-stale-after-navmenu.md`](../../debugging/frontend/panel-editor-tab-label-stale-after-navmenu.md)
— a stale gateway assertion (`"tab Field"` → `"Field"` after the NavMenu migration) that predated this
slice; fixed so the Field-tab path is testable for the usability gates.

## Follow-ups (named, not silent)

- The `viz.query` query-options (`maxDataPoints`/`minInterval`/`relativeTime`) are forwarded on the panel
  but the backend resolver does not yet clamp to them — a backend follow-up (the frontend authoring + the
  wire field ship now).
- Pie `displayLabels` multi-toggle stays bespoke (registry non-goal).
- BarChart per-viz options were not expanded (not in the step-5 list).
