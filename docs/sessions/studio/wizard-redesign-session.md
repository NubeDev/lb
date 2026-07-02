# Extension Studio — wizard redesign session

**Date:** 2026-07-01 · **Area:** `ui/src/features/studio` · **Type:** UX/UI refactor (frontend-only)

## The ask

The Studio was hard to understand: one dense two-column screen showed every stage of the
extension pipeline at once (Generate, Open folder, Build, Publish, Inspect, Build log), with two
competing entry paths stacked in the same column and disabled buttons as dead-ends. Users couldn't
tell what to do first. Requested: a wizard / tabs to make the flow legible.

## Decision: a 3-step wizard, not tabs

The Studio is a **strict linear pipeline** (create → build → publish) with dependent steps — you
can't publish before you build. Tabs imply "any order" and are the wrong model. A **stepper wizard**
matches the real flow and makes impossible states unreachable instead of merely disabled.

Rejected alternative — **2 tabs** (as the user floated): tabs would split "new vs existing" but
leave the build/publish ordering ambiguous, and re-create the "which do I click" problem one level
down. The branch is a *step-1 choice*, not a top-level mode.

The old "Inspect" stage was folded into step 1's success (auto-inspect on generate/open), collapsing
4 stages to 3 for a snappier common path.

## Shape

- **Step 1 · Create** — the one branch, made explicit: two selectable path cards
  (*Generate new* vs *Open existing*). Picking one reveals only its inputs (ID/tier/features, or a
  folder path). The footer's primary action runs it and advances.
- **Step 2 · Build** — inspection summary on top, with **toolchain readiness surfaced up front**
  (missing cargo/pnpm/wasm-target warns *before* a doomed build, not after). Streaming build log as a
  proper terminal panel below. Primary becomes "Continue" once an artifact exists.
- **Step 3 · Publish** — plain-language confirmation of what/where, then a success state that closes
  the loop with "Build another". No dangling controls.

A horizontal **stepper** spine (done = check + clickable-back, active = amber, upcoming = quiet)
always answers "where am I, what's left". The **footer** is the single control surface everywhere:
`[status] [Back] [Primary]`, where Primary is stateful per step and always in the same place.

## Files (one responsibility each, per FILE-LAYOUT)

- `studio.wizard.ts` — the state machine hook: all devkit side effects (scaffold/inspect/build+log
  stream/publish) + step gating. View components stay presentational.
- `steps.ts` — the three step definitions (shared by spine + views so labels never drift).
- `Stepper.tsx`, `WizardFooter.tsx` — the spine and the unified control footer.
- `CreateStep.tsx`, `BuildStep.tsx`, `PublishStep.tsx` — the three step bodies.
- `InspectionSummary.tsx`, `BuildLog.tsx` — the folder read-out and the terminal log panel.
- `StudioView.tsx` — thin orchestrator: header + stepper + active step + footer.

## Design system

Reused the existing "quiet control-surface" tokens verbatim (warm paper / near-black, single amber
accent, hairline borders, small type scale). No new palette. Quiet per-step crossfade reuses the
existing `wizard-panel` motion idiom (reduced-motion safe). Icons from the project's `lucide-react`.

## UI-standard migration (bonus)

The refactor converts every raw control to the shadcn `<Button>`/`<Input>` primitives and **removes
`src/features/studio/StudioView.tsx` from `LEGACY_VIEWS`** in `ui/eslint.config.js`, so the Studio is
now build-breaking-enforced under the UI standard (`scope/frontend/ui-standards-scope.md`) rather than
warn-only. This shrinks the legacy list per its stated migration path.

## Verification

- `npx tsc --noEmit` — no errors in `features/studio` (two pre-existing errors in an unrelated
  `flows` gateway test are untouched).
- `npx eslint src/features/studio` — clean (0 errors), including the now-enforced standard.
- `npx vite build` — production build passes.
- **Visual QA** — rendered all six states (both step-1 modes, step-2 toolchain-missing + built,
  step-3 confirm + success) in a throwaway dev-only harness and screenshotted in **light and dark**
  via Playwright/Chrome. Both themes read cleanly; the terminal recedes into the dark surface, amber
  carries state, contrast holds. Harness deleted after QA.

Not run: `pnpm test:gateway` (`StudioView.gateway.test.tsx`). It drives the real SDK chain through
`devkit.api` directly and does **not** render `StudioView`; this change is a presentational refactor
that doesn't touch the API layer, so the existing gateway test remains valid and covers the real path
unchanged.

## Follow-ups

- Consider a render-level test of the wizard's step gating (currently only the API chain is tested).
- If the folder-path field gains a real picker, wire it into step 1's "Open existing".
