// The v2 `chart` view — a live line of a source's numeric rows. Reads `source` through the bridge
// (`useSource`: a one-shot read, or a folded `series.watch`/`bus.watch` stream). Recharts owns the SVG
// drawing through the same renderer as the v1 ChartWidget.

import { WidgetHeader, WidgetMessage } from "../widgets/chrome";
import { SeriesLineChart } from "../widgets/recharts";
import type { VarScope } from "@/lib/vars";
import { emptyScope } from "@/lib/vars";
import { useSource } from "../builder/useSource";
import { rowNumber } from "./num";
import type { Source } from "@/lib/dashboard";

interface Props {
  source?: Source;
  tools: string[];
  options?: Record<string, unknown>;
  label?: string;
  scope?: VarScope;
  refreshKey?: number;
}

export function ChartView({ source, tools, options, label, scope = emptyScope(), refreshKey = 0 }: Props) {
  const { rows, loading, denied } = useSource(source, tools, scope, refreshKey);
  const unit = typeof options?.unit === "string" ? (options.unit as string) : "";

  if (denied) return <WidgetMessage tone="denied">no access to this source</WidgetMessage>;
  if (loading) return <WidgetMessage tone="muted">loading…</WidgetMessage>;

  const points = rows.map(rowNumber).filter((n): n is number => n !== null);
  if (points.length === 0) return <WidgetMessage tone="muted">no data yet</WidgetMessage>;

  const last = points[points.length - 1];

  return (
    <div className="flex h-full flex-col" aria-label={`chart ${source?.tool ?? ""}`}>
      <WidgetHeader label={label ?? source?.tool ?? ""} />
      <SeriesLineChart points={points} ariaLabel="chart line" />
      <div className="mt-1 text-right text-xs text-muted" aria-label="chart latest">
        {last}
        {unit}
      </div>
    </div>
  );
}
