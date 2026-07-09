// The visualization picker (viz panel-editor scope: the viz picker; chart-types scope: the view set +
// "Result-shape ↔ type validation"). Phase 1+2 offer the everyday standard set
// (timeseries/barchart/stat/gauge/bargauge/table/piechart). The picker offers only the views a target's
// data shape can HONESTLY fill (`viewFitsShape`) — a scalar can't be a table, tabular rows can't be a
// gauge — disabling the rest rather than hiding them (honest about why). Switching the viz preserves the
// targets/fieldConfig (panel-editor scope, Risks); only the per-view `options` reset, by the caller.
//
// One responsibility: pick a view, shape-validated.

import { Button } from "@/components/ui/button";
import type { View } from "@/lib/dashboard";
import { type ResultShape, viewFitsShape } from "@/features/dashboard/views/shape";

/** The buildable standard set (Phase 1+2), in Grafana's panel-picker order. */
const VIEWS: { id: View; label: string }[] = [
  { id: "timeseries", label: "Time series" },
  { id: "barchart", label: "Bar chart" },
  { id: "stat", label: "Stat" },
  { id: "gauge", label: "Gauge" },
  { id: "bargauge", label: "Bar gauge" },
  { id: "piechart", label: "Pie chart" },
  { id: "table", label: "Table" },
  // The AI-authored generative widget (genui-scope). Not shape-gated (`viewFitsShape` returns true for
  // non-standard views) — its data comes through arbitrary `sources[]` the agent binds, not one query
  // shape. Picking it swaps the Panel-options tab to the "AI widget" author surface.
  { id: "genui", label: "AI widget" },
  // The eval-free HTML template — author markup bound to the source rows via `{{path}}`/`{{#each}}`,
  // rendered IN-PROCESS (render-template-inprocess scope). Not shape-gated (data flows through any
  // `sources[]` like genui); picking it swaps the Panel-options tab to the template body editor. Stays
  // on the standard viz row (a template is a data view, not a flow control/read view).
  { id: "template", label: "Template" },
  // The insights triage list (insights-package-scope) — a list of the workspace's findings raised by
  // rules/flows/agents. Not shape-gated: it isn't source-bound (it reads the `insight.*` verbs through
  // the shell's InsightsClient), so it needs no target — picking it clears the source requirement.
  { id: "insights", label: "Insights" },
];

/** The flow control/read views (flow-dashboard-binding-ux-scope). An INPUT-port binding offers the
 *  WRITE controls (a switch sets a boolean, a slider a number, a JSON control a structured payload); an
 *  OUTPUT-port binding offers the JSON/object read view. Shape-validation doesn't apply (these aren't
 *  query-shaped), so they're always enabled when offered. */
const FLOW_CONTROL_VIEWS: { id: View; label: string }[] = [
  { id: "switch", label: "Switch" },
  { id: "slider", label: "Slider" },
  { id: "json", label: "JSON" },
];
const FLOW_READ_VIEWS: { id: View; label: string }[] = [{ id: "jsonview", label: "JSON view" }];

interface Props {
  view: View;
  onChange: (view: View) => void;
  /** The detected shape of the current target's data — disables views that shape can't honestly fill.
   *  `unknown` (no data yet) leaves every view enabled. */
  shape?: ResultShape;
  /** When a Flows INPUT port is bound: offer the write controls. When a Flows OUTPUT port is bound:
   *  offer the JSON read view. Absent → the standard viz set only. */
  flowKind?: "input" | "output" | null;
}

export function VizPicker({ view, onChange, shape = "unknown", flowKind = null }: Props) {
  // A flow binding swaps the offered set: an input port → write controls (no viz; a control isn't a
  // chart), an output port → the JSON read view alongside the scalar viz (a node value can also stat/
  // gauge). The standard viz set otherwise.
  if (flowKind === "input") {
    return (
      <div className="grid gap-2" aria-label="visualization picker">
        <div className="flex flex-wrap gap-1.5">
          {FLOW_CONTROL_VIEWS.map((v) => (
            <Button
              key={v.id}
              variant={view === v.id ? "default" : "outline"}
              size="sm"
              aria-label={`viz ${v.id}`}
              aria-pressed={view === v.id}
              className="h-auto px-2.5 py-1"
              onClick={() => onChange(v.id)}
            >
              {v.label}
            </Button>
          ))}
        </div>
      </div>
    );
  }
  const offered = flowKind === "output" ? [...FLOW_READ_VIEWS, ...VIEWS] : VIEWS;
  const alwaysOn = new Set<View>(FLOW_READ_VIEWS.map((v) => v.id));
  return (
    <div className="grid gap-2" aria-label="visualization picker">
      <div className="flex flex-wrap gap-1.5">
        {offered.map((v) => {
          const fits = alwaysOn.has(v.id) || viewFitsShape(v.id, shape);
          const selected = view === v.id;
          return (
            <Button
              key={v.id}
              variant={selected ? "default" : "outline"}
              size="sm"
              disabled={!fits && !selected}
              aria-label={`viz ${v.id}`}
              aria-pressed={selected}
              aria-disabled={!fits && !selected}
              title={!fits ? `the current data shape can't honestly fill a ${v.id}` : undefined}
              className={`h-auto px-2.5 py-1 ${!fits && !selected ? "cursor-not-allowed border-dashed text-muted/50" : ""}`}
              onClick={() => fits && onChange(v.id)}
            >
              {v.label}
            </Button>
          );
        })}
      </div>
    </div>
  );
}
