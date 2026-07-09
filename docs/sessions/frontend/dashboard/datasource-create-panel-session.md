# Session — "Create panel" from the Datasources query page

## The ask

From the Datasources detail page, after a user runs a query, give them a way to turn
that query into a dashboard panel. Because the panel wizard (`new-panel`) is normally
opened *from a dashboard*, it assumes a destination dashboard exists — but launched from
Datasources there is none. So the wizard's Save step must let the user pick the dashboard.

## What shipped

A one-way bridge from the Query workbench into the panel wizard, plus a Save-step
dashboard picker for the "no dashboard yet" launch.

### 1. Query workbench → "Create panel" action
`QueryWorkbench` gained an optional `onCreatePanel(sql)` seam. When set (federation
sources only — the surreal ad-hoc box has no datasource to bind), a **Create panel**
button appears in the run bar, enabled once a query has run. It hands back `run.lastSql`
(the real statement the engine saw — compiled SQL for PRQL).

`DatasourceDetail` wires it: it navigates to the wizard route under the `PICK_DASHBOARD`
sentinel id with `?ds=<source>&sql=<text>`.

### 2. Sentinel-id route (long-term choice)
Rather than a duplicate dashboard-less route, the existing single deep-linkable
`/dashboards/$d/new-panel` route is reused with a sentinel dashboard id `__pick__`
(`PICK_DASHBOARD`, in `wizard/steps.ts`). The route's `validateSearch` now also reads
`ds` + `sql`. `NewPanelRoute` derives `pickDashboard = dashboardId === PICK_DASHBOARD`
and passes a `prefill = { source, sql }` to the wizard. One route, no new persistence,
deep-linkable.

### 3. Wizard prefill + Save-step dashboard picker
`PanelWizard` seeds its `EditorState` from the prefill (a `federation.query` target,
table view, the SQL in code mode — mirrors `SourceStep.selectDatasource` + `adoptSql`,
so it lands already bound to the ran query). In `pickDashboard` mode it holds a
`pickedDashboard` and gates Save on it. `TransformStep` renders a "Save into" dashboard
`Select` (fed by `listDashboards()` — the same membership-filtered `dashboard.list` the
roster reads) next to the Save button. On save the panel lands in the chosen dashboard
and `onExit(landOn)` returns there so the user lands on that dashboard.

## Rule-10 note

No core chokepoint learned about a datasource. The source name + SQL are opaque config
threaded through the existing generic `federation.query` target shape and the generic
`new-panel` route. The sentinel is a UI-only routing token, not an extension branch.

## Tests

Added a PICK-mode case to `panelWizardSave.gateway.test.tsx`: launch the wizard with
`pickDashboard` + `prefill`, walk to Transform, assert Save is disabled until a dashboard
is chosen, pick "Beta", save, and confirm the panel lands in Beta (not Alpha) and
`onExit` reports `d-pick-b`. Also confirmed workspace-isolation + cap-gate cases still
green (existing tests in the same suite).

- `pnpm exec vitest run --config vitest.gateway.config.ts panelWizardSave.gateway` → 7/7 green.
- `pnpm exec tsc --noEmit` → clean.

(Reminder: `pnpm test:gateway` ignores a file-path filter — use the `--config` form to
scope to one file.)

## Files touched

- `ui/src/features/query-workbench/QueryWorkbench.tsx` — `onCreatePanel` seam + button
- `ui/src/features/datasources/DatasourceDetail.tsx` — wire the button to the wizard route
- `ui/src/features/panel-builder/wizard/steps.ts` — `PICK_DASHBOARD` sentinel
- `ui/src/features/routing/createAppRouter.tsx` — route search (`ds`/`sql`) + prefill/pick wiring
- `ui/src/features/panel-builder/wizard/PanelWizard.tsx` — prefill seed + pick-mode save target
- `ui/src/features/panel-builder/wizard/TransformStep.tsx` — Save-step dashboard picker
- `ui/src/features/panel-builder/wizard/panelWizardSave.gateway.test.tsx` — PICK-mode test
