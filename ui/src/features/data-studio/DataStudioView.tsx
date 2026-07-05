// Data Studio (data-studio scope, "v2: multi-pane workbench" + the rail cleanup) — a dockable data
// workbench: the shared `StudioRail` (Sources/Library, the RosterRail-kit chrome + minimize) on the
// left, and a FlexLayout dock of N panel-builder tabs beside it; drag to split/tab/dock/float/close,
// double-click to rename; the dock arrangement (incl. every tab's draft cell) persists PER USER in
// SurrealDB (`layout.get`/`set`, member-owned — rule 4). The panes compose the shipped substrate —
// `@nube/source-picker`, `WidgetHost`/`viz.query` (ONE render/query path), the panel-kit logic layer,
// the `panel.*` library asset — this file owns only the workbench: the FlexLayout model, the tab
// factory, the open-tab actions, and the rail open/collapsed state.

import { useState } from "react";
import { FlaskConical, Plus, RotateCcw } from "lucide-react";
import { Actions, DockLocation, Layout, type TabNode } from "flexlayout-react";

import "flexlayout-react/style/light.css";
import "./datastudio-dock.css";

import { AppPage } from "@/components/app/page";
import { CollapsedRail } from "@/components/app/rail-collapsed";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useSourcePicker } from "@/features/dashboard/builder/useSourcePicker";
import { DashboardCacheProvider } from "@/features/dashboard/cache/DashboardQueryProvider";
import { useVarScope } from "@/features/dashboard/vars/useVarScope";
import { defaultOptionsForView } from "@/features/panel-builder/viewOptions";
import { defaultCell, draftFromSelection } from "@/lib/panel-kit";
import type { Cell } from "@/lib/dashboard";
import type { DashboardSearch } from "@/features/routing/search";
import { useAppRoutingContext } from "@/features/routing/RoutingContextProvider";
import type { SourceSelection } from "@nube/source-picker";

import {
  builderTabJson,
  mintTabId,
  MAIN_TABSET_ID,
  type BuilderConfig,
} from "./workbenchModel";
import { useWorkbenchLayout } from "./useWorkbenchLayout";
import { StudioRail } from "./StudioRail";
import { BuilderTabPane } from "./panes/BuilderTabPane";

interface Props {
  ws: string;
  range?: DashboardSearch;
  onSearchChange?: (search: DashboardSearch) => void;
}

/** The studio, wrapped in the shared per-visit read cache (keyed on `ws`) so the picker bundle +
 *  `viz.query` de-dup are the SAME shared fetches the dashboard uses. */
export function DataStudioView(props: Props) {
  return (
    <DashboardCacheProvider key={props.ws} ws={props.ws}>
      <DataStudioInner {...props} />
    </DashboardCacheProvider>
  );
}

function DataStudioInner({ ws, range, onSearchChange }: Props) {
  const picker = useSourcePicker(ws);
  const { caps } = useAppRoutingContext();
  const bench = useWorkbenchLayout(ws, caps);
  // Bumped when any builder tab saves a library panel, so the rail's Library tab re-lists.
  const [libraryTick, setLibraryTick] = useState(0);
  // The studio rail (Sources/Library) — same minimize/expand as every other surface's roster rail.
  const [railOpen, setRailOpen] = useState(true);

  // The variable scope (built-ins + any `?var-` URL selections) — the studio has no dashboard
  // variable defs, so it resolves the same shipped scope with an empty def set.
  const scope = useVarScope([], range, "data-studio", ws);

  /** Add a tab to the model — into the main tabset (recreated if the user closed it). */
  const addTab = (json: ReturnType<typeof builderTabJson>) => {
    const model = bench.model;
    if (!model) return;
    const active = model.getActiveTabset();
    const target =
      active?.getId() ?? (model.getNodeById(MAIN_TABSET_ID) ? MAIN_TABSET_ID : model.getFirstTabSet().getId());
    model.doAction(Actions.addNode(json, target, DockLocation.CENTER, -1, true));
  };

  const openBuilder = (cell: Cell, name: string, savedAs?: string) => {
    const id = mintTabId("builder");
    // A builder draft keeps its own stable cell key (the editor re-seeds on key change).
    addTab(builderTabJson(id, name, { cell: { ...cell, i: id }, savedAs }));
  };

  // v3: picking a source opens a BUILDER tab directly (the stacked query/preview view) — no read-only
  // explore hop. Seed a chart draft from the selection; the builder's own preview + viz picker cover
  // data inspection, and its Query section shows the SQL editor when the source needs it.
  const openExplore = (sel: SourceSelection, label: string) => {
    const cell = draftFromSelection(sel, "timeseries", "explore", defaultOptionsForView("timeseries"));
    openBuilder(cell, label);
  };

  const newPanel = () =>
    openBuilder(defaultCell("timeseries", "new", undefined, defaultOptionsForView("timeseries")), "New panel");

  /** The FlexLayout tab factory — one component per tab kind, config in/out through the model. */
  const factory = (node: TabNode) => {
    const model = bench.model;
    const updateConfig = (config: unknown) => {
      model?.doAction(Actions.updateNodeAttributes(node.getId(), { config }));
    };
    switch (node.getComponent()) {
      case "builder": {
        const config = node.getConfig() as BuilderConfig;
        return (
          <BuilderTabPane
            config={config}
            ws={ws}
            scope={scope}
            onConfigChange={updateConfig}
            onSavedToLibrary={() => setLibraryTick((t) => t + 1)}
          />
        );
      }
      default:
        return <div className="p-3 text-xs text-muted">Unknown pane.</div>;
    }
  };

  return (
    <AppPage
      label="data studio"
      icon={FlaskConical}
      title="Data Studio"
      description="Explore any data source and build reusable panels — many tabs, side by side."
      workspace={ws}
      actions={
        <div className="flex items-center gap-2">
          {range && (
            <div className="hidden items-center gap-1 text-xs text-muted md:flex">
              <Input
                aria-label="explore range from"
                className="h-8 w-[8.5rem] text-xs"
                type="date"
                value={range.from}
                onChange={(e) => onSearchChange?.({ ...range, from: e.target.value })}
              />
              <span>to</span>
              <Input
                aria-label="explore range to"
                className="h-8 w-[8.5rem] text-xs"
                type="date"
                value={range.to}
                onChange={(e) => onSearchChange?.({ ...range, to: e.target.value })}
              />
            </div>
          )}
          <Button aria-label="reset workbench layout" size="sm" variant="ghost" onClick={bench.reset}>
            <RotateCcw size={12} /> Reset layout
          </Button>
          <Button aria-label="new panel" size="sm" onClick={newPanel}>
            <Plus size={12} /> New panel
          </Button>
        </div>
      }
    >
      {railOpen ? (
        <StudioRail
          entries={picker.entries}
          loading={picker.loading}
          onOpenSource={openExplore}
          onOpenPanel={(panelId, title, cell) => openBuilder(cell, title, panelId)}
          libraryTick={libraryTick}
          onCollapse={() => setRailOpen(false)}
        />
      ) : (
        <CollapsedRail noun="studio" onExpand={() => setRailOpen(true)} />
      )}
      <div className="data-studio-dock" data-testid="data-studio-dock">
        {bench.model && (
          <Layout
            model={bench.model}
            factory={factory}
            onModelChange={bench.onModelChange}
            popoutURL="/popout.html"
          />
        )}
      </div>
    </AppPage>
  );
}
