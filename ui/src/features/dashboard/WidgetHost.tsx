// The widget host — renders one grid cell by its `widget_type` (dashboard scope). Phase 1 dispatches
// to the built-in chart/stat/gauge components, all bound to real series through the same `useSeries`
// data contract. This is the seam Phase 2 swaps: an `ext:<id>` type will render a federated remote /
// sandboxed iframe through the host-mediated bridge — the binding contract stays byte-for-byte.

import { ChartWidget } from "./widgets/ChartWidget";
import { StatWidget } from "./widgets/StatWidget";
import { GaugeWidget } from "./widgets/GaugeWidget";
import type { Cell } from "@/lib/dashboard";

export function WidgetHost({ cell }: { cell: Cell }) {
  switch (cell.widget_type) {
    case "chart":
      return <ChartWidget binding={cell.binding} options={cell.options} />;
    case "stat":
      return <StatWidget binding={cell.binding} options={cell.options} />;
    case "gauge":
      return <GaugeWidget binding={cell.binding} options={cell.options} />;
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
