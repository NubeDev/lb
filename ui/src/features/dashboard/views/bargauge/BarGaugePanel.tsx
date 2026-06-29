// The v3 `bargauge` panel renderer (viz chart-types scope, Phase 2). One filled bar per value: a single
// reduced value (default) or every value (`reduceOptions.values`), each filled to its fraction of the
// field's canonical min/max and colored by the threshold step it lands in. The value(s) come from the
// visible `reduceOptions` bridge; each is formatted through the ONE user-prefs bridge (`format.ts`) and
// colored through `fieldConfig` thresholds — never a local toFixed/color. Data ONLY through
// `usePanelData` (invariant A); no client transform (invariant B).
//
// One responsibility: render a bargauge panel from a cell. The bar markup is `Bars.tsx`'s job.

import type { Cell } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import { emptyScope } from "@/lib/vars";

import { WidgetHeader, WidgetMessage } from "../../widgets/chrome";
import { usePanelData } from "../../builder/usePanelData";
import { reduceFrameValues } from "../reduce";
import { valueFieldOptions, valueColor } from "../field";
import { readBarGaugeOptions } from "./options";
import { formatValue } from "../../fieldconfig/format";
import { BarGaugeBars } from "./Bars";

interface Props {
  cell: Cell;
  label?: string;
  scope?: VarScope;
  refreshKey?: number;
}

export function BarGaugePanel({ cell, label, scope = emptyScope(), refreshKey = 0 }: Props) {
  const { rows, loading, denied } = usePanelData(cell, scope, refreshKey);

  if (denied) return <WidgetMessage tone="denied">no access to this source</WidgetMessage>;
  if (loading) return <WidgetMessage tone="muted">loading…</WidgetMessage>;

  const options = readBarGaugeOptions(cell.options);
  const opts = valueFieldOptions(cell);

  const values = reduceFrameValues(rows, options.reduceOptions);
  if (values.length === 0) {
    if (opts.noValue) return <WidgetMessage tone="muted">{opts.noValue}</WidgetMessage>;
    return <WidgetMessage tone="muted">no value yet</WidgetMessage>;
  }

  const min = opts.min ?? 0;
  const max = opts.max ?? 100;
  const bars = values.map((v) => ({
    fraction: Math.max(0, Math.min(1, (v - min) / (max - min || 1))),
    color: valueColor(v, opts),
    text: formatValue(v, opts).text,
  }));

  const horizontal = options.orientation !== "vertical";

  return (
    <div className="flex h-full flex-col" aria-label="bargauge panel">
      <WidgetHeader label={label ?? opts.displayName ?? ""} />
      <BarGaugeBars
        bars={bars}
        horizontal={horizontal}
        displayMode={options.displayMode}
        showUnfilled={options.showUnfilled}
        showValue={options.valueMode !== "hidden"}
      />
    </div>
  );
}
