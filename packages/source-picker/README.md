# @nube/source-picker

The Lazybones **"pick a value from the DB / datasources / Zenoh (live series) / flows / extension
widgets"** machinery, extracted from the dashboard so any surface reuses ONE picker — the dashboard
panel editor, an extension UI (e.g. `thecrew` graphics-canvas), a channel composer, wherever.

It is **transport-agnostic**: the host injects a `SourceLoaders` (how to reach the node). The shell
delegates to its gateway/Tauri clients; a standalone extension delegates to its host bridge. The
package never imports an API client, `invoke`, or `@/` — that's what makes one picker work everywhere.

## Three layers — adopt what you need

```ts
import {
  buildSourceEntries,     // MODEL (pure): loader results → SourceEntry[]
  useSourcePicker,        // HOOK: orchestrates the injected loaders (deny-tolerant, ws-keyed)
  SourcePicker,           // UI: the props-driven grouped <select>
  type SourceLoaders, type SourceSelection,
} from "@nube/source-picker";
import "@nube/source-picker/style.css";
```

## Wiring (the injected seam)

```tsx
// The host implements the reads over its own transport. Every fn is optional + may reject
// (a denied/absent read → that group is simply empty — an honest, capability-scoped offer).
const loaders: SourceLoaders = {
  listSeries: () => listSeries(),              // series.list  → Series + Live (Zenoh) groups
  listExtensions: () => listExtensions(),      // ext.list     → Installed-extension + Extension-widget
  listFlows: () => listFlows(),                // flows.list   ┐
  getFlow: (id) => getFlow(id),                // flows.get    ├ Flows group (node ports)
  listFlowNodes: () => listFlowNodes(),        // flows.nodes  ┘
  listDatasources: () => listDatasources(),    // datasource.list → federation roster
};

function Picker({ ws }: { ws: string }) {
  const { entries, loading } = useSourcePicker(loaders, ws);
  return (
    <SourcePicker
      entries={entries}
      loading={loading}
      onSelect={(sel) => {
        // sel is a SourceSelection: exactly one of source {tool,args} / action {tool,argsTemplate} /
        // viewKey "ext:<id>/<widget>". Map it onto whatever you persist (a dashboard cell, a scene
        // bind, a variable query …). The host still gates every call server-side.
      }}
    />
  );
}
```

`useSourcePicker` reads `loaders` through a ref and keys the reload on `ws` only, so an unmemoized
`loaders` literal per render does **not** loop (the host-stability guarantee is soft, not required).

## Theming

Self-themed via `--sp-*` tokens scoped to `.sp-root`, aliasing the host's shadcn vars (`--bg`, `--fg`,
`--border`, `--accent`) with dark fallbacks. Override by setting `--sp-*` on any ancestor. No preflight,
no global utilities — the stylesheet can't touch the host app.

## What it is / isn't

- **Is:** the source MODEL + loader orchestration + the "pick a source by label" `<select>`.
- **Isn't:** the federation datasource dropdown or the flow node→port sub-picker (those shape a
  host-specific target — compose them around this), nor how a selection lands on your record (that's
  the host's concern; the package stops at returning a `SourceSelection`).

Scope: `docs/scope/frontend/dashboard/source-picker-package-scope.md`.
