// The trivial federated PAGE for echarts-panel. Deliberately minimal: it just confirms the extension is
// installed. The real surface this extension exists for is the `chart` `[[widget]]` data tile — the page
// is here only to match proof-panel's shape (a page + a widget on one remote).

/** Root page: a small "installed" card. No bridge, no data — the tile is where the work is. */
export function App() {
  return (
    <div className="flex min-h-full flex-col gap-1 p-6 text-fg" data-ext-page="echarts-panel">
      <h1 className="text-lg font-semibold">ECharts Panel</h1>
      <p className="text-sm text-muted">
        Installed. This extension contributes a frames-in <strong>Chart</strong> widget — add it to a
        dashboard to render <code>ctx.data</code> with Apache ECharts, driven by the shared Field-tab
        options.
      </p>
    </div>
  );
}
