// The v3 `barchart` panel renderer (viz chart-types scope, Phase 2). One bar per frame category
// (`frameCategories`), each colored from `fieldConfig` and its tooltip value formatted through the ONE
// user-prefs bridge (`format.ts`). orientation/legend/tooltip are the per-viz options. Data ONLY through
// `usePanelData` (invariant A); no client transform (invariant B). One responsibility: render a barchart
// panel from a cell.

import type { Cell } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import { emptyScope } from "@/lib/vars";

import { WidgetHeader, WidgetMessage } from "../../widgets/chrome";
import { CategoryBarChart, type CategoryDatum } from "../../widgets/recharts";
import { usePanelData } from "../../builder/usePanelData";
import { frameCategories, defaultReduceOptions } from "../reduce";
import { valueFieldOptions, categoryColor } from "../field";
import { readBarChartOptions } from "./options";
import { formatValue } from "../../fieldconfig/format";

interface Props {
  cell: Cell;
  label?: string;
  scope?: VarScope;
  refreshKey?: number;
}

export function BarChartPanel({ cell, label, scope = emptyScope(), refreshKey = 0 }: Props) {
  const { rows, loading, denied } = usePanelData(cell, scope, refreshKey);

  if (denied) return <WidgetMessage tone="denied">no access to this source</WidgetMessage>;
  if (loading) return <WidgetMessage tone="muted">loading…</WidgetMessage>;

  const options = readBarChartOptions(cell.options);
  const opts = valueFieldOptions(cell);

  // A barchart draws every category — `reduceOptions.values`-style per-row collapse, capped by limit.
  const cats = frameCategories(rows, defaultReduceOptions());
  if (cats.length === 0) return <WidgetMessage tone="muted">no data yet</WidgetMessage>;

  const data: CategoryDatum[] = cats.map((c, i) => ({
    name: c.name,
    value: c.value,
    color: categoryColor(c.value, i, opts),
    text: formatValue(c.value, opts).text,
  }));

  // Grafana defaults `auto` orientation to horizontal bars for a category axis.
  const horizontal = options.orientation === "horizontal";

  return (
    <div className="flex h-full flex-col" aria-label="barchart panel">
      <WidgetHeader label={label ?? opts.displayName ?? ""} />
      <CategoryBarChart
        data={data}
        horizontal={horizontal}
        showTooltip={options.tooltip.mode !== "none"}
        valueFormatter={(v) => formatValue(v, opts).text}
        ariaLabel="barchart"
      />
    </div>
  );
}
