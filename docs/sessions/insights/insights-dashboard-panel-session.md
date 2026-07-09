# Session — the Insights dashboard panel (new-panel wizard + read-only)

Date: 2026-07-09. Scope: `docs/scope/insights/insights-package-scope.md`. Follows the
`insights-package-extraction-session.md` (which built `@nube/insights`).

## Ask

Make a dashboard widget/panel from `@nube/insights`, available on the **new-panel** page. For step 2,
offer a **read-only** option. End-user UX, done well.

## Decision (asked)

One panel type + a **Read only toggle** (default ON), not two separate panel types. Off ⇒ inline
Ack / Resolve / Dismiss.

## What shipped

A first-class `view:"insights"` dashboard panel, buildable from the new-panel wizard:

- **View + catalog:** `"insights"` added to the `View` union (`lib/dashboard/dashboard.types.ts`), the
  host `widget_catalog.json` (`kind:"read"`, `data:false`, `action:true`), and the `WidgetView`
  render switch — the renderer↔catalog↔union consistency test stays green (18 views).
- **Render:** `features/dashboard/views/insights/InsightsView.tsx` mounts `@nube/insights`'s
  `InsightsWidget` over the shell's `insightsClient`; `options.ts` folds `options.insights` into the
  widget's `filter` + `interactive` + `showRefresh`.
- **Picker:** added to `VizPicker` (wizard step 2) + `VizGallery` (stacked builder), not shape-gated.
- **Sourceless wizard path:** `SOURCELESS_VIEWS` (panel-kit) + a Source-step "no data source needed"
  Insights affordance + a relaxed `canAdvance` gate — the user picks Insights without binding a query.
- **Step 2 read-only:** `wizard/InsightsBasics.tsx` — the Read-only toggle (the headline choice) +
  status/severity, right where the view is picked.
- **Step 3 options:** `options/defs/insights.ts` (readOnly / showRefresh / status / severity / limit),
  with a new `OptionDef.excludeViews` so the fieldConfig-less list opts out of the universal standard
  options (unit/decimals/thresholds are noise for a list). Liveness rows added (`optionLiveness.ts`).

## Tests (all green, real gateway — no mocks)

- Unit: `views/insights/InsightsView.test.tsx` (options folding + read-only vs interactive), updated
  `VizGallery.test.tsx` (10 cards), options round-trip + catalog consistency.
- Gateway (real node): `wizard/insightsPanelWizard.gateway.test.tsx` — the full flow (pick sourceless
  Insights → toggle interactive on step 2 → Save → persisted `insights` cell with `readOnly:false`) +
  **workspace isolation** (ws-B panel never crosses into ws-A). Existing `panelWizard*`,
  `panelEditor`, `optionsStep`, and `fieldTabBaseline` gateway suites still pass (the last needed
  insights excluded from its field-render iteration — it has no fieldConfig).
- `cargo check -p lb-host` clean (catalog JSON is `include_str!`'d).
- Full UI `tsc --noEmit` clean.

## Follow-ups

Packaged detail-drawer preset (open a row → `useInsight`), and migrating the standalone Insights
*page* to consume the package widgets (still on its shadcn components). See the scope.
