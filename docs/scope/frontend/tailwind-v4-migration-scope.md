# frontend scope — migrate `ui/` from Tailwind v3 to v4

Status: **shipped (2026-07-02)** — `ui/` builds + runs on Tailwind v4 via `@tailwindcss/vite`;
CSS-first `@theme` (no `tailwind.config.ts`/`postcss.config.js`); `tw-animate-css` replaces
`tailwindcss-animate`; class dark mode via `@custom-variant`. 322 unit tests + the real-gateway
suite green with **no new failures** (the 4 pre-existing gateway failures are unchanged from the v3
baseline; the panel-editor/NavMenu suites pass). The `@nube/nav-rail` v4 stylesheet now drops in
cleanly. Session: [`tailwind-v4-migration`](../../sessions/frontend/tailwind-v4-migration-session.md).
Promotes to `public/frontend/frontend.md`.

Move the lb React shell (`ui/`) from **Tailwind CSS v3.4** to **v4**. Today `ui` is v3
(`tailwind.config.ts` + PostCSS plugin + `@tailwind` directives + `tailwindcss-animate`); the new
`@nube/nav-rail` package is v4, and a v4 library stylesheet dropped into a v3 host is the immediate
pain (a v4 `@layer base` has no matching `@tailwind base` in the v3 PostCSS context → build error).
The durable fix is to put the app on v4 too, so the whole workspace speaks one Tailwind.

## Goals

- `ui/` builds and runs on Tailwind v4 via the **`@tailwindcss/vite`** plugin (drop PostCSS +
  `autoprefixer` + `postcss.config.js`).
- The JS `tailwind.config.ts` (custom color tokens, `borderRadius`, `darkMode:"class"`) is expressed
  as CSS **`@theme`** in `globals.css`; the config file is retired (or reduced to the CSS-first form).
- The `@tailwind base/components/utilities` directives become the v4 `@import "tailwindcss"` (+ the
  `@custom-variant dark` for class-based dark mode).
- `tailwindcss-animate` → its v4 successor **`tw-animate-css`** (the 3 files using `animate-in`/
  `fade-in`/`zoom-in` keep working).
- **No visual regression** in the shipped surfaces: the token palette (bg/panel/border/fg/muted/
  accent + the shadcn aliases), light/dark, the three accent swatches (`data-theme-accent`), the
  `@layer components` classes (`.control-field`, `.soft-button`, `.page-header`, …), the React Flow
  chrome, and the wizard keyframes all render identically.
- `@nube/nav-rail`'s stylesheet drops in cleanly (its `@layer base`/theme now has a host that
  understands v4 layers).

## Non-goals

- **No redesign.** Same tokens, same look — a build-system migration, not a restyle.
- **Not a shadcn re-vendor.** The `components/ui/*` primitives stay; only utilities that v4 renamed
  are touched, and only where they actually changed behavior.
- **Not the ce-wiresheet repo** (separate; already v4).
- **Not the nav-rail package** (already v4; this only makes its host compatible).

## Intent / approach

v3→v4 is CSS-first. The steps:

1. **Deps:** add `tailwindcss@^4` + `@tailwindcss/vite@^4`; remove `autoprefixer`, `postcss` (unless
   another tool needs it), delete `postcss.config.js`. Swap `tailwindcss-animate` → `tw-animate-css`.
2. **Vite:** add `tailwindcss()` to `vite.config.ts` plugins (and the two vitest configs that build
   CSS — `vite.config.ts` is shared; `vitest.gateway.config.ts` uses `plugins:[react()]` only, so
   confirm whether its jsdom tests need CSS — they assert DOM/behavior, not computed styles, so
   likely no).
3. **`globals.css`:** replace the three `@tailwind` directives with `@import "tailwindcss";`; add
   `@custom-variant dark (&:where(.dark, .dark *));` for the class dark mode; move the config's
   `colors`/`borderRadius` into an `@theme { --color-bg: hsl(var(--bg)); … --radius-lg: var(--radius); }`
   block. The existing `:root`/`.dark`/`[data-theme-accent]` CSS-var blocks stay **as-is** (they're
   already CSS vars — v4 loves that). `@layer components`/`@apply` classes stay; verify `@apply` of
   token utilities (`bg-bg`, `border-border`) resolves under the new `@theme`.
4. **Utility deltas (small, audited):** the breaking set in use is tiny — no `*-opacity-*`, one bare
   `ring` (v4 default ring is 1px not 3px → make it `ring` intent explicit if the 3px mattered),
   `outline-none`/`shadow-sm` class names persist. Grep-audit and fix only real changes; don't
   churn 264 files.
5. **`tailwind.config.ts`:** delete once `@theme` covers it (v4 auto-detects content — no `content`
   array needed).

**Rejected alternative — keep `ui` on v3, only harden the package CSS** (theme+utilities, no
preflight). It unblocks nav-rail today (and is a fine fallback), but leaves two Tailwind majors in
one workspace forever — every future v4 library hits the same wall, and the token systems can drift.
The user chose the root fix. **Rejected — a v4 `@config` shim pointing at the old JS config:** works
as a bridge but keeps the JS config alive; the CSS-first `@theme` is the v4-native end state and is
cleaner to maintain.

## How it fits the core

Frontend build-tooling change; node/core concerns are N/A and stated so:

- **Tenancy / caps / SurrealDB / Zenoh / MCP / sync / secrets:** **N/A** — no backend, no verbs, no
  records. Pure client CSS pipeline.
- **Symmetric nodes / one datastore / state-vs-motion:** N/A.
- **One responsibility per file** (FILE-LAYOUT): the migration keeps `globals.css` as the one theme
  file; it does not add a `utils.css`. If `@theme` grows large, it may split into a `theme.css`
  imported by `globals.css` (one responsibility: tokens), but not required.
- **No mocks / no fake backend:** unaffected — the test suites (unit + real-gateway) are the
  regression harness; they must stay green.

## Testing plan

Per `scope/testing/testing-scope.md`. This is a visual/build change, so the gates are build +
behavior, not new logic:

- **Build green:** `pnpm -C ui build` (`tsc --noEmit && vite build`) succeeds on v4; the produced CSS
  contains the token utilities (`bg-bg`, `text-fg`, `border-border`, the component classes).
- **Unit suite green:** `pnpm -C ui test` — no regressions (these assert DOM/behavior; a CSS migration
  must not change them).
- **Real-gateway suite green:** `pnpm -C ui test:gateway` — the dashboard/editor/flows suites (which
  render real components, including the new `NavMenu`) still pass.
- **Visual spot-check (manual, recorded in the session):** run the app; confirm light/dark toggle, the
  three accent swatches, a dashboard panel editor (the NavMenu rail), the flows canvas chrome, and a
  `.soft-button`/`.control-field` render **unchanged**. Screenshot before/after the headline surfaces.
- **nav-rail drop-in:** the earlier `@layer base` PostCSS error is gone; `@nube/nav-rail/style.css`
  imports cleanly in the v4 host.

Capability-deny / workspace-isolation tests are **N/A** (no backend surface changes) — the existing
mandatory suites must simply stay green, proving no behavioral drift.

## Risks & hard problems

- **`@apply` of custom token utilities under `@theme`.** The component layer `@apply bg-bg
  border-border` must still resolve after colors move from JS `theme.extend.colors` to `@theme
  --color-*`. This is the most likely breakage; test the component classes explicitly.
- **Class-based dark mode.** v4 drops the `darkMode:"class"` config knob; the `.dark` variant must be
  re-declared with `@custom-variant`. Miss it and dark mode silently no-ops.
- **Preflight/reset deltas.** v4's preflight differs subtly from v3 (default border color is
  `currentColor` not gray-200; placeholder color; button cursor). The app already sets
  `* { @apply border-border }`, which mitigates the border-color change — but audit form controls and
  the scrollbar/`::selection` rules.
- **Ring/shadow default deltas.** v4's default `ring` is 1px (v3 was 3px) and `shadow` scale shifted;
  the audit found near-zero bare uses, but confirm focus rings on inputs/buttons still read.
- **`tw-animate-css` parity.** Confirm the `animate-in fade-in-0 zoom-in-95` classes used by the
  tooltip/sheet render the same entrance; it's a drop-in but verify.
- **Two vite/vitest configs.** Ensure the CSS plugin is wired wherever CSS is built; ensure the
  gateway config (no Tailwind plugin today) doesn't need it (jsdom asserts behavior, not styles).

## Open questions

1. Keep a minimal `tailwind.config.ts` via `@config`, or go fully CSS-first `@theme` (recommended)?
   Default: fully CSS-first, delete the JS config.
2. Does any tooling (Storybook, an eslint/tailwind plugin, the shadcn CLI) still expect
   `tailwind.config.ts`? If so, keep a thin shim; otherwise delete.
3. Is `postcss` needed by anything else in `ui` (e.g. another PostCSS plugin)? Grep before removing.

## Related

- `scope/frontend/nav-rail-scope.md` — the v4 package that motivated this (its stylesheet drops into
  the v4 host cleanly).
- `scope/frontend/shadcn-migration-scope.md` — the shadcn primitives that must keep rendering.
- `scope/frontend/theme-switcher-scope.md` / `ui-standards-scope.md` — the token system + look this
  migration must preserve.
- `public/frontend/frontend.md` — where the shipped truth promotes.
- Tailwind v4 upgrade guide (`@tailwindcss/vite`, `@theme`, `@custom-variant`, `tw-animate-css`).
