// The v2 `gauge` view — a single value against a min/max + thresholds arc. Reads `source` through the
// bridge (`useSource`); a non-numeric latest degrades to an honest "no value" (never a fake needle).
// Recharts owns the arc drawing through the same renderer as the v1 GaugeWidget.

import { WidgetHeader, WidgetMessage } from "../widgets/chrome";
import { GaugeArcChart } from "../widgets/recharts";
import type { VarScope } from "@/lib/vars";
import { emptyScope } from "@/lib/vars";
import { useSource } from "../builder/useSource";
import { asNumber } from "./num";
import type { Source } from "@/lib/dashboard";

interface Props {
  source?: Source;
  tools: string[];
  options?: Record<string, unknown>;
  label?: string;
  scope?: VarScope;
  refreshKey?: number;
}

export function GaugeView({ source, tools, options, label, scope = emptyScope(), refreshKey = 0 }: Props) {
  const { latest, loading, denied } = useSource(source, tools, scope, refreshKey);
  const min = typeof options?.min === "number" ? (options.min as number) : 0;
  const max = typeof options?.max === "number" ? (options.max as number) : 100;
  const unit = typeof options?.unit === "string" ? (options.unit as string) : "";

  if (denied) return <WidgetMessage tone="denied">no access to this source</WidgetMessage>;
  if (loading) return <WidgetMessage tone="muted">loading…</WidgetMessage>;

  const n = asNumber(latest);
  if (n === null) return <WidgetMessage tone="muted">no value yet</WidgetMessage>;

  const frac = Math.max(0, Math.min(1, (n - min) / (max - min || 1)));

  return (
    <div className="flex h-full flex-col" aria-label={`gauge ${source?.tool ?? ""}`}>
      <WidgetHeader label={label ?? source?.tool ?? ""} />
      <div className="flex flex-1 flex-col items-center justify-center">
        <GaugeArcChart fraction={frac} ariaLabel="gauge arc" />
        <span className="text-lg font-semibold text-fg" aria-label="gauge value">
          {n}
          {unit && <span className="ml-1 text-sm text-muted">{unit}</span>}
        </span>
      </div>
    </div>
  );
}
