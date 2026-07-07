# Viz scope — the panel wizard (stepped create, preview-per-option, one engine with the Field tab)

Status: **scope (the ask).** Part of the [`viz/`](README.md) slice — the **create-flow** companion to
[`panel-editor-scope.md`](panel-editor-scope.md). Promotes to
[`public/frontend/dashboard.md`](../../../../public/frontend/dashboard.md) as it ships.

One paragraph: a **stepped wizard** for building a panel — pick a source → pick a chart type → step
through a SMALL set of options, each showing a **live preview of its effect** on the user's own data — so
the "Field tab is overwhelming and half of it does nothing" complaint
([`debugging/frontend/field-tab-options-that-do-nothing.md`](../../../../debugging/frontend/field-tab-options-that-do-nothing.md))
is solved by *showing*, not by listing options. The headline is not the steps, it is **preview-per-option**:
every option row carries its own live mini-render, so a live option's effect is immediate and a dead
option's absence is self-revealing. The wizard is **a thin presentation shell over the EXISTING panel
model** (`cellEditorState` + `writeOption` + `usePanelData`/`useVizQuery` + the shipped `viz.query`
fetch/shape split) — NOT a second authoring surface — so add and edit can never drift (the hard-won
guarantee of [`panel-editor-scope.md`](panel-editor-scope.md)). And the engine built here — **simplified
option sections + preview-per-option** — is the same engine that later **ports back into the editor's
Field tab** to replace its dead-option rows, so the edit path gets fixed with zero new architecture.

## Goals

- **Preview-per-option.** Every option the wizard exposes renders a live mini-preview of its effect on the
  user's actual data (or a real seeded sample before a source is picked). This is the single weapon against
  "I don't know what works": live options show their effect instantly; dead options show nothing — surfacing
  the [`fieldTabBaseline`](../../../../../ui/src/features/dashboard/views/fieldTabBaseline.gateway.test.tsx)
  findings to the user, not just to us.
- **A stepped create flow.** Pick source → pick chart type → walk the option sections (Standard → Graph
  styles → Thresholds/Mappings as applicable per view). Each step is small; the panel preview is always
  visible. A "skip to section" nav keeps it non-linear when the user knows what they want.
- **Reuse the panel model — do NOT build a second authoring surface.** The wizard's working state IS
  `EditorState` (`cellEditorState.ts`); every option writes through `writeOption`; the preview resolves
  through `usePanelData` + the `formatValue`/`applyMappings`/`thresholdColor` bridge — the same path the
  editor and the renderer use. Drift between wizard and editor is **impossible by construction**, not by
  discipline.
- **Isolation from Data Studio until proven.** The wizard ships as a **new entry point**, not a rewrite of
  the existing `PanelEditor`. The current editor and Field tab are **untouched** until the wizard is
  validated; data studio stays clean. Once the wizard is right, the simplified-option-sections engine
  ports back into the editor's Field tab (Phase 2).
- **Cheap previews.** Presentation options (unit/decimals/thresholds/color/mappings/draw style) re-shape
  **cached frames** through the shipped `viz.query` fetch/shape split
  ([`../data-studio-ux-scope.md`](../data-studio-ux-scope.md) "edit-without-requery") — **no backend hit**
  per toggle. Only the data steps (source, chart type, transformations) re-query.
- **A one-time concepts tour** ([react-joyride](https://github.com/gilbarbara/react-joyride)) for the
  *naming* layer ("a threshold colors values above a limit"), dismissible. Live preview carries the
  *showing*; joyride carries the *concepts*. If an option needs more than a one-line tour, it is too
  complex for the simple track and belongs in an "Advanced" drawer.

## Non-goals

- **No new option semantics.** What each option means/renders is owned by
  [`field-config-scope.md`](field-config-scope.md) + [`chart-types-scope.md`](chart-types-scope.md). This
  scope only *arranges and previews* them.
- **No new backend.** No new MCP verb, capability, datastore, or bus subject. The wizard consumes the
  shipped `viz.query` (incl. its inline-`frames` shape mode), `dashboard.save`/`get`, and the seeded-sample
  `/_seed/*` test routes (in dev/test only).
- **No edit-flow change in Phase 1.** Editing an existing panel still opens the current `PanelEditor`.
  The wizard is the **create** entry point in Phase 1; the simplified sections reach the edit flow in
  Phase 2 (the port-back). This is the explicit sequencing decision (see Risks).
- **No dead-option pruning upfront.** We do NOT remove dead options from the registry before the wizard.
  The preview-per-option surface surfaces them empirically; per-option fate (implement the render path vs
  drop from the per-view registry) is decided during/after wizard validation, against
  [`fieldTabBaseline`](../../../../../ui/src/features/dashboard/views/fieldTabBaseline.gateway.test.tsx).
- **No fakes.** Previews never use hand-authored sample data (rule 9 / testing §0). They use the user's
  real query results once a source is picked, and **real seeded rows** for the pre-source illustrations
  (see Intent).
- **No import/export, no library-panel ref cells, no variables.** Those are sibling scopes; the wizard
  authors one ordinary cell.

## Intent / approach

**Two ideas, load-bearing together.**

**1. Preview-per-option, not a long form.** The Field tab today is a vertical list of controls with a
single shared preview at the top. The wizard inverts that: each option SECTION is a small card whose left
half is the option's controls and whose right half is a **mini-preview isolating that option's effect** —
the user's chart, re-rendered with the option on/off or at its current value. Toggling `decimals: 2` shows
the value readout snap to two digits immediately; toggling `custom.spanNulls` shows *nothing change* —
which is the honest signal it's dead, exactly as the baseline test proves. The full panel preview stays
pinned beside the steps so the cumulative effect is always visible. This is the design answer to the
complaint: instead of explaining 25 options, show each one working.

**2. One engine, two surfaces (create wizard + edit Field tab).** The simplified option section + its
preview row is a **reusable component** (`OptionSectionCard`), not wizard-specific. The wizard composes
these into steps; Phase 2 composes the SAME cards into the editor's Field tab, replacing today's flat
`OptionGroups` list. Because both surfaces read/write through `writeOption` + render through the same
`formatValue` bridge, a Field-tab option and its wizard twin behave identically — the no-drift guarantee
is structural.

**The preview's data:** before a source is picked, each option's mini-preview renders a **real seeded
sample** (`seedIotDemo`-style: a tiny `cooler.temp`/`fryer.state`-style series the gateway already seeds
for demos) — not a hand-written fake, a canned real row. Once the user picks a source, the mini-preview
switches to **their actual frames**, cached from the step-1 query. The full panel preview always uses the
real frames.

**The preview's cost model (the user's backend concern, answered):**
- **Data steps** (source / chart type / transformations) → re-query via `viz.query`. Debounced; reuses the
  shipped `usePanelData`/`useVizQuery` throttle. Transformations ARE backend (the `lb-viz` pipeline runs
  server-side); this is the existing path, not new.
- **Presentation steps** (everything in the Field tab) → re-shape **cached frames** through `viz.query`'s
  inline-`frames` shape mode (the fetch/shape split shipped in
  [`../data-studio-ux-scope.md`](../data-studio-ux-scope.md)). No backend round-trip per toggle — formatting
  is a pure function over frames (`formatValue`), so 80% of options are free.

**Rejected alternative — a bespoke wizard state machine.** We considered a wizard that mints its own panel-
in-progress shape (steps → wizardState → cell on finish). Rejected: it is exactly the second authoring
surface that caused the original "add ≠ edit" drift ([`panel-editor-scope.md`](panel-editor-scope.md),
"Rejected alternative — patch the existing add-form + edit-drawer"). The wizard's state IS `EditorState`;
finishing a step is `writeOption`; finishing the wizard is `editorStateToCell`. The round-trip identity test
extends to cover `editorStateToCell(wizardState) ≡ editorStateToCell(cellToEditorState(c))`.

**Rejected alternative — prune dead options before building.** Rejected: the preview-per-option surface is
a better pruning instrument than up-front debate. A dead option that visibly does nothing in the wizard is
its own argument for removal (or for implementing its render path). Build the surface, let it speak, then
decide per option against the baseline.

## How it fits the core

- **Tenancy / isolation (rule 6):** unchanged. The wizard reads/writes the workspace-scoped `dashboard:{id}`
  cell via the existing verbs; the seeded preview samples are workspace-scoped rows. A ws-B wizard never
  sees ws-A samples or sources.
- **Capabilities (rule 5/7):** **none added.** Saving the finished panel reuses `mcp:dashboard.save:call`
  (the editor's existing gate); the host re-checks it on save. The live preview reuses `mcp:viz.query:call`
  (composed with each target's own cap under `caller ∩ grant`). A viewer without `dashboard.save` sees no
  wizard entry point.
- **Placement (rule 1):** one wizard, two transports (Tauri `invoke` / gateway SSE+HTTP). No `if cloud`.
- **MCP surface (§6.5):** **none added.** Consumes `viz.query` (incl. the inline-`frames` shape mode),
  `dashboard.save`/`get`, and the seeded sample routes (dev/test only). The wizard is a frontend-only
  shell over verbs that exist.
- **Data (SurrealDB, rule 2):** unchanged — one cell record, authored additively via the existing
  `dashboard.save` UPSERT. No new table, no new field (the cell shape is the shipped v3).
- **Bus (Zenoh):** unchanged. Live preview samples ride the shipped series/bus SSE; the cell is state.
- **Sync / authority:** unchanged — additive UPSERT, idempotent replay.
- **Secrets:** none.
- **SDK/WIT impact:** none. The wizard consumes the shipped v3 cell contract; it defines no new contract.
- **One responsibility per file (rule 8):** the wizard lands one concern per file under
  `ui/src/features/panel-builder/wizard/` — the route shell, one step per file (`SourceStep.tsx`,
  `ChartTypeStep.tsx`, `OptionsStep.tsx`, `TransformStep.tsx`), `useWizardPreview.ts` (the debounced
  re-query-vs-re-shape hook), and `seedSample.ts` (the dev/test real-sample helper for pre-source
  previews). The shared engine lives under `panel-builder/options/` (sibling to `OptionGroups.tsx`) so the
  Field-tab port-back reuses it: `OptionSectionCard.tsx` (control + label + live/dead note), the one
  `OptionFocusPreview.tsx` (the configurable `WidgetView`-rendering preview with its `optionFocus` prop),
  and `optionLiveness.ts` (the declared LIVE/DEAD table the baseline test enforces).
- **Skill doc:** **N/A.** The wizard is a human UI surface; the agent-drivable paths (`dashboard.catalog`,
  `dashboard.save`, `viz.query`) already have their skills. No new drivable surface.

## Example flow

1. The user clicks **Add panel** → the wizard opens at a **dedicated route** (e.g. `/t/$ws/dashboard/$d/new-panel`),
   NOT the existing editor's Sheet. Step 1: **Source** — the searchable source picker (reused from the Query
   tab) over the workspace's ws-scoped sources. The full-panel preview beside it shows "pick a source to
   preview"; the per-option cards are hidden until a source is chosen.
2. They pick `series.read: cooler.temp`. The wizard runs `viz.query` once; the preview shows the real
   series. Step 2: **Chart type** — the VizPicker, filtered to views valid for the result shape (reused
   from the editor). Picking `timeseries` advances; the preview re-renders as a timeseries (same cached
   frames, new view).
3. Step 3: **Options** — a stack of `OptionSectionCard`s, one per option group the view supports (Standard
   → Graph styles → Thresholds). Each card: the control on the left, a **mini-preview isolating that
   option's effect** on the right (rendered from the cached frames, no re-query). react-joyride fires once
   on first entry ("these cards each preview one option — toggle to see its effect"), then is dismissed.
4. They open the **Decimals** card, set `2`; the mini-preview's value readout snaps to `42.00` instantly
   (a re-shape of cached frames — no backend call). They open **Connect nulls** (`custom.spanNulls`),
   toggle it; the mini-preview does not change — the honest signal it is dead today (the baseline test's
   finding, surfaced by the surface itself). They leave it off.
5. They open **Thresholds**, add a red step at 30; the mini-preview's line turns red (re-shape, no
   re-query — threshold color is a pure function over the canonical value).
6. They click **Transform** (a data step) and add `reduce: max`. This DOES re-query (`viz.query` runs the
   pipeline server-side); the preview + all mini-previews refresh against the new frames after the debounce.
7. **Save** → `editorStateToCell(state, defaultCell)` emits the v3 cell → `dashboard.save` UPSERTs it. The
   host re-checks `dashboard.save`. The panel lands on the dashboard.
8. (Phase 2) The user later clicks **Edit** on that panel → the editor opens → the Field tab now renders
   the SAME `OptionSectionCard`s the wizard used (the port-back), each still with its mini-preview. The
   two surfaces are one engine; nothing has drifted.

## Testing plan

Per [`../../../testing/testing-scope.md`](../../../testing/testing-scope.md) — real gateway, real seeded
rows, **no `*.fake.ts`**. Frontend tests are `*.gateway.test.tsx`.

- **No-drift invariant (the headline).** In one real-gateway test: build a panel through the WIZARD's
  state (`writeOption` per step), and through the EDITOR's state for the same options; assert both
  serialize to the SAME cell: `editorStateToCell(wizardState) ≡ editorStateToCell(editorState)`. This is
  the regression test for "the wizard is a second surface that drifts" — it cannot, by construction.
- **Preview-per-option re-renders.** For a LIVE option (e.g. `decimals`), toggling the option changes its
  mini-preview's rendered DOM (assert via the same `plainDom` approach as the baseline test). For a DEAD
  option (e.g. `custom.spanNulls`), toggling leaves the mini-preview byte-identical — reusing the
  baseline's classification as the wizard's contract.
- **Re-shape vs re-query (the cost model).** Assert a presentation-option toggle (decimals/threshold) does
  NOT trigger a second `viz.query` call (count gateway calls); a data-step change (transform/chart-type)
  DOES. This pins the "preview-per-option is cheap" goal.
- **Real samples, no fakes.** The pre-source mini-previews render REAL seeded rows (assert the seeded
  `cooler.temp` value appears, not a hand-written literal). Rule 9.
- **Edit-cap gate + host backstop.** A viewer without `mcp:dashboard.save:call` sees no wizard entry
  point; a forced `dashboard.save` for that identity is denied by the host (opaque) — the backstop holds.
- **Workspace isolation.** A ws-B wizard's source picker + seeded previews contain only ws-B rows; the
  wizard can never resolve a ws-A source (two-session test).
- **Baseline stays green.** The shipped
  [`fieldTabBaseline`](../../../../../ui/src/features/dashboard/views/fieldTabBaseline.gateway.test.tsx)
  must stay green throughout — the wizard neither implements nor removes any option until port-back, so the
  dead-option classifications must not change.

## Risks & hard problems

- **The wizard becomes a second surface (the load-bearing risk).** Every temptation to "just add a
  wizard-specific field" is the drift seed. Mitigation: wizard state IS `EditorState`; the no-drift
  invariant test is mandatory; any option the wizard exposes must be a registered `OptionDef` written via
  `writeOption` — never a bespoke field. If a wizard step needs state the editor lacks, that is a finding
  (extend `EditorState` for BOTH), not a wizard-only addition.
- **Isolation discipline (the user's explicit ask).** The wizard must not perturb the existing editor or
  data studio while being validated. Mitigation: a new entry point + route; zero edits to `PanelEditor.tsx`
  / `FieldTab.tsx` / `OptionGroups.tsx` in Phase 1; the shared `OptionSectionCard` is ADDITIVE under
  `panel-builder/options/`. The port-back (Phase 2) is a separate, later change.
- **Preview cost is still real for data steps.** A transformation-heavy panel re-queries on each transform
  edit. Mitigation: debounce + the shipped `viz.query` throttle + the freeze-current-data toggle
  ([`../data-studio-ux-scope.md`](../data-studio-ux-scope.md)) is offered in the wizard's Transform step.
- **Dead options in the preview confuse users.** A toggle that does nothing looks broken. Mitigation:
  during validation, a dead option's card shows an honest "no visible effect (renderer pending)" note
  rather than silent nothing — driven by a per-option `live` flag sourced from the baseline test's
  classification. After port-back, dead options are removed/implemented, not annotated.
- **react-joyride weight.** A heavy tour is its own overwhelm. Mitigation: one dismissible pass; the live
  preview is the primary teacher.
- **Step friction.** A strictly linear wizard penalizes the user who knows what they want. Mitigation: a
  "skip to section" nav makes the steps addressable, not forced.

## Open questions

None — the long-term-best call is taken for each (see Resolved decisions).

## Resolved decisions

- **The wizard is create-only in Phase 1; edit keeps the current editor until port-back.** The user's
  explicit call: keep data studio clean until the wizard idea is validated. The simplified-sections engine
  reaches the edit flow in Phase 2.
- **Dedicated route entry point (not a Sheet).** `/t/$ws/dashboard/$d/new-panel` via the shipped
  [@tanstack/react-router](../../routing-scope.md) setup. Long-term-best: deep-linkable, refresh-safe
  mid-author, shareable ("here's a half-built panel"), and MAXIMALLY isolated from data studio (a new route
  cannot perturb the editor's Sheet mount). It also sets up the eventual edit deep-link (`?edit=<cellId>`,
  deferred in [`panel-editor-scope.md`](panel-editor-scope.md)) on the same routing seam.
- **One configurable preview component with an `optionFocus` prop (not per-option renderers).** A per-option
  `<PreviewFragment>` family would drift from the real panel render — the exact disease this scope cures.
  One `OptionFocusPreview` component renders the SAME `WidgetView` the dashboard renders, with an
  `optionFocus: { optionId }` prop that zooms/highlights the region that option affects (the value readout
  for `decimals`, the line color for `thresholds`, …). No drift is possible — there is one render path.
- **A declared `optionLiveness.ts` table is the wizard's source of truth; the baseline test ENFORCES it.**
  The wizard's dead-option annotation ("no visible effect — renderer pending") reads a declared per-option
  `live: boolean` table — NOT hand-maintained freehand: the `fieldTabBaseline.gateway.test.tsx` suite
  asserts (a) every registered option has a table entry (exhaustiveness) and (b) each entry matches the
  rendered reality (a row claiming LIVE must observably render; a row claiming DEAD must render
  byte-identical). Implementing a dead option or removing one → update the table → the test guides it. This
  mirrors the project's house pattern (the radius-scale guard, the registry round-trip): declare + test,
  never declare-only. Rejected: build-time codegen from test output — a fragile test→codegen→import pipeline
  for no robustness gain over a tested declaration.
- **Real seeded samples for pre-source previews; the user's real frames after.** No fakes (rule 9).
- **react-joyride for the concepts tour; the live preview is the primary teacher.** A tour is for naming,
  not for explaining the option's effect.
- **The preview-per-option surface IS the dead-option pruning instrument.** Do not prune upfront; let the
  surface surface them, then decide per option.

## Related

- [`README.md`](README.md) — the viz umbrella + phasing (this is a new Phase alongside the editor).
- [`panel-editor-scope.md`](panel-editor-scope.md) — the editor whose state machine + (de)serializer +
  `writeOption`/option registry the wizard REUSES (no-drift invariant).
- [`field-config-scope.md`](field-config-scope.md) — the option semantics the wizard arranges + previews;
  the "Known gaps" section is the dead-option input.
- [`chart-types-scope.md`](chart-types-scope.md) — the VizPicker (reused) + per-view result-shape validity.
- [`../data-studio-ux-scope.md`](../data-studio-ux-scope.md) — the **shipped** `viz.query` fetch/shape
  split + freeze-current-data the wizard's cheap previews ride.
- [`debugging/frontend/field-tab-options-that-do-nothing.md`](../../../../debugging/frontend/field-tab-options-that-do-nothing.md)
  — the baseline audit; the wizard's preview-per-option makes its findings visible to the user.
- [`../../../../../ui/src/features/dashboard/views/fieldTabBaseline.gateway.test.tsx`](../../../../../ui/src/features/dashboard/views/fieldTabBaseline.gateway.test.tsx)
  — the LIVE/DEAD classification contract the wizard's per-option previews + `live` flag are sourced from.
- [`../../../prefs/user-prefs-scope.md`](../../../prefs/user-prefs-scope.md) — the formatter the preview
  resolves through (`format.quantity`/`format.number`/`format.datetime`).
- README **§3** (rules 1/2/5/6/7/8), **§6.1**, **§6.13**.
