# Reveal crashes outside ThemeProvider ("useTheme must be used within ThemeProvider")

**Date:** 2026-07-05 · **Area:** frontend (motion seam / theme) · **Found while:** converting the
rules rail onto the shared `RosterRail` and running `RulesView.gateway.test.tsx`.

## Symptom

Every test in `src/features/rules/RulesView.gateway.test.tsx` failed at first render with
`Error: useTheme must be used within ThemeProvider`, thrown from `Reveal` (`src/lib/motion/Reveal.tsx`).
Pre-existing on the clean tree (verified via `git stash` → same 11 failures), so it was masked as
"another failing gateway suite", not caught by the change that introduced it.

## Cause

`useMotionPref` (the JS motion seam) called the **throwing** `useTheme()`. Any page that wraps
content in `<Reveal>`/motion primitives therefore hard-required a `ThemeProvider` above it. Gateway
tests (and any embedded mount) render surfaces bare — `RulesView` had grown a `Reveal` in its tree,
so the whole page crashed. `CodeEditor` had already hit the same trap and solved it by reading the
theme context optionally.

## Fix

`src/lib/motion/useMotionPref.ts` now uses `useThemeOptional()` and falls back to `DEFAULT_THEME`
when no provider is present (same pattern as `CodeEditor`). Motion resolves from the default look in
that case — never a crash.

## Regression test

`src/lib/motion/useMotionPref.test.tsx` — renders a `useMotionPref` probe and a `<Reveal>` with NO
provider; plus the 11 `RulesView.gateway.test.tsx` tests now pass again (they render bare).

## Lesson

Shared seams (`lib/*`) must not hard-require an app-shell provider; use the optional context read
and a default. A gateway suite that fails wholesale at render time is worth bisecting immediately —
this one hid behind the "pre-existing failing tests" list.
