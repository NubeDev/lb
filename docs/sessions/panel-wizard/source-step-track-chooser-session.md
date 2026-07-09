# Session — Source step: three-track chooser

## Ask

The new-panel wizard's **Source** step (step 1) stacked all three source affordances
at once — the Insights card, the "Source" combobox, and the "Datasource" `<select>` —
which read as cluttered and unclear. User ask: turn it into a **3-icon chooser** where
the user clicks the track they want and only *that* track's controls appear, and make
the obvious complete choices advance the flow.

## What changed

`ui/src/features/panel-builder/wizard/SourceStep.tsx`

- **Three track cards** (`role="radiogroup"`, one `role="radio"` each), responsive
  `grid` (1 / 2 / 3 cols). Icons: `Lightbulb` (Insights), `ListTree` (Workspace
  source), `Database` (Datasource). The Datasource card only renders when the
  workspace has ≥1 federation datasource (mirrors the old conditional `<select>`).
- **Selected track is DERIVED from state** (no new persisted field, per the wizard's
  no-drift rule): `insights` ⇐ a sourceless view, `datasource` ⇐ a `federation.query`
  target, `workspace` ⇐ any other target. A transient `pickedTrack` covers the gap
  between clicking a card and binding within it. Back/Next preserves the choice for
  free because it re-derives.
- **Only the chosen track's chooser shows** below the cards — never all three at once.
- **Insights advances in one click.** `pickTrack("insights")` sets the sourceless view
  and calls the new `onAdvance` prop (wired in `PanelWizard.tsx` to step +1). Workspace
  and Datasource still bind a source, then the user clicks Next (a datasource needs a
  query authored first — advancing on bare selection would leave an empty query, so the
  honest completion is the normal Next once bound).
- Shared `picked:` readout for the query tracks (kept the `wizard source picked` label
  the tests assert).

`ui/src/features/panel-builder/wizard/PanelWizard.tsx`

- Pass `onAdvance` to `SourceStep` (advances to the next wizard step).

## Tests

Updated the five wizard gateway tests to the new flow (click the track card before its
control; Insights now auto-advances):

- `panelWizard.gateway.test.tsx`, `panelWizardSave.gateway.test.tsx`,
  `wizardPreviewModes.gateway.test.tsx` — click **source track workspace** before the
  `wizard source` combobox.
- `sourceStepDatasource.gateway.test.tsx` — click **source track datasource** before the
  `wizard datasource` select.
- `insightsPanelWizard.gateway.test.tsx` — click **source track insights** (one click
  lands on step 2; dropped the now-redundant Next).

Green:

```
pnpm exec vitest run --config vitest.gateway.config.ts \
  src/features/panel-builder/wizard/{panelWizard,insightsPanelWizard,sourceStepDatasource,panelWizardSave,wizardPreviewModes}.gateway.test.tsx
 Test Files  5 passed (5)
      Tests  16 passed (16)
```

`npx tsc --noEmit` clean. No new behavior branched on an extension id (Insights is a
generic sourceless *view*, not a named extension). No debugging entry — nothing broke
beyond the expected test-selector updates.
