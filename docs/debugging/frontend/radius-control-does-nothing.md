# The Customizer radius control visibly does nothing

- **Area:** frontend
- **Date:** 2026-07-04
- **Status:** resolved
- **Symptom (as reported):** the theme Customizer's **radius** control moves, and the picker writes the
  token, but the app's corners don't change — cards, inputs, chips, and buttons keep the same radius no
  matter the setting. Shipped in the theme customizer; only the handful of `rounded-sm/md/lg` surfaces
  ever moved.

## Root cause (two layers)

`theme-dom.ts` writes `--radius` to `<html>` correctly — its unit test passes. The break is downstream,
in the CSS, and it hid behind jsdom (which runs no Tailwind and computes no `var()`).

**Layer 1 — most utilities never referenced the token.** In `styles/globals.css` the `@theme` block
derived only `--radius-sm/md/lg` from `var(--radius)`. Tailwind's **bare `rounded`** (`--radius-DEFAULT`,
static `0.25rem`), `rounded-xl` (static `0.75rem`), and `rounded-2xl` (static `1rem`) fell through to
Tailwind's compiled defaults. The app has ~114 bare `rounded` + `rounded-xl` call sites — all pinned,
none tracking the token. So a radius nudge moved a few shadcn surfaces and nothing else.

**Layer 2 (found during live-verify) — `tw-animate-css` wins the cascade.** Even after deriving every
stop from `var(--radius)` in `@theme`, the compiled sheet still resolved `.rounded-md` to the static
`.375rem`. Cause: `@import "tw-animate-css"` re-imports Tailwind's **default `@theme`**, which re-declares
`--radius-sm/md/lg` statically into `:root` **after** our block in the emitted CSS. Custom-property
resolution obeys the cascade, so the later static `:root` beat our earlier derived `:root`. Source order
didn't save us — the bundler emits the dependency's theme last.

This is a jsdom blind spot: no Tailwind compilation, no `var()` computation, so every unit test passed
while the real app was broken. It can only be caught by compiling the CSS and reading computed style in
a real engine.

## Fix

`styles/globals.css`:
1. **Declare the full ladder in `@theme`** (`--radius-xs…--radius-3xl` + `--radius-DEFAULT`) off
   `var(--radius)`, so Tailwind *generates* the `rounded-xl/2xl/3xl` utilities.
2. **Cascade-last override** — a `:root:root { --radius-xs…3xl: … }` block at the very end of the file.
   `:root:root` has specificity (0,2,0), so it beats `tw-animate-css`'s plain `:root` (0,1,0) re-emission
   **by specificity, order-independent**. This is the load-bearing line.
   - The `sm/md/lg` anchors keep their shipped offsets (`--radius - 2px`, `--radius`) so shadcn
     components are byte-identical — no visual regression. `rounded` (DEFAULT) is pinned to the `md` stop.
   - `max(0px, …)` clamps the small stops so radius `0rem` yields true squares.
3. **Sweep** all bare `rounded` → `rounded-md` across 44 `.tsx` files (word-boundary replace;
   `rounded-full`/`rounded-none` untouched) so the intended stop is explicit and the guard can forbid
   bare `rounded`.

## Regression test

`ui/src/styles/radius-scale.guard.test.ts` — a build-level guard (jsdom can't verify the compiled
result, so it asserts on source):
- every stop derives from `var(--radius)`;
- the `:root:root` cascade-last override exists (drop it → the static default wins again);
- no `.tsx` uses bare `rounded`.

**Live-verify (real Chromium via `@playwright/test`)** — loaded the compiled CSS, set `--radius` to
0.5 / 1 / 0 rem, read `getComputedStyle().borderRadius`: every stop tracks the token
(`md` 6→14px, `lg` 8→16px, `xl` 12→20px), `rounded-full` stays a pill, `0rem` clamps to square.
Fail-before / pass-after confirmed (reverting the `:root:root` block reverts `.rounded-md` to `.375rem`).

## Lessons

- **jsdom cannot verify Tailwind or `var()`.** Any token→utility derivation must be live-verified by
  compiling the CSS and reading computed style in a real engine; a green jsdom suite proves nothing here.
- A dependency that re-imports Tailwind's default theme (`tw-animate-css`) **re-emits the default token
  values last**. Overriding a default theme var needs specificity (`:root:root`), not just later source.
