// The v3 `gauge` panel renderer (viz chart-types scope, Phase 2). A single reduced value drawn as a
// radial arc against the field's canonical min/max, colored by the threshold step it falls in. The
// value is collapsed by `reduceOptions` (the visible frame→value bridge), formatted through the ONE
// user-prefs bridge (`format.ts`), and colored through `fieldConfig` thresholds — never a local toFixed
// or color string. Data comes ONLY through `usePanelData` (invariant A); no client transform (invariant
// B). Replaces the v2 GaugeView's `min/max/unit`-only options.
//
// One responsibility: render a gauge panel from a cell.

import type { Cell } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import { emptyScope } from "@/lib/vars";

import { WidgetHeader, WidgetMessage } from "../../widgets/chrome";
import { GaugeArcChart } from "../../widgets/recharts";
import { usePanelData } from "../../builder/usePanelData";
import { reduceFrame } from "../reduce";
import { valueFieldOptions, valueColor } from "../field";
import { readGaugeOptions } from "./options";
import { formatValue } from "../../fieldconfig/format";

interface Props {
  cell: Cell;
  label?: string;
  scope?: VarScope;
  refreshKey?: number;
}

export function GaugePanel({ cell, label, scope = emptyScope(), refreshKey = 0 }: Props) {
  const { rows, loading, denied } = usePanelData(cell, scope, refreshKey);

  if (denied) return <WidgetMessage tone="denied">no access to this source</WidgetMessage>;
  if (loading) return <WidgetMessage tone="muted">loading…</WidgetMessage>;

  const options = readGaugeOptions(cell.options);
  const opts = valueFieldOptions(cell);

  const value = reduceFrame(rows, options.reduceOptions);
  if (value === null) {
    if (opts.noValue) return <WidgetMessage tone="muted">{opts.noValue}</WidgetMessage>;
    return <WidgetMessage tone="muted">no value yet</WidgetMessage>;
  }

  // The arc fraction over the canonical min/max (Grafana's gauge defaults 0..100 when unset).
  const min = opts.min ?? 0;
  const max = opts.max ?? 100;
  const frac = Math.max(0, Math.min(1, (value - min) / (max - min || 1)));
  const color = valueColor(value, opts);
  const display = formatValue(value, opts).text;

  return (
    <div className="flex h-full flex-col" aria-label="gauge panel">
      <WidgetHeader label={label ?? opts.displayName ?? ""} />
      <div className="flex flex-1 flex-col items-center justify-center gap-1">
        <GaugeArcChart fraction={frac} color={color} ariaLabel="gauge arc" />
        <span className="text-lg font-semibold tabular-nums" style={{ color }} aria-label="gauge value">
          {display}
        </span>
        {options.showThresholdLabels && (
          <span className="text-[11px] text-muted tabular-nums" aria-label="gauge bounds">
            {formatValue(min, opts).text} – {formatValue(max, opts).text}
          </span>
        )}
      </div>
    </div>
  );
}
