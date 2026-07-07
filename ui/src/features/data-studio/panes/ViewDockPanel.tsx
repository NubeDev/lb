// The Dockview panel wrapper for a pages-as-panes view (data-studio-10x scope, phase 2) — mounts
// the registry's REAL routed view component inside the embedded-page context (the page's own
// full-width header folds away; the dock tab is the title bar). The pane's in-pane selection (open
// flow/rule/datasource) persists in the panel's params, so a restored layout restores the whole
// debugging setup. Authority is unchanged: the view re-checks its own caps under the caller exactly
// as the routed page does. One responsibility: the dock↔view adapter.

import type { IDockviewPanelProps } from "dockview-react";

import { EmbeddedPageContext } from "@/components/app/page-embedded";

import type { ViewPaneConfig } from "../workbenchModel";
import { useWorkbench } from "../workbenchContext";
import { viewPane } from "../workbenchPanes";

export function ViewDockPanel({ api, params }: IDockviewPanelProps<ViewPaneConfig>) {
  const bench = useWorkbench();
  const def = viewPane(params.kind);
  if (!def) {
    return <div className="p-3 text-xs text-muted">Unknown view pane.</div>;
  }
  const { Component } = def;
  return (
    <EmbeddedPageContext.Provider value={true}>
      <div className="h-full min-h-0 min-w-0 overflow-hidden">
        <Component
          ws={bench.ws}
          sel={params.sel ?? null}
          onSel={(sel) => {
            api.updateParameters({ ...params, sel });
            bench.touch();
          }}
        />
      </div>
    </EmbeddedPageContext.Provider>
  );
}
