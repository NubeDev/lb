# Session — Data Studio rail cleanup (RosterRail-kit alignment)

**Date:** 2026-07-05 · **Area:** frontend / data-studio · **Follow-on to:** the roster-rail kit
rollout in [brand-palette-refresh-session.md](brand-palette-refresh-session.md) (Dashboards, Rules,
Flows, Ingest, Data all moved onto `components/app/{roster,rail-collapsed}.tsx`).

## The ask

The Data Studio's UX read "wild and messy" next to the cleaned-up surfaces: its Sources/Library
panels lived in a **FlexLayout left BORDER dock** — a rotated vertical tab strip with hand-rolled
list styling — nothing like the `AppRail`/`RosterRail` chrome every other surface now shares, and
with no minimize/expand affordance.

## What shipped

- **`features/data-studio/StudioRail.tsx` (new)** — the studio's left rail on the shared `AppRail`
  chrome: a Sources/Library segmented tab row in the header + the kit's minimize button
  (`minimize studio rail`). Tab bodies stay in `panes/` (one responsibility per file).
- **`panes/SourcesPane.tsx` / `panes/LibraryPane.tsx`** — now rail-tab bodies (the rail owns
  padding/scroll). The library list items adopt the RosterRail item look: ghost button, leading
  icon, truncated title, trailing view badge; roster-style dashed empty state; error uses the
  `text-danger` token (was raw `text-red-500`).
- **`DataStudioView.tsx`** — hosts `railOpen` state and renders `StudioRail` or the shared
  `CollapsedRail` (`noun="studio"`), same as `RulesView`/`DashboardView`. The FlexLayout dock now
  holds ONLY builder tabs.
- **`workbenchModel.ts`** — the default model loses the left border; `PaneKind` narrows to
  `"builder"`.
- **`useWorkbenchLayout.ts`** — persisted v2 layouts carried the Sources/Library border tabs;
  `modelFrom` now strips `borders` on load (center tabsets incl. every builder draft restore
  untouched), so an old saved layout can't resurrect the dock strip or render "Unknown pane".
- **`panes/BuilderTabPane.tsx`** — the saved-as strip gained `aria-label="saved as"` (see debugging).

Rejected alternative: keeping Sources/Library as border-dock tabs and only restyling them. The
border strip is the inconsistency — every other surface's roster is shell chrome with one minimize
affordance; a second docking vocabulary for the same job is exactly the drift the kit exists to stop.
(Losing "drag Sources into the center" is no real loss — those panes were pinned `enableClose:false`
anyway.)

## Follow-up in-session: dock tab legibility

The FlexLayout tab strip was near-invisible (bare text, selected ≈ unselected) and a tab grew as
wide as its title (a long source label ate the whole strip). `datastudio-dock.css` now shapes tabs
as bordered pills — selected raised on `--panel` with an accent top edge, unselected muted until
hover — and caps titles at 11rem with ellipsis. FlexLayout's hover tooltip reads `helpText` (not
`name`), so `builderTabJson` now sets `helpText: name` — a truncated title shows in full on hover.

## Tests (real gateway, no mocks)

`pnpm test:gateway src/features/data-studio/DataStudio.gateway.test.tsx` → **7/7 green**, including
a NEW minimize/expand case (rail folds to the shared collapsed strip; expand restores the picker).
The library-open test now switches the rail's Library tab instead of clicking a
`.flexlayout__border_button`. Workspace-isolation + capability-deny cases unchanged and green.
`pnpm test` → 109 files / 672 green. `pnpm tsc --noEmit` → clean except the two pre-existing
`FlowsCanvas.gateway` / `transformDebug.gateway` errors.

## Debugging

Two Data Studio gateway tests were **already red on the clean tree** (verified via `git stash`):
builder-UI drift had broken them (an ambiguous `role="status"` lookup, and `selectOptions` driven at
what is now a combobox). Both fixed this session — entry:
[debugging/frontend/data-studio-gateway-tests-broken-status-and-combobox.md](../../debugging/frontend/data-studio-gateway-tests-broken-status-and-combobox.md).

## Open follow-ups

- The builder tab (`BuilderPane` stacked layout) is still dense at narrow widths — the options
  section list (Query/Plot/Transform/…) could collapse to an accordion; that's panel-builder scope,
  not the rail's.
- `panel.delete` exists on the host but the Library roster deliberately grew no delete affordance
  (kit rule: don't invent behavior the surface doesn't have; PanelPage owns lifecycle today). If
  library management moves into the studio, wire `onRemove` + `ConfirmDestructive` there.
