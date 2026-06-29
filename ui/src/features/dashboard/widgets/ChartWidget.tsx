// The time-series chart widget — a live line of a series' numeric samples (dashboard scope). Backfills
// history with `series.read` and folds live samples from the series SSE (via `useSeries`); the line
// advances as motion arrives. Recharts owns the SVG drawing so widget charts share one charting stack.

import { useSeries } from "../useSeries";
import { asNumber } from "./num";
import { WidgetHeader, WidgetMessage } from "./chrome";
import { SeriesLineChart } from "./recharts";
import type { Binding } from "@/lib/dashboard";
import type { DashboardSearch } from "@/features/routing/search";

interface Props {
  binding: Binding;
  options?: Record<string, unknown>;
  range?: DashboardSearch;
  label?: string;
}

export function ChartWidget({ binding, options, range, label }: Props) {
  const { series, samples, loading, denied } = useSeries(binding, range);
  const unit = typeof options?.unit === "string" ? (options.unit as string) : "";

  if (denied) return <WidgetMessage tone="denied">no access to this series</WidgetMessage>;
  if (loading) return <WidgetMessage tone="muted">loading…</WidgetMessage>;

  const points = samples.map((s) => asNumber(s.payload)).filter((n): n is number => n !== null);
  if (points.length === 0) return <WidgetMessage tone="muted">no samples yet</WidgetMessage>;

  const last = points[points.length - 1];

  return (
    <div className="flex h-full flex-col" aria-label={`chart ${series ?? ""}`}>
      <WidgetHeader label={label ?? series ?? ""} />
      <SeriesLineChart points={points} ariaLabel={`series ${series} line`} />
      <div className="mt-1 text-right text-xs text-muted" aria-label="chart latest">
        {last}
        {unit}
      </div>
    </div>
  );
}
