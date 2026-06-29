// The gauge widget — the latest numeric value as a fill against a [min,max] range (dashboard scope).
// Reads via `useSeries` (backfill + live SSE). `min`/`max` come from the cell `options` (default
// 0..100). A non-numeric or un-granted binding degrades to an honest empty/denied state.

import { useSeries } from "../useSeries";
import { asNumber } from "./num";
import { WidgetHeader, WidgetMessage } from "./chrome";
import { GaugeArcChart } from "./recharts";
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

  const frac = Math.max(0, Math.min(1, (value - min) / (max - min || 1)));

  return (
    <div className="flex h-full flex-col" aria-label={`gauge ${series ?? ""}`}>
      <WidgetHeader label={label ?? series ?? ""} />
      <div className="flex flex-1 flex-col items-center justify-center">
        <GaugeArcChart fraction={frac} ariaLabel="gauge arc" />
        <span className="text-xl font-semibold text-fg" aria-label="gauge value">
          {value}
          {unit}
        </span>
      </div>
    </div>
  );
}
