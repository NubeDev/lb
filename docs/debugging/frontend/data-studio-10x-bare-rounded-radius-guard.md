# Data Studio 10x files used bare `rounded` — radius-scale guard red

- Date: 2026-07-05
- Area: frontend/styles
- Status: **fixed**
- Symptom: `pnpm test src/styles/radius-scale.guard.test.ts` red on the `no bare \`rounded\` utility`
  case after the data-studio-10x CODE-ONLY session landed its new files; the guard listed six offenders
  across the new feature folders.
- Session: [../../sessions/frontend/data-studio-10x-session.md](../../sessions/frontend/data-studio-10x-session.md)

## Root cause

The radius bug shipped 2026-07-04 with a repo-wide `rounded` → token-derived stops (`rounded-md`,
`rounded-sm`, …) sweep + a source guard (`radius-scale.guard.test.ts`). The data-studio-10x CODE-ONLY
session added six new files (`OpenViewMenu.tsx`, `WorkbenchTab.tsx`, `panes/BuilderTabPane.tsx`,
`BuilderPane.tsx`, `BuilderToolbar.tsx`) and reintroduced bare `rounded` in menu item rows + small
chips/badges — the convention didn't propagate to the new feature folder. Identical pattern to the
insights-UI gap logged earlier
([../insights/insights-ui-used-bare-rounded.md](../insights/insights-ui-used-bare-rounded.md)).

The radius guard runs the regex `\brounded\b(?!-)` over every `.tsx` under `src/`, so the new files
red-ing the guard red-ed the whole UI unit suite (705 → 704). Same risk the radius control exists to
prevent: a bare `rounded` maps to Tailwind's un-derived DEFAULT unless pinned; the guard keeps the
whole app on token-derived stops so the radius knob actually does something.

## Fix

Six offenders, each mapped to the closest neighboring pattern:

| File | Site | Before | After | Why this stop |
|---|---|---|---|---|
| `data-studio/OpenViewMenu.tsx` (×2) | menu item rows (New panel + each view) | `rounded px-2 py-1.5` | `rounded-md px-2 py-1.5` | matches the existing menu in `BuilderToolbar.tsx:120` (`w-48 rounded-md`) |
| `data-studio/WorkbenchTab.tsx` | inline close button (icon-only) | `rounded p-0.5` | `rounded-sm p-0.5` | tight icon-button stop |
| `data-studio/panes/BuilderTabPane.tsx` | tiny uppercase "library" badge | `rounded border …` | `rounded-sm border …` | tight chip stop |
| `panel-builder/BuilderPane.tsx` | "demo data badge" | `rounded border …` | `rounded-sm border …` | tight chip stop (parity with the saved-as badge) |
| `panel-builder/BuilderToolbar.tsx` | "save as library panel" menu item | `rounded px-2 py-1.5` | `rounded-md px-2 py-1.5` | menu item parity with the sibling caret menu |

The deliberate `rounded-full` pills elsewhere are untouched (the guard's allowlist).

## Regression

The radius-scale guard test IS the regression — it fails-before (six offenders listed) and passes-after
(zero offenders). Re-asserted: `pnpm test src/styles/radius-scale.guard.test.ts` 4/4 green; the full UI
unit suite back to 705/705.

## Lesson

A source-level convention guard catches new feature folders that didn't learn the convention — fix is
applying the convention, not loosening the guard. This is the second feature folder (after insights-UI)
to reintroduce bare `rounded` after the 2026-07-04 sweep. The guard is doing its job; the onboarding
for a new feature folder should include "run `pnpm test src/styles/radius-scale.guard.test.ts` before
declaring code-done." Recorded in CLAUDE.md's testing-step expectations implicitly via the HOW-TO-CODE
"paste the green output" rule.
