# Session: brand palette refresh (light/dark defaults → house teal + deep navy)

**Date:** 2026-07-05
**Area:** `ui/` theme system (frontend scope, theme-appearance scope)

## Ask

Improve the default light/dark colors. The house palette is the one the Professional
look carries: deep navy ink, clean paper, a deep-teal accent (teal brightened on dark).
Make that the out-of-box light/dark experience while keeping amber/blue as explicit
accent choices.

## What changed

- `ui/src/styles/globals.css`
  - `:root` (light): warm amber paper → clean cool paper (`--bg 210 20% 98.5%`), true-white
    panels, deep navy ink (`--fg 222 30% 16%`), default `--accent` is now the deep teal
    (`178 72% 27%`); `--accent-2` is the deep navy sibling; `--muted-bg`/`--panel-2`/`--overlay`
    retinted to the cool family.
  - `.dark`: neutral charcoal (hue 228) → deep navy near-black family (hue ~218, sat 14–22%),
    teal accent `174 62% 50%`, sky-blue `--accent-2`; dark shadows retinted to hue 220.
  - Added explicit `[data-theme-accent="amber"]` blocks (light + dark) so amber remains a
    first-class accent choice now that it is no longer the `:root` default.
- `ui/src/lib/theme/theme-options.ts`: `DEFAULT_THEME.preset` `"amber"` → `"teal"`.
- `ui/src/lib/theme/theme-looks.data.ts`: the `default` (Operator Console) look now defaults
  to the teal preset; blurb updated.
- `ui/src/lib/theme/theme-presets.data.ts`: the `slate` preset (Professional look) re-authored —
  light keeps clean paper but with deep navy ink and the deep-teal accent; dark is now a deep
  navy night mode (oklch hue ~252) with the teal accent, instead of a generic cool slate.
- `ui/src/lib/theme/contrast.test.ts`: the static amber mirror of globals.css replaced with a
  `BUILTIN_ACCENTS` map (amber/teal/blue share the base neutrals); added an AA suite over all
  three built-ins in both modes. This test is the lockstep guard for globals.css values.
- `ui/src/lib/theme/theme-storage.test.ts`: legacy-normalization expectation follows the new
  default preset.

## Contrast (verified)

All AA checks enforced by `contrast.test.ts` (fg-on-bg ≥4.5, fg-on-panel ≥4.5, accent-on-bg ≥3.0)
pass for every shipped look and all three built-in accents. Hand-checked ratios: light fg/bg 15.0,
muted/bg 5.9, teal/bg 5.2; dark fg/bg 15.7, muted/bg 7.8, teal/bg 9.7.

## Decisions

- Kept the accent-swap architecture (`data-theme-accent`) instead of adding a new preset: the
  teal simply became the `:root` default and amber moved to an explicit attribute block —
  zero component changes, existing prefs with `preset:"amber"` keep rendering amber.
- Dark ground is navy-tinted (not gray) so the surface itself carries the brand family; the
  alternative (neutral gray + teal accent only) was rejected as indistinguishable from the
  previous charcoal console.

## Follow-up in the same session: floating/inset sidebar alignment

The `floating`/`inset` sidebar variants inset the rail as an 8px-padded card, but the content
column (`SidebarInset`) stayed full-bleed — the page header's top edge and the status bar's
bottom edge overshot the rail card, so the two chrome bands never lined up. Fix: a cascade rule
in `globals.css` (`[data-slot="sidebar"][data-variant="floating"|"inset"] ~ [data-slot="sidebar-inset"]`)
gives the content column a matching 8px margin, radius, hairline border and shadow on desktop;
the rail-facing side keeps margin 0 because the rail container's own padding provides the gutter.
Mobile (Sheet rail) stays full-bleed. This mirrors upstream shadcn's `inset` treatment, done as a
sibling-selector cascade instead of peer-variant utility chains (deterministic, one place).

## Follow-up: agent dock header alignment

The dock's header used compact padding (`py-2`) while routed pages use `.page-header`
(`min-h-[3.75rem]`), so the two bottom hairlines sat at different heights across the split.
The dock header now uses the same band metrics (`min-h-[3.75rem]`, `bg-panel-2/80`, `py-2.5`).

## Follow-up: reusable roster rail (`RosterRail`)

Many surfaces (Dashboards, Rules, Flows, Ingest, Data) hand-roll the same inner-left list. The
dashboard roster — the most complete one (create, select, hover rename/delete, minimize) — was
extracted into `ui/src/components/app/roster.tsx` as `RosterRail<T>`: an `AppRail` plus the item
list, where every behavior is opt-in by prop (`onCreate` → header create field, `onCollapse` →
minimize control, `onRename` → hover pencil + inline editor, `onRemove` → hover trash). A `noun`
prop seeds all aria-labels so each surface keeps its own vocabulary. The destructive confirm stays
with the FEATURE (the rail hands the item back) because `components/` never imports `features/` —
`DashboardRoster` is now a thin adapter (slug, visibility badge, `ConfirmDestructive`) with an
unchanged public API. Rules adoption landed in the next follow-up; Flows/Ingest/Data adoption landed in
the "Follow-up: flows/ingest/data rails onto `RosterRail`" section below.

## Follow-up: rules rail onto `RosterRail`

`RuleRail` is now a thin adapter over the shared `RosterRail` (same pattern as `DashboardRoster`):
inline create field in the header (replacing the two-step "New rule" reveal — the gateway test's
create interaction was updated to the new labels), hover-reveal delete, and a NEW
`ConfirmDestructive` gate on delete (rules previously deleted with no confirm — a gap).

The minimize affordance is now part of the shared kit too: the collapsed thin strip was extracted
from `DashboardView` into `components/app/rail-collapsed.tsx` (`CollapsedRail`, noun-parameterized
aria-labels); `DashboardView` consumes it, and `RulesView` gained the same minimize/expand
(`railOpen` state + `onCollapse` on `RuleRail`) so both surfaces fold identically.

While verifying against the real gateway, `RulesView.gateway.test.tsx` turned out to be red on the
CLEAN tree: `<Reveal>`'s `useMotionPref` called the throwing `useTheme()`, crashing any page
rendered outside a `ThemeProvider`. Fixed (`useThemeOptional()?.theme ?? DEFAULT_THEME`), regression
test added, and the previously-open CommandPalette gateway failures (2026-07-04 entry) are resolved
by the same fix. Debug entry: `docs/debugging/frontend/reveal-crashes-outside-theme-provider.md`.

## Follow-up: flows/ingest/data rails onto `RosterRail`

The remaining three hand-rolled inner-left rails were converted onto the shared kit, so every
full-screen surface now folds to the identical strip and reads as one app.

- **Flows** (`features/flows/FlowRail.tsx`): rewritten as a thin adapter — the version tag (`v{n}`)
  rides the RosterItem `badge`, the timestamp id scheme (`flow-{ms}`) stays in the adapter's
  `flowId()` (the host's `blankFlow(id, name)` no longer derives the id), and delete now routes
  through a `ConfirmDestructive` gate (flows previously deleted with no confirm — the same gap rules
  had). Name-first create (the rail's inline field) replaces the old no-args "New" button: the typed
  title becomes the flow's name. No rename was wired (flows have no rename verb). `FlowsView` gained
  the `railOpen` toggle + `CollapsedRail`.
- **Ingest** (`features/ingest/SeriesRail.tsx`, new): the inline `<aside>` (search + new-series
  button + list) was split — the LIST + CREATE are now a `SeriesRail` adapter over the rail's inline
  "New series…" field, matching Dashboards/Rules/Flows where create lives in the rail header. Ingest's
  create is a multi-step schema wizard, so `onCreate(name)` opens the wizard pre-seeded with the typed
  name (`CreateSeriesWizard` gained an `initialName` prop) rather than inserting immediately — same
  name-first shape, the schema step still runs. Select + minimize round out the rail (no rename/delete
  from this surface). The series search filter stayed in the page header (the kit's header slot is the
  create field + minimize; search is an Ingest-specific list filter with no kit equivalent). The old
  first-run `EmptyState` CTA button was retired in favour of the rail's always-visible create field
  (the empty state is now a pointer message, same as Dashboard/Flows `AppEmptyState`).
- **Data** (`features/data/TableRail.tsx`, new): the inline `TablePicker` component was replaced by a
  `TableRail` adapter — the row count rides the `badge`, READ-ONLY so only select + minimize is wired
  (no create/rename/delete — the raw grid never edits). The old "N tables / M rows" rail-header
  summary was dropped (the per-table count badges + the header's selected-table metrics carry the
  info); `DataView` gained the `railOpen` toggle + `CollapsedRail`.

aria-label drift: the ingest rail item label changed from `select {series}` to `select series
{series}` (the kit's `select {noun} {id}` shape, noun="series"); `IngestView.gateway.test.tsx` was
updated to the new label (3 sites). The ingest create flow changed from an empty-state CTA button to
the rail's name-first inline field (type a name → "create series" → wizard pre-seeded);
`CreateSeriesWizard.gateway.test.tsx` was updated to drive the wizard through the rail field and drop
the now-redundant in-wizard name typing (the name carries over via `initialName`). The data rail label
is unchanged (`select table {table}` matched the prior `select table {table}`). No flows rail test
exists (the canvas gateway test exercises the api client + serialization, not the rail).

Known follow-up (out of scope here): the flow CANVAS header (`FlowCanvasHeader.tsx`) also has a
delete button with no confirm gate — that is a body affordance, not the rail, so it was left as-is;
it should route through `ConfirmDestructive` when touched. Ingest and Data still render the legacy
`.page-header` band (not `AppPage`); adopting `AppPage` is a separate header-migration that would
also clear their pre-existing raw-`<button>`/`<input>` lint warnings.

## Tests

`cd ui && pnpm test` → 105 files / 653 tests green (includes the new built-in AA suite).
Nothing broke during the session; no debugging entry needed.

### Re-run after the flows/ingest/data rail adoption

`cd ui && pnpm test` → 109 files / 672 tests green. `pnpm tsc --noEmit` → clean except the two
pre-existing errors called out in the task (`FlowsCanvas.gateway.test.ts`, `transformDebug.gateway.test.tsx`).
`pnpm test:gateway` for the affected surfaces → `IngestView` (4), `CreateSeriesWizard` (3), `DataView`
(3) all green against the real gateway. Nothing broke; no debugging entry needed.
