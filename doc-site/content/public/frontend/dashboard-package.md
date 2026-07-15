# @nube/dashboard — the reusable dashboard grid core

`packages/dashboard` extracts the shell's dashboard grid into a shared frontend package — the
sixth `packages/*` sibling of `@nube/insights`, `@nube/source-picker`, `@nube/panel`,
`@nube/genui`, `@nube/nav-rail`. One grid implementation for every Nube surface.

## What it is

- **Pure model** — the `Cell` record + `View` vocabulary (`canonicalView` alias map,
  `cellSources`, `cellLabel`, …), the Grafana `fieldConfig` **types** (data only), panel-rows
  section math (positional membership, collapse visibility), and `mergeLayout` (the
  drag/resize merge, including "a moved row carries its members").
- **Widget registry** — `createRegistry<S>()`: consumers register a renderer per view id;
  an unknown view renders an honest placeholder (never a crash). `ext:*` is the federation
  wildcard key — extension tiles are ordinary registry entries, the package knows no extension.
- **`DashboardGrid`** — the 12-column react-grid-layout host (bundled + CSS-scoped under
  `.lbdg-root` so nothing leaks into a host shell). Editable vs read-only; drag/resize-stop →
  `onLayout(cells)` — the ONLY persistence seam; row collapse/rename callbacks; hover chrome
  gated by which callbacks the host passes.
- **`DashboardStack`** — the same cells as a single-column, read-only stack in y,x order
  (the grid degrades to it below 768px).

## The hard rule

The package never fetches, persists, or knows a workspace exists — cells + data in, DOM out.
Data flows through the consumer's registered renderers; persistence through the consumer's
callbacks. The host re-checks every capability + the workspace wall on every call its
renderers make — the package cannot and does not gate access.

## Consuming

In-repo: `"@nube/dashboard": "workspace:*"`. External:
`"@nube/dashboard": "github:NubeDev/lb#dashboard-v0.1.0&path:/packages/dashboard"` (pnpm ≥8.15;
`path:` is a `&`-joined URL fragment). Import `@nube/dashboard/style.css`; theme via `--lbdg-*`
token overrides (they alias the host's shadcn channel vars by default).

Scope: `docs/scope/frontend/dashboard/dashboard-package-scope.md`. v0.1 is the grid core only —
no fieldConfig apply logic, wizard, datasources, or variables; shells migrate in a later slice.
