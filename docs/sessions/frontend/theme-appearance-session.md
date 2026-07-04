# Session — theme appearance (looks, fonts, surfaces, motion)

Status: **in-progress**. Scope: [`scope/frontend/theme-appearance-scope.md`](../../scope/frontend/theme-appearance-scope.md).
Public: [`public/frontend/frontend.md`](../../public/frontend/frontend.md) (promoted on ship).

Successor to the shipped theme customizer. "10x the theme" — look packs, fonts, surfaces, a motion
system, wider tone palette, and the extension `ctx.theme` v4 fan-out. Plus slice 0: the two shipped
Customizer bugs (radius no-op, non-clickable brand swatches). Zero backend/Rust change — everything
rides the existing `ui_theme` prefs blob (client-side shape widen + normalize defaults).

Working directly on `master`, committing in reviewable slices.

---

## Slice 0a — radius bug (shipped, live-verified)

**Symptom (shipped).** The Customizer's radius control visibly did nothing for most of the app. Cards,
inputs, chips, buttons kept their corners no matter the slider.

**Root cause.** `theme-dom.ts` writes `--radius` correctly (unit-tested). But in `globals.css`'s
`@theme`, only `--radius-sm/md/lg` derived from `var(--radius)`; **bare `rounded`** (Tailwind
`--radius-DEFAULT` = static `0.25rem`), `rounded-xl` (static `0.75rem`), and `rounded-2xl` never
referenced the token — so ~114 `rounded` + `rounded-xl` call sites were pinned to Tailwind's compiled
defaults. **Second, deeper cause found during live-verify:** `@import "tw-animate-css"` re-imports
Tailwind's default `@theme`, which re-declares `--radius-sm/md/lg` **statically** *after* our `@theme`
block in the compiled sheet — so even deriving them in `@theme` lost the cascade to the static
`.375rem`. jsdom can't see any of this (no Tailwind, no `var()` computation), which is why it shipped.

**Fix (two parts).**
1. `globals.css` `@theme` now declares the full ladder `--radius-xs…--radius-3xl` (+ `--radius-DEFAULT`)
   off `var(--radius)`, so Tailwind *generates* `rounded-xl/2xl/3xl` utilities.
2. A **cascade-last `:root:root` override** (specificity 0,2,0) at the end of `globals.css` re-asserts
   the derived values so it beats `tw-animate-css`'s later plain-`:root` static re-emission by
   specificity, not source order (the bundler reorders past us). This is the load-bearing line.
   - sm/md/lg keep their shipped offsets (`--radius-2px`, `--radius`) so shadcn components are
     byte-identical — no visual regression. `rounded` (DEFAULT) is pinned to the `md` stop.
3. **Sweep:** all bare `rounded` → `rounded-md` across 44 `.tsx` files (perl word-boundary replace,
   `rounded-full`/`rounded-none` untouched). Makes the intended stop explicit and lets the guard forbid
   bare `rounded`.

**Live-verify (real Chromium, `@playwright/test`).** Loaded the compiled CSS, set `--radius` to
0.5rem / 1rem / 0rem, read `getComputedStyle().borderRadius`:

```
--radius=0.5rem : {"md":"6px","lg":"8px","xl":"12px","x2":"16px","full":"3.35e7px"}
--radius=1rem   : {"md":"14px","lg":"16px","xl":"20px","x2":"24px","full":"3.35e7px"}
--radius=0rem   : {"md":"0px","lg":"0px","xl":"4px","x2":"8px","full":"3.35e7px"}
```

Every stop tracks the token; `rounded-full` stays a pill; `0rem` clamps to square. Bug fixed.

**Guard test.** `src/styles/radius-scale.guard.test.ts` — asserts (a) every stop derives from
`var(--radius)` in source, (b) the `:root:root` cascade-last override exists, (c) no `.tsx` uses bare
`rounded`. Fails-before / passes-after; keeps the regression out.

**Debug entry:** [`debugging/frontend/radius-control-does-nothing.md`](../../debugging/frontend/radius-control-does-nothing.md).

Green: `radius-scale.guard.test.ts` 4/4; full unit suite 475/475 (was 472 + 3, then +1 override guard).

---

## Slice 0b — brand-color picker bug

(in progress)
