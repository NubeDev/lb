// One explore tab (data-studio scope v2) — a picked source rendered live through the SHIPPED render
// path (`WidgetHost` → `usePanelData` → `viz.query` — no parallel renderer/query), toggleable
// Table / Chart / JSON so the user inspects the real shape of the data. N of these are open at once;
// each owns nothing but its tab-config draft cell. "Build panel" hands the explored draft to a new
// builder tab.

import { Hammer } from "lucide-react";

import { Button } from "@/components/ui/button";
import { WidgetHost } from "@/features/dashboard/WidgetHost";
import type { Cell, View } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import type { ExtRow } from "@/lib/ext/ext.api";
import type { DashboardSearch } from "@/features/routing/search";

import { EXPLORE_VIEWS, type ExploreConfig } from "../workbenchModel";

interface Props {
  config: ExploreConfig;
  ws: string;
  scope: VarScope;
  range?: DashboardSearch;
  installed?: ExtRow[];
  /** Persist a config change into this tab's FlexLayout node (the view toggle). */
  onConfigChange: (config: ExploreConfig) => void;
  /** Open a builder tab seeded from the explored draft. */
  onBuild: (cell: Cell) => void;
}

export function ExplorePane({ config, ws, scope, range, installed, onConfigChange, onBuild }: Props) {
  const cell = config.cell;
  const view = (cell.view as View) ?? "table";
  const setView = (v: View) => onConfigChange({ ...config, cell: { ...cell, view: v } });

  return (
    <div className="flex h-full min-h-0 flex-col gap-2 p-2">
      <div className="flex items-center gap-1">
        <div role="tablist" aria-label="preview view" className="flex items-center gap-1">
          {EXPLORE_VIEWS.map((t) => (
            <Button
              key={t.view}
              role="tab"
              aria-selected={view === t.view}
              size="sm"
              variant={view === t.view ? "default" : "ghost"}
              onClick={() => setView(t.view)}
            >
              {t.label}
            </Button>
          ))}
        </div>
        <div className="flex-1" />
        <Button aria-label="build panel from explore" size="sm" variant="outline" onClick={() => onBuild(cell)}>
          <Hammer size={12} /> Build panel
        </Button>
      </div>
      <div className="min-h-0 flex-1 rounded-md border border-border bg-panel p-2" data-testid="explore-preview">
        <WidgetHost cell={cell} range={range} workspace={ws} scope={scope} installed={installed} />
      </div>
    </div>
  );
}
