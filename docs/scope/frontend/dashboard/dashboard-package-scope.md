# Dashboard package scope — `@nube/dashboard`, the reusable grid core

Status: **shipped** (v0.1, this slice). Extracts the shell's dashboard grid into a workspace
package so any Nube surface (the lb shell, rubix-ai, an extension UI) hosts ONE grid
implementation. The sixth sibling of `@nube/insights`, `@nube/source-picker`, `@nube/panel`,
`@nube/genui`, `@nube/nav-rail` — same extraction discipline as
`docs/scope/insights/insights-package-scope.md` and `source-picker-package-scope.md`.

## The ask

The dashboard grid (12-col react-grid-layout host, panel-rows section math, cell vocabulary)
lived only in the shell (`features/dashboard/Grid.tsx` + `lib/dashboard/*`; the rubix-ai shell
carries a mirrored copy — the exact drift this package exists to end). Extract the **grid core
only** into `packages/dashboard` so a consumer renders the same `cells[]` record with its own
widgets and its own persistence.

## Shape (mirrors `@nube/insights`)

Three layers; **the look is scoped and optional to restyle**:

1. **Model (pure, no React)** — `dashboard.types.ts` (the `Cell` record, the `View` render
   vocabulary + `canonicalView` alias map, sources/targets, the 12-col geometry vocabulary),
   `fieldconfig.types.ts` (the Grafana fieldConfig **types only** — moved verbatim; the *apply*
   logic — units/format/thresholds render bridge — is **NOT** ported in v0.1), `rows.ts`
   (panel-rows positional membership + collapse geometry), `layout.ts` (`mergeLayout` — the
   drag/resize merge incl. the row-carries-its-members Δy math), `timerange.ts`.
2. **Grid host** — `DashboardGrid` on `react-grid-layout` (bundled, with `react-resizable`;
   `react`/`react-dom` are peers). Editable vs read-only, measured width with the deterministic
   1200px test fallback, drag/resize-stop → `onLayout(cells)` persistence seam, row header bar
   with collapse/rename callbacks (all optional), hover chrome (move/edit/duplicate/export/
   remove) gated by which callbacks the host passes.
3. **Stack host** — `DashboardStack`: the same cells as a single-column, read-only stack in
   y,x order (rows become plain section dividers; a collapsed row still hides its members).
   `DashboardGrid` auto-degrades to the stack below `stackBelow` px (default 768, "below md");
   pass `stackBelow={0}` to opt out.

## The cut seams (the four shell entanglements)

- **WidgetHost dispatch → a widget registry.** `createRegistry<S>()`; the consumer registers a
  renderer per `View` id (resolved through `canonicalView`, so `chart` ⇒ the `timeseries`
  renderer). An unregistered view renders an **honest placeholder** naming the view id — never
  a crash, never a fabricated widget.
- **VarScope → an opaque generic.** The grid is `DashboardGrid<S>`; `scope?: S` passes through
  to every renderer untouched. The package has no variables machinery.
- **DashboardSearch → `TimeRange`.** A package-owned `{ from, to }` (ISO strings), passed
  through to renderers. URL/search parsing stays in the shell.
- **ExtRow / `ext:<id>` cells → out.** No `ext.list` type in the package. A shell registers its
  federation mount like any other renderer — exact id, or the `"ext:*"` wildcard key that
  catches every `ext:` view.

Also cut (shell-side concerns, not seams): the display-mode toggle (`useDisplayOverride` reads
frames — fieldConfig-adjacent, deferred with it) and the row-options popout dialog (shadcn
`Dialog`; the `collapsed`/rename callbacks remain).

## The hard rule

**The package never fetches, persists, or knows a workspace exists.** Cells + data in, DOM
out. Zero `@/` imports. Persistence is the consumer's `onLayout`/`onRemove`/… callbacks; data
is the consumer's registered renderers (which close over the consumer's client — the same
injected-seam doctrine as `InsightsClient`, one level up).

## Styles

Self-themed via scoped `--lbdg-*` tokens on `.lbdg-root` (aliasing the host's shadcn vars with
dark fallbacks — the `@nube/panel` discipline), host-overridable;
`import '@nube/dashboard/style.css'`. The react-grid-layout + react-resizable stylesheets are
**copied into the package CSS, every rule prefixed under `.lbdg-root`**, so the library cannot
leak `.react-grid-item`/`.react-resizable-handle` rules into a host shell that has its own grid.

## Tests (real, in `packages/dashboard/src/*.test.*`)

- `rows.test.ts` — positional membership, ungrouped region, collapse visibility.
- `layout.test.ts` — `mergeLayout` geometry merge + the moved-row member-carry Δy math
  (hidden members shift; layout-touched members are authoritative from the new layout).
- `registry.test.tsx` — dispatch through `canonicalView`, `ext:*` wildcard, unknown-view
  placeholder (honest, non-throwing).
- `Grid.test.tsx` — read-only renders **no** drag handles/chrome; editable does; a layout
  change fires `onLayout` with a correct full-cells payload.
- `Stack.test.tsx` — y,x ordering, row dividers, collapsed members hidden, no edit chrome.

All green: `pnpm --filter @nube/dashboard test`, `typecheck`, `build` (ESM+CJS+dts+css).

## Consuming from outside the monorepo

Tagged `dashboard-v0.1.0`. **The pnpm git-subdir dep was verified from a scratch dir** against
a local clone (`file:` protocol; the `github:` form is the same resolver over a remote):

```jsonc
// package.json — note "path:" is a URL FRAGMENT joined with &, not a query param:
"@nube/dashboard": "github:NubeDev/lb#dashboard-v0.1.0&path:/packages/dashboard"
```

Caveats found in verification (loudly, per the ask): pnpm **builds the subdir on install** (it
runs the package's `prepare`/build via its own toolchain), so the tagged commit must carry a
buildable package — it does; and the consumer's pnpm must be ≥ 8.15 for `path:` fragments. If
a consumer's install environment can't build (no network for devDeps, CI sandbox), the
fallback is the mirrored-copy pattern rubix-ai uses today — but prefer the tag.

## NOT in v0.1 (deliberately)

fieldConfig **apply** logic (format/units/thresholds bridge), the panel wizard/editors,
SQL/datasources, variables machinery, iframe/scripted views, import/export, library-panel ref
hydration (the `panelRef` fields ride along as data). **No shell migrates in this slice** —
the lb `ui` and rubix-ai keep their in-app dashboard features untouched; migrating them onto
the package is a later slice.

## Open questions / follow-ups

- Migrate the lb shell's `features/dashboard/Grid.tsx` onto the package (the proof-of-reuse
  slice), then rubix-ai.
- Port the fieldConfig apply/render bridge as `@nube/dashboard/fieldconfig` or a sibling
  package.
- Row-options popout as packaged UI (today only collapse/rename are packaged).
- Breakpoint-aware responsive layouts (react-grid-layout's `ResponsiveGridLayout`) vs today's
  fixed 12-col + stack degrade.
