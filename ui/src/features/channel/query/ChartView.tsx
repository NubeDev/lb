// The in-channel chart renderer (channels-query-charts scope). It renders a `PlotSpec` — either the
// host's auto-pick (converted from the `ChartSpec` every subscriber agrees on) or the viewer's saved
// customization — through the SHARED `PlotChart`, so an in-channel result looks identical to a dashboard
// panel (real titled axes, gridlines, themed tooltip, legend). RENDER ONLY; the rows + spec are passed
// in (FILE-LAYOUT). The old hand-drawn per-type recharts blocks are gone — one renderer, no drift.

import { PlotChart } from "@/features/charts";
import type { PlotSpec } from "@/lib/charts";

interface Props {
  spec: PlotSpec;
  rows: Record<string, unknown>[];
}

export function ChartView({ spec, rows }: Props) {
  return (
    <div className="h-56 w-full">
      <PlotChart rows={rows} spec={spec} ariaLabel={`${spec.type} chart`} />
    </div>
  );
}
