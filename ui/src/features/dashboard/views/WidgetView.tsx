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
import { StatView } from "./StatView";
import { GaugeView } from "./GaugeView";
import { TableView } from "./TableView";
import { ScriptedView } from "./ScriptedView";
import { SwitchControl } from "./SwitchControl";
import { SliderControl } from "./SliderControl";
import { ButtonControl } from "./ButtonControl";
import { ExtWidget } from "../builder/ExtWidget";

/** The tools a cell may forward through the bridge = its source + action tools (host ∩ grant). */
export function cellTools(cell: Cell): string[] {
  const tools = new Set<string>();
  if (cell.source?.tool) tools.add(cell.source.tool);
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
    case "chart":
      return <ChartView source={cell.source} tools={tools} options={options} label={label} scope={scope} refreshKey={refreshKey} />;
    case "stat":
      return <StatView source={cell.source} tools={tools} options={options} label={label} scope={scope} refreshKey={refreshKey} />;
    case "gauge":
      return <GaugeView source={cell.source} tools={tools} options={options} label={label} scope={scope} refreshKey={refreshKey} />;
    case "table":
      return <TableView source={cell.source} tools={tools} options={options} label={label} scope={scope} refreshKey={refreshKey} />;
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
