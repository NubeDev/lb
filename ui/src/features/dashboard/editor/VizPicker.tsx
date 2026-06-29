// The visualization picker (viz panel-editor scope: the viz picker; chart-types scope: the view set).
// Phase 1 offers `timeseries` (the shipped `chart` aliases to it); the structure lists every Phase-2
// view as "coming soon" so the surface is visibly the standard set, not a one-off. Switching the viz
// preserves the targets/fieldConfig (panel-editor scope, Risks: "viz switch must preserve compatible
// state") — only the per-view `options` reset, handled by the caller. One responsibility: pick a view.

import { Button } from "@/components/ui/button";
import type { View } from "@/lib/dashboard";

/** The Phase-1 buildable views + the named Phase-2 set (shown disabled, honest about what ships). */
const PHASE1: { id: View; label: string }[] = [{ id: "timeseries", label: "Time series" }];
const PHASE2: { id: View; label: string }[] = [
  { id: "barchart", label: "Bar chart" },
  { id: "stat", label: "Stat" },
  { id: "gauge", label: "Gauge" },
  { id: "bargauge", label: "Bar gauge" },
  { id: "table", label: "Table" },
  { id: "piechart", label: "Pie chart" },
];

interface Props {
  view: View;
  onChange: (view: View) => void;
}

export function VizPicker({ view, onChange }: Props) {
  return (
    <div className="grid gap-2" aria-label="visualization picker">
      <div className="flex flex-wrap gap-1.5">
        {PHASE1.map((v) => (
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
        {PHASE2.map((v) => (
          <Button
            key={v.id}
            variant="outline"
            size="sm"
            disabled
            title="Coming in Phase 2"
            className="h-auto cursor-not-allowed border-dashed px-2.5 py-1 text-muted/50"
          >
            {v.label}
          </Button>
        ))}
      </div>
    </div>
  );
}
