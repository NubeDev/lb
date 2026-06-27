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
import { cellView } from "@/lib/dashboard";
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
}

/** Render `cell` by its v2 view. Returns the right view component; an unknown view degrades honestly. */
export function WidgetView({ cell, installed = [], workspace, label }: Props) {
  const view = cellView(cell);
  const tools = cellTools(cell);
  const options = cell.options;

  if (view.startsWith("ext:")) {
    return <ExtWidget viewKey={view} installed={installed} workspace={workspace} />;
  }

  switch (view) {
    case "chart":
      return <ChartView source={cell.source} tools={tools} options={options} label={label} />;
    case "stat":
      return <StatView source={cell.source} tools={tools} options={options} label={label} />;
    case "gauge":
      return <GaugeView source={cell.source} tools={tools} options={options} label={label} />;
    case "table":
      return <TableView source={cell.source} tools={tools} options={options} label={label} />;
    case "plot":
      return <ScriptedView engine="plot" tools={tools} options={options} />;
    case "d3":
      return <ScriptedView engine="d3" tools={tools} options={options} />;
    case "template":
      return <ScriptedView engine="template" tools={tools} options={options} />;
    case "switch":
      return (
        <SwitchControl source={cell.source} action={cell.action} tools={tools} options={options} label={label} />
      );
    case "slider":
      return <SliderControl action={cell.action} tools={tools} options={options} label={label} />;
    case "button":
      return <ButtonControl action={cell.action} tools={tools} options={options} label={label} />;
    default:
      return (
        <div className="flex h-full items-center justify-center text-xs text-muted" role="status">
          unsupported view: {view}
        </div>
      );
  }
}
