// The gauge widget — the latest numeric value as a fill against a [min,max] range (dashboard scope).
// Reads via `useSeries` (backfill + live SSE). `min`/`max` come from the cell `options` (default
// 0..100). A non-numeric or un-granted binding degrades to an honest empty/denied state.

import { useSeries } from "../useSeries";
import { asNumber } from "./num";
import { WidgetHeader, WidgetMessage } from "./chrome";
import type { Binding } from "@/lib/dashboard";
import type { DashboardSearch } from "@/features/routing/search";

interface Props {
  binding: Binding;
  options?: Record<string, unknown>;
  range?: DashboardSearch;
  label?: string;
}

function opt(options: Record<string, unknown> | undefined, key: string, fallback: number): number {
  const v = options?.[key];
  return typeof v === "number" && Number.isFinite(v) ? v : fallback;
}

export function GaugeWidget({ binding, options, range, label }: Props) {
  const { series, latest, loading, denied } = useSeries(binding, range);
  const min = opt(options, "min", 0);
  const max = opt(options, "max", 100);
  const unit = typeof options?.unit === "string" ? (options.unit as string) : "";

  if (denied) return <WidgetMessage tone="denied">no access to this series</WidgetMessage>;
  if (loading) return <WidgetMessage tone="muted">loading…</WidgetMessage>;

  const value = latest ? asNumber(latest.payload) : null;
  if (value === null) return <WidgetMessage tone="muted">no numeric value</WidgetMessage>;

  // A half-circle gauge: map [min,max] → a 0..1 fraction → a 180° sweep.
  const frac = Math.max(0, Math.min(1, (value - min) / (max - min || 1)));
  const angle = Math.PI * (1 - frac); // π (left) → 0 (right)
  const cx = 50;
  const cy = 50;
  const r = 40;
  const x = cx + r * Math.cos(angle);
  const y = cy - r * Math.sin(angle);

  return (
    <div className="flex h-full flex-col" aria-label={`gauge ${series ?? ""}`}>
      <WidgetHeader label={label ?? series ?? ""} />
      <div className="flex flex-1 flex-col items-center justify-center">
        <svg viewBox="0 0 100 60" className="w-full max-w-[160px]" role="img" aria-label="gauge arc">
          <path d="M10,50 A40,40 0 0 1 90,50" fill="none" stroke="currentColor" strokeWidth={6} className="text-border" />
          <path
            d={`M10,50 A40,40 0 0 1 ${x.toFixed(1)},${y.toFixed(1)}`}
            fill="none"
            stroke="currentColor"
            strokeWidth={6}
            strokeLinecap="round"
            className="text-accent"
          />
        </svg>
        <span className="text-xl font-semibold text-fg" aria-label="gauge value">
          {value}
          {unit}
        </span>
      </div>
    </div>
  );
}
