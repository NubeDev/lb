// The v3 `piechart` panel renderer (viz chart-types scope, Phase 2). One slice per frame category
// (`frameCategories` — the explicit per-row collapse), each colored from `fieldConfig` (threshold/fixed/
// palette) and labelled through the ONE user-prefs bridge (`format.ts`). pie/donut + display labels +
// legend/tooltip are the per-viz options. Data ONLY through `usePanelData` (invariant A); no client
// transform (invariant B). One responsibility: render a piechart panel from a cell.

import type { Cell } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import { emptyScope } from "@/lib/vars";

import { WidgetHeader, WidgetMessage } from "../../widgets/chrome";
import { PieChartSvg, type CategoryDatum } from "../../widgets/recharts";
import { usePanelData } from "../../builder/usePanelData";
import { frameCategories } from "../reduce";
import { valueFieldOptions, categoryColor } from "../field";
import { readPieChartOptions } from "./options";
import { formatValue } from "../../fieldconfig/format";

interface Props {
  cell: Cell;
  label?: string;
  scope?: VarScope;
  refreshKey?: number;
}

export function PieChartPanel({ cell, label, scope = emptyScope(), refreshKey = 0 }: Props) {
  const { rows, loading, denied } = usePanelData(cell, scope, refreshKey);

  if (denied) return <WidgetMessage tone="denied">no access to this source</WidgetMessage>;
  if (loading) return <WidgetMessage tone="muted">loading…</WidgetMessage>;

  const options = readPieChartOptions(cell.options);
  const opts = valueFieldOptions(cell);

  const cats = frameCategories(rows, options.reduceOptions);
  if (cats.length === 0) return <WidgetMessage tone="muted">no data yet</WidgetMessage>;

  const total = cats.reduce((a, c) => a + c.value, 0);
  const data: CategoryDatum[] = cats.map((c, i) => ({
    name: sliceLabel(c.name, c.value, total, options.displayLabels, opts),
    value: c.value,
    color: categoryColor(c.value, i, opts),
    text: formatValue(c.value, opts).text,
  }));

  return (
    <div className="flex h-full flex-col" aria-label="piechart panel">
      <WidgetHeader label={label ?? opts.displayName ?? ""} />
      <PieChartSvg
        data={data}
        pieType={options.pieType}
        showLegend={options.legend.showLegend && options.legend.displayMode !== "hidden"}
        showTooltip={options.tooltip.mode !== "none"}
        valueFormatter={(v) => formatValue(v, opts).text}
        ariaLabel="piechart"
      />
    </div>
  );
}

/** The slice label per the `displayLabels` option — name / formatted value / percent (any combination),
 *  the value formatted through the one bridge. Empty `displayLabels` → just the category name. */
function sliceLabel(
  name: string,
  value: number,
  total: number,
  labels: Array<"name" | "value" | "percent">,
  opts: Parameters<typeof formatValue>[1],
): string {
  if (labels.length === 0) return name;
  const parts: string[] = [];
  if (labels.includes("name")) parts.push(name);
  if (labels.includes("value")) parts.push(formatValue(value, opts).text);
  if (labels.includes("percent")) parts.push(total > 0 ? `${Math.round((value / total) * 100)}%` : "0%");
  return parts.join(" ");
}
