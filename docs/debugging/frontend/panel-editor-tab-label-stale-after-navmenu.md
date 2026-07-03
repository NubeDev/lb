# Panel-editor "tab Field" label stale after the NavMenu migration

- **Area:** frontend (dashboard viz panel editor)
- **Status:** resolved
- **Date:** 2026-07-03
- **Session:** [dashboard-editor-parity step 1](../../sessions/frontend/dashboard-editor-parity-step1-session.md)

## Symptom

`src/features/dashboard/DashboardView.gateway.test.tsx` →
"renders a timeseries panel over a bridged source with the full option surface" failed at
`screen.getByLabelText("tab Field")` with `Unable to find a label with the text of: tab Field`.
The test could open the editor and pick a source, but never reach the Field tab, so the whole
fieldConfig-through-the-UI assertion was dead.

## Root cause

The panel editor's options rail was migrated to the shared `@nube/panel` `NavMenu`
(`PanelEditor.tsx`), which renders each item with `aria-label={item.label}` — i.e. the bare
`"Field"`, `"Query"`, … . The test predated that migration and still clicked the old
`PanelTabs`-style label `"tab Field"` (that `tab ${label}` scheme survives only in
`features/rules/panel/PanelTabs.tsx`). So the assertion had been failing since the NavMenu
migration — a stale test, not a code regression. It was NOT caused by the step-1 primitives work;
verified by reverting `PanelEditor.tsx`/`combobox.tsx` to `HEAD` and reproducing the identical
failure.

## Fix

Point the assertion at the label the editor actually renders:
`getByLabelText("tab Field")` → `getByLabelText("Field")`. One line; the rest of the test
(select the source, set `field unit` = celsius, save, assert the rendered value) was already
correct and now runs green.

## Regression

The test itself is the regression — it now exercises the Field tab through the real NavMenu and
the real gateway. No production change was needed (the editor was correct; the label expectation
was stale).

## A second stale assertion in the same file (fixed together)

The "renames a dashboard from the roster" test used `findByText("Operations")` after mounting a
SECOND `DashboardView` into the same document (the reload assertion) — and the title renders in more
than one place (roster row + header) even in one view. `findByText`/`findByLabelText` THROW on
multiple matches, so the test failed `Found multiple elements with the text: Operations`. Also
pre-existing (byte-identical to before this slice; verified by diffing the test body against an
earlier commit) — an editor-adjacent brittleness, not a regression. Fixed by asserting **presence,
not uniqueness** (`findAllByText(...).length > 0`, and `findAllByLabelText(...)[0]` for the click).

## Lesson

When a component migrates onto a shared primitive that changes its accessible names
(`tab ${label}` → `${label}`), grep the test suite for the OLD label scheme in the same change —
a stale `getByLabelText` fails silently in a big gateway run and reads as "flaky", masking that
the tab is simply unreachable under its new name. And a title rendered in >1 place (or a test that
mounts two views into one document) must assert with `findAllBy*`, never the throw-on-multiple
`findBy*`.
