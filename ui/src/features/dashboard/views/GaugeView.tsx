// The v2 `gauge` view — a single value against a min/max + thresholds arc. Reads `source` through the
// bridge (`useSource`); a non-numeric latest degrades to an honest "no value" (never a fake needle).

import { WidgetHeader, WidgetMessage } from "../widgets/chrome";
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
  // A 180° arc; the needle sweeps left→right with the fraction.
  const angle = -90 + frac * 180;

  return (
    <div className="flex h-full flex-col" aria-label={`gauge ${source?.tool ?? ""}`}>
      <WidgetHeader label={label ?? source?.tool ?? ""} />
      <div className="flex flex-1 flex-col items-center justify-center">
        <svg viewBox="0 0 100 60" className="w-full max-w-[140px]" role="img" aria-label="gauge arc">
          <path d="M5,55 A45,45 0 0 1 95,55" fill="none" stroke="currentColor" strokeWidth={6} className="text-border" />
          <line
            x1="50"
            y1="55"
            x2={50 + 38 * Math.cos((angle - 90) * (Math.PI / 180))}
            y2={55 + 38 * Math.sin((angle - 90) * (Math.PI / 180))}
            stroke="currentColor"
            strokeWidth={2}
            className="text-accent"
          />
        </svg>
        <span className="text-lg font-semibold text-fg" aria-label="gauge value">
          {n}
          {unit && <span className="ml-1 text-sm text-muted">{unit}</span>}
        </span>
      </div>
    </div>
  );
}
