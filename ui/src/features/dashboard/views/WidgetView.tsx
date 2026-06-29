// The v2 widget dispatcher — render one cell by its `view` (widget-builder scope, "The widget
// taxonomy"). The single seam over the whole render vocabulary: read views (chart/stat/gauge/table)
// run the source through the bridge; scripted views (plot/d3/template) sandbox author code; control
// views (switch/slider/button) call a write tool; `ext:<id>/<widget>` mounts an extension tile trust-
// tiered. The host re-checks the cap + workspace on every bridged call regardless of this dispatch.
//
// The cell's TOOL SET (what the bridge may forward) is its declared `source`/`action` tools — the host
// intersects with the install grant server-side, so a cell naming a tool it wasn't granted is denied
// there even if it reached the bridge.

import type { Cell } from "@/lib/dashboard";
import { cellView, cellLabel } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import { emptyScope } from "@/lib/vars";
import type { ExtRow } from "@/lib/ext/ext.api";
import { ChartView } from "./ChartView";
import { TimeseriesView } from "./timeseries/TimeseriesView";
import { StatPanel } from "./stat/StatPanel";
import { GaugePanel } from "./gauge/GaugePanel";
import { BarGaugePanel } from "./bargauge/BarGaugePanel";
import { TablePanel } from "./table/TablePanel";
import { BarChartPanel } from "./barchart/BarChartPanel";
import { PieChartPanel } from "./piechart/PieChartPanel";
import { ScriptedView } from "./ScriptedView";
import { SwitchControl } from "./SwitchControl";
import { SliderControl } from "./SliderControl";
import { ButtonControl } from "./ButtonControl";
import { ExtWidget } from "../builder/ExtWidget";

/** The tools a cell may forward through the bridge = its source + action + v3 target tools (host ∩
 *  grant). v3 cells carry `sources[]` (targets); a v2 cell carries the single `source`. Both are folded
 *  in so the bridge leash covers every tool the cell can read, regardless of contract version. */
export function cellTools(cell: Cell): string[] {
  const tools = new Set<string>();
  if (cell.source?.tool) tools.add(cell.source.tool);
  for (const t of cell.sources ?? []) if (t.tool) tools.add(t.tool);
  if (cell.action?.tool) tools.add(cell.action.tool);
  // A read-only stat/gauge control may also read its own source; covered above. The series read sibling
  // of a watch source is added by the builder when it sets the source.
  return [...tools];
}

interface Props {
  cell: Cell;
  /** Installed extensions (from `ext.list`) — needed to mount an `ext:<id>/<widget>` cell. */
  installed?: ExtRow[];
  workspace: string;
  label?: string;
  /** The resolved variable scope (widget-config-vars Slice 3). Interpolated into every cell call +
   *  handed to an extension tile as `ctx.vars`/`ctx.timeRange` (the shell resolves it; the cell never
   *  resolves identity or query vars itself). */
  scope?: VarScope;
  /** Auto-refresh tick (Slice 4) — re-runs a read cell's source on each interval. */
  refreshKey?: number;
}

/** Render `cell` by its v2 view. Returns the right view component; an unknown view degrades honestly. */
export function WidgetView({
  cell,
  installed = [],
  workspace,
  label,
  scope = emptyScope(),
  refreshKey = 0,
}: Props) {
  const view = cellView(cell);
  const tools = cellTools(cell);
  const options = cell.options;
  // Default the header label to the cell's effective label (title → derived) so every built-in view
  // shows the configured title (widget-config-vars scope, Slice 1).
  label = label ?? cellLabel(cell);

  if (view.startsWith("ext:")) {
    return <ExtWidget viewKey={view} installed={installed} workspace={workspace} scope={scope} />;
  }

  switch (view) {
    case "timeseries":
      // v3 timeseries (the canonical id `chart` aliases to via `cellView`). The full Grafana option
      // surface + the fieldConfig render path; data through the one `usePanelData` hook.
      return <TimeseriesView cell={cell} label={label} scope={scope} refreshKey={refreshKey} />;
    case "chart":
      // Defensive: `cellView` canonicalizes `chart` → `timeseries`, so this is unreachable for a real
      // cell, but kept so a direct `view:"chart"` (e.g. a hand-built test cell bypassing cellView)
      // still renders the v2 chart rather than the unsupported-view fallback.
      return <ChartView source={cell.source} tools={tools} options={options} label={label} scope={scope} refreshKey={refreshKey} />;
    case "stat":
      // v3 stat (also the canonical id for a v2 `stat` cell): the reduceOptions frame→value bridge +
      // the full Grafana stat option surface + the fieldConfig render path; data through `usePanelData`.
      return <StatPanel cell={cell} label={label} scope={scope} refreshKey={refreshKey} />;
    case "gauge":
      return <GaugePanel cell={cell} label={label} scope={scope} refreshKey={refreshKey} />;
    case "bargauge":
      return <BarGaugePanel cell={cell} label={label} scope={scope} refreshKey={refreshKey} />;
    case "table":
      return <TablePanel cell={cell} label={label} scope={scope} refreshKey={refreshKey} />;
    case "barchart":
      return <BarChartPanel cell={cell} label={label} scope={scope} refreshKey={refreshKey} />;
    case "piechart":
      return <PieChartPanel cell={cell} label={label} scope={scope} refreshKey={refreshKey} />;
    case "plot":
      return <ScriptedView engine="plot" tools={tools} options={options} />;
    case "d3":
      return <ScriptedView engine="d3" tools={tools} options={options} />;
    case "template":
      return <ScriptedView engine="template" tools={tools} options={options} />;
    case "switch":
      return (
        <SwitchControl source={cell.source} action={cell.action} tools={tools} options={options} label={label} scope={scope} />
      );
    case "slider":
      return <SliderControl action={cell.action} tools={tools} options={options} label={label} scope={scope} />;
    case "button":
      return <ButtonControl action={cell.action} tools={tools} options={options} label={label} scope={scope} />;
    default:
      return (
        <div className="flex h-full items-center justify-center text-xs text-muted" role="status">
          unsupported view: {view}
        </div>
      );
  }
}
