// The widget host — renders one grid cell (dashboard + widget-builder scopes). A v1 cell (series
// binding, `widget_type`) dispatches to the built-in chart/stat/gauge components over `useSeries`. A
// v2 cell (a `view` + a `{tool,args}` source, any granted tool read OR write) dispatches to the v2
// `WidgetView` over the host-mediated bridge — the generalization the widget-builder scope freezes.
// The seam: a v2 marker (`v:2`, a `view`, or a `source`) routes to `WidgetView`; else the v1 path.

import { ChartWidget } from "./widgets/ChartWidget";
import { StatWidget } from "./widgets/StatWidget";
import { GaugeWidget } from "./widgets/GaugeWidget";
import { WidgetView } from "./views/WidgetView";
import type { Cell } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import type { ExtRow } from "@/lib/ext/ext.api";
import type { DashboardSearch } from "@/features/routing/search";

/** True when a cell uses the v2 tool-bound contract (a version marker, a view, or a source). */
function isV2(cell: Cell): boolean {
  return cell.v === 2 || !!cell.view || !!cell.source?.tool;
}

export function WidgetHost({
  cell,
  range,
  installed,
  workspace,
  scope,
  refreshKey,
}: {
  cell: Cell;
  range?: DashboardSearch;
  installed?: ExtRow[];
  workspace?: string;
  scope?: VarScope;
  refreshKey?: number;
}) {
  // A ref cell whose panel the host could not resolve (deleted/unshared/unreadable) hydrates with
  // `panelMissing` — render the honest "panel not accessible" placeholder, never a blank chart or a
  // crash (library-panels scope, "Dangling refs").
  if (cell.panelMissing) {
    return (
      <div
        className="flex h-full flex-col items-center justify-center gap-1 text-center text-xs text-muted"
        role="status"
        data-testid="panel-missing"
      >
        <span className="font-medium">Panel not accessible</span>
        <span className="opacity-70">This library panel was removed or isn’t shared with you.</span>
      </div>
    );
  }
  if (isV2(cell)) {
    return (
      <WidgetView
        cell={cell}
        installed={installed}
        workspace={workspace ?? ""}
        scope={scope}
        refreshKey={refreshKey}
      />
    );
  }
  switch (cell.widget_type) {
    case "chart":
      return <ChartWidget binding={cell.binding} options={cell.options} range={range} />;
    case "stat":
      return <StatWidget binding={cell.binding} options={cell.options} range={range} />;
    case "gauge":
      return <GaugeWidget binding={cell.binding} options={cell.options} range={range} />;
    default:
      // Phase 2 (federated widgets) renders `ext:<id>` here through the bridge; Phase 1 shows an
      // honest "not available" placeholder rather than a blank cell.
      return (
        <div className="flex h-full items-center justify-center text-xs text-muted" role="status">
          unsupported widget: {cell.widget_type}
        </div>
      );
  }
}
