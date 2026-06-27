// The time-series chart widget — a live line of a series' numeric samples (dashboard scope). Backfills
// history with `series.read` and folds live samples from the series SSE (via `useSeries`); the line
// advances as motion arrives. Hand-drawn SVG (no charting dep) keeps it light and trivially testable.

import { useSeries } from "../useSeries";
import { asNumber } from "./num";
import { WidgetHeader, WidgetMessage } from "./chrome";
import type { Binding } from "@/lib/dashboard";

interface Props {
  binding: Binding;
  options?: Record<string, unknown>;
  label?: string;
}

export function ChartWidget({ binding, options, label }: Props) {
  const { series, samples, loading, denied } = useSeries(binding);
  const unit = typeof options?.unit === "string" ? (options.unit as string) : "";

  if (denied) return <WidgetMessage tone="denied">no access to this series</WidgetMessage>;
  if (loading) return <WidgetMessage tone="muted">loading…</WidgetMessage>;

  const points = samples.map((s) => asNumber(s.payload)).filter((n): n is number => n !== null);
  if (points.length === 0) return <WidgetMessage tone="muted">no samples yet</WidgetMessage>;

  const min = Math.min(...points);
  const max = Math.max(...points);
  const span = max - min || 1;
  const w = 100;
  const h = 40;
  const path = points
    .map((p, i) => {
      const x = points.length === 1 ? w : (i / (points.length - 1)) * w;
      const y = h - ((p - min) / span) * h;
      return `${i === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");
  const last = points[points.length - 1];

  return (
    <div className="flex h-full flex-col" aria-label={`chart ${series ?? ""}`}>
      <WidgetHeader label={label ?? series ?? ""} />
      <svg
        viewBox={`0 0 ${w} ${h}`}
        preserveAspectRatio="none"
        className="min-h-0 flex-1"
        role="img"
        aria-label={`series ${series} line`}
      >
        <path d={path} fill="none" stroke="currentColor" strokeWidth={1} className="text-accent" />
      </svg>
      <div className="mt-1 text-right text-xs text-muted" aria-label="chart latest">
        {last}
        {unit}
      </div>
    </div>
  );
}
