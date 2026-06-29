// The Panel options tab (viz panel-editor scope) — the per-VIZ options (how the chart draws as a whole),
// dispatched by the cell's canonical view to that view's option editor (one file per view, FILE-LAYOUT).
// The per-FIELD options (unit/decimals/draw-style) are the Field tab's job, not this one (Grafana's
// line). One responsibility: route to the right per-view options editor.

import { canonicalView, type View } from "@/lib/dashboard";
import type { EditorState } from "../cellEditorState";
import { TimeseriesOptionsEditor } from "./options/TimeseriesOptionsEditor";
import { StatOptionsEditor } from "./options/StatOptionsEditor";
import { GaugeOptionsEditor } from "./options/GaugeOptionsEditor";
import { BarGaugeOptionsEditor } from "./options/BarGaugeOptionsEditor";
import { BarChartOptionsEditor } from "./options/BarChartOptionsEditor";
import { PieChartOptionsEditor } from "./options/PieChartOptionsEditor";
import { TableOptionsEditor } from "./options/TableOptionsEditor";

interface Props {
  state: EditorState;
  patch: (next: Partial<EditorState>) => void;
}

export function PanelOptionsTab({ state, patch }: Props) {
  // `state.view` may be the canonical id already (PanelEditor passes the canonicalized `stateC`); guard
  // anyway so a raw `chart` alias routes to timeseries.
  const view = canonicalView((state.view || "timeseries") as View);
  switch (view) {
    case "timeseries":
      return <TimeseriesOptionsEditor state={state} patch={patch} />;
    case "stat":
      return <StatOptionsEditor state={state} patch={patch} />;
    case "gauge":
      return <GaugeOptionsEditor state={state} patch={patch} />;
    case "bargauge":
      return <BarGaugeOptionsEditor state={state} patch={patch} />;
    case "barchart":
      return <BarChartOptionsEditor state={state} patch={patch} />;
    case "piechart":
      return <PieChartOptionsEditor state={state} patch={patch} />;
    case "table":
      return <TableOptionsEditor state={state} patch={patch} />;
    default:
      return (
        <div className="py-3 text-xs text-muted" aria-label="panel options tab">
          {view} has no per-viz options.
        </div>
      );
  }
}
