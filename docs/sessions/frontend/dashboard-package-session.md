# Session — `@nube/dashboard`: extract the shell dashboard grid into a shared package

- **Scope:** [`../../scope/frontend/dashboard/dashboard-package-scope.md`](../../scope/frontend/dashboard/dashboard-package-scope.md)
- **Status:** shipped (v0.1, grid core only)
- **Date:** 2026-07-15

## What was done

Created `packages/dashboard` (`@nube/dashboard` 0.1.0), the sixth `packages/*` sibling,
mirroring the `@nube/insights` / `@nube/panel` scaffold (vite lib build ESM+CJS+dts+css, tsc,
vitest, same package.json shape).

**Model (pure, no React)** — ported from the shell dashboard feature (the rubix-ai `ui/src`
copy, which carries the same files):

- `dashboard.types.ts` — `Cell`, the `View` vocabulary + `canonicalView` alias map,
  sources/targets/queryOptions, `GRID_COLS`/`GRID_ROW_PX`. The `@/lib/vars` `Variable` import
  was cut: a `Dashboard.variables` is `unknown[]` here (the variables machinery is the
  consumer's).
- `fieldconfig.types.ts` — copied **verbatim** (it was already pure). Types only; the apply
  logic (units/format/thresholds bridge) deliberately NOT ported.
- `rows.ts` — copied verbatim (panel-rows positional membership + collapse geometry).
- `layout.ts` — `mergeLayout`, the drag/resize merge **extracted out of Grid.tsx into a pure
  function** (incl. the moved-row member-carry Δy math) so it is directly testable.
- `timeOverrideBadge.ts`, `timerange.ts` (the package-owned `TimeRange` replacing the shell's
  URL `DashboardSearch`).

**The four entanglement cuts:** WidgetHost → `createRegistry<S>()` (exact canonical id, then
the `ext:*` wildcard for federation views, else the honest `UnknownView` placeholder);
VarScope → the opaque generic `scope?: S`; DashboardSearch → `TimeRange`; ExtRow/`ext:` →
gone from the package (a shell registers its federation mount like any renderer). Also cut as
shell-side: the display-mode toggle (fieldConfig-adjacent) and the row-options shadcn dialog
(collapse/rename callbacks remain).

**Hosts:** `DashboardGrid` (editable/read-only, measured width with the 1200px jsdom-deterministic
fallback, hover chrome gated by which callbacks the host passes, `RowHeader` on plain elements)
and `DashboardStack` (y,x order, read-only; `DashboardGrid` degrades to it below `stackBelow`,
default 768).

**Styles:** plain scoped CSS (`.lbdg-root` / `--lbdg-*` tokens aliasing the host shadcn channel
vars, dark fallbacks — the insights/panel discipline). The react-grid-layout + react-resizable
vendor rules are copied into `dashboard.css` with every selector prefixed under `.lbdg-root`,
so the grid library CSS cannot leak into a host shell. The RGL placeholder was retinted from
hard red to the accent token.

## Tests (24, all green)

`rows.test.ts` (5), `layout.test.ts` (5), `registry.test.tsx` (4), `Grid.test.tsx` (7),
`Stack.test.tsx` (3). Highlights:

- The onLayout persistence-seam test drives a **real drag** end-to-end through
  react-grid-layout/react-draggable (mousedown on the handle → mousemove → mouseup →
  `onDragStop` → `mergeLayout` → `onLayout`), asserting the FULL cells payload with the hidden
  collapsed-row member carried by the row's Δy.
- Read-only vs editable pins the chrome (no drag handle / remove / duplicate when read-only).
- Unknown view renders the honest placeholder naming the id — inside both the grid and stack.

```
Test Files  5 passed (5)
     Tests  24 passed (24)
```

`pnpm --filter @nube/dashboard typecheck` and `build` (ESM+CJS+dts+css) both clean.

Note on the repo-wide mandatory tests: the capability-deny and workspace-isolation tests do
not apply inside this package — it has **no client seam and no workspace concept by design**
(rule: the consumer's renderers/callbacks own data + persistence; those tests live where the
consumer wires them, as `@nube/insights`'s `denyClient` test does).

## Issues hit (and how they were fixed)

1. **jsdom `offsetParent` is null → react-grid-layout's `onDragStart` bails**, so the real-drag
   test's `onDragStop` never fired ("onDragEnd called before onDragStart"). Fixed with an
   environment shim in the test (`offsetParent` → `parentElement`) — an environment gap fill,
   not a fake: the real RGL code path runs. Logged:
   [`../../debugging/frontend/react-grid-layout-drag-never-fires-in-jsdom.md`](../../debugging/frontend/react-grid-layout-drag-never-fires-in-jsdom.md).
2. **Vertical compaction ate the drag** — dragging the top row down compacts it straight back
   to `y:0`, so the payload showed no movement. The test now drags the row UP past a full-width
   cell (a real reorder), asserting relative Δy rather than absolute rows.

## Verification of the external pin

Tagged `dashboard-v0.1.0`. Verified from a scratch dir that pnpm's git subdir syntax resolves
(see the scope doc's "Consuming from outside the monorepo" for the exact result + caveats).

## Not done (later slices)

Migrating the lb shell / rubix-ai onto the package (both untouched this slice, per scope);
fieldConfig apply bridge; wizard/editors; variables.
