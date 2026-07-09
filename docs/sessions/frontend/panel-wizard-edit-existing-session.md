# Session — the panel wizard edits an existing panel (dashboard cell → EDIT mode)

Date: 2026-07-09 · Area: frontend / dashboard / panel-wizard

## The ask

On the dashboard, each widget's hover affordance opened **Data Studio** ("Open in Data
Studio", the `ExternalLink` icon). The ask: make that affordance **edit the existing
panel** instead — open the stepped panel wizard **seeded from that cell**, and on Save
**replace the cell in place** (not append a new panel). "All needs to be as it was" — the
separate **New panel** button (the create flow) is untouched.

## What shipped

The wizard is a thin shell over `EditorState` (`cellToEditorState`/`editorStateToCell`),
so edit mode is a seed + a save-target change — no wizard-only state, no drift with the
editor (the panel-wizard load-bearing rule holds).

- `PanelWizard` — new optional `editCell?: Cell` prop. When set: seed state from
  `cellToEditorState(editCell)`; header reads "Edit panel"; Save serializes with the
  existing cell as the `editorStateToCell` base (keeps key + geometry) and **replaces**
  that cell in the dashboard's `cells[]` (matched by `i`) rather than appending.
  ([PanelWizard.tsx](../../../ui/src/features/panel-builder/wizard/PanelWizard.tsx))
- Route `/dashboards/$d/new-panel` — added an optional `?cell=<i>` search. `NewPanelRoute`
  loads the dashboard, finds the cell, and passes it as `editCell` (holds the wizard mount
  until the cell resolves so state seeds once). No `cell` ⇒ the create flow, unchanged.
  ([createAppRouter.tsx](../../../ui/src/features/routing/createAppRouter.tsx))
- `Grid` — the per-cell hover button changed from `onOpenInDataStudio()` (no-arg, →
  `/data-studio`) to `onEditPanel(i)` (→ `…/new-panel?cell=<i>`), `Pencil` icon, "Edit
  panel" title. ([Grid.tsx](../../../ui/src/features/dashboard/Grid.tsx))
- `DashboardView` — prop `onOpenInDataStudio` → `onEditPanel(dashboardId, cellId)`, passed
  down to the grid. The **New panel** button (`onOpenPanelWizard`) is unchanged.
  ([DashboardView.tsx](../../../ui/src/features/dashboard/DashboardView.tsx))

Reused the `dashboard.save` cap (the wizard's only cap) — no new verb, table, or bus
subject. This supersedes the panel-wizard scope's "No edit-flow change in Phase 1"
sequencing decision: the wizard is now the dashboard's edit entry point too.

## Tests (green)

- `panelWizardSave.gateway.test.tsx` — **new** `EDIT MODE` case: seed a cell at a fixed
  key + geometry (`w7`, `[3,5,6,4]`), open the wizard with `editCell`, walk to Save;
  assert the dashboard still has **one** cell, same key + geometry (replace-in-place, not
  append). 6 tests pass.
- `DashboardView.gateway.test.tsx` — the removal-regression case now asserts the placed
  cell carries an **`edit cell w1`** affordance (was "open cell w1 in data studio"). 11
  tests pass.

`cd ui && pnpm test:gateway <two files>` → 2 files, 17 tests passed. `tsc --noEmit` clean.

## Nothing broke

No debugging entry — no regression surfaced; the change is a seed + save-target swap over
the existing no-drift model.
