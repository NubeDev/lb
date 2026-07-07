// Data Studio (data-studio-10x scope, phase 1: the Dockview workbench) — a dockable data workbench:
// the shared `StudioRail` (Sources/Library, the RosterRail-kit chrome + minimize) on the left, and a
// Dockview dock of N panel-builder tabs beside it; drag to split/tab/dock/float, maximize, pop out,
// close; double-click a tab to rename. The dock arrangement (incl. every tab's draft cell) persists
// PER USER in SurrealDB (`layout.get`/`set`, member-owned — rule 4) as the versioned
// `{engine:"dockview", model}` record. The panes compose the shipped substrate — `@nube/source-picker`,
// `WidgetHost`/`viz.query` (ONE render/query path), the panel-kit logic layer, the `panel.*` library
// asset — this file owns only the workbench: the dock mount, the open-tab actions, and the rail state.

import { useMemo, useState } from "react";
import { FlaskConical, Plus, RotateCcw, X } from "lucide-react";
import { DockviewReact, type IDockviewPanelProps } from "dockview-react";

import "dockview-react/dist/styles/dockview.css";
import "./datastudio-dock.css";

import { AppPage } from "@/components/app/page";
import { CollapsedRail } from "@/components/app/rail-collapsed";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { DashboardCacheProvider } from "@/features/dashboard/cache/DashboardQueryProvider";
import { useVarScope } from "@/features/dashboard/vars/useVarScope";
import { defaultOptionsForView } from "@/features/panel-builder/viewOptions";
import { defaultCell, draftFromSelection } from "@/lib/panel-kit";
import type { Cell } from "@/lib/dashboard";
import type { DashboardSearch } from "@/features/routing/search";
import { useAppRoutingContext } from "@/features/routing/RoutingContextProvider";
import type { SourceSelection } from "@nube/source-picker";

import { mintTabId, viewPaneId, type BuilderConfig, type ViewPaneConfig } from "./workbenchModel";
import { useWorkbenchLayout } from "./useWorkbenchLayout";
import { WorkbenchContext } from "./workbenchContext";
import { WorkbenchTab } from "./WorkbenchTab";
import { LB_DOCKVIEW_THEME } from "./dockviewTheme";
import { StudioRail } from "./StudioRail";
import { OpenViewMenu } from "./OpenViewMenu";
import { viewPane } from "./workbenchPanes";
import { BuilderDockPanel } from "./panes/BuilderDockPanel";
import { ViewDockPanel } from "./panes/ViewDockPanel";

interface Props {
  ws: string;
  range?: DashboardSearch;
  onSearchChange?: (search: DashboardSearch) => void;
}

/** The dock's panel components — module-level so the map is stable across renders. */
const PANE_COMPONENTS: Record<string, React.FunctionComponent<IDockviewPanelProps>> = {
  builder: BuilderDockPanel as React.FunctionComponent<IDockviewPanelProps>,
  view: ViewDockPanel as React.FunctionComponent<IDockviewPanelProps>,
};

/** The empty-dock watermark — the default workbench before any tab opens. */
function Watermark() {
  return (
    <div className="flex h-full items-center justify-center p-6 text-center text-xs text-muted">
      Pick a source from the rail or open a New panel to start.
    </div>
  );
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
  const { caps, allowed } = useAppRoutingContext();
  const bench = useWorkbenchLayout(ws, caps);
  // Bumped when any builder tab saves a library panel, so the rail's Library tab re-lists.
  const [libraryTick, setLibraryTick] = useState(0);
  // The studio rail (Sources/Library) — same minimize/expand as every other surface's roster rail.
  const [railOpen, setRailOpen] = useState(true);

  // The variable scope (built-ins + any `?var-` URL selections) — the studio has no dashboard
  // variable defs, so it resolves the same shipped scope with an empty def set.
  const scope = useVarScope([], range, "data-studio", ws);

  const openBuilder = (cell: Cell, name: string, savedAs?: string) => {
    if (!bench.api) return;
    const id = mintTabId("builder");
    // A builder draft keeps its own stable cell key (the editor re-seeds on key change).
    const params: BuilderConfig = { cell: { ...cell, i: id }, savedAs };
    bench.api.addPanel({ id, component: "builder", title: name, params });
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

  // Pages-as-panes: ONE pane per view kind (deterministic id) — opening an open kind refocuses it.
  const openView = (kind: string) => {
    if (!bench.api) return;
    const existing = bench.api.getPanel(viewPaneId(kind));
    if (existing) {
      existing.api.setActive();
      return;
    }
    const def = viewPane(kind);
    if (!def) return;
    const params: ViewPaneConfig = { kind, sel: null };
    bench.api.addPanel({ id: viewPaneId(kind), component: "view", title: def.title, params });
  };

  const benchCtx = useMemo(
    () => ({
      ws,
      scope,
      onSavedToLibrary: () => setLibraryTick((t) => t + 1),
      touch: bench.touch,
    }),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [ws, scope, bench.api],
  );

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
          <Button aria-label="new panel" size="sm" variant="ghost" onClick={newPanel}>
            <Plus size={12} /> New panel
          </Button>
          <OpenViewMenu
            allowed={allowed}
            isOpen={(kind) => !!bench.api?.getPanel(viewPaneId(kind))}
            onOpenView={openView}
            onNewPanel={newPanel}
          />
        </div>
      }
    >
      {railOpen ? (
        <StudioRail
          ws={ws}
          onOpenSource={openExplore}
          onOpenPanel={(panelId, title, cell) => openBuilder(cell, title, panelId)}
          libraryTick={libraryTick}
          onCollapse={() => setRailOpen(false)}
        />
      ) : (
        <CollapsedRail noun="studio" onExpand={() => setRailOpen(true)} />
      )}
      <div className="flex min-w-0 flex-1 flex-col">
        {bench.resetNotice && (
          <div
            role="status"
            aria-label="layout reset notice"
            className="flex items-center gap-2 border-b border-border bg-panel px-3 py-1.5 text-xs text-muted"
          >
            <span>
              Your saved workbench layout was from the previous engine and has been reset. Saved
              library panels are untouched.
            </span>
            <Button
              aria-label="dismiss layout reset notice"
              variant="ghost"
              size="icon"
              className="ml-auto h-6 w-6"
              onClick={bench.dismissResetNotice}
            >
              <X size={12} />
            </Button>
          </div>
        )}
        <div className="data-studio-dock" data-testid="data-studio-dock">
          {bench.ready && (
            <WorkbenchContext.Provider value={benchCtx}>
              <DockviewReact
                components={PANE_COMPONENTS}
                defaultTabComponent={WorkbenchTab}
                watermarkComponent={Watermark}
                theme={LB_DOCKVIEW_THEME}
                onReady={bench.onReady}
              />
            </WorkbenchContext.Provider>
          )}
        </div>
      </div>
    </AppPage>
  );
}
