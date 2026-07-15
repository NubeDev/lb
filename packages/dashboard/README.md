# @nube/dashboard

The reusable Nube **dashboard grid core** — the shell's 12-column `react-grid-layout` host
extracted behind a package boundary. Cells + data in, DOM out: the package **never fetches,
persists, or knows a workspace exists**.

Scope + design decisions: `docs/scope/frontend/dashboard/dashboard-package-scope.md`.

## Layers

1. **Model (pure)** — the `Cell` record + `View` vocabulary (`canonicalView`, `cellView`,
   `cellSources`, …), the Grafana `fieldConfig` **types** (data only; no apply/format logic),
   panel-rows section math (`rowMembers`, `visibleCells`, …), and `mergeLayout` (the
   drag/resize merge incl. the row-carries-its-members math).
2. **Registry** — `createRegistry<S>()`: the consumer maps view ids → renderers. Unknown view
   → an honest placeholder, never a crash. `ext:*` is the federation wildcard key.
3. **Hosts** — `DashboardGrid` (editable/read-only; drag/resize-stop → `onLayout(cells)`;
   degrades to the stack below `stackBelow` px) and `DashboardStack` (single-column, y,x
   order, read-only).

## Use

```tsx
import { DashboardGrid, createRegistry, type Cell } from "@nube/dashboard";
import "@nube/dashboard/style.css";

const registry = createRegistry<MyScope>()
  .register("timeseries", TimeseriesView)   // "chart" resolves here too (alias)
  .register("stat", StatView)
  .register("ext:*", FederatedTile);        // every ext:<id>/<widget> cell

<DashboardGrid
  cells={dashboard.cells}
  editable={canEdit}
  registry={registry}
  range={{ from, to }}
  scope={myVarScope}                        // opaque — passed through to renderers
  onLayout={(cells) => saveDashboard({ ...dashboard, cells })}
  onRemove={removeCell}
  onToggleRow={toggleRowCollapsed}
/>
```

Styles are scoped under `.lbdg-root` (react-grid-layout/react-resizable CSS included,
prefixed — nothing leaks into the host). Theme by overriding `--lbdg-*` tokens on any
ancestor; by default they alias the host's shadcn channel vars (`--bg`, `--panel`, `--fg`, …).

## Commands

`pnpm --filter @nube/dashboard test | typecheck | build`

## Not here (v0.1)

fieldConfig apply/format logic, panel wizard/editors, datasources/SQL, variables machinery,
scripted/iframe views, import/export. The shells keep their in-app dashboard features; the
migration onto this package is a later slice.
