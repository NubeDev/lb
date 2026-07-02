# tailwind-v4-migration — session log

Status: **done (2026-07-02)**. Scope:
[`scope/frontend/tailwind-v4-migration-scope.md`](../../scope/frontend/tailwind-v4-migration-scope.md).

## Ask

Migrate lb's `ui/` from Tailwind **v3.4 → v4**, so the whole workspace speaks one Tailwind (the new
`@nube/nav-rail` package is v4, and a v4 library stylesheet dropped into a v3 host errored:
`@layer base is used but no matching @tailwind base directive`).

## What shipped

CSS-first v4 migration — no `tailwind.config.ts`, no PostCSS:

- **Deps** (`ui/package.json`): `tailwindcss@^4.1` + `@tailwindcss/vite@^4.1` (dev); removed
  `autoprefixer` + `postcss` (nothing else used them); `tailwindcss-animate` → **`tw-animate-css@^1.2`**.
- **Vite** (`ui/vite.config.ts`): added `tailwindcss()` to plugins.
- **Deleted** `ui/tailwind.config.ts` and `ui/postcss.config.js`.
- **`ui/src/styles/globals.css`**: the three `@tailwind base/components/utilities` directives →
  `@import "tailwindcss"; @import "tw-animate-css";`. Added `@custom-variant dark (&:where(.dark, .dark *))`
  (v4 dropped the `darkMode:"class"` config knob). Moved the config's `colors`/`borderRadius` into an
  `@theme { --color-*: hsl(var(--token)); --radius-*: … }` block — the existing
  `:root`/`.dark`/`[data-theme-accent]` CSS-var blocks and the whole `@layer components`
  (`.control-field`/`.soft-button`/`.page-header`/…), React Flow chrome, and wizard keyframes are
  **unchanged**.
- **Utility deltas:** audited — no `*-opacity-*`, one bare `ring`, `outline-none`/`shadow-sm` names
  persist. No file churn was needed across the 264 tsx files.
- **`@nube/nav-rail`** stylesheet hardened to ship **theme + utilities only, no preflight**
  (`@import "tailwindcss/theme.css"` + `.../utilities.css`, not the full `tailwindcss`) — a library must
  never drop a global reset into a host. Now imports cleanly in the v4 host; the shipped
  `dist/nav-rail.css` contains `0` `@layer base` blocks.

## Tests / verification (green)

- **Build:** `vite build` succeeds on v4; the emitted CSS contains the token utilities (`bg-bg`,
  `text-fg`, `border-border`, `bg-panel`, `text-accent`), the `.dark` variant, the `@layer components`
  classes (`.control-field`/`.soft-button`/`.page-header`/`.icon-button`), and `animate-in`
  (`tw-animate-css`).
- **Unit:** `pnpm -C ui test` — **322/322 pass**.
- **Real gateway:** `pnpm -C ui test:gateway` — **235 pass**, same **4 pre-existing failures** as the
  v3 baseline (`DashboardView`, `SystemView` subsystem sheet, `sqlSource` visual-editor, agent-command
  palette) — proven pre-existing by stashing all my changes and re-running: they fail identically on
  v3. **Zero new failures.** The panel-editor + flows-panel-editor suites (the NavMenu integration)
  pass.
- **No new type errors:** the migration is CSS/build-only. `ui` `tsc --noEmit` has the same
  pre-existing errors as baseline (2 in `FlowsCanvas.gateway.test.ts`; 1 in `iframeRuntime.ts` from
  the in-tree widget-iframe WIP — none touched by this change).

## Notes

- The workspace still has a stray `tailwindcss@3.4.19` in the pnpm store (unlinked leftover); `ui`
  resolves `tailwindcss@4.3.2` + the vite plugin. Harmless; a future `pnpm prune`/`dedupe` clears it.
- Manual visual spot-check of light/dark, the three accent swatches, the panel editor rail, and the
  flows canvas is recommended before release (the automated suites assert behavior, not computed
  styles).

## Related

- [`sessions/shell/nav-rail-session.md`](../shell/nav-rail-session.md) — the v4 package that motivated
  this; its stylesheet now drops into the v4 host.
